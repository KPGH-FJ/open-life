use openlife_core::agent::main_chat_agent_productization_v1::{
    assemble_main_chat_agent_state, main_chat_agent_product_scenarios,
    MainChatAgentProductScenario, MainChatAgentProductScenarioExpectation,
    MainChatAgentProductScenarioRunMode, MainChatAgentProductStrategyRoute,
    MainChatAgentStateAssemblerInput, MainChatAgentStateEventType, MainChatAgentStateSnapshot,
};
use openlife_core::agent::main_chat_agent_v1::{
    ActionQueueStore, AgentTaskSessionDraft, AgentTaskSessionStore, ExecutionAction,
    ExecutionPolicy, ExecutionQueueStatus, ExecutionTranscriptEntryDraft,
    ExecutionTranscriptEntryKind, MainChatAgentStrategy,
};
use openlife_core::agent::proposal_store::ProposalStore;
use openlife_core::agent::types::{
    AgentProposal, AgentRun, AgentRunStatus, AgentTaskKind, ContextSummary, ModelRouteTrace,
    ProposalSource, ProposalStatus, ProposalType, RedactionLevel, RiskLevel,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentProductizationRouteCount {
    pub passed: usize,
    pub failed: usize,
    pub expected_blocker: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentProductizationUnsupportedScenario {
    pub scenario_id: String,
    pub route: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentProductizationFailedScenario {
    pub scenario_id: String,
    pub route: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductScenarioRuntimeProof {
    pub scenario_id: String,
    pub group: String,
    pub passed: bool,
    pub runtime_object_count: usize,
    pub observation_count: usize,
    pub created_action_ids: Vec<String>,
    pub created_observation_ids: Vec<String>,
    pub created_proposal_ids: Vec<String>,
    pub final_delivery_id: Option<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentProductizationV1GateReport {
    pub total_scenario_count: usize,
    pub default_deterministic_scenario_count: usize,
    pub readiness_semantics: String,
    pub runtime_execution_scope: String,
    pub executed_scenario_count: usize,
    pub passed_scenario_count: usize,
    pub expected_blocker_scenario_count: usize,
    pub failed_scenario_count: usize,
    pub external_live_excluded_count: usize,
    pub runtime_payload_snapshot_event_gate_passed: bool,
    pub runtime_required_group_count: usize,
    pub runtime_required_group_passed_count: usize,
    pub representative_runtime_group_count: usize,
    pub representative_runtime_group_passed_count: usize,
    pub full_deterministic_runtime_scenario_count: usize,
    pub full_deterministic_runtime_scenario_executed_count: usize,
    pub runtime_required_group_evidence: Vec<ProductScenarioRuntimeProof>,
    pub event_semantics: String,
    pub final_readiness_ready: bool,
    pub full_productization_v1_complete: bool,
    pub future_work: Vec<String>,
    pub route_counts: BTreeMap<String, MainChatAgentProductizationRouteCount>,
    pub unsupported_scenarios: Vec<MainChatAgentProductizationUnsupportedScenario>,
    pub failed_scenarios: Vec<MainChatAgentProductizationFailedScenario>,
    pub blockers: Vec<String>,
}

pub(crate) fn run_main_chat_agent_productization_v1_gate_report(
) -> MainChatAgentProductizationV1GateReport {
    run_main_chat_agent_productization_v1_gate_report_with_runtime(
        execute_runtime_backed_product_scenario,
    )
}

pub(crate) fn run_main_chat_agent_productization_v1_gate_report_with_runtime<F>(
    runtime_executor: F,
) -> MainChatAgentProductizationV1GateReport
where
    F: Fn(&MainChatAgentProductScenario) -> Result<ProductScenarioRuntimeProof, String>,
{
    let scenarios = main_chat_agent_product_scenarios();
    let runtime_payload_snapshot_event_gate_passed =
        main_chat_productization_payload_smoke_gate_passes();
    let runtime_required_group_evidence =
        execute_required_runtime_product_scenarios(&scenarios, runtime_executor);
    let runtime_required_group_count = runtime_required_group_evidence.len();
    let runtime_required_group_passed_count = runtime_required_group_evidence
        .iter()
        .filter(|proof| proof.passed)
        .count();
    let mut route_counts = canonical_route_count_map();
    let mut unsupported_scenarios = Vec::new();
    let mut failed_scenarios = Vec::new();
    let mut external_live_excluded_count = 0usize;
    let mut default_deterministic_scenario_count = 0usize;
    let mut executed_scenario_count = 0usize;
    let mut passed_scenario_count = 0usize;
    let mut expected_blocker_scenario_count = 0usize;
    let mut failed_scenario_count = 0usize;

    for scenario in &scenarios {
        if scenario.run_mode == MainChatAgentProductScenarioRunMode::ExternalLiveOptIn {
            external_live_excluded_count += 1;
            continue;
        }
        if !scenario.included_in_default_gate {
            continue;
        }
        default_deterministic_scenario_count += 1;
        let route = scenario.expected_strategy_route.as_str().to_string();
        let counts = route_counts.entry(route.clone()).or_default();
        match scenario.expectation {
            MainChatAgentProductScenarioExpectation::MustPass => {
                executed_scenario_count += 1;
                match execute_deterministic_product_scenario(scenario) {
                    Ok(()) => {
                        counts.passed += 1;
                        passed_scenario_count += 1;
                    }
                    Err(reason) => {
                        counts.failed += 1;
                        failed_scenario_count += 1;
                        failed_scenarios.push(MainChatAgentProductizationFailedScenario {
                            scenario_id: scenario.id.clone(),
                            route,
                            reason,
                        });
                    }
                }
            }
            MainChatAgentProductScenarioExpectation::ExpectedBlocker => {
                executed_scenario_count += 1;
                match execute_deterministic_product_scenario(scenario) {
                    Ok(()) => {
                        counts.expected_blocker += 1;
                        expected_blocker_scenario_count += 1;
                    }
                    Err(reason) => {
                        counts.failed += 1;
                        failed_scenario_count += 1;
                        failed_scenarios.push(MainChatAgentProductizationFailedScenario {
                            scenario_id: scenario.id.clone(),
                            route,
                            reason,
                        });
                    }
                }
            }
            MainChatAgentProductScenarioExpectation::OptionalUnsupported => {
                counts.unsupported += 1;
                unsupported_scenarios.push(MainChatAgentProductizationUnsupportedScenario {
                    scenario_id: scenario.id.clone(),
                    route,
                    reason: scenario
                        .unsupported_reason
                        .clone()
                        .unwrap_or_else(|| "Optional unsupported scenario.".into()),
                });
            }
        }
    }

    let mut blockers = Vec::new();
    if !runtime_payload_snapshot_event_gate_passed {
        blockers.push("runtime_payload_snapshot_event_gate_failed".into());
    }
    if failed_scenario_count > 0 {
        blockers.push("default_product_scenarios_failed".into());
    }
    if runtime_required_group_passed_count != runtime_required_group_count {
        blockers.push("runtime_required_scenarios_not_executed".into());
    }
    if unsupported_scenarios
        .iter()
        .any(|scenario| scenario.scenario_id != "MP-06")
    {
        blockers.push("unexpected_unsupported_scenario_present".into());
    }
    if executed_scenario_count + unsupported_scenarios.len() != default_deterministic_scenario_count
    {
        blockers.push("default_product_scenario_accounting_mismatch".into());
    }

    MainChatAgentProductizationV1GateReport {
        total_scenario_count: scenarios.len(),
        default_deterministic_scenario_count,
        readiness_semantics: "acceptance_hardening_representative_gate_ready".into(),
        runtime_execution_scope:
            "representative_runtime_groups_only_full_92_scenario_runtime_execution_future_work"
                .into(),
        executed_scenario_count,
        passed_scenario_count,
        expected_blocker_scenario_count,
        failed_scenario_count,
        external_live_excluded_count,
        runtime_payload_snapshot_event_gate_passed,
        runtime_required_group_count,
        runtime_required_group_passed_count,
        representative_runtime_group_count: runtime_required_group_count,
        representative_runtime_group_passed_count: runtime_required_group_passed_count,
        full_deterministic_runtime_scenario_count: default_deterministic_scenario_count,
        full_deterministic_runtime_scenario_executed_count: runtime_required_group_count,
        runtime_required_group_evidence,
        event_semantics: "snapshot_derived_ordered_events_not_live_delta_stream".into(),
        final_readiness_ready: blockers.is_empty(),
        full_productization_v1_complete: false,
        future_work: vec!["full_92_scenario_runtime_execution".into()],
        route_counts,
        unsupported_scenarios,
        failed_scenarios,
        blockers,
    }
}

fn required_runtime_product_scenarios() -> [(&'static str, &'static str); 11] {
    [
        ("OA-02", "direct_answer"),
        ("FR-01", "file_read"),
        ("MS-01", "memory_session_read"),
        ("WR-01", "fixture_web_read"),
        ("MCP-01", "registered_mcp_read"),
        ("RA-01", "multi_step_react_two_observations"),
        ("PE-01", "plan_execute_mvp"),
        ("MP-01", "memory_proposal_lifecycle_or_mp06_unsupported"),
        ("PB-01", "permission_request_exact_action"),
        ("LT-03", "task_control_resume_retry_cancel"),
        ("FD-02", "final_delivery_separation"),
    ]
}

fn execute_required_runtime_product_scenarios<F>(
    scenarios: &[MainChatAgentProductScenario],
    runtime_executor: F,
) -> Vec<ProductScenarioRuntimeProof>
where
    F: Fn(&MainChatAgentProductScenario) -> Result<ProductScenarioRuntimeProof, String>,
{
    required_runtime_product_scenarios()
        .into_iter()
        .map(|(scenario_id, group)| {
            let Some(scenario) = scenarios.iter().find(|scenario| scenario.id == scenario_id)
            else {
                return failed_runtime_proof(
                    scenario_id,
                    group,
                    "required scenario row missing from productization inventory",
                );
            };
            let mut proof = match runtime_executor(scenario) {
                Ok(proof) => proof,
                Err(reason) => return failed_runtime_proof(scenario_id, group, &reason),
            };
            proof
                .diagnostics
                .extend(validate_runtime_product_proof(scenario_id, group, &proof));
            if !proof.diagnostics.is_empty() {
                proof.passed = false;
            }
            proof
        })
        .collect()
}

fn validate_runtime_product_proof(
    scenario_id: &str,
    group: &str,
    proof: &ProductScenarioRuntimeProof,
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if proof.scenario_id != scenario_id {
        diagnostics.push(format!(
            "runtime proof scenario mismatch: expected {scenario_id}, got {}",
            proof.scenario_id
        ));
    }
    if proof.group != group {
        diagnostics.push(format!(
            "runtime proof group mismatch: expected {group}, got {}",
            proof.group
        ));
    }
    if proof.runtime_object_count == 0 {
        diagnostics.push("runtime proof did not create or load runtime objects".into());
    }
    if !proof.passed {
        diagnostics.push("runtime proof reported failure".into());
    }
    match group {
        "direct_answer" => {
            if !proof.created_action_ids.is_empty() || proof.observation_count != 0 {
                diagnostics
                    .push("DirectAnswer proof must not fabricate action observations".into());
            }
            if proof.final_delivery_id.is_none() {
                diagnostics.push("DirectAnswer proof lacks final delivery evidence".into());
            }
        }
        "file_read" | "memory_session_read" | "fixture_web_read" | "registered_mcp_read" => {
            if proof.created_action_ids.is_empty() || proof.observation_count == 0 {
                diagnostics.push("read proof lacks action/observation runtime evidence".into());
            }
            if proof.final_delivery_id.is_none() {
                diagnostics.push("read proof lacks final delivery evidence".into());
            }
        }
        "multi_step_react_two_observations" => {
            if proof.observation_count < 2 || proof.created_action_ids.len() < 2 {
                diagnostics
                    .push("multi-step ReAct proof requires at least two observations".into());
            }
            if proof.final_delivery_id.is_none() {
                diagnostics.push("multi-step ReAct proof lacks final delivery evidence".into());
            }
        }
        "plan_execute_mvp" => {
            if proof.final_delivery_id.is_none() {
                diagnostics.push("PlanExecute proof lacks final delivery evidence".into());
            }
        }
        "memory_proposal_lifecycle_or_mp06_unsupported" => {
            if proof.created_proposal_ids.len() < 5 {
                diagnostics.push(
                    "memory proposal lifecycle proof must create/edit/accept/reject/defer proposals"
                        .into(),
                );
            }
        }
        "permission_request_exact_action" => {
            if proof.created_action_ids.len() != 1 || proof.created_proposal_ids.len() != 1 {
                diagnostics.push(
                    "permission proof must bind one proposal/blocker to one exact action".into(),
                );
            }
        }
        "task_control_resume_retry_cancel" => {
            if proof.runtime_object_count < 6 {
                diagnostics.push(
                    "task control proof must load prior sessions/actions for resume/retry/cancel"
                        .into(),
                );
            }
        }
        "final_delivery_separation" => {
            if proof.final_delivery_id.is_none()
                || proof.created_action_ids.is_empty()
                || proof.created_observation_ids.is_empty()
                || proof.created_proposal_ids.is_empty()
            {
                diagnostics.push(
                    "final delivery separation proof lacks separated action/source/proposal evidence"
                        .into(),
                );
            }
        }
        _ => diagnostics.push("unknown runtime proof group".into()),
    }
    diagnostics
}

fn failed_runtime_proof(
    scenario_id: &str,
    group: &str,
    reason: &str,
) -> ProductScenarioRuntimeProof {
    ProductScenarioRuntimeProof {
        scenario_id: scenario_id.into(),
        group: group.into(),
        passed: false,
        runtime_object_count: 0,
        observation_count: 0,
        created_action_ids: Vec::new(),
        created_observation_ids: Vec::new(),
        created_proposal_ids: Vec::new(),
        final_delivery_id: None,
        diagnostics: vec![reason.into()],
    }
}

fn execute_deterministic_product_scenario(
    scenario: &MainChatAgentProductScenario,
) -> Result<(), String> {
    if scenario.run_mode != MainChatAgentProductScenarioRunMode::DeterministicFixture
        && scenario.run_mode != MainChatAgentProductScenarioRunMode::MockIpcUi
    {
        return Err("default gate cannot execute non-deterministic scenario".into());
    }
    if !scenario.included_in_default_gate {
        return Err("scenario excluded from default gate".into());
    }
    if scenario.required_ui_states.is_empty() {
        return Err("missing required UI state contract".into());
    }
    if scenario.required_runtime_evidence.is_empty() {
        return Err("missing required runtime evidence contract".into());
    }
    if !scenario
        .negative_assertions
        .iter()
        .any(|assertion| assertion == "no_silent_durable_write")
    {
        return Err("missing no-silent-write negative assertion".into());
    }
    let anti_fake_assertion = if scenario.user_turn_type == "task_control" {
        "no_fake_control_result"
    } else {
        "no_fake_execution_ui"
    };
    if !scenario
        .negative_assertions
        .iter()
        .any(|assertion| assertion == anti_fake_assertion)
    {
        return Err("missing anti-fake UI assertion".into());
    }
    if scenario.expected_strategy_route == MainChatAgentProductStrategyRoute::TaskControl {
        validate_task_control_scenario(scenario)?;
    }
    match scenario.expectation {
        MainChatAgentProductScenarioExpectation::ExpectedBlocker => {
            if !scenario
                .required_runtime_evidence
                .iter()
                .any(|evidence| evidence == "blocker_id")
            {
                return Err("expected blocker scenario lacks blocker evidence".into());
            }
            if scenario.expected_strategy_route != MainChatAgentProductStrategyRoute::Blocked
                && scenario.expected_strategy_route
                    != MainChatAgentProductStrategyRoute::PermissionRequest
            {
                return Err("expected blocker must use blocked or permission_request route".into());
            }
        }
        MainChatAgentProductScenarioExpectation::MustPass => {
            validate_success_scenario_evidence(scenario)?;
        }
        MainChatAgentProductScenarioExpectation::OptionalUnsupported => {
            return Err("optional unsupported scenario cannot execute as supported".into());
        }
    }
    Ok(())
}

fn validate_success_scenario_evidence(
    scenario: &MainChatAgentProductScenario,
) -> Result<(), String> {
    let evidence = |needle: &str| {
        scenario
            .required_runtime_evidence
            .iter()
            .any(|value| value == needle)
    };
    match scenario.expected_strategy_route {
        MainChatAgentProductStrategyRoute::DirectAnswer => {
            require(
                evidence("task_id") && evidence("run_id") && evidence("final_delivery"),
                "direct answer requires task/run/final delivery evidence",
            )?;
        }
        MainChatAgentProductStrategyRoute::ReadAction => {
            require(
                evidence("task_id")
                    && evidence("action_id")
                    && evidence("observation_id")
                    && evidence("final_delivery"),
                "read action requires action, observation, and final delivery evidence",
            )?;
        }
        MainChatAgentProductStrategyRoute::ReactToolExecution => {
            require(
                evidence("task_id")
                    && evidence("action_id")
                    && evidence("observation_id")
                    && evidence("final_delivery"),
                "ReAct requires action, observation, and final delivery evidence",
            )?;
        }
        MainChatAgentProductStrategyRoute::PlanExecute => {
            require(
                evidence("task_id") && evidence("plan_id") && evidence("final_delivery"),
                "PlanExecute requires plan and final delivery evidence",
            )?;
        }
        MainChatAgentProductStrategyRoute::MemoryProposal => {
            require(
                evidence("task_id") && evidence("proposal_id") && evidence("evidence_id"),
                "memory proposal requires proposal and evidence ids",
            )?;
            require(
                scenario.durable_change == "proposal_only",
                "memory proposal must remain proposal-only before acceptance",
            )?;
        }
        MainChatAgentProductStrategyRoute::PermissionRequest => {
            require(
                evidence("task_id") && evidence("proposal_id") && evidence("blocker_id"),
                "permission request requires proposal and blocker evidence",
            )?;
        }
        MainChatAgentProductStrategyRoute::TaskControl => {
            require(
                evidence("prior_task_session_id")
                    && evidence("prior_run_id")
                    && evidence("target_object_id")
                    && evidence("state_transition"),
                "task control requires prior object and exact state transition evidence",
            )?;
        }
        MainChatAgentProductStrategyRoute::Blocked => {
            require(
                evidence("task_id") && evidence("blocker_id"),
                "blocked route requires blocker evidence",
            )?;
        }
        MainChatAgentProductStrategyRoute::LegacyFallback
        | MainChatAgentProductStrategyRoute::Unknown => {
            return Err("legacy/unknown route cannot be supported deterministic completion".into());
        }
    }
    Ok(())
}

fn validate_task_control_scenario(scenario: &MainChatAgentProductScenario) -> Result<(), String> {
    let preconditions = scenario
        .preconditions
        .as_ref()
        .ok_or_else(|| "task_control scenario lacks preconditions".to_string())?;
    require(
        preconditions.prior_task_session_id.is_some() && preconditions.prior_run_id.is_some(),
        "task_control scenario lacks prior task/run references",
    )?;
    require(
        preconditions.target_action_id.is_some()
            || preconditions.target_proposal_id.is_some()
            || preconditions.target_blocker_id.is_some()
            || preconditions.target_final_delivery_id.is_some(),
        "task_control scenario lacks exact target object reference",
    )?;
    require(
        scenario.control_action.is_some() && scenario.expected_state_transition.is_some(),
        "task_control scenario lacks control action or state transition",
    )?;
    require(
        scenario
            .negative_assertions
            .iter()
            .any(|assertion| assertion == "no_changed_target_replay"),
        "task_control scenario lacks exact-target negative assertion",
    )?;
    Ok(())
}

fn require(condition: bool, reason: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(reason.into())
    }
}

fn execute_runtime_backed_product_scenario(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    match scenario.id.as_str() {
        "OA-02" => runtime_direct_answer_proof(scenario),
        "FR-01" => runtime_single_read_proof(
            scenario,
            "file_read",
            "file.read",
            "plans/main_chat_agent_productization_v1_goal_spec.md",
            "file",
            "plans/main_chat_agent_productization_v1_goal_spec.md",
        ),
        "MS-01" => runtime_single_read_proof(
            scenario,
            "memory_session_read",
            "memory.search",
            "accepted Main Chat memory and previous session consensus",
            "memory",
            "memory:main_chat_consensus",
        ),
        "WR-01" => runtime_single_read_proof(
            scenario,
            "fixture_web_read",
            "web.fetch",
            "fixture://main-chat-agent-productization",
            "web_fixture",
            "fixture://main-chat-agent-productization",
        ),
        "MCP-01" => runtime_single_read_proof(
            scenario,
            "registered_mcp_read",
            "mcp.read_only",
            "registered://openlife.project_status.read",
            "mcp",
            "openlife.project_status.read",
        ),
        "RA-01" => runtime_multi_step_react_proof(scenario),
        "PE-01" => runtime_plan_execute_mvp_proof(scenario),
        "MP-01" => runtime_memory_proposal_lifecycle_proof(scenario),
        "PB-01" => runtime_permission_request_exact_action_proof(scenario),
        "LT-03" => productization_task_control_resume_retry_cancel_runtime_proof(scenario),
        "FD-02" => runtime_final_delivery_separation_proof(scenario),
        _ => Err(format!(
            "no runtime-backed productization proof registered for {}",
            scenario.id
        )),
    }
}

fn runtime_direct_answer_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::DirectAnswer,
            current_plan_summary: None,
            context_snapshot_refs: vec!["ctx:productization:direct_answer".into()],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "direct_answer")?;
    append_final_result(
        &session_store,
        &session.id,
        "DirectAnswer completed without tool execution.",
        serde_json::json!({ "directWritesExecuted": false }),
    )?;
    session_store
        .complete_session(
            &session.id,
            "DirectAnswer completed without tool execution.",
        )
        .map_err(|err| err.to_string())?;
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(&session.chat_session_id, scenario, "direct_answer"),
        Vec::new(),
    )?;
    let mut proof = proof_from_snapshot(scenario, "direct_answer", &snapshot, 0);
    if !snapshot.actions.is_empty() || !snapshot.observations.is_empty() {
        proof
            .diagnostics
            .push("DirectAnswer snapshot contained fake action evidence".into());
        proof.passed = false;
    }
    Ok(proof)
}

fn runtime_single_read_proof(
    scenario: &MainChatAgentProductScenario,
    group: &str,
    action_type: &str,
    target: &str,
    source_kind: &str,
    source_label: &str,
) -> Result<ProductScenarioRuntimeProof, String> {
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some(format!("Read {source_label} and synthesize.")),
            context_snapshot_refs: vec![format!("ctx:productization:{group}")],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "read_action")?;
    append_plan(
        &session_store,
        &session.id,
        &format!("Read {source_label} and produce a grounded answer."),
    )?;
    let completed = enqueue_completed_action(
        &session_store,
        &action_queue,
        &session.id,
        action_type,
        target,
        serde_json::json!({
            "sourceKind": source_kind,
            "sourceLabel": source_label,
            "directWritesExecuted": false
        }),
    )?;
    append_observation(
        &session_store,
        &session.id,
        &completed.id,
        source_kind,
        source_label,
        "Runtime-backed read observation was produced by an action queue item.",
    )?;
    append_final_result(
        &session_store,
        &session.id,
        "Read action completed with runtime-backed observation evidence.",
        serde_json::json!({
            "actionId": completed.id,
            "directWritesExecuted": false
        }),
    )?;
    session_store
        .complete_session(
            &session.id,
            "Read action completed with runtime-backed observation evidence.",
        )
        .map_err(|err| err.to_string())?;
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(&session.chat_session_id, scenario, group),
        Vec::new(),
    )?;
    Ok(proof_from_snapshot(scenario, group, &snapshot, 0))
}

