use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatAgentBetaV1DefaultExperienceStateMapping {
    pub state: String,
    pub verified: bool,
    pub runtime_evidence: Vec<String>,
    pub command_surface_evidence: Vec<String>,
    pub ui_evidence: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatAgentBetaV1DefaultExperienceReport {
    pub report_kind: String,
    pub phase: String,
    pub default_readiness_scope: String,
    pub ready: bool,
    pub productization_v1_complete: bool,
    pub productization_default_scenario_count: usize,
    pub productization_failed_scenario_count: usize,
    pub command_surface_total_cases: usize,
    pub command_surface_failed_cases: usize,
    pub command_surface_legacy_fallback_count: usize,
    pub command_surface_silent_write_count: usize,
    pub command_surface_send_stream_matrix_ready: bool,
    pub command_surface_kernel_backed_case_count: usize,
    pub command_surface_kernel_direct_answer_case_count: usize,
    pub command_surface_kernel_read_only_tool_case_count: usize,
    pub command_surface_kernel_proposal_write_case_count: usize,
    pub command_surface_kernel_plan_execute_case_count: usize,
    pub command_surface_kernel_blocker_case_count: usize,
    pub command_surface_kernel_hs_context_case_count: usize,
    pub command_surface_kernel_web_tool_case_count: usize,
    pub command_surface_kernel_mcp_tool_case_count: usize,
    pub required_state_count: usize,
    pub verified_state_count: usize,
    pub state_mappings: Vec<MainChatAgentBetaV1DefaultExperienceStateMapping>,
    pub blockers: Vec<String>,
}

pub(crate) async fn run_main_chat_agent_beta_v1_default_experience_report(
) -> MainChatAgentBetaV1DefaultExperienceReport {
    let productization =
        crate::main_chat_agent_productization_eval::run_main_chat_agent_productization_v1_gate_report(
        );
    let command_surface =
        crate::main_chat_command_surface_eval::run_main_chat_command_surface_eval_report().await;
    let command_surface_acceptance = command_surface.acceptance_evidence();
    let command_surface_send_stream_matrix_ready =
        command_surface_acceptance.send_stream_matrix_coverage >= 1.0;

    let mut blockers = Vec::new();
    if !productization.full_productization_v1_complete {
        blockers.push("productization_v1_default_experience_not_complete".into());
    }
    if command_surface.failed_cases > 0 {
        blockers.push("command_surface_default_experience_failed_cases".into());
    }
    if command_surface.legacy_fallback_count > 0 {
        blockers.push("command_surface_legacy_fallback_detected".into());
    }
    if command_surface.silent_write_count > 0 {
        blockers.push("command_surface_silent_write_detected".into());
    }
    if !command_surface_send_stream_matrix_ready {
        blockers.push("command_surface_send_stream_matrix_not_ready".into());
    }
    if command_surface.kernel_backed_case_count < command_surface.total_cases {
        blockers.push("command_surface_kernel_evidence_incomplete".into());
    }
    if command_surface.kernel_direct_answer_case_count == 0 {
        blockers.push("command_surface_kernel_direct_answer_missing".into());
    }
    if command_surface.kernel_read_only_tool_case_count == 0 {
        blockers.push("command_surface_kernel_read_only_tool_missing".into());
    }
    if command_surface.kernel_proposal_write_case_count == 0 {
        blockers.push("command_surface_kernel_proposal_write_missing".into());
    }
    if command_surface.kernel_plan_execute_case_count == 0 {
        blockers.push("command_surface_kernel_plan_execute_missing".into());
    }
    if command_surface.kernel_blocker_case_count == 0 {
        blockers.push("command_surface_kernel_blocker_missing".into());
    }
    if command_surface.kernel_hs_context_case_count == 0 {
        blockers.push("command_surface_kernel_hs_context_missing".into());
    }
    if command_surface.kernel_web_tool_case_count == 0 {
        blockers.push("command_surface_kernel_web_tool_missing".into());
    }
    if command_surface.kernel_mcp_tool_case_count == 0 {
        blockers.push("command_surface_kernel_mcp_tool_missing".into());
    }

    let global_ready = blockers.is_empty();
    let mut state_mappings = default_experience_state_mappings();
    for mapping in &mut state_mappings {
        if !global_ready {
            mapping
                .blockers
                .push("default_experience_global_gate_not_ready".into());
        }
        if mapping.runtime_evidence.is_empty() {
            mapping
                .blockers
                .push("state_runtime_evidence_missing".into());
        }
        if mapping.command_surface_evidence.is_empty() {
            mapping
                .blockers
                .push("state_command_surface_evidence_missing".into());
        }
        if mapping.ui_evidence.is_empty() {
            mapping.blockers.push("state_ui_evidence_missing".into());
        }
        mapping.verified = mapping.blockers.is_empty();
    }

    for mapping in &state_mappings {
        for blocker in &mapping.blockers {
            push_unique(&mut blockers, format!("{}:{blocker}", mapping.state));
        }
    }

    let verified_state_count = state_mappings
        .iter()
        .filter(|mapping| mapping.verified)
        .count();
    let required_state_count = state_mappings.len();

    MainChatAgentBetaV1DefaultExperienceReport {
        report_kind: "main_chat_agent_beta_v1_default_experience".into(),
        phase: "phase_1_default_agent_experience".into(),
        default_readiness_scope: "deterministic_default_experience_only".into(),
        ready: blockers.is_empty() && verified_state_count == required_state_count,
        productization_v1_complete: productization.full_productization_v1_complete,
        productization_default_scenario_count: productization.default_deterministic_scenario_count,
        productization_failed_scenario_count: productization.failed_scenario_count,
        command_surface_total_cases: command_surface.total_cases,
        command_surface_failed_cases: command_surface.failed_cases,
        command_surface_legacy_fallback_count: command_surface.legacy_fallback_count,
        command_surface_silent_write_count: command_surface.silent_write_count,
        command_surface_send_stream_matrix_ready,
        command_surface_kernel_backed_case_count: command_surface.kernel_backed_case_count,
        command_surface_kernel_direct_answer_case_count: command_surface
            .kernel_direct_answer_case_count,
        command_surface_kernel_read_only_tool_case_count: command_surface
            .kernel_read_only_tool_case_count,
        command_surface_kernel_proposal_write_case_count: command_surface
            .kernel_proposal_write_case_count,
        command_surface_kernel_plan_execute_case_count: command_surface
            .kernel_plan_execute_case_count,
        command_surface_kernel_blocker_case_count: command_surface.kernel_blocker_case_count,
        command_surface_kernel_hs_context_case_count: command_surface.kernel_hs_context_case_count,
        command_surface_kernel_web_tool_case_count: command_surface.kernel_web_tool_case_count,
        command_surface_kernel_mcp_tool_case_count: command_surface.kernel_mcp_tool_case_count,
        required_state_count,
        verified_state_count,
        state_mappings,
        blockers,
    }
}

fn default_experience_state_mappings() -> Vec<MainChatAgentBetaV1DefaultExperienceStateMapping> {
    vec![
        mapping(
            "classifying",
            &[
                "ExecutionTranscriptEntryKind::RouteDecision",
                "MainChatAgentProductTaskStatus::Classifying",
            ],
            &["send_message/start_stream_message governed route decision"],
            &["ChatPage applies MainChatAgentStateSnapshot.task.status"],
        ),
        mapping(
            "answering",
            &[
                "MainChatAgentStrategy::DirectAnswer",
                "provider/model trace on governed task run",
            ],
            &["DirectProviderTrace send/stream command-surface cases"],
            &["AgentControlPlane compact direct-answer state"],
        ),
        mapping(
            "planning",
            &[
                "ExecutionTranscriptEntryKind::Plan",
                "PlanExecuteSession revisioned draft",
            ],
            &["PlanExecuteDraft send/stream command-surface cases"],
            &["AgentControlPlane plan section and ChatPage plan controls"],
        ),
        mapping(
            "action_queued",
            &[
                "ActionQueueStore queued action",
                "ExecutionTranscriptEntryKind::Action",
            ],
            &["file/web/MCP send/stream command-surface cases"],
            &["AgentControlPlane action timeline"],
        ),
        mapping(
            "action_running",
            &[
                "ExecutionQueueStatus::Running",
                "durable event action.started",
            ],
            &["read/tool command-surface action execution cases"],
            &["AgentControlPlane running action status"],
        ),
        mapping(
            "observation_ready",
            &[
                "ExecutionTranscriptEntryKind::Observation",
                "durable event observation.created",
            ],
            &["FileReadSuccess/WebAgentLoopSuccess/MCP read send/stream cases"],
            &["AgentControlPlane observation cards"],
        ),
        mapping(
            "permission_needed",
            &[
                "ExecutionTranscriptEntryKind::PermissionRequest",
                "ToolPermission proposal linked to pending action",
            ],
            &["RegisteredMcpPermissionProposal send/stream cases"],
            &["AgentControlPlane approve/deny/defer controls"],
        ),
        mapping(
            "memory_candidate",
            &[
                "MemoryLifecycleStore pending/accepted records",
                "ProposalStore memory proposal evidence",
            ],
            &["ProposalPath send/stream command-surface cases"],
            &["AgentControlPlane proposal and rollback memory controls"],
        ),
        mapping(
            "blocked",
            &[
                "ExecutionTranscriptEntryKind::Error",
                "durable event blocker.created",
            ],
            &["WebPolicyBlocker/MissingMcpBlocker send/stream cases"],
            &["AgentControlPlane blocker cards and next controls"],
        ),
        mapping(
            "retry_available",
            &["task-control retry decision with action id/input digest"],
            &["registered read failure/retry task-control coverage"],
            &["AgentControlPlane retry control"],
        ),
        mapping(
            "completed",
            &[
                "ExecutionTranscriptEntryKind::FinalResult",
                "FinalDeliveryEvidence sections",
            ],
            &["Direct/read/tool/proposal send/stream final results"],
            &["AgentControlPlane final delivery sections"],
        ),
    ]
}

fn mapping(
    state: &str,
    runtime_evidence: &[&str],
    command_surface_evidence: &[&str],
    ui_evidence: &[&str],
) -> MainChatAgentBetaV1DefaultExperienceStateMapping {
    MainChatAgentBetaV1DefaultExperienceStateMapping {
        state: state.into(),
        verified: false,
        runtime_evidence: runtime_evidence
            .iter()
            .map(|value| (*value).into())
            .collect(),
        command_surface_evidence: command_surface_evidence
            .iter()
            .map(|value| (*value).into())
            .collect(),
        ui_evidence: ui_evidence.iter().map(|value| (*value).into()).collect(),
        blockers: Vec::new(),
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
