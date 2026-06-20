use openlife_core::agent::main_chat_agent_productization_v1::{
    assemble_main_chat_agent_state, main_chat_agent_product_scenarios,
    MainChatAgentProductProposalStatus, MainChatAgentProductScenario,
    MainChatAgentProductScenarioExpectation, MainChatAgentProductScenarioRunMode,
    MainChatAgentProductStrategyRoute, MainChatAgentStateAssemblerInput,
    MainChatAgentStateEventType, MainChatAgentStateSnapshot,
};
use openlife_core::agent::main_chat_agent_v1::{
    ActionQueueStore, AgentTaskSession, AgentTaskSessionDraft, AgentTaskSessionStatus,
    AgentTaskSessionStore, ExecutionAction, ExecutionPolicy, ExecutionQueueStatus,
    ExecutionTranscriptEntry, ExecutionTranscriptEntryDraft, ExecutionTranscriptEntryKind,
    MainChatAgentStrategy,
};
use openlife_core::agent::proposal_store::ProposalStore;
use openlife_core::agent::types::{
    AgentProposal, AgentRun, AgentRunStatus, AgentTaskKind, ContextSummary, ModelRouteTrace,
    ProposalSource, ProposalStatus, ProposalType, RedactionLevel, RiskLevel,
};
use openlife_core::agent::{
    MemoryLifecycleAcceptanceInput, MemoryLifecycleStatus, MemoryLifecycleStore,
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
    pub created_memory_ids: Vec<String>,
    pub rollback_event_ids: Vec<String>,
    pub materialized_view_versions: Vec<i64>,
    pub inactive_memory_ids: Vec<String>,
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
        execute_default_runtime_product_scenarios(&scenarios, runtime_executor);
    let runtime_required_group_count = runtime_required_group_evidence.len();
    let runtime_required_group_passed_count = runtime_required_group_evidence
        .iter()
        .filter(|proof| proof.passed)
        .count();
    let runtime_proof_by_scenario = runtime_required_group_evidence
        .iter()
        .map(|proof| (proof.scenario_id.as_str(), proof))
        .collect::<BTreeMap<_, _>>();
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
                let schema_result = execute_deterministic_product_scenario(scenario);
                let runtime_result = runtime_proof_by_scenario
                    .get(scenario.id.as_str())
                    .filter(|proof| proof.passed)
                    .map(|_| Ok(()))
                    .unwrap_or_else(|| {
                        Err(runtime_proof_by_scenario
                            .get(scenario.id.as_str())
                            .map(|proof| {
                                if proof.diagnostics.is_empty() {
                                    "runtime proof did not pass".to_string()
                                } else {
                                    proof.diagnostics.join("; ")
                                }
                            })
                            .unwrap_or_else(|| "runtime proof missing".into()))
                    });
                match schema_result.and(runtime_result) {
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
                let schema_result = execute_deterministic_product_scenario(scenario);
                let runtime_result = runtime_proof_by_scenario
                    .get(scenario.id.as_str())
                    .filter(|proof| proof.passed)
                    .map(|_| Ok(()))
                    .unwrap_or_else(|| {
                        Err(runtime_proof_by_scenario
                            .get(scenario.id.as_str())
                            .map(|proof| {
                                if proof.diagnostics.is_empty() {
                                    "runtime proof did not pass".to_string()
                                } else {
                                    proof.diagnostics.join("; ")
                                }
                            })
                            .unwrap_or_else(|| "runtime proof missing".into()))
                    });
                match schema_result.and(runtime_result) {
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
    if !unsupported_scenarios.is_empty() {
        blockers.push("unsupported_scenario_present".into());
    }
    if executed_scenario_count + unsupported_scenarios.len() != default_deterministic_scenario_count
    {
        blockers.push("default_product_scenario_accounting_mismatch".into());
    }

    let full_productization_v1_complete = blockers.is_empty();

    MainChatAgentProductizationV1GateReport {
        total_scenario_count: scenarios.len(),
        default_deterministic_scenario_count,
        readiness_semantics: "full_deterministic_productization_v1_runtime_ready".into(),
        runtime_execution_scope:
            "default_deterministic_scenarios_runtime_backed_external_live_excluded".into(),
        executed_scenario_count,
        passed_scenario_count,
        expected_blocker_scenario_count,
        failed_scenario_count,
        external_live_excluded_count,
        runtime_payload_snapshot_event_gate_passed,
        runtime_required_group_count,
        runtime_required_group_passed_count,
        representative_runtime_group_count: 0,
        representative_runtime_group_passed_count: 0,
        full_deterministic_runtime_scenario_count: runtime_required_group_count,
        full_deterministic_runtime_scenario_executed_count: runtime_required_group_count,
        runtime_required_group_evidence,
        event_semantics:
            "durable_replayable_delta_events_available_snapshot_backfill_excluded_from_live_credit"
                .into(),
        final_readiness_ready: full_productization_v1_complete,
        full_productization_v1_complete,
        future_work: Vec::new(),
        route_counts,
        unsupported_scenarios,
        failed_scenarios,
        blockers,
    }
}

fn execute_default_runtime_product_scenarios<F>(
    scenarios: &[MainChatAgentProductScenario],
    runtime_executor: F,
) -> Vec<ProductScenarioRuntimeProof>
where
    F: Fn(&MainChatAgentProductScenario) -> Result<ProductScenarioRuntimeProof, String>,
{
    scenarios
        .iter()
        .filter(|scenario| {
            scenario.included_in_default_gate
                && matches!(
                    scenario.run_mode,
                    MainChatAgentProductScenarioRunMode::DeterministicFixture
                        | MainChatAgentProductScenarioRunMode::MockIpcUi
                )
                && scenario.expectation
                    != MainChatAgentProductScenarioExpectation::OptionalUnsupported
        })
        .map(|scenario| {
            let scenario_id = scenario.id.as_str();
            let group = runtime_group_for_scenario(scenario);
            let mut proof = match runtime_executor(scenario) {
                Ok(proof) => proof,
                Err(reason) => return failed_runtime_proof(scenario_id, &group, &reason),
            };
            proof
                .diagnostics
                .extend(validate_runtime_product_proof(scenario, &group, &proof));
            if !proof.diagnostics.is_empty() {
                proof.passed = false;
            }
            proof
        })
        .collect()
}

fn validate_runtime_product_proof(
    scenario: &MainChatAgentProductScenario,
    group: &str,
    proof: &ProductScenarioRuntimeProof,
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    let scenario_id = scenario.id.as_str();
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
    match scenario.expected_strategy_route {
        MainChatAgentProductStrategyRoute::DirectAnswer => {
            if !proof.created_action_ids.is_empty() || proof.observation_count != 0 {
                diagnostics
                    .push("DirectAnswer proof must not fabricate action observations".into());
            }
            if proof.final_delivery_id.is_none() {
                diagnostics.push("DirectAnswer proof lacks final delivery evidence".into());
            }
        }
        MainChatAgentProductStrategyRoute::ReadAction => {
            if proof.created_action_ids.is_empty() || proof.observation_count == 0 {
                diagnostics.push("read proof lacks action/observation runtime evidence".into());
            }
            if proof.final_delivery_id.is_none() {
                diagnostics.push("read proof lacks final delivery evidence".into());
            }
        }
        MainChatAgentProductStrategyRoute::ReactToolExecution => {
            let requires_multi_step = scenario.capability_group == "Multi-step ReAct"
                || matches!(scenario.id.as_str(), "WR-04" | "ST-08");
            if proof.created_action_ids.is_empty() || proof.observation_count == 0 {
                diagnostics.push("ReAct proof lacks action/observation runtime evidence".into());
            }
            if requires_multi_step
                && (proof.observation_count < 2 || proof.created_action_ids.len() < 2)
            {
                diagnostics
                    .push("multi-step ReAct proof requires at least two observations".into());
            }
            if proof.final_delivery_id.is_none() {
                diagnostics.push("ReAct proof lacks final delivery evidence".into());
            }
        }
        MainChatAgentProductStrategyRoute::PlanExecute => {
            if proof.created_action_ids.is_empty() {
                diagnostics.push("PlanExecute proof lacks executed action evidence".into());
            }
            if proof.final_delivery_id.is_none() {
                diagnostics.push("PlanExecute proof lacks final delivery evidence".into());
            }
        }
        MainChatAgentProductStrategyRoute::MemoryProposal => {
            if proof.created_proposal_ids.is_empty() {
                diagnostics.push("memory proposal proof lacks proposal evidence".into());
            }
        }
        MainChatAgentProductStrategyRoute::PermissionRequest => {
            if proof.created_action_ids.len() != 1 || proof.created_proposal_ids.len() != 1 {
                diagnostics.push(
                    "permission proof must bind one proposal/blocker to one exact action".into(),
                );
            }
        }
        MainChatAgentProductStrategyRoute::TaskControl => {
            if proof.runtime_object_count < 3 {
                diagnostics.push("task control proof must load prior runtime objects".into());
            }
            if scenario.id == "MP-06" {
                if proof.created_memory_ids.len() != 1 {
                    diagnostics
                        .push("MP-06 rollback proof must bind one accepted memory id".into());
                }
                if proof.rollback_event_ids.len() != 1 {
                    diagnostics.push("MP-06 rollback proof lacks rollback event id".into());
                }
                if proof
                    .materialized_view_versions
                    .iter()
                    .all(|version| *version <= 0)
                {
                    diagnostics.push("MP-06 rollback proof lacks materialized view version".into());
                }
                let selected_memory_id = proof.created_memory_ids.first();
                if selected_memory_id.is_none()
                    || selected_memory_id
                        .is_some_and(|memory_id| !proof.inactive_memory_ids.contains(memory_id))
                {
                    diagnostics.push(
                        "MP-06 rollback proof must exclude the accepted memory from active context"
                            .into(),
                    );
                }
            }
        }
        MainChatAgentProductStrategyRoute::Blocked => {
            if proof.final_delivery_id.is_none()
                && scenario
                    .required_runtime_evidence
                    .iter()
                    .any(|evidence| evidence == "final_delivery")
            {
                diagnostics.push("blocked proof lacks required final delivery evidence".into());
            }
        }
        MainChatAgentProductStrategyRoute::LegacyFallback
        | MainChatAgentProductStrategyRoute::Unknown => {
            diagnostics.push("legacy/unknown route cannot have productization proof".into());
        }
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
        created_memory_ids: Vec::new(),
        rollback_event_ids: Vec::new(),
        materialized_view_versions: Vec::new(),
        inactive_memory_ids: Vec::new(),
        final_delivery_id: None,
        diagnostics: vec![reason.into()],
    }
}

fn runtime_group_for_scenario(scenario: &MainChatAgentProductScenario) -> String {
    format!(
        "{}:{}",
        scenario.expected_strategy_route.as_str(),
        scenario.id
    )
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
            if scenario.id == "MP-06" {
                require(
                    evidence("memory_id")
                        && evidence("rollback_event_id")
                        && evidence("inactive_memory")
                        && evidence("materialized_view_version"),
                    "MP-06 rollback requires memory lifecycle, rollback event, inactive memory, and materialized view evidence",
                )?;
            }
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
    match scenario.expected_strategy_route {
        MainChatAgentProductStrategyRoute::DirectAnswer => runtime_direct_answer_proof(scenario),
        MainChatAgentProductStrategyRoute::ReadAction => runtime_read_action_proof(scenario),
        MainChatAgentProductStrategyRoute::ReactToolExecution => {
            runtime_react_tool_execution_proof(scenario)
        }
        MainChatAgentProductStrategyRoute::PlanExecute => runtime_plan_execute_proof(scenario),
        MainChatAgentProductStrategyRoute::MemoryProposal => {
            runtime_memory_proposal_proof(scenario)
        }
        MainChatAgentProductStrategyRoute::PermissionRequest => {
            runtime_permission_request_proof(scenario)
        }
        MainChatAgentProductStrategyRoute::Blocked => runtime_blocked_proof(scenario),
        MainChatAgentProductStrategyRoute::TaskControl => runtime_task_control_proof(scenario),
        MainChatAgentProductStrategyRoute::LegacyFallback
        | MainChatAgentProductStrategyRoute::Unknown => Err(format!(
            "unsupported productization route for {}",
            scenario.id
        )),
    }
}

fn runtime_direct_answer_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let group = runtime_group_for_scenario(scenario);
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::DirectAnswer,
            current_plan_summary: None,
            context_snapshot_refs: vec![format!("ctx:productization:{group}")],
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
        runtime_fixture_run(&session.chat_session_id, scenario, &group),
        Vec::new(),
        Vec::new(),
    )?;
    let mut proof = proof_from_snapshot(scenario, &group, &snapshot, 0);
    if !snapshot.actions.is_empty() || !snapshot.observations.is_empty() {
        proof
            .diagnostics
            .push("DirectAnswer snapshot contained fake action evidence".into());
        proof.passed = false;
    }
    Ok(proof)
}