fn runtime_multi_step_react_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Read two sources, compare them, then synthesize.".into()),
            context_snapshot_refs: vec!["ctx:productization:multi_step_react".into()],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "react_tool_execution")?;
    append_plan(
        &session_store,
        &session.id,
        "Read the matrix, then read README, then compare evidence.",
    )?;
    let first = enqueue_completed_action(
        &session_store,
        &action_queue,
        &session.id,
        "file.read",
        "plans/main_chat_agent_product_eval_scenarios_v1.md",
        serde_json::json!({ "sourceKind": "file", "sourceLabel": "plans/main_chat_agent_product_eval_scenarios_v1.md" }),
    )?;
    append_observation(
        &session_store,
        &session.id,
        &first.id,
        "file",
        "plans/main_chat_agent_product_eval_scenarios_v1.md",
        "Observed scenario matrix evidence.",
    )?;
    let second = enqueue_completed_action(
        &session_store,
        &action_queue,
        &session.id,
        "file.read",
        "README.md",
        serde_json::json!({ "sourceKind": "file", "sourceLabel": "README.md" }),
    )?;
    append_observation(
        &session_store,
        &session.id,
        &second.id,
        "file",
        "README.md",
        "Observed README productization status evidence.",
    )?;
    append_final_result(
        &session_store,
        &session.id,
        "Multi-step ReAct completed after two runtime observations.",
        serde_json::json!({
            "observationCount": 2,
            "directWritesExecuted": false
        }),
    )?;
    session_store
        .complete_session(
            &session.id,
            "Multi-step ReAct completed after two runtime observations.",
        )
        .map_err(|err| err.to_string())?;
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(
            &session.chat_session_id,
            scenario,
            "multi_step_react_two_observations",
        ),
        Vec::new(),
    )?;
    Ok(proof_from_snapshot(
        scenario,
        "multi_step_react_two_observations",
        &snapshot,
        0,
    ))
}

