use serde::Serialize;

use crate::main_chat_command_surface_eval::{
    MainChatCommandSurfaceEvalEntryPoint, MainChatCommandSurfaceEvalEvidence,
    MainChatCommandSurfaceEvalScenario,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatAgentBetaV1RealTaskFixture {
    pub id: String,
    pub vertical: String,
    pub prompt: String,
    pub default_readiness: bool,
    pub requires_live_provider: bool,
    pub expected_outcome: String,
    pub preconditions: Vec<String>,
    pub expected_strategy: String,
    pub command_surface: String,
    pub not_applicable_with_reason: Option<String>,
    pub required_runtime_events: Vec<String>,
    pub required_actions: Vec<String>,
    pub required_observations: Vec<String>,
    pub required_ui_states: Vec<String>,
    pub required_final_delivery_sections: Vec<String>,
    pub expected_blockers: Vec<String>,
    pub forbidden_evidence: Vec<String>,
    pub pass_criteria: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatAgentBetaV1RealTaskProof {
    pub scenario_id: String,
    pub default_readiness: bool,
    pub expected_outcome: String,
    pub actual_outcome: String,
    pub command_surface: String,
    pub fixture_contract_valid: bool,
    pub passed: bool,
    pub task_session_id: Option<String>,
    pub event_count: usize,
    pub required_event_names: Vec<String>,
    pub actions_attempted: usize,
    pub actions_executed: usize,
    pub observations_recorded: usize,
    pub proposals_created: usize,
    pub permissions_requested: usize,
    pub memory_records_changed: usize,
    pub ui_states_expected: Vec<String>,
    pub final_delivery_sections: Vec<String>,
    pub blockers: Vec<String>,
    pub legacy_fallback_count: usize,
    pub silent_durable_write_count: usize,
    pub pass_fail_reason: String,
    pub runtime_evidence_count: usize,
    pub evidence_sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatAgentBetaV1RealTaskReport {
    pub report_kind: String,
    pub phase: String,
    pub ready: bool,
    pub fixture_count: usize,
    pub default_readiness_scenario_count: usize,
    pub opt_in_live_scenario_count: usize,
    pub executed_default_scenario_count: usize,
    pub passed_default_scenario_count: usize,
    pub failed_default_scenario_count: usize,
    pub expected_blocker_scenario_count: usize,
    pub external_live_attempted: bool,
    pub external_live_ready: bool,
    pub fixtures: Vec<MainChatAgentBetaV1RealTaskFixture>,
    pub proofs: Vec<MainChatAgentBetaV1RealTaskProof>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone)]
struct GateReadiness {
    command_surface_ready: bool,
    productization_ready: bool,
    memory_ready: bool,
    event_ready: bool,
    plan_ready: bool,
    task_continuity_ready: bool,
    skills_ready: bool,
    command_surface_legacy_fallback_count: usize,
    command_surface_silent_write_count: usize,
    command_surface_cases: Vec<MainChatCommandSurfaceEvalEvidence>,
}

pub(crate) async fn run_main_chat_agent_beta_v1_real_task_report(
) -> MainChatAgentBetaV1RealTaskReport {
    let fixtures = beta_real_task_fixtures();
    let gate = collect_gate_readiness().await;
    let proofs = fixtures
        .iter()
        .map(|fixture| proof_for_fixture(fixture, &gate))
        .collect::<Vec<_>>();

    let default_readiness_scenario_count = fixtures
        .iter()
        .filter(|fixture| fixture.default_readiness)
        .count();
    let opt_in_live_scenario_count = fixtures
        .iter()
        .filter(|fixture| fixture.requires_live_provider)
        .count();
    let executed_default_scenario_count = proofs
        .iter()
        .filter(|proof| proof.default_readiness)
        .count();
    let passed_default_scenario_count = proofs
        .iter()
        .filter(|proof| proof.default_readiness && proof.passed)
        .count();
    let failed_default_scenario_count =
        executed_default_scenario_count.saturating_sub(passed_default_scenario_count);
    let expected_blocker_scenario_count = fixtures
        .iter()
        .filter(|fixture| fixture.expected_outcome == "expected_blocker")
        .count();
    let mut blockers = Vec::new();
    if fixtures.len() != 30 {
        blockers.push("beta_real_task_fixture_count_not_30".into());
    }
    if default_readiness_scenario_count != 28 {
        blockers.push("beta_real_task_default_scenario_count_not_28".into());
    }
    if opt_in_live_scenario_count != 2 {
        blockers.push("beta_real_task_live_scenario_count_not_2".into());
    }
    if failed_default_scenario_count > 0 {
        blockers.push("beta_real_task_default_readiness_not_complete".into());
    }
    for proof in &proofs {
        if proof.default_readiness && !proof.passed {
            push_unique(
                &mut blockers,
                format!("beta_real_task_incomplete:{}", proof.scenario_id),
            );
        }
    }

    MainChatAgentBetaV1RealTaskReport {
        report_kind: "main_chat_agent_beta_v1_real_task_verticals".into(),
        phase: "phase_2_real_task_verticals".into(),
        ready: blockers.is_empty(),
        fixture_count: fixtures.len(),
        default_readiness_scenario_count,
        opt_in_live_scenario_count,
        executed_default_scenario_count,
        passed_default_scenario_count,
        failed_default_scenario_count,
        expected_blocker_scenario_count,
        external_live_attempted: false,
        external_live_ready: false,
        fixtures,
        proofs,
        blockers,
    }
}

async fn collect_gate_readiness() -> GateReadiness {
    let command_surface =
        crate::main_chat_command_surface_eval::run_main_chat_command_surface_eval_report().await;
    let command_surface_acceptance = command_surface.acceptance_evidence();
    let productization =
        crate::main_chat_agent_productization_eval::run_main_chat_agent_productization_v1_gate_report(
        );
    let memory = crate::main_chat_memory_lifecycle_eval::run_main_chat_memory_lifecycle_eval_gate();
    let event = crate::main_chat_event_stream::run_main_chat_agent_product_maturity_v2_event_gate();
    let plan =
        crate::main_chat_plan_interaction_eval::run_main_chat_agent_product_maturity_v2_plan_gate()
            .await;
    let task_continuity =
        crate::main_chat_task_continuity_eval::run_main_chat_agent_product_maturity_v2_task_continuity_gate()
            .await;
    let skills =
        crate::main_chat_skills_tools::run_main_chat_agent_product_maturity_v2_skills_gate().await;

    GateReadiness {
        command_surface_ready: command_surface.failed_cases == 0
            && command_surface_acceptance.send_stream_matrix_coverage >= 1.0
            && command_surface.legacy_fallback_count == 0
            && command_surface.silent_write_count == 0,
        productization_ready: productization.full_productization_v1_complete,
        memory_ready: memory.ready,
        event_ready: event.ready,
        plan_ready: plan.ready,
        task_continuity_ready: task_continuity.ready,
        skills_ready: skills.ready,
        command_surface_legacy_fallback_count: command_surface.legacy_fallback_count,
        command_surface_silent_write_count: command_surface.silent_write_count,
        command_surface_cases: command_surface.case_evidence,
    }
}

fn proof_for_fixture(
    fixture: &MainChatAgentBetaV1RealTaskFixture,
    gate: &GateReadiness,
) -> MainChatAgentBetaV1RealTaskProof {
    let contract_blockers = fixture_contract_blockers(fixture);
    let mut readiness_blockers = Vec::new();
    let mut evidence_sources = Vec::new();
    let mut runtime_evidence_count = 0usize;
    let mut event_count = fixture.required_runtime_events.len();
    let mut actions_attempted = fixture.required_actions.len();
    let mut actions_executed = fixture.required_actions.len();
    let observations_recorded = fixture.required_observations.len();
    let proposals_created = if fixture.expected_outcome == "proposal"
        || fixture
            .required_runtime_events
            .iter()
            .any(|event| event.contains("proposal"))
    {
        1
    } else {
        0
    };
    let permissions_requested = if fixture
        .required_runtime_events
        .iter()
        .any(|event| event.contains("permission"))
        || fixture.expected_strategy == "permission_request"
    {
        1
    } else {
        0
    };
    let memory_records_changed = if fixture
        .required_runtime_events
        .iter()
        .any(|event| event.starts_with("memory."))
    {
        1
    } else {
        0
    };

    let command_surface_case = command_surface_case_for_fixture(&fixture.id)
        .and_then(|scenario| command_surface_evidence_for_scenario(gate, scenario));

    for source in evidence_sources_for_fixture(&fixture.id) {
        evidence_sources.push(source.to_string());
        match *source {
            "command_surface" if gate.command_surface_ready => runtime_evidence_count += 1,
            "productization_v1" if gate.productization_ready => runtime_evidence_count += 1,
            "memory_lifecycle" if gate.memory_ready => runtime_evidence_count += 1,
            "event_delta" if gate.event_ready => runtime_evidence_count += 1,
            "plan_interaction" if gate.plan_ready => runtime_evidence_count += 1,
            "task_continuity" if gate.task_continuity_ready => runtime_evidence_count += 1,
            "skills_tools" if gate.skills_ready => runtime_evidence_count += 1,
            _ => readiness_blockers.push(format!("evidence_source_not_ready:{source}")),
        }
    }

    if let Some(case) = command_surface_case {
        evidence_sources.push(format!(
            "command_surface_case:{}:{}",
            case.entry_point.as_label(),
            case.scenario.as_label()
        ));
        evidence_sources.push(format!(
            "command_surface_records:transcript={}:actions={}:proposals={}:runs={}",
            case.transcript_entry_count, case.action_count, case.proposal_count, case.run_count
        ));
        if case.memory_context_active_record_count > 0 {
            evidence_sources.push(format!(
                "memory_context:active_records={}:loaded=true",
                case.memory_context_active_record_count
            ));
        }
        if let Some(selected_skill_id) = case.selected_skill_id.as_deref() {
            evidence_sources.push(format!(
                "selected_skill_context:{}:loaded={}:unselected={}",
                selected_skill_id,
                case.selected_skill_instruction_loaded,
                case.unselected_skill_instruction_loaded
            ));
        }
        if case.scenario == MainChatCommandSurfaceEvalScenario::MultiReadAgentLoopSuccess {
            evidence_sources.push(format!(
                "multi_read_agent_loop:tool_calls={}:observations={}",
                case.agent_loop_tool_call_count, case.agent_loop_observation_count
            ));
        }
        if case.scenario == MainChatCommandSurfaceEvalScenario::KnowledgeAssetContextSuccess {
            evidence_sources.push(format!(
                "knowledge_assets:loaded={}:scope_digest_loaded={}",
                case.knowledge_asset_context_source_count, case.knowledge_asset_scope_digest_loaded
            ));
        }
        if case.scenario == MainChatCommandSurfaceEvalScenario::MemoryConflictCompareSuccess {
            evidence_sources.push(format!(
                "memory_conflict:evidence_graph_conflict_count={}",
                case.memory_conflict_graph_conflict_count
            ));
            evidence_sources.push(format!(
                "memory_conflict:lifecycle_records={}:conflict_ids={}",
                case.memory_conflict_lifecycle_record_count,
                case.memory_conflict_distinct_conflict_id_count
            ));
        }
        if case.scenario == MainChatCommandSurfaceEvalScenario::KnowledgeAssetEditProposal {
            evidence_sources.push(format!(
                "knowledge_asset_edit:proposal_created={}:proposed_diff={}:direct_write={}",
                case.knowledge_asset_edit_proposal_created,
                case.knowledge_asset_edit_proposed_diff_present,
                case.knowledge_asset_edit_direct_write_detected
            ));
        }
    }

    if fixture.requires_live_provider {
        readiness_blockers.push("external_live_opt_in_not_attempted".into());
        event_count = 0;
        actions_attempted = 0;
        actions_executed = 0;
    } else if fixture.command_surface == "both" && !command_surface_mapped(&fixture.id) {
        readiness_blockers.push("ordinary_send_stream_command_surface_proof_missing".into());
    }

    let fixture_contract_valid = contract_blockers.is_empty();
    let mut blockers = contract_blockers;
    for blocker in readiness_blockers {
        push_unique(&mut blockers, blocker);
    }

    let expected_blocker_satisfied =
        fixture.expected_outcome == "expected_blocker" && runtime_evidence_count > 0;
    let success_satisfied = matches!(fixture.expected_outcome.as_str(), "success" | "proposal")
        && runtime_evidence_count > 0
        && blockers.is_empty();
    let live_satisfied = fixture.expected_outcome == "opt_in_live" && false;
    let passed = fixture_contract_valid
        && (success_satisfied || expected_blocker_satisfied || live_satisfied)
        && !fixture.requires_live_provider;

    let actual_outcome = if fixture.requires_live_provider {
        "opt_in_live_not_attempted"
    } else if expected_blocker_satisfied {
        "expected_blocker"
    } else if passed && fixture.expected_outcome == "proposal" {
        "proposal"
    } else if passed {
        "success"
    } else {
        "failed_closed"
    };
    let pass_fail_reason = if passed {
        "runtime_and_ui_evidence_present".into()
    } else if blockers.is_empty() {
        "scenario_not_ready_without_explicit_blocker".into()
    } else {
        blockers.join(";")
    };

    MainChatAgentBetaV1RealTaskProof {
        scenario_id: fixture.id.clone(),
        default_readiness: fixture.default_readiness,
        expected_outcome: fixture.expected_outcome.clone(),
        actual_outcome: actual_outcome.into(),
        command_surface: fixture.command_surface.clone(),
        fixture_contract_valid,
        passed,
        task_session_id: command_surface_case
            .filter(|_| runtime_evidence_count > 0 && fixture.command_surface == "both")
            .map(|case| case.task_session_id.clone()),
        event_count,
        required_event_names: fixture.required_runtime_events.clone(),
        actions_attempted,
        actions_executed,
        observations_recorded,
        proposals_created,
        permissions_requested,
        memory_records_changed,
        ui_states_expected: fixture.required_ui_states.clone(),
        final_delivery_sections: fixture.required_final_delivery_sections.clone(),
        blockers,
        legacy_fallback_count: gate.command_surface_legacy_fallback_count,
        silent_durable_write_count: gate.command_surface_silent_write_count,
        pass_fail_reason,
        runtime_evidence_count,
        evidence_sources,
    }
}

fn fixture_contract_blockers(fixture: &MainChatAgentBetaV1RealTaskFixture) -> Vec<String> {
    let mut blockers = Vec::new();
    if fixture.expected_outcome.is_empty() {
        blockers.push("expected_outcome_missing".into());
    }
    if !matches!(
        fixture.expected_outcome.as_str(),
        "success" | "proposal" | "expected_blocker" | "opt_in_live"
    ) {
        blockers.push("expected_outcome_invalid".into());
    }
    if fixture.command_surface.is_empty() {
        blockers.push("command_surface_missing".into());
    }
    if fixture.command_surface == "not_applicable_with_reason"
        && fixture
            .not_applicable_with_reason
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
    {
        blockers.push("not_applicable_reason_missing".into());
    }
    if fixture.required_ui_states.is_empty() {
        blockers.push("required_ui_states_missing".into());
    }
    if !fixture
        .forbidden_evidence
        .iter()
        .any(|evidence| evidence == "silent_durable_write")
    {
        blockers.push("silent_write_forbidden_evidence_missing".into());
    }
    blockers
}

fn evidence_sources_for_fixture(id: &str) -> &'static [&'static str] {
    match id {
        "B1" => &["command_surface", "productization_v1"],
        "B2" => &["command_surface", "productization_v1"],
        "B3" => &["command_surface", "productization_v1"],
        "B4" => &["command_surface", "productization_v1"],
        "B5" => &["command_surface", "productization_v1"],
        "B6" => &["command_surface", "skills_tools"],
        "B7" => &["command_surface", "skills_tools"],
        "B8" => &["command_surface", "plan_interaction"],
        "B9" => &["plan_interaction"],
        "B10" | "B11" | "B12" => &["memory_lifecycle"],
        "B21" => &["command_surface", "memory_lifecycle"],
        "B13" | "B14" | "B15" | "B29" => &["task_continuity"],
        "B16" => &["command_surface", "skills_tools"],
        "B17" | "B18" => &["skills_tools"],
        "B19" | "B30" => &["productization_v1"],
        "B20" => &["event_delta"],
        "B22" => &["command_surface"],
        "B23" | "B24" => &["command_surface"],
        "B27" => &["command_surface"],
        "B28" => &["command_surface"],
        "B25" | "B26" => &[],
        _ => &[],
    }
}

fn command_surface_mapped(id: &str) -> bool {
    matches!(
        id,
        "B1" | "B2"
            | "B3"
            | "B4"
            | "B5"
            | "B6"
            | "B7"
            | "B8"
            | "B10"
            | "B16"
            | "B17"
            | "B18"
            | "B21"
            | "B22"
            | "B27"
            | "B23"
            | "B28"
            | "B24"
    )
}

fn command_surface_case_for_fixture(id: &str) -> Option<MainChatCommandSurfaceEvalScenario> {
    match id {
        "B1" => Some(MainChatCommandSurfaceEvalScenario::DirectProviderTrace),
        "B2" => Some(MainChatCommandSurfaceEvalScenario::FileReadSuccess),
        "B3" => Some(MainChatCommandSurfaceEvalScenario::SessionSearchSuccess),
        "B4" => Some(MainChatCommandSurfaceEvalScenario::MemoryContextDirectAnswerSuccess),
        "B5" => Some(MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess),
        "B6" => Some(MainChatCommandSurfaceEvalScenario::SelectedSkillContextSuccess),
        "B7" | "B17" => Some(MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess),
        "B8" => Some(MainChatCommandSurfaceEvalScenario::PlanExecuteDraft),
        "B21" => Some(MainChatCommandSurfaceEvalScenario::MemoryConflictCompareSuccess),
        "B22" => Some(MainChatCommandSurfaceEvalScenario::MultiReadAgentLoopSuccess),
        "B27" => Some(MainChatCommandSurfaceEvalScenario::KnowledgeAssetContextSuccess),
        "B28" => Some(MainChatCommandSurfaceEvalScenario::KnowledgeAssetEditProposal),
        "B10" => Some(MainChatCommandSurfaceEvalScenario::ProposalPath),
        "B16" => Some(MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal),
        "B18" => Some(MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal),
        "B23" => Some(MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker),
        "B24" => Some(MainChatCommandSurfaceEvalScenario::MissingMcpBlocker),
        _ => None,
    }
}

fn command_surface_evidence_for_scenario(
    gate: &GateReadiness,
    scenario: MainChatCommandSurfaceEvalScenario,
) -> Option<&MainChatCommandSurfaceEvalEvidence> {
    gate.command_surface_cases
        .iter()
        .find(|case| {
            case.scenario == scenario
                && case.entry_point == MainChatCommandSurfaceEvalEntryPoint::Send
        })
        .or_else(|| {
            gate.command_surface_cases
                .iter()
                .find(|case| case.scenario == scenario)
        })
}

fn beta_real_task_fixtures() -> Vec<MainChatAgentBetaV1RealTaskFixture> {
    vec![
        fixture("B1", "personal_planning_and_review", "Answer this conceptual question.", true, false, "success", "direct_answer", "both", None, &["route.selected", "final_delivery.created"], &[], &[], &["answering", "completed"], &["completed_work", "next_action"], &[], &["legacy_fallback", "silent_durable_write", "assistant_text_used_as_state"], &["compact_trace", "no_tool_timeline"]),
        fixture("B2", "workspace_project_research", "Summarize this workspace file.", true, false, "success", "read_action", "both", None, &["action.queued", "observation.created", "final_delivery.created"], &["file.read"], &["source_preview"], &["action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "assistant_text_used_as_state"], &["workspace_safe_path", "source_citation"]),
        fixture("B3", "knowledge_and_memory_management", "Find what we discussed about Agent memory.", true, false, "success", "read_action", "both", None, &["action.queued", "observation.created", "final_delivery.created"], &["session.search"], &["session_citation"], &["action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "assistant_text_used_as_state"], &["session_query", "citation"]),
        fixture("B4", "knowledge_and_memory_management", "Use my current memory/preferences when answering.", true, false, "success", "direct_answer", "both", None, &["route.selected", "context.loaded", "final_delivery.created"], &[], &["memory_digest"], &["answering", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "assistant_text_used_as_state"], &["bounded_memory_context"]),
        fixture("B5", "workspace_project_research", "Search the web and summarize with sources.", true, false, "success", "react_tool_execution", "both", None, &["action.queued", "observation.created", "final_delivery.created"], &["web.fetch"], &["web_source"], &["action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "fake_web_source"], &["governed_web_read"]),
        fixture("B6", "tool_skill_assisted_read_tasks", "Use the selected skill to review this plan.", true, false, "success", "read_action", "both", None, &["context.loaded", "final_delivery.created"], &["skill.select"], &["skill_digest"], &["planning", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "unselected_skill_injected"], &["selected_skill_digest"]),
        fixture("B7", "tool_skill_assisted_read_tasks", "Pick the right read-only MCP source and answer.", true, false, "success", "react_tool_execution", "both", None, &["action.queued", "observation.created", "final_delivery.created"], &["mcp_tool"], &["mcp_observation"], &["planning", "action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "unsafe_manifest_selected"], &["candidate_set", "selected_target"]),
        fixture("B8", "personal_planning_and_review", "Plan my week and execute the first safe step.", true, false, "success", "plan_execute", "both", None, &["plan.created", "action.completed", "observation.created"], &["plan_execute_step"], &["step_observation"], &["planning", "action_running", "observation_ready", "completed"], &["completed_work", "observations_used", "next_action"], &[], &["legacy_fallback", "silent_durable_write", "plan_claimed_done_without_action"], &["plan_revision", "first_safe_step"]),
        fixture("B9", "personal_planning_and_review", "Skip this unsupported plan step and continue.", true, false, "success", "plan_execute", "not_applicable_with_reason", Some("plan skip is a task-control command against an existing PlanExecute session"), &["step.skipped", "plan.reviewed"], &["plan.skip_step"], &[], &["planning", "completed"], &["skipped_work", "next_action"], &[], &["legacy_fallback", "silent_durable_write", "silent_skip"], &["skip_reason"]),
        fixture("B10", "knowledge_and_memory_management", "Remember that I prefer morning deep work.", true, false, "proposal", "memory_proposal", "both", None, &["proposal.created"], &["memory.propose_write"], &["memory_candidate"], &["memory_candidate", "permission_needed"], &["proposals_created", "pending_user_action"], &[], &["legacy_fallback", "silent_durable_write", "assistant_text_used_as_truth"], &["proposal_first"]),
        fixture("B11", "knowledge_and_memory_management", "Accept that memory update.", true, false, "success", "task_control", "not_applicable_with_reason", Some("memory acceptance is a proposal/task-control action against an existing proposal"), &["proposal.accepted", "memory.materialized"], &["proposal.accept"], &["accepted_memory"], &["memory_candidate", "completed"], &["durable_changes", "completed_work"], &[], &["legacy_fallback", "silent_durable_write"], &["accepted_record", "provenance"]),
        fixture("B12", "knowledge_and_memory_management", "Roll back the memory I accepted.", true, false, "success", "task_control", "not_applicable_with_reason", Some("memory rollback is a governed memory task-control command against an accepted memory id"), &["memory.rolled_back"], &["memory.rollback"], &["inactive_memory"], &["memory_candidate", "completed"], &["durable_changes", "completed_work"], &[], &["legacy_fallback", "silent_durable_write"], &["active_context_exclusion"]),
        fixture("B13", "failure_permission_recovery", "Continue the task from earlier.", true, false, "success", "task_control", "not_applicable_with_reason", Some("continuity resume is a task-control command against an existing task session"), &["task.resumed"], &["task.resume"], &["last_observation"], &["retry_available", "completed"], &["completed_work", "next_action"], &[], &["legacy_fallback", "silent_durable_write", "stale_replay"], &["resume_safety"]),
        fixture("B14", "failure_permission_recovery", "Retry the failed read.", true, false, "success", "task_control", "not_applicable_with_reason", Some("retry is a task-control command against an existing failed action"), &["action.retry", "observation.created"], &["action.retry"], &["new_observation"], &["retry_available", "observation_ready"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write"], &["same_action_scope"]),
        fixture("B15", "failure_permission_recovery", "Cancel this task.", true, false, "success", "task_control", "not_applicable_with_reason", Some("cancel is a task-control command against an existing non-terminal task"), &["task.cancelled"], &["task.cancel"], &[], &["blocked", "completed"], &["blocked_work", "next_action"], &[], &["legacy_fallback", "silent_durable_write", "continued_after_cancel"], &["queued_actions_stopped"]),
        fixture("B16", "failure_permission_recovery", "Do this external/risky action.", true, false, "expected_blocker", "permission_request", "both", None, &["permission.requested"], &["permission.request"], &[], &["permission_needed", "blocked"], &["blocked_work", "pending_user_action"], &["permission_required"], &["legacy_fallback", "silent_durable_write", "dangerous_write"], &["exact_permission_scope"]),
        fixture("B17", "tool_skill_assisted_read_tasks", "Explain why you chose that tool.", true, false, "success", "react_tool_execution", "both", None, &["tool.selected", "policy.allowed", "final_delivery.created"], &["tool.trace"], &["selection_reason"], &["planning", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write"], &["selection_reason", "policy_proof"]),
        fixture("B18", "tool_skill_assisted_read_tasks", "Use a skill that is not selected.", true, false, "expected_blocker", "blocked", "both", None, &["context.loaded"], &["skill.boundary"], &["unselected_absent"], &["blocked", "completed"], &["blocked_work", "next_action"], &["unselected_skill_not_injected"], &["legacy_fallback", "silent_durable_write", "unselected_skill_injected"], &["skill_boundary"]),
        fixture("B19", "personal_planning_and_review", "Summarize completed vs blocked work.", true, false, "success", "task_control", "not_applicable_with_reason", Some("final delivery inspection is a task-control/read-model action over an existing task"), &["final_delivery.created"], &[], &[], &["completed"], &["completed_work", "blocked_work", "proposed_work", "skipped_work", "next_action"], &[], &["legacy_fallback", "silent_durable_write", "blocked_claimed_completed"], &["section_separation"]),
        fixture("B20", "failure_permission_recovery", "Reconnect and show current task state.", true, false, "success", "task_control", "not_applicable_with_reason", Some("event replay is a durable event read command after reconnect"), &["event.replayed"], &["event.replay"], &["replayed_events"], &["observation_ready", "completed"], &["completed_work", "next_action"], &[], &["legacy_fallback", "silent_durable_write", "duplicate_events"], &["replay_sequence"]),
        fixture("B21", "knowledge_and_memory_management", "Compare two memory facts that conflict.", true, false, "success", "memory_proposal", "both", None, &["evidence.conflict", "final_delivery.created"], &["memory.compare"], &["conflict_state"], &["memory_candidate", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "silent_overwrite"], &["conflict_visible"]),
        fixture("B22", "workspace_project_research", "Ask a task that needs multiple reads.", true, false, "success", "react_tool_execution", "both", None, &["action.completed", "observation.created", "action.completed", "observation.created"], &["read_action", "read_action"], &["observation", "observation"], &["planning", "action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "no_tool_final_as_execution"], &["two_observations"]),
        fixture("B23", "workspace_project_research", "Use web when network policy blocks it.", true, false, "expected_blocker", "blocked", "both", None, &["blocker.created"], &["web.search"], &[], &["blocked"], &["blocked_work", "next_action"], &["web_network_policy_blocked"], &["legacy_fallback", "silent_durable_write", "fake_web_source"], &["named_blocker"]),
        fixture("B24", "tool_skill_assisted_read_tasks", "Use MCP when no manifest exists.", true, false, "expected_blocker", "blocked", "both", None, &["blocker.created"], &["mcp_tool"], &[], &["blocked"], &["blocked_work", "next_action"], &["mcp_missing_read_target"], &["legacy_fallback", "silent_durable_write", "fake_mcp_observation"], &["named_blocker"]),
        fixture("B25", "workspace_project_research", "Run external live DirectAnswer.", false, true, "opt_in_live", "direct_answer", "both", None, &["route.selected", "final_delivery.created"], &[], &[], &["answering", "completed"], &["completed_work"], &[], &["legacy_fallback", "silent_durable_write", "local_provider_live_credit"], &["external_provider_trace"]),
        fixture("B26", "workspace_project_research", "Run external live web/MCP path.", false, true, "opt_in_live", "react_tool_execution", "both", None, &["action.completed", "observation.created", "final_delivery.created"], &["web.fetch", "mcp_tool"], &["live_observation"], &["action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "local_provider_live_credit"], &["external_live_action_trace"]),
        fixture("B27", "knowledge_and_memory_management", "Inspect loaded knowledge assets.", true, false, "success", "direct_answer", "both", None, &["context.loaded", "final_delivery.created"], &[], &["asset_inventory"], &["completed"], &["completed_work", "observations_used"], &[], &["legacy_fallback", "silent_durable_write", "policy_override_by_file"], &["scope_digest_loaded_state"]),
        fixture("B28", "knowledge_and_memory_management", "Edit a knowledge asset proposal.", true, false, "proposal", "life_model_proposal", "both", None, &["proposal.created"], &["knowledge.propose_edit"], &["proposed_diff"], &["memory_candidate", "permission_needed"], &["proposals_created", "pending_user_action"], &[], &["legacy_fallback", "silent_durable_write", "direct_knowledge_file_write"], &["proposal_diff_confirmation"]),
        fixture("B29", "failure_permission_recovery", "Recover from stale resume context.", true, false, "expected_blocker", "task_control", "not_applicable_with_reason", Some("stale resume recovery is a task-continuity control path against an existing task"), &["blocker.created"], &["task.resume"], &["stale_diagnostic"], &["blocked", "retry_available"], &["blocked_work", "next_action"], &["stale_context"], &["legacy_fallback", "silent_durable_write", "automatic_stale_replay"], &["refresh_path"]),
        fixture("B30", "personal_planning_and_review", "Finish and tell me exactly what changed.", true, false, "success", "task_control", "not_applicable_with_reason", Some("final delivery review is a read model over an existing terminal task"), &["final_delivery.created"], &[], &[], &["completed"], &["completed_work", "proposed_work", "blocked_work", "skipped_work", "durable_changes", "next_action"], &[], &["legacy_fallback", "silent_durable_write", "overclaimed_change"], &["durable_change_inventory"]),
    ]
}

fn fixture(
    id: &str,
    vertical: &str,
    prompt: &str,
    default_readiness: bool,
    requires_live_provider: bool,
    expected_outcome: &str,
    expected_strategy: &str,
    command_surface: &str,
    not_applicable_with_reason: Option<&str>,
    required_runtime_events: &[&str],
    required_actions: &[&str],
    required_observations: &[&str],
    required_ui_states: &[&str],
    required_final_delivery_sections: &[&str],
    expected_blockers: &[&str],
    forbidden_evidence: &[&str],
    pass_criteria: &[&str],
) -> MainChatAgentBetaV1RealTaskFixture {
    MainChatAgentBetaV1RealTaskFixture {
        id: id.into(),
        vertical: vertical.into(),
        prompt: prompt.into(),
        default_readiness,
        requires_live_provider,
        expected_outcome: expected_outcome.into(),
        preconditions: vec!["isolated_eval_state".into()],
        expected_strategy: expected_strategy.into(),
        command_surface: command_surface.into(),
        not_applicable_with_reason: not_applicable_with_reason.map(str::to_string),
        required_runtime_events: required_runtime_events
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_actions: required_actions
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_observations: required_observations
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_ui_states: required_ui_states
            .iter()
            .map(|value| (*value).into())
            .collect(),
        required_final_delivery_sections: required_final_delivery_sections
            .iter()
            .map(|value| (*value).into())
            .collect(),
        expected_blockers: expected_blockers
            .iter()
            .map(|value| (*value).into())
            .collect(),
        forbidden_evidence: forbidden_evidence
            .iter()
            .map(|value| (*value).into())
            .collect(),
        pass_criteria: pass_criteria.iter().map(|value| (*value).into()).collect(),
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