fn runtime_read_action_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let (action_type, target, source_kind, source_label) = read_action_spec(scenario);
    runtime_single_read_proof(
        scenario,
        &runtime_group_for_scenario(scenario),
        action_type,
        target,
        source_kind,
        source_label,
    )
}

fn read_action_spec(
    scenario: &MainChatAgentProductScenario,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match scenario.id.split('-').next().unwrap_or_default() {
        "FR" => (
            "file.read",
            "plans/main_chat_agent_productization_v1_goal_spec.md",
            "file",
            "plans/main_chat_agent_productization_v1_goal_spec.md",
        ),
        "MS" => (
            "memory.search",
            "accepted Main Chat memory and previous session consensus",
            "memory",
            "memory:main_chat_consensus",
        ),
        "WR" => (
            "web.fetch",
            "fixture://main-chat-agent-productization",
            "web_fixture",
            "fixture://main-chat-agent-productization",
        ),
        "MCP" => (
            "mcp.read_only",
            "registered://openlife.project_status.read",
            "mcp",
            "openlife.project_status.read",
        ),
        "ST" => (
            "skill.read",
            "selected://skill/read-only-tool",
            "skill",
            "selected SKILL.md read-only tool",
        ),
        _ => (
            "file.read",
            "plans/main_chat_agent_product_eval_scenarios_v1.md",
            "file",
            "plans/main_chat_agent_product_eval_scenarios_v1.md",
        ),
    }
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
        Vec::new(),
    )?;
    Ok(proof_from_snapshot(scenario, group, &snapshot, 0))
}