fn runtime_plan_execute_mvp_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::PlanExecute,
            current_plan_summary: Some("Draft a plan and execute the first safe read step.".into()),
            context_snapshot_refs: vec!["ctx:productization:plan_execute".into()],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "plan_execute")?;
    append_plan(
        &session_store,
        &session.id,
        "PlanExecute MVP created a plan and ran the first safe read step.",
    )?;
    let completed = enqueue_completed_action(
        &session_store,
        &action_queue,
        &session.id,
        "file.read",
        "plans/main_chat_agent_productization_v1_goal_spec.md",
        serde_json::json!({ "sourceKind": "file", "sourceLabel": "plans/main_chat_agent_productization_v1_goal_spec.md" }),
    )?;
    append_observation(
        &session_store,
        &session.id,
        &completed.id,
        "file",
        "plans/main_chat_agent_productization_v1_goal_spec.md",
        "Observed first PlanExecute read step.",
    )?;
    append_final_result(
        &session_store,
        &session.id,
        "PlanExecute MVP completed one governed read step.",
        serde_json::json!({ "planId": "plan-productization-mvp" }),
    )?;
    session_store
        .complete_session(
            &session.id,
            "PlanExecute MVP completed one governed read step.",
        )
        .map_err(|err| err.to_string())?;
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(&session.chat_session_id, scenario, "plan_execute_mvp"),
        Vec::new(),
    )?;
    let mut proof = proof_from_snapshot(scenario, "plan_execute_mvp", &snapshot, 0);
    if snapshot.plan.is_none() {
        proof
            .diagnostics
            .push("PlanExecute proof did not assemble plan evidence".into());
        proof.passed = false;
    }
    Ok(proof)
}