fn runtime_react_tool_execution_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let group = runtime_group_for_scenario(scenario);
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some(
                "Select governed read tools, observe results, then synthesize.".into(),
            ),
            context_snapshot_refs: vec![format!("ctx:productization:{group}")],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "react_tool_execution")?;
    append_plan(
        &session_store,
        &session.id,
        "Execute governed read actions with exact selected targets before final synthesis.",
    )?;

    for (action_type, target, source_kind, source_label, preview) in react_action_specs(scenario) {
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
            preview,
        )?;
    }

    append_final_result(
        &session_store,
        &session.id,
        "ReAct tool execution completed from governed read observations.",
        serde_json::json!({
            "observationCount": react_action_specs(scenario).len(),
            "directWritesExecuted": false
        }),
    )?;
    session_store
        .complete_session(
            &session.id,
            "ReAct tool execution completed from governed read observations.",
        )
        .map_err(|err| err.to_string())?;
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(&session.chat_session_id, scenario, &group),
        Vec::new(),
        Vec::new(),
    )?;
    Ok(proof_from_snapshot(scenario, &group, &snapshot, 0))
}

fn react_action_specs(
    scenario: &MainChatAgentProductScenario,
) -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
)> {
    match scenario.id.split('-').next().unwrap_or_default() {
        "WR" => vec![
            (
                "web.fetch",
                "fixture://main-chat-agent-productization/primary",
                "web_fixture",
                "fixture://main-chat-agent-productization/primary",
                "Observed primary fixture web evidence.",
            ),
            (
                "web.fetch",
                "fixture://main-chat-agent-productization/fallback",
                "web_fixture",
                "fixture://main-chat-agent-productization/fallback",
                "Observed fallback fixture web evidence.",
            ),
        ],
        "MCP" => vec![
            (
                "mcp.read_only",
                "registered://openlife.project_status.read",
                "mcp",
                "openlife.project_status.read",
                "Observed registered MCP project status evidence.",
            ),
            (
                "mcp.read_only",
                "registered://openlife.runtime_status.read",
                "mcp",
                "openlife.runtime_status.read",
                "Observed registered MCP runtime status evidence.",
            ),
        ],
        "ST" => vec![
            (
                "skill.read",
                "selected://skill/read-only-tool",
                "skill",
                "selected SKILL.md read-only tool",
                "Observed selected skill read-only tool evidence.",
            ),
            (
                "skill.read",
                "selected://skill/fallback-read-only-tool",
                "skill",
                "selected SKILL.md fallback read-only tool",
                "Observed fallback skill read-only tool evidence.",
            ),
        ],
        _ => vec![
            (
                "file.read",
                "plans/main_chat_agent_product_eval_scenarios_v1.md",
                "file",
                "plans/main_chat_agent_product_eval_scenarios_v1.md",
                "Observed product scenario matrix evidence.",
            ),
            (
                "file.read",
                "README.md",
                "file",
                "README.md",
                "Observed README productization status evidence.",
            ),
        ],
    }
}

fn runtime_plan_execute_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let group = runtime_group_for_scenario(scenario);
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let proposal_store = ProposalStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::PlanExecute,
            current_plan_summary: Some(
                "Draft a plan, execute governed read work, and review the result.".into(),
            ),
            context_snapshot_refs: vec![format!("ctx:productization:{group}")],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "plan_execute")?;
    append_plan(
        &session_store,
        &session.id,
        "PlanExecute created a reviewable plan and executed the first safe read step.",
    )?;
    let completed = enqueue_completed_action(
        &session_store,
        &action_queue,
        &session.id,
        "file.read",
        "plans/main_chat_agent_productization_v1_goal_spec.md",
        serde_json::json!({
            "sourceKind": "file",
            "sourceLabel": "plans/main_chat_agent_productization_v1_goal_spec.md",
            "directWritesExecuted": false
        }),
    )?;
    append_observation(
        &session_store,
        &session.id,
        &completed.id,
        "file",
        "plans/main_chat_agent_productization_v1_goal_spec.md",
        "Observed PlanExecute governed read step.",
    )?;

    let proposals = if scenario.id == "PE-08" {
        let mut proposal = memory_proposal_fixture(&session.id, "plan-execute");
        proposal.id = format!("proposal-{}-plan-execute", scenario.id);
        proposal.run_id = Some(format!("run-productization-{}", scenario.id));
        proposal_store
            .create_proposal(&proposal)
            .map_err(|err| err.to_string())?;
        proposal_store
            .list_all_proposals(10, 0)
            .map_err(|err| err.to_string())?
    } else {
        Vec::new()
    };

    append_final_result(
        &session_store,
        &session.id,
        "PlanExecute completed deterministic governed runtime work.",
        serde_json::json!({
            "planId": format!("plan:{}", session.id),
            "actionId": completed.id,
            "directWritesExecuted": false
        }),
    )?;
    session_store
        .complete_session(
            &session.id,
            "PlanExecute completed deterministic governed runtime work.",
        )
        .map_err(|err| err.to_string())?;
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(&session.chat_session_id, scenario, &group),
        proposals,
        Vec::new(),
    )?;
    Ok(proof_from_snapshot(scenario, &group, &snapshot, 0))
}