fn runtime_memory_proposal_lifecycle_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let proposal_store = ProposalStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::MemoryProposal,
            current_plan_summary: Some("Create reviewable memory proposal evidence.".into()),
            context_snapshot_refs: vec!["ctx:productization:memory_proposal".into()],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "memory_proposal")?;
    append_final_result(
        &session_store,
        &session.id,
        "Memory proposal lifecycle remains proposal-first and reviewable.",
        serde_json::json!({ "directWritesExecuted": false }),
    )?;
    session_store
        .complete_session(
            &session.id,
            "Memory proposal lifecycle remains proposal-first and reviewable.",
        )
        .map_err(|err| err.to_string())?;

    let mut proposals = Vec::new();
    for (suffix, outcome) in [
        ("create", "pending"),
        ("edit", "edited"),
        ("accept", "accepted"),
        ("reject", "rejected"),
        ("defer", "postponed"),
    ] {
        let mut proposal = memory_proposal_fixture(&session.id, suffix);
        proposal.id = format!("proposal-{}-{suffix}", scenario.id);
        proposal.run_id = Some(format!("run-productization-{}", scenario.id));
        proposal_store
            .create_proposal(&proposal)
            .map_err(|err| err.to_string())?;
        match outcome {
            "edited" => proposal.edit(serde_json::json!({
                "text": "Prefer execution-first Agent behavior, scoped to this project."
            })),
            "accepted" => proposal.accept(),
            "rejected" => proposal.reject(),
            "postponed" => proposal.postpone(),
            _ => {}
        }
        if outcome != "pending" {
            proposal_store
                .update_proposal(&proposal)
                .map_err(|err| err.to_string())?;
        }
        proposals.push(proposal);
    }
    let loaded = proposal_store
        .list_all_proposals(20, 0)
        .map_err(|err| err.to_string())?;
    let statuses = loaded
        .iter()
        .map(|proposal| proposal.status)
        .collect::<Vec<_>>();
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(
            &session.chat_session_id,
            scenario,
            "memory_proposal_lifecycle_or_mp06_unsupported",
        ),
        loaded,
    )?;
    let mut proof = proof_from_snapshot(
        scenario,
        "memory_proposal_lifecycle_or_mp06_unsupported",
        &snapshot,
        proposals.len(),
    );
    for required in [
        ProposalStatus::Pending,
        ProposalStatus::Edited,
        ProposalStatus::Accepted,
        ProposalStatus::Rejected,
        ProposalStatus::Postponed,
    ] {
        if !statuses.contains(&required) {
            proof
                .diagnostics
                .push(format!("proposal lifecycle missing {required:?} status"));
            proof.passed = false;
        }
    }
    Ok(proof)
}

fn runtime_permission_request_exact_action_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let proposal_store = ProposalStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Request permission before exact action replay.".into()),
            context_snapshot_refs: vec!["ctx:productization:permission".into()],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "permission_request")?;
    let policy = ExecutionPolicy;
    let action = ExecutionAction::new("memory.write", "long-term memory write exact action");
    let queued = action_queue
        .enqueue(&session.id, action.clone(), policy.classify(&action))
        .map_err(|err| err.to_string())?;
    session_store
        .record_action_queue_id(&session.id, &queued.id)
        .map_err(|err| err.to_string())?;
    session_store
        .set_pending_blockers(&session.id, vec![format!("permission:{}", queued.id)])
        .map_err(|err| err.to_string())?;
    session_store
        .mark_waiting_permission(&session.id)
        .map_err(|err| err.to_string())?;
    let mut proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tools.permissions.memory.write",
        serde_json::json!({ "actionId": queued.id, "permission": "allow_once" }),
        "Exact action permission is required before continuing.",
        0.87,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    proposal.id = format!("proposal-{}-permission", scenario.id);
    proposal.run_id = Some(format!("run-productization-{}", scenario.id));
    proposal.source_detail = Some(session.id.clone());
    proposal_store
        .create_proposal(&proposal)
        .map_err(|err| err.to_string())?;
    append_final_result(
        &session_store,
        &session.id,
        "Permission request is waiting on the exact queued action.",
        serde_json::json!({
            "actionId": queued.id,
            "proposalId": proposal.id,
            "directWritesExecuted": false
        }),
    )?;
    let loaded = proposal_store
        .list_all_proposals(10, 0)
        .map_err(|err| err.to_string())?;
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(
            &session.chat_session_id,
            scenario,
            "permission_request_exact_action",
        ),
        loaded,
    )?;
    let mut proof = proof_from_snapshot(scenario, "permission_request_exact_action", &snapshot, 0);
    let exact_blocker = snapshot
        .blockers
        .iter()
        .any(|blocker| blocker.affected_action_id.as_deref() == Some(&queued.id));
    if !exact_blocker {
        proof
            .diagnostics
            .push("permission blocker did not target the exact queued action".into());
        proof.passed = false;
    }
    Ok(proof)
}