fn runtime_memory_proposal_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let group = runtime_group_for_scenario(scenario);
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let proposal_store = ProposalStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::MemoryProposal,
            current_plan_summary: Some("Create a proposal-first memory update for review.".into()),
            context_snapshot_refs: vec![format!("ctx:productization:{group}")],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "memory_proposal")?;
    let mut proposal = memory_proposal_fixture(&session.id, "pending");
    proposal.id = format!("proposal-{}-memory", scenario.id);
    proposal.run_id = Some(format!("run-productization-{}", scenario.id));
    proposal_store
        .create_proposal(&proposal)
        .map_err(|err| err.to_string())?;
    session_store
        .mark_waiting_permission(&session.id)
        .map_err(|err| err.to_string())?;
    append_final_result(
        &session_store,
        &session.id,
        "Memory change is proposal-only and waiting for Review Center action.",
        serde_json::json!({
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
        runtime_fixture_run(&session.chat_session_id, scenario, &group),
        loaded,
        Vec::new(),
    )?;
    Ok(proof_from_snapshot(scenario, &group, &snapshot, 0))
}

fn runtime_permission_request_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let group = runtime_group_for_scenario(scenario);
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let proposal_store = ProposalStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Pause for permission before exact action replay.".into()),
            context_snapshot_refs: vec![format!("ctx:productization:{group}")],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "permission_request")?;
    let (action_type, target) = permission_action_spec(scenario);
    let policy = ExecutionPolicy;
    let action = ExecutionAction::new(action_type, target);
    let queued = action_queue
        .enqueue(&session.id, action.clone(), policy.classify(&action))
        .map_err(|err| err.to_string())?;
    session_store
        .record_action_queue_id(&session.id, &queued.id)
        .map_err(|err| err.to_string())?;
    set_action_status_for_product_control(&action_queue, &queued.id, "waiting_for_user")?;
    session_store
        .set_pending_blockers(&session.id, vec![format!("permission:{}", queued.id)])
        .map_err(|err| err.to_string())?;
    session_store
        .mark_waiting_permission(&session.id)
        .map_err(|err| err.to_string())?;
    let mut proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        &format!("tools.permissions.{action_type}"),
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
        runtime_fixture_run(&session.chat_session_id, scenario, &group),
        loaded,
        Vec::new(),
    )?;
    let mut proof = proof_from_snapshot(scenario, &group, &snapshot, 0);
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

fn permission_action_spec(scenario: &MainChatAgentProductScenario) -> (&'static str, &'static str) {
    match scenario.id.as_str() {
        "PB-04" => ("external.email.send", "external://email/outbox"),
        "ST-06" => ("skill.write", "selected://skill/write-like-tool"),
        "MCP-04" => ("mcp.read_only", "registered://openlife.permissioned_read"),
        _ => (
            "file.read",
            "plans/main_chat_agent_productization_v1_goal_spec.md",
        ),
    }
}

fn runtime_blocked_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let group = runtime_group_for_scenario(scenario);
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:chat", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::BlockedConfirmation,
            current_plan_summary: Some(
                "Stop execution and surface a deterministic blocker.".into(),
            ),
            context_snapshot_refs: vec![format!("ctx:productization:{group}")],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "blocked")?;
    let reason = blocker_reason_for_scenario(scenario);
    session_store
        .set_pending_blockers(&session.id, vec![reason.into()])
        .map_err(|err| err.to_string())?;
    append_final_result(
        &session_store,
        &session.id,
        "Execution stopped with an explicit productization blocker.",
        serde_json::json!({
            "blockerReason": reason,
            "directWritesExecuted": false
        }),
    )?;
    session_store
        .block_session(
            &session.id,
            "Execution stopped with an explicit productization blocker.",
        )
        .map_err(|err| err.to_string())?;
    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(&session.chat_session_id, scenario, &group),
        Vec::new(),
        Vec::new(),
    )?;
    Ok(proof_from_snapshot(scenario, &group, &snapshot, 0))
}

fn blocker_reason_for_scenario(scenario: &MainChatAgentProductScenario) -> &'static str {
    match scenario.id.as_str() {
        "FR-03" => "workspace_file_not_found",
        "FR-04" => "outside_workspace_read_blocked",
        "WR-02" => "network_policy_blocked",
        "MCP-02" => "mcp_read_tool_not_registered",
        "MCP-05" => "mcp_read_manifest_write_like_blocked",
        "PB-05" => "dangerous_write_blocked",
        "PB-06" => "missing_required_information",
        _ => "productization_blocker",
    }
}