pub(crate) fn productization_task_control_resume_retry_cancel_runtime_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let policy = ExecutionPolicy;
    let mut diagnostics = Vec::new();
    let mut runtime_object_count = 0usize;
    let mut action_ids = Vec::new();

    let resume_session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:resume", scenario.id),
            user_goal: "Resume a blocked prior task.".into(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Resume exact prior task.".into()),
            context_snapshot_refs: vec![],
        })
        .map_err(|err| err.to_string())?;
    session_store
        .block_session(&resume_session.id, "Blocked before resume.")
        .map_err(|err| err.to_string())?;
    let loaded_resume = session_store
        .load_session(&resume_session.id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "resume prior task missing".to_string())?;
    runtime_object_count += 1;
    let resume_decision = openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_task_resume(
        Some(&loaded_resume),
        &[],
    );
    if !resume_decision.allowed {
        diagnostics.push(format!(
            "resume rejected for existing task: {}",
            resume_decision.reason_code
        ));
    }
    let resumed = session_store
        .resume_session(&loaded_resume.id)
        .map_err(|err| err.to_string())?;
    if resumed.status.as_str() != "running" {
        diagnostics.push("resume transition did not reach running".into());
    }

    let retry_session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:retry", scenario.id),
            user_goal: "Retry a failed prior action.".into(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Retry exact failed action.".into()),
            context_snapshot_refs: vec![],
        })
        .map_err(|err| err.to_string())?;
    let retry_action = ExecutionAction::new(
        "file.read",
        "plans/main_chat_agent_productization_v1_goal_spec.md",
    );
    let queued_retry = action_queue
        .enqueue(
            &retry_session.id,
            retry_action.clone(),
            policy.classify(&retry_action),
        )
        .map_err(|err| err.to_string())?;
    action_ids.push(queued_retry.id.clone());
    session_store
        .record_action_queue_id(&retry_session.id, &queued_retry.id)
        .map_err(|err| err.to_string())?;
    action_queue
        .transition(&queued_retry.id, ExecutionQueueStatus::Executing, None)
        .map_err(|err| err.to_string())?;
    action_queue
        .fail(
            &queued_retry.id,
            "fixture failure",
            Some(serde_json::json!({ "retryReplayable": false })),
        )
        .map_err(|err| err.to_string())?;
    session_store
        .fail_session(&retry_session.id, "Failed before retry.")
        .map_err(|err| err.to_string())?;
    let loaded_retry_session = session_store
        .load_session(&retry_session.id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "retry prior task missing".to_string())?;
    let loaded_retry_action = action_queue
        .load(&queued_retry.id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "retry target action missing".to_string())?;
    runtime_object_count += 2;
    let retry_decision = openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_action_retry(
        Some(&loaded_retry_session),
        Some(&loaded_retry_action),
    );
    if !retry_decision.allowed || !retry_decision.manual_blocker_required {
        diagnostics.push(format!(
            "retry did not require exact failed-action review: {}",
            retry_decision.reason_code
        ));
    }
    let retried = action_queue
        .transition(
            &loaded_retry_action.id,
            ExecutionQueueStatus::Retrying,
            None,
        )
        .map_err(|err| err.to_string())?;
    if retried.status != ExecutionQueueStatus::Retrying {
        diagnostics.push("retry action did not transition to retrying".into());
    }

    let cancel_session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:cancel", scenario.id),
            user_goal: "Cancel queued prior action.".into(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Cancel exact queued action.".into()),
            context_snapshot_refs: vec![],
        })
        .map_err(|err| err.to_string())?;
    let cancel_action = ExecutionAction::new("file.read", "README.md");
    let queued_cancel = action_queue
        .enqueue(
            &cancel_session.id,
            cancel_action.clone(),
            policy.classify(&cancel_action),
        )
        .map_err(|err| err.to_string())?;
    action_ids.push(queued_cancel.id.clone());
    session_store
        .record_action_queue_id(&cancel_session.id, &queued_cancel.id)
        .map_err(|err| err.to_string())?;
    let loaded_cancel_session = session_store
        .load_session(&cancel_session.id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "cancel prior task missing".to_string())?;
    let loaded_cancel_action = action_queue
        .load(&queued_cancel.id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "cancel target action missing".to_string())?;
    runtime_object_count += 2;
    session_store
        .cancel_session(&loaded_cancel_session.id, "Cancelled exact prior task.")
        .map_err(|err| err.to_string())?;
    let cancelled_action = action_queue
        .transition(
            &loaded_cancel_action.id,
            ExecutionQueueStatus::Cancelled,
            Some(serde_json::json!({ "targetActionId": loaded_cancel_action.id })),
        )
        .map_err(|err| err.to_string())?;
    if cancelled_action.status != ExecutionQueueStatus::Cancelled {
        diagnostics.push("cancel action did not transition to cancelled".into());
    }
    runtime_object_count += 1;

    Ok(ProductScenarioRuntimeProof {
        scenario_id: scenario.id.clone(),
        group: "task_control_resume_retry_cancel".into(),
        passed: diagnostics.is_empty(),
        runtime_object_count,
        observation_count: 0,
        created_action_ids: action_ids,
        created_observation_ids: Vec::new(),
        created_proposal_ids: Vec::new(),
        final_delivery_id: Some(format!("task-control-transition-proof:{}", scenario.id)),
        diagnostics,
    })
}

pub(crate) fn productization_task_control_missing_target_runtime_proof(
) -> ProductScenarioRuntimeProof {
    let scenario_id = "LT-03";
    let session_store = match AgentTaskSessionStore::new_in_memory() {
        Ok(store) => store,
        Err(err) => {
            return failed_runtime_proof(
                scenario_id,
                "task_control_resume_retry_cancel",
                &err.to_string(),
            )
        }
    };
    let session = match session_store.create_session(AgentTaskSessionDraft {
        chat_session_id: "productization:missing-target".into(),
        user_goal: "Retry a missing prior action.".into(),
        selected_strategy: MainChatAgentStrategy::ReActToolExecution,
        current_plan_summary: None,
        context_snapshot_refs: vec![],
    }) {
        Ok(session) => session,
        Err(err) => {
            return failed_runtime_proof(
                scenario_id,
                "task_control_resume_retry_cancel",
                &err.to_string(),
            )
        }
    };
    let loaded = match session_store.load_session(&session.id) {
        Ok(Some(session)) => session,
        Ok(None) => {
            return failed_runtime_proof(
                scenario_id,
                "task_control_resume_retry_cancel",
                "task_session_missing",
            )
        }
        Err(err) => {
            return failed_runtime_proof(
                scenario_id,
                "task_control_resume_retry_cancel",
                &err.to_string(),
            )
        }
    };
    let retry_decision = openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_action_retry(
        Some(&loaded),
        None,
    );
    ProductScenarioRuntimeProof {
        scenario_id: scenario_id.into(),
        group: "task_control_resume_retry_cancel".into(),
        passed: false,
        runtime_object_count: 1,
        observation_count: 0,
        created_action_ids: Vec::new(),
        created_observation_ids: Vec::new(),
        created_proposal_ids: Vec::new(),
        final_delivery_id: None,
        diagnostics: vec!["target_object_missing".into(), retry_decision.reason_code],
    }
}

fn runtime_final_delivery_separation_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let proposal_store = ProposalStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some(
                "Separate completed actions, sources, proposals, and blockers.".into(),
            ),
            context_snapshot_refs: vec!["ctx:productization:final_delivery".into()],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "react_tool_execution")?;
    append_plan(
        &session_store,
        &session.id,
        "Read a source, create a proposal, keep pending items separate from done work.",
    )?;
    let completed = enqueue_completed_action(
        &session_store,
        &action_queue,
        &session.id,
        "file.read",
        "plans/main_chat_final_delivery_contract_v1.md",
        serde_json::json!({ "sourceKind": "file", "sourceLabel": "plans/main_chat_final_delivery_contract_v1.md" }),
    )?;
    append_observation(
        &session_store,
        &session.id,
        &completed.id,
        "file",
        "plans/main_chat_final_delivery_contract_v1.md",
        "Observed final delivery contract evidence.",
    )?;
    let mut proposal = memory_proposal_fixture(&session.id, "final-delivery");
    proposal.id = format!("proposal-{}-final-delivery", scenario.id);
    proposal.run_id = Some(format!("run-productization-{}", scenario.id));
    proposal_store
        .create_proposal(&proposal)
        .map_err(|err| err.to_string())?;
    append_final_result(
        &session_store,
        &session.id,
        "Final delivery separates completed action, source, proposal, blocker, and next step state.",
        serde_json::json!({
            "completedActionId": completed.id,
            "proposalId": proposal.id,
            "directWritesExecuted": false
        }),
    )?;
    session_store
        .complete_session(
            &session.id,
            "Final delivery separates completed action, source, proposal, blocker, and next step state.",
        )
        .map_err(|err| err.to_string())?;
    let loaded = proposal_store
        .list_all_proposals(10, 0)
        .map_err(|err| err.to_string())?;
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(
            &session.chat_session_id,
            scenario,
            "final_delivery_separation",
        ),
        loaded,
    )?;
    let mut proof = proof_from_snapshot(scenario, "final_delivery_separation", &snapshot, 0);
    let Some(delivery) = snapshot.final_delivery.as_ref() else {
        proof
            .diagnostics
            .push("missing final delivery object".into());
        proof.passed = false;
        return Ok(proof);
    };
    if delivery.completed_actions.is_empty()
        || delivery.observations_used.is_empty()
        || delivery.proposals_created.is_empty()
        || !delivery.durable_changes.is_empty()
    {
        proof.diagnostics.push(
            "final delivery did not keep executed work, sources, proposals, and durable changes separate"
                .into(),
        );
        proof.passed = false;
    }
    Ok(proof)
}

fn append_route_decision(
    session_store: &AgentTaskSessionStore,
    session_id: &str,
    selected_strategy: &str,
) -> Result<(), String> {
    session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session_id.into(),
            kind: ExecutionTranscriptEntryKind::RouteDecision,
            summary: format!("Productization runtime selected {selected_strategy}."),
            metadata: serde_json::json!({ "selectedStrategy": selected_strategy }),
        })
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn append_plan(
    session_store: &AgentTaskSessionStore,
    session_id: &str,
    summary: &str,
) -> Result<(), String> {
    session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session_id.into(),
            kind: ExecutionTranscriptEntryKind::Plan,
            summary: summary.into(),
            metadata: serde_json::json!({ "planId": format!("plan:{session_id}") }),
        })
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn append_observation(
    session_store: &AgentTaskSessionStore,
    session_id: &str,
    action_id: &str,
    source_kind: &str,
    source_label: &str,
    preview: &str,
) -> Result<(), String> {
    session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session_id.into(),
            kind: ExecutionTranscriptEntryKind::Observation,
            summary: preview.into(),
            metadata: serde_json::json!({
                "actionId": action_id,
                "sourceKind": source_kind,
                "sourceLabel": source_label,
                "preview": preview,
                "directWritesExecuted": false
            }),
        })
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn append_final_result(
    session_store: &AgentTaskSessionStore,
    session_id: &str,
    summary: &str,
    metadata: serde_json::Value,
) -> Result<(), String> {
    session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session_id.into(),
            kind: ExecutionTranscriptEntryKind::FinalResult,
            summary: summary.into(),
            metadata,
        })
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn enqueue_completed_action(
    session_store: &AgentTaskSessionStore,
    action_queue: &ActionQueueStore,
    session_id: &str,
    action_type: &str,
    target: &str,
    observation_metadata: serde_json::Value,
) -> Result<openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction, String> {
    let policy = ExecutionPolicy;
    let action = ExecutionAction::new(action_type, target);
    let queued = action_queue
        .enqueue(session_id, action.clone(), policy.classify(&action))
        .map_err(|err| err.to_string())?;
    session_store
        .record_action_queue_id(session_id, &queued.id)
        .map_err(|err| err.to_string())?;
    action_queue
        .transition(&queued.id, ExecutionQueueStatus::Executing, None)
        .map_err(|err| err.to_string())?;
    action_queue
        .transition(
            &queued.id,
            ExecutionQueueStatus::Observed,
            Some(observation_metadata),
        )
        .map_err(|err| err.to_string())?;
    action_queue
        .transition(&queued.id, ExecutionQueueStatus::Completed, None)
        .map_err(|err| err.to_string())
}

fn assemble_snapshot_from_stores(
    session_store: &AgentTaskSessionStore,
    action_queue: &ActionQueueStore,
    session_id: &str,
    run: AgentRun,
    proposals: Vec<AgentProposal>,
) -> Result<MainChatAgentStateSnapshot, String> {
    let session = session_store
        .load_session(session_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("runtime session missing: {session_id}"))?;
    let transcript = session_store
        .list_transcript_entries(session_id)
        .map_err(|err| err.to_string())?;
    let actions = action_queue
        .list_for_session(session_id)
        .map_err(|err| err.to_string())?;
    assemble_main_chat_agent_state(MainChatAgentStateAssemblerInput {
        session,
        run: Some(run),
        transcript,
        actions,
        proposals,
    })
    .map_err(|err| err.to_string())
}

fn proof_from_snapshot(
    scenario: &MainChatAgentProductScenario,
    group: &str,
    snapshot: &MainChatAgentStateSnapshot,
    extra_runtime_objects: usize,
) -> ProductScenarioRuntimeProof {
    let runtime_object_count = 1
        + usize::from(snapshot.provider.is_some())
        + snapshot.context.len()
        + snapshot.actions.len()
        + snapshot.observations.len()
        + snapshot.proposals.len()
        + usize::from(snapshot.plan.is_some())
        + usize::from(snapshot.final_delivery.is_some())
        + extra_runtime_objects;
    ProductScenarioRuntimeProof {
        scenario_id: scenario.id.clone(),
        group: group.into(),
        passed: snapshot.diagnostics.is_empty(),
        runtime_object_count,
        observation_count: snapshot.observations.len(),
        created_action_ids: snapshot
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect(),
        created_observation_ids: snapshot
            .observations
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect(),
        created_proposal_ids: snapshot
            .proposals
            .iter()
            .map(|proposal| proposal.proposal_id.clone())
            .collect(),
        final_delivery_id: snapshot
            .final_delivery
            .as_ref()
            .map(|delivery| delivery.delivery_id.clone()),
        diagnostics: snapshot
            .diagnostics
            .iter()
            .map(|gap| format!("{}:{}", gap.gap_code, gap.detail))
            .collect(),
    }
}

fn memory_proposal_fixture(session_id: &str, suffix: &str) -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::MemoryWrite,
        &format!("memory.productization.{suffix}"),
        serde_json::json!({ "text": format!("Productization memory proposal {suffix}") }),
        "Productization memory proposal is reviewable and proposal-first.",
        0.82,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    proposal.source_detail = Some(session_id.into());
    proposal
}

fn runtime_fixture_run(
    session_id: &str,
    scenario: &MainChatAgentProductScenario,
    group: &str,
) -> AgentRun {
    let mut run = AgentRun::new_chat_run(session_id, &scenario.prompt);
    run.id = format!("run-productization-{}-{group}", scenario.id);
    run.task_id = format!("task-productization-{}-{group}", scenario.id);
    run.status = AgentRunStatus::Completed;
    run.kind = AgentTaskKind::Conversation;
    run.output_preview = Some(format!("Runtime proof for {group}."));
    run.model_route = Some(ModelRouteTrace {
        provider: "scripted_eval".into(),
        model: "productization-runtime-fixture".into(),
        route_type: "local".into(),
        prefer_local: true,
        local_model: "productization-runtime-fixture".into(),
        reason: format!("deterministic productization runtime proof: {group}"),
        privacy_level: RedactionLevel::LocalOnly,
        latency_ms: Some(1),
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(false),
    });
    run.context_summary = Some(ContextSummary {
        life_model_empty: true,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 0,
        memory_sources: vec![format!("ctx:productization:{group}")],
        used_tools_prompt: group != "direct_answer",
        redaction_applied: true,
        redaction_level: RedactionLevel::LocalOnly,
    });
    run
}