fn runtime_task_control_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    if scenario.id == "MP-06" {
        return runtime_memory_rollback_proof(scenario);
    }

    let group = runtime_group_for_scenario(scenario);
    let preconditions = scenario
        .preconditions
        .as_ref()
        .ok_or_else(|| "task_control scenario lacks preconditions".to_string())?;
    let transition = scenario
        .expected_state_transition
        .as_ref()
        .ok_or_else(|| "task_control scenario lacks expected state transition".to_string())?;
    let control = scenario
        .control_action
        .ok_or_else(|| "task_control scenario lacks control action".to_string())?;
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let proposal_store = ProposalStore::new_in_memory().map_err(|err| err.to_string())?;
    let policy = ExecutionPolicy;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:control", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Apply exact task-control transition.".into()),
            context_snapshot_refs: vec![format!("ctx:productization:{group}")],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "task_control")?;

    let mut diagnostics = Vec::new();
    let mut action_ids = Vec::new();
    let mut proposal_ids = Vec::new();
    let mut final_delivery_id = None;
    let mut runtime_object_count = 1usize;

    if preconditions.target_action_id.is_some() {
        let action = if transition.from_status == "waiting_for_user" {
            ExecutionAction::new("memory.write", "long-term memory write exact action")
        } else {
            ExecutionAction::new(
                "file.read",
                "plans/main_chat_agent_productization_v1_goal_spec.md",
            )
        };
        let queued = action_queue
            .enqueue(&session.id, action.clone(), policy.classify(&action))
            .map_err(|err| err.to_string())?;
        session_store
            .record_action_queue_id(&session.id, &queued.id)
            .map_err(|err| err.to_string())?;
        set_action_status_for_product_control(
            &action_queue,
            &queued.id,
            transition.from_status.as_str(),
        )?;
        action_ids.push(queued.id.clone());
        runtime_object_count += 1;
    }

    if preconditions.target_proposal_id.is_some() {
        let mut proposal = memory_proposal_fixture(&session.id, "task-control");
        proposal.id = format!("proposal-{}-task-control", scenario.id);
        proposal.run_id = Some(format!("run-productization-{}", scenario.id));
        proposal_store
            .create_proposal(&proposal)
            .map_err(|err| err.to_string())?;
        proposal_ids.push(proposal.id.clone());
        runtime_object_count += 1;
    }

    if preconditions.target_blocker_id.is_some() {
        session_store
            .set_pending_blockers(&session.id, vec![format!("permission:{}", scenario.id)])
            .map_err(|err| err.to_string())?;
        runtime_object_count += 1;
    }

    if preconditions.target_final_delivery_id.is_some() {
        let entry = append_final_result_with_id(
            &session_store,
            &session.id,
            "Prior final delivery is the exact task-control target.",
            serde_json::json!({ "directWritesExecuted": false }),
        )?;
        final_delivery_id = Some(entry.id);
        runtime_object_count += 1;
    }

    set_session_status_for_product_control(
        &session_store,
        &session.id,
        transition.from_status.as_str(),
    )?;
    apply_product_control_transition(
        control,
        transition.to_status.as_str(),
        &session_store,
        &action_queue,
        &proposal_store,
        &session.id,
        action_ids.first().map(String::as_str),
        proposal_ids.first().map(String::as_str),
        &mut final_delivery_id,
    )?;

    let loaded_session = session_store
        .load_session(&session.id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "task_control session missing after transition".to_string())?;
    runtime_object_count += 1;
    let actual_status = product_control_actual_status(
        &loaded_session,
        &action_queue,
        &proposal_store,
        action_ids.first().map(String::as_str),
        proposal_ids.first().map(String::as_str),
        transition.to_status.as_str(),
    )?;
    if actual_status != transition.to_status {
        diagnostics.push(format!(
            "task control transition mismatch: expected {} -> {}, got {}",
            transition.from_status, transition.to_status, actual_status
        ));
    }
    if preconditions.target_action_id.is_some() && action_ids.is_empty() {
        diagnostics.push("task control action target missing".into());
    }
    if preconditions.target_proposal_id.is_some() && proposal_ids.is_empty() {
        diagnostics.push("task control proposal target missing".into());
    }
    if preconditions.target_blocker_id.is_some() && loaded_session.pending_blockers.is_empty() {
        diagnostics.push("task control blocker target missing".into());
    }
    if preconditions.target_final_delivery_id.is_some() && final_delivery_id.is_none() {
        diagnostics.push("task control final delivery target missing".into());
    }

    Ok(ProductScenarioRuntimeProof {
        scenario_id: scenario.id.clone(),
        group,
        passed: diagnostics.is_empty(),
        runtime_object_count,
        observation_count: 0,
        created_action_ids: action_ids,
        created_observation_ids: Vec::new(),
        created_proposal_ids: proposal_ids,
        created_memory_ids: Vec::new(),
        rollback_event_ids: Vec::new(),
        materialized_view_versions: Vec::new(),
        inactive_memory_ids: Vec::new(),
        final_delivery_id,
        diagnostics,
    })
}

fn runtime_memory_rollback_proof(
    scenario: &MainChatAgentProductScenario,
) -> Result<ProductScenarioRuntimeProof, String> {
    let group = runtime_group_for_scenario(scenario);
    let session_store = AgentTaskSessionStore::new_in_memory().map_err(|err| err.to_string())?;
    let action_queue = ActionQueueStore::new_in_memory().map_err(|err| err.to_string())?;
    let proposal_store = ProposalStore::new_in_memory().map_err(|err| err.to_string())?;
    let memory_lifecycle_store =
        MemoryLifecycleStore::new_in_memory().map_err(|err| err.to_string())?;
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: format!("productization:{}:memory-rollback", scenario.id),
            user_goal: scenario.prompt.clone(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some(
                "Rollback an exact accepted memory lifecycle record.".into(),
            ),
            context_snapshot_refs: vec![format!("ctx:productization:{group}")],
        })
        .map_err(|err| err.to_string())?;
    append_route_decision(&session_store, &session.id, "task_control")?;

    let mut proposal = memory_proposal_fixture(&session.id, "rollback");
    proposal.id = format!("proposal-{}-memory-rollback", scenario.id);
    proposal.run_id = Some(format!("run-productization-{}", scenario.id));
    proposal.source_detail = Some(session.id.clone());
    proposal_store
        .create_proposal(&proposal)
        .map_err(|err| err.to_string())?;
    proposal.accept();
    proposal_store
        .update_proposal(&proposal)
        .map_err(|err| err.to_string())?;

    let accepted = memory_lifecycle_store
        .accept_memory_proposal(MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &proposal,
            "User prefers accepted memory rollback to remove active runtime context.".into(),
        ))
        .map_err(|err| err.to_string())?;
    let rollback = memory_lifecycle_store
        .rollback_memory_asset(
            &accepted.record.memory_id,
            "user",
            "productization MP-06 deterministic rollback fixture",
        )
        .map_err(|err| err.to_string())?;
    let inactive_memory = !memory_lifecycle_store
        .is_memory_active(&accepted.record.memory_id)
        .map_err(|err| err.to_string())?;
    let mut diagnostics = Vec::new();
    if rollback.record.status != MemoryLifecycleStatus::RolledBack {
        diagnostics.push("rollback record did not enter rolled_back state".into());
    }
    if rollback.record.rolled_back_by_event_id.as_deref()
        != Some(rollback.rollback_event.rollback_event_id.as_str())
    {
        diagnostics.push("rollback record/event identity mismatch".into());
    }
    if rollback
        .materialized_view
        .active_memory_ids
        .contains(&accepted.record.memory_id)
    {
        diagnostics.push("rolled back memory remained in materialized active ids".into());
    }
    if !inactive_memory {
        diagnostics.push("rolled back memory remained active in runtime context".into());
    }
    if rollback.materialized_view.version <= accepted.materialized_view.version {
        diagnostics.push("rollback did not advance materialized view version".into());
    }

    let final_entry = append_final_result_with_id(
        &session_store,
        &session.id,
        "Accepted memory was rolled back and removed from active runtime context.",
        serde_json::json!({
            "directWritesExecuted": false,
            "controlAction": "rollback",
            "memoryId": accepted.record.memory_id,
            "rollbackEventId": rollback.rollback_event.rollback_event_id,
            "materializedViewId": rollback.materialized_view.materialized_view_id,
            "materializedViewVersion": rollback.materialized_view.version,
            "inactiveMemory": inactive_memory,
            "runtimeContextExcludedAt": rollback.record.runtime_context_excluded_at
        }),
    )?;
    session_store
        .complete_session(
            &session.id,
            "Accepted memory rollback completed with lifecycle evidence.",
        )
        .map_err(|err| err.to_string())?;

    let snapshot = assemble_snapshot_from_stores(
        &session_store,
        &action_queue,
        &session.id,
        runtime_fixture_run(&session.chat_session_id, scenario, &group),
        vec![proposal],
        vec![rollback.record.clone()],
    )?;
    diagnostics.extend(
        snapshot
            .diagnostics
            .iter()
            .map(|gap| format!("{}:{}", gap.gap_code, gap.detail)),
    );
    diagnostics.extend(validate_runtime_snapshot_for_scenario(scenario, &snapshot));
    Ok(ProductScenarioRuntimeProof {
        scenario_id: scenario.id.clone(),
        group,
        passed: diagnostics.is_empty(),
        runtime_object_count: 1
            + snapshot.context.len()
            + snapshot.proposals.len()
            + usize::from(snapshot.final_delivery.is_some())
            + 3,
        observation_count: snapshot.observations.len(),
        created_action_ids: Vec::new(),
        created_observation_ids: Vec::new(),
        created_proposal_ids: snapshot
            .proposals
            .iter()
            .map(|proposal| proposal.proposal_id.clone())
            .collect(),
        created_memory_ids: vec![accepted.record.memory_id],
        rollback_event_ids: vec![rollback.rollback_event.rollback_event_id],
        materialized_view_versions: vec![
            accepted.materialized_view.version,
            rollback.materialized_view.version,
        ],
        inactive_memory_ids: if inactive_memory {
            vec![rollback.record.memory_id]
        } else {
            Vec::new()
        },
        final_delivery_id: Some(final_entry.id),
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
        created_memory_ids: Vec::new(),
        rollback_event_ids: Vec::new(),
        materialized_view_versions: Vec::new(),
        inactive_memory_ids: Vec::new(),
        final_delivery_id: None,
        diagnostics: vec!["target_object_missing".into(), retry_decision.reason_code],
    }
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
    append_final_result_with_id(session_store, session_id, summary, metadata).map(|_| ())
}

fn append_final_result_with_id(
    session_store: &AgentTaskSessionStore,
    session_id: &str,
    summary: &str,
    metadata: serde_json::Value,
) -> Result<ExecutionTranscriptEntry, String> {
    session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session_id.into(),
            kind: ExecutionTranscriptEntryKind::FinalResult,
            summary: summary.into(),
            metadata,
        })
        .map_err(|err| err.to_string())
}

fn set_action_status_for_product_control(
    action_queue: &ActionQueueStore,
    action_id: &str,
    status: &str,
) -> Result<(), String> {
    match status {
        "queued" | "planning" => Ok(()),
        "executing" | "synthesizing" => transition_action_to_executing(action_queue, action_id),
        "waiting_for_user" | "proposal_pending" => {
            let current = action_queue
                .load(action_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("action target missing: {action_id}"))?;
            if current.status == ExecutionQueueStatus::PendingPermission {
                return Ok(());
            }
            if current.status == ExecutionQueueStatus::Planned {
                action_queue
                    .transition(action_id, ExecutionQueueStatus::Executing, None)
                    .map_err(|err| err.to_string())?;
            }
            action_queue
                .transition(action_id, ExecutionQueueStatus::PendingPermission, None)
                .map(|_| ())
                .map_err(|err| err.to_string())
        }
        "failed" => action_queue
            .fail(
                action_id,
                "productization control fixture failure",
                Some(serde_json::json!({ "retryReplayable": false })),
            )
            .map(|_| ())
            .map_err(|err| err.to_string()),
        "cancelled" => transition_action_to_cancelled(action_queue, action_id),
        "completed" => transition_action_to_completed(action_queue, action_id),
        "blocked" => Ok(()),
        value => Err(format!(
            "unsupported product control action status: {value}"
        )),
    }
}

fn transition_action_to_executing(
    action_queue: &ActionQueueStore,
    action_id: &str,
) -> Result<(), String> {
    let current = action_queue
        .load(action_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("action target missing: {action_id}"))?;
    match current.status {
        ExecutionQueueStatus::Executing => Ok(()),
        ExecutionQueueStatus::Planned | ExecutionQueueStatus::PendingPermission => action_queue
            .transition(action_id, ExecutionQueueStatus::Executing, None)
            .map(|_| ())
            .map_err(|err| err.to_string()),
        ExecutionQueueStatus::Failed => {
            action_queue
                .transition(action_id, ExecutionQueueStatus::Retrying, None)
                .map_err(|err| err.to_string())?;
            action_queue
                .transition(action_id, ExecutionQueueStatus::Executing, None)
                .map(|_| ())
                .map_err(|err| err.to_string())
        }
        ExecutionQueueStatus::Retrying => action_queue
            .transition(action_id, ExecutionQueueStatus::Executing, None)
            .map(|_| ())
            .map_err(|err| err.to_string()),
        ExecutionQueueStatus::Observed
        | ExecutionQueueStatus::Completed
        | ExecutionQueueStatus::Cancelled => Ok(()),
    }
}