fn canonical_route_count_map() -> BTreeMap<String, MainChatAgentProductizationRouteCount> {
    [
        "direct_answer",
        "read_action",
        "react_tool_execution",
        "plan_execute",
        "memory_proposal",
        "permission_request",
        "task_control",
        "blocked",
    ]
    .into_iter()
    .map(|route| {
        (
            route.to_string(),
            MainChatAgentProductizationRouteCount::default(),
        )
    })
    .collect()
}

fn main_chat_productization_payload_smoke_gate_passes() -> bool {
    let session_store = match AgentTaskSessionStore::new_in_memory() {
        Ok(store) => store,
        Err(_) => return false,
    };
    let action_queue = match ActionQueueStore::new_in_memory() {
        Ok(store) => store,
        Err(_) => return false,
    };
    let policy = ExecutionPolicy;
    let session = match session_store.create_session(AgentTaskSessionDraft {
        chat_session_id: "productization-gate-chat".into(),
        user_goal: "Read a deterministic productization fixture.".into(),
        selected_strategy: MainChatAgentStrategy::ReActToolExecution,
        current_plan_summary: Some("Read fixture and synthesize.".into()),
        context_snapshot_refs: vec!["ctx:productization:gate".into()],
    }) {
        Ok(session) => session,
        Err(_) => return false,
    };
    let action = ExecutionAction::new(
        "file.read",
        "plans/main_chat_agent_productization_v1_goal_spec.md",
    );
    let queued = match action_queue.enqueue(&session.id, action.clone(), policy.classify(&action)) {
        Ok(queued) => queued,
        Err(_) => return false,
    };
    if session_store
        .record_action_queue_id(&session.id, &queued.id)
        .is_err()
    {
        return false;
    }
    if action_queue
        .transition(&queued.id, ExecutionQueueStatus::Executing, None)
        .is_err()
    {
        return false;
    }
    if action_queue
        .transition(
            &queued.id,
            ExecutionQueueStatus::Observed,
            Some(serde_json::json!({
                "sourceKind": "file",
                "sourceLabel": "plans/main_chat_agent_productization_v1_goal_spec.md"
            })),
        )
        .is_err()
    {
        return false;
    }
    let completed = match action_queue.transition(&queued.id, ExecutionQueueStatus::Completed, None)
    {
        Ok(action) => action,
        Err(_) => return false,
    };
    if session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::RouteDecision,
            summary: "Productization gate route selected.".into(),
            metadata: serde_json::json!({ "selectedStrategy": "react_tool_execution" }),
        })
        .is_err()
    {
        return false;
    }
    if session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::Plan,
            summary: "Read fixture and synthesize.".into(),
            metadata: serde_json::json!({ "planId": "productization-gate-plan" }),
        })
        .is_err()
    {
        return false;
    }
    let observation = match session_store.append_transcript_entry(ExecutionTranscriptEntryDraft {
        session_id: session.id.clone(),
        kind: ExecutionTranscriptEntryKind::Observation,
        summary: "Observed deterministic productization fixture.".into(),
        metadata: serde_json::json!({
            "actionId": completed.id,
            "sourceKind": "file",
            "sourceLabel": "plans/main_chat_agent_productization_v1_goal_spec.md",
            "preview": "Runtime-backed Agent Control Plane payload evidence exists."
        }),
    }) {
        Ok(entry) => entry,
        Err(_) => return false,
    };
    if session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::FinalResult,
            summary: "Productization payload smoke delivery.".into(),
            metadata: serde_json::json!({
                "observationIds": [observation.id],
                "directWritesExecuted": false
            }),
        })
        .is_err()
    {
        return false;
    }
    if session_store
        .complete_session(&session.id, "Productization payload smoke delivery.")
        .is_err()
    {
        return false;
    }
    let session = match session_store.load_session(&session.id) {
        Ok(Some(session)) => session,
        _ => return false,
    };
    let transcript = match session_store.list_transcript_entries(&session.id) {
        Ok(transcript) => transcript,
        Err(_) => return false,
    };
    let actions = match action_queue.list_for_session(&session.id) {
        Ok(actions) => actions,
        Err(_) => return false,
    };
    let snapshot = match assemble_main_chat_agent_state(MainChatAgentStateAssemblerInput {
        session: session.clone(),
        run: Some(productization_gate_run(&session.chat_session_id)),
        transcript,
        actions,
        proposals: Vec::new(),
    }) {
        Ok(snapshot) => snapshot,
        Err(_) => return false,
    };
    let event_types = snapshot
        .events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    snapshot.diagnostics.is_empty()
        && snapshot.final_delivery.is_some()
        && snapshot.sequence == snapshot.events.len() as u64
        && [
            MainChatAgentStateEventType::TaskCreated,
            MainChatAgentStateEventType::TaskUpdated,
            MainChatAgentStateEventType::RouteSelected,
            MainChatAgentStateEventType::ContextSelected,
            MainChatAgentStateEventType::PlanUpdated,
            MainChatAgentStateEventType::ActionQueued,
            MainChatAgentStateEventType::ActionUpdated,
            MainChatAgentStateEventType::ObservationCreated,
            MainChatAgentStateEventType::FinalDeliveryCreated,
        ]
        .into_iter()
        .all(|event_type| event_types.contains(&event_type))
}

fn productization_gate_run(session_id: &str) -> AgentRun {
    let mut run = AgentRun::new_chat_run(session_id, "productization payload smoke");
    run.id = "run-productization-tauri-gate".into();
    run.task_id = "task-productization-tauri-gate".into();
    run.status = AgentRunStatus::Completed;
    run.kind = AgentTaskKind::Conversation;
    run.output_preview = Some("Productization payload smoke delivery.".into());
    run.model_route = Some(ModelRouteTrace {
        provider: "scripted_eval".into(),
        model: "productization-fixture".into(),
        route_type: "local".into(),
        prefer_local: true,
        local_model: "productization-fixture".into(),
        reason: "deterministic productization gate".into(),
        privacy_level: RedactionLevel::LocalOnly,
        latency_ms: Some(1),
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(false),
    });
    run.context_summary = Some(ContextSummary {
        life_model_empty: true,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 0,
        memory_sources: vec!["ctx:productization:gate".into()],
        used_tools_prompt: true,
        redaction_applied: true,
        redaction_level: RedactionLevel::LocalOnly,
    });
    run
}