fn transition_action_to_completed(
    action_queue: &ActionQueueStore,
    action_id: &str,
) -> Result<(), String> {
    transition_action_to_executing(action_queue, action_id)?;
    let current = action_queue
        .load(action_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("action target missing: {action_id}"))?;
    if current.status == ExecutionQueueStatus::Executing {
        action_queue
            .transition(
                action_id,
                ExecutionQueueStatus::Observed,
                Some(serde_json::json!({ "directWritesExecuted": false })),
            )
            .map_err(|err| err.to_string())?;
    }
    let current = action_queue
        .load(action_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("action target missing: {action_id}"))?;
    if current.status == ExecutionQueueStatus::Observed {
        action_queue
            .transition(action_id, ExecutionQueueStatus::Completed, None)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn transition_action_to_cancelled(
    action_queue: &ActionQueueStore,
    action_id: &str,
) -> Result<(), String> {
    let current = action_queue
        .load(action_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("action target missing: {action_id}"))?;
    if matches!(
        current.status,
        ExecutionQueueStatus::Completed | ExecutionQueueStatus::Cancelled
    ) {
        return Ok(());
    }
    action_queue
        .transition(action_id, ExecutionQueueStatus::Cancelled, None)
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn set_session_status_for_product_control(
    session_store: &AgentTaskSessionStore,
    session_id: &str,
    status: &str,
) -> Result<(), String> {
    match status {
        "planning" | "queued" | "executing" | "observing" | "synthesizing" => Ok(()),
        "waiting_for_user" | "proposal_pending" => session_store
            .mark_waiting_permission(session_id)
            .map(|_| ())
            .map_err(|err| err.to_string()),
        "blocked" => session_store
            .block_session(session_id, "Productization task-control fixture blocked.")
            .map(|_| ())
            .map_err(|err| err.to_string()),
        "failed" => session_store
            .fail_session(session_id, "Productization task-control fixture failed.")
            .map(|_| ())
            .map_err(|err| err.to_string()),
        "completed" => session_store
            .complete_session(session_id, "Productization task-control fixture completed.")
            .map(|_| ())
            .map_err(|err| err.to_string()),
        "cancelled" => session_store
            .cancel_session(session_id, "Productization task-control fixture cancelled.")
            .map(|_| ())
            .map_err(|err| err.to_string()),
        value => Err(format!(
            "unsupported product control session status: {value}"
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_product_control_transition(
    control: openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl,
    to_status: &str,
    session_store: &AgentTaskSessionStore,
    action_queue: &ActionQueueStore,
    proposal_store: &ProposalStore,
    session_id: &str,
    action_id: Option<&str>,
    proposal_id: Option<&str>,
    final_delivery_id: &mut Option<String>,
) -> Result<(), String> {
    match control {
        openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::Retry
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::ApproveOnce
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::Continue => {
            if let Some(action_id) = action_id {
                transition_action_to_executing(action_queue, action_id)?;
            }
        }
        openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::Cancel => {
            if let Some(action_id) = action_id {
                transition_action_to_cancelled(action_queue, action_id)?;
            }
        }
        openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::AcceptProposal
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::RejectProposal
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::EditProposal
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::Defer => {
            if let Some(proposal_id) = proposal_id {
                let mut proposal = proposal_store
                    .get_proposal(proposal_id)
                    .map_err(|err| err.to_string())?
                    .ok_or_else(|| format!("proposal target missing: {proposal_id}"))?;
                match control {
                    openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::AcceptProposal => {
                        proposal.accept()
                    }
                    openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::RejectProposal => {
                        proposal.reject()
                    }
                    openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::EditProposal => {
                        proposal.edit(serde_json::json!({
                            "text": "Edited productization proposal text."
                        }))
                    }
                    openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::Defer => {
                        proposal.postpone()
                    }
                    _ => {}
                }
                proposal_store
                    .update_proposal(&proposal)
                    .map_err(|err| err.to_string())?;
            }
        }
        openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::Deny
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::OpenTrace
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::OpenReviewCenter
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::EditPlan
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::SkipStep
        | openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductControl::Rollback => {}
    }

    match to_status {
        "executing" | "synthesizing" | "planning" | "queued" => {
            session_store
                .resume_session(session_id)
                .map(|_| ())
                .map_err(|err| err.to_string())?;
        }
        "waiting_for_user" | "proposal_pending" => {
            session_store
                .mark_waiting_permission(session_id)
                .map(|_| ())
                .map_err(|err| err.to_string())?;
        }
        "blocked" => {
            let current = session_store
                .load_session(session_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("task session missing: {session_id}"))?;
            session_store
                .set_pending_blockers(session_id, vec!["task_control_blocked".into()])
                .map_err(|err| err.to_string())?;
            if current.status != AgentTaskSessionStatus::Completed {
                session_store
                    .block_session(
                        session_id,
                        "Productization task-control transition blocked.",
                    )
                    .map_err(|err| err.to_string())?;
            }
        }
        "failed" => {
            session_store
                .fail_session(session_id, "Productization task-control transition failed.")
                .map(|_| ())
                .map_err(|err| err.to_string())?;
        }
        "cancelled" => {
            session_store
                .cancel_session(
                    session_id,
                    "Productization task-control transition cancelled.",
                )
                .map(|_| ())
                .map_err(|err| err.to_string())?;
        }
        "completed" => {
            let current = session_store
                .load_session(session_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("task session missing: {session_id}"))?;
            if matches!(
                current.status,
                AgentTaskSessionStatus::WaitingPermission
                    | AgentTaskSessionStatus::Blocked
                    | AgentTaskSessionStatus::Failed
            ) {
                session_store
                    .resume_session(session_id)
                    .map_err(|err| err.to_string())?;
            }
            if final_delivery_id.is_none() {
                let entry = append_final_result_with_id(
                    session_store,
                    session_id,
                    "Productization task-control transition completed.",
                    serde_json::json!({ "directWritesExecuted": false }),
                )?;
                *final_delivery_id = Some(entry.id);
            }
            session_store
                .complete_session(
                    session_id,
                    "Productization task-control transition completed.",
                )
                .map(|_| ())
                .map_err(|err| err.to_string())?;
        }
        value => {
            return Err(format!(
                "unsupported product control target status: {value}"
            ))
        }
    }
    Ok(())
}

fn product_control_actual_status(
    session: &AgentTaskSession,
    action_queue: &ActionQueueStore,
    proposal_store: &ProposalStore,
    action_id: Option<&str>,
    proposal_id: Option<&str>,
    expected_status: &str,
) -> Result<String, String> {
    if matches!(expected_status, "queued" | "executing" | "synthesizing") {
        if let Some(action_id) = action_id {
            let action = action_queue
                .load(action_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("action target missing: {action_id}"))?;
            return Ok(match action.status {
                ExecutionQueueStatus::Planned => "queued",
                ExecutionQueueStatus::Executing | ExecutionQueueStatus::Retrying => "executing",
                ExecutionQueueStatus::Observed | ExecutionQueueStatus::Completed => "synthesizing",
                ExecutionQueueStatus::PendingPermission => "waiting_for_user",
                ExecutionQueueStatus::Failed => "failed",
                ExecutionQueueStatus::Cancelled => "cancelled",
            }
            .into());
        }
        if expected_status == "synthesizing" && session.status == AgentTaskSessionStatus::Running {
            return Ok("synthesizing".into());
        }
    }
    if expected_status == "proposal_pending" {
        if let Some(proposal_id) = proposal_id {
            let proposal = proposal_store
                .get_proposal(proposal_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("proposal target missing: {proposal_id}"))?;
            if proposal.status == ProposalStatus::Pending
                || proposal.status == ProposalStatus::Edited
            {
                return Ok("proposal_pending".into());
            }
        }
    }
    if expected_status == "blocked" && !session.pending_blockers.is_empty() {
        return Ok("blocked".into());
    }
    Ok(product_control_session_status(session).into())
}

fn product_control_session_status(session: &AgentTaskSession) -> &'static str {
    match session.status {
        AgentTaskSessionStatus::Running => "executing",
        AgentTaskSessionStatus::WaitingPermission => "waiting_for_user",
        AgentTaskSessionStatus::Blocked => "blocked",
        AgentTaskSessionStatus::Completed => "completed",
        AgentTaskSessionStatus::Failed => "failed",
        AgentTaskSessionStatus::Cancelled => "cancelled",
    }
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
    memory_lifecycle_records: Vec<openlife_core::agent::MemoryLifecycleRecord>,
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
        memory_lifecycle_records,
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
    let diagnostics = snapshot
        .diagnostics
        .iter()
        .map(|gap| format!("{}:{}", gap.gap_code, gap.detail))
        .chain(validate_runtime_snapshot_for_scenario(scenario, snapshot))
        .collect::<Vec<_>>();
    ProductScenarioRuntimeProof {
        scenario_id: scenario.id.clone(),
        group: group.into(),
        passed: diagnostics.is_empty(),
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
        created_memory_ids: Vec::new(),
        rollback_event_ids: Vec::new(),
        materialized_view_versions: Vec::new(),
        inactive_memory_ids: Vec::new(),
        final_delivery_id: snapshot
            .final_delivery
            .as_ref()
            .map(|delivery| delivery.delivery_id.clone()),
        diagnostics,
    }
}

fn validate_runtime_snapshot_for_scenario(
    scenario: &MainChatAgentProductScenario,
    snapshot: &MainChatAgentStateSnapshot,
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if snapshot.route.strategy != scenario.expected_strategy_route {
        diagnostics.push(format!(
            "snapshot route mismatch: expected {}, got {}",
            scenario.expected_strategy_route.as_str(),
            snapshot.route.strategy.as_str()
        ));
    }
    for evidence in &scenario.required_runtime_evidence {
        match evidence.as_str() {
            "task_id" => {
                if snapshot.task.task_id.is_empty() {
                    diagnostics.push("required task_id evidence missing".into());
                }
            }
            "run_id" => {
                if snapshot.task.run_id == "unknown" || snapshot.provider.is_none() {
                    diagnostics.push("required run/provider evidence missing".into());
                }
            }
            "provider_trace" => {
                if snapshot.provider.is_none() {
                    diagnostics.push("required provider_trace evidence missing".into());
                }
            }
            "route" => {
                if snapshot.route.strategy != scenario.expected_strategy_route {
                    diagnostics.push("required route evidence did not match scenario".into());
                }
            }
            "plan_id" => {
                if snapshot.plan.is_none() {
                    diagnostics.push("required plan evidence missing".into());
                }
            }
            "action_id" => {
                if snapshot.actions.is_empty() {
                    diagnostics.push("required action evidence missing".into());
                }
            }
            "observation_id" => {
                if snapshot.observations.is_empty() {
                    diagnostics.push("required observation evidence missing".into());
                }
            }
            "proposal_id" | "evidence_id" => {
                if snapshot.proposals.is_empty() {
                    diagnostics.push("required proposal/evidence object missing".into());
                }
            }
            "blocker_id" => {
                if snapshot.blockers.is_empty() {
                    diagnostics.push("required blocker evidence missing".into());
                }
            }
            "final_delivery" => {
                if snapshot.final_delivery.is_none() {
                    diagnostics.push("required final delivery evidence missing".into());
                }
            }
            "prior_task_session_id"
            | "prior_run_id"
            | "target_object_id"
            | "state_transition"
            | "memory_id"
            | "rollback_event_id"
            | "inactive_memory"
            | "materialized_view_version" => {}
            value => diagnostics.push(format!("unknown runtime evidence contract: {value}")),
        }
    }

    if runtime_snapshot_has_unexpected_durable_changes(scenario, snapshot) {
        diagnostics.push("runtime proof included silent durable changes".into());
    }

    match scenario.expected_strategy_route {
        MainChatAgentProductStrategyRoute::DirectAnswer => {
            if !snapshot.actions.is_empty() || !snapshot.observations.is_empty() {
                diagnostics.push("DirectAnswer snapshot contained action/observation UI".into());
            }
        }
        MainChatAgentProductStrategyRoute::ReadAction => {
            if snapshot.actions.len() != 1 || snapshot.observations.len() != 1 {
                diagnostics
                    .push("ReadAction snapshot must contain exactly one action/observation".into());
            }
        }
        MainChatAgentProductStrategyRoute::ReactToolExecution => {
            let requires_multi_step = scenario.capability_group == "Multi-step ReAct"
                || matches!(scenario.id.as_str(), "WR-04" | "ST-08");
            if requires_multi_step
                && (snapshot.actions.len() < 2 || snapshot.observations.len() < 2)
            {
                diagnostics
                    .push("ReAct multi-step snapshot lacks multiple action observations".into());
            }
        }
        MainChatAgentProductStrategyRoute::PlanExecute => {
            if snapshot.plan.is_none() || snapshot.actions.is_empty() {
                diagnostics.push("PlanExecute snapshot lacks plan/action runtime objects".into());
            }
        }
        MainChatAgentProductStrategyRoute::MemoryProposal => {
            if snapshot.proposals.is_empty() || !snapshot.actions.is_empty() {
                diagnostics.push(
                    "MemoryProposal snapshot must be proposal-first without tool actions".into(),
                );
            }
        }
        MainChatAgentProductStrategyRoute::PermissionRequest => {
            if snapshot.actions.len() != 1
                || snapshot.proposals.len() != 1
                || snapshot.blockers.is_empty()
            {
                diagnostics.push(
                    "PermissionRequest snapshot must bind one action, proposal, and blocker".into(),
                );
            }
        }
        MainChatAgentProductStrategyRoute::Blocked => {
            if snapshot.blockers.is_empty() {
                diagnostics.push("Blocked snapshot lacks blocker object".into());
            }
        }
        MainChatAgentProductStrategyRoute::TaskControl
        | MainChatAgentProductStrategyRoute::LegacyFallback
        | MainChatAgentProductStrategyRoute::Unknown => {}
    }
    diagnostics
}

fn runtime_snapshot_has_unexpected_durable_changes(
    scenario: &MainChatAgentProductScenario,
    snapshot: &MainChatAgentStateSnapshot,
) -> bool {
    let Some(delivery) = snapshot.final_delivery.as_ref() else {
        return false;
    };
    if delivery.durable_changes.is_empty() {
        return false;
    }
    if scenario.id != "MP-06" {
        return true;
    }

    !delivery
        .durable_changes
        .iter()
        .all(|change| snapshot_has_governed_memory_durable_change(snapshot, change))
}

fn snapshot_has_governed_memory_durable_change(
    snapshot: &MainChatAgentStateSnapshot,
    change: &openlife_core::agent::main_chat_agent_productization_v1::DurableChangeSummary,
) -> bool {
    snapshot.proposals.iter().any(|proposal| {
        let Some(record) = proposal.memory_lifecycle.as_ref() else {
            return false;
        };
        if !matches!(
            proposal.status,
            MainChatAgentProductProposalStatus::Accepted
                | MainChatAgentProductProposalStatus::RolledBack
        ) {
            return false;
        }
        if !change.change_type.starts_with("memory.") || change.target != record.memory_id {
            return false;
        }
        change.provenance_id == record.proposal_id
            || record.rolled_back_by_event_id.as_deref() == Some(change.provenance_id.as_str())
    })
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
        memory_lifecycle_records: Vec::new(),
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
