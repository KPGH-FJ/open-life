use crate::main_chat_live_provider_harness::main_chat_live_provider_eval_opt_in_from_env;
use crate::AppState;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2ScenarioStatus {
    pub scenario_id: String,
    pub phase_id: String,
    pub capability_group: String,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2PhaseCount {
    pub phase_id: String,
    pub phase_label: String,
    pub capability_group: String,
    pub scenario_count: usize,
    pub passed: usize,
    pub expected_blocker: usize,
    pub failed: usize,
    pub blocked: usize,
    pub status: String,
    pub ready: bool,
    pub default_gate: bool,
    pub opt_in_only: bool,
    pub blockers: Vec<String>,
    pub supported_scenarios: Vec<String>,
    pub blocked_scenarios: Vec<String>,
    pub unsupported_scenarios: Vec<String>,
    pub future_scenarios: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatProductMaturityV2FinalReadinessReport {
    pub report_kind: String,
    pub readiness_semantics: String,
    pub default_readiness_scope: String,
    pub opt_in_live_readiness_scope: String,
    pub final_ready: bool,
    pub deterministic_ready: bool,
    pub opt_in_live_ready: bool,
    pub final_readiness_status: String,
    pub deterministic_readiness_status: String,
    pub opt_in_live_readiness_status: String,
    pub default_deterministic_scenario_count: usize,
    pub default_live_prod_excluded_count: usize,
    pub external_live_scenario_count: usize,
    pub default_scenario_passed_count: usize,
    pub default_scenario_expected_blocker_count: usize,
    pub default_scenario_failed_count: usize,
    pub default_scenario_blocked_count: usize,
    pub external_live_passed_count: usize,
    pub external_live_blocked_count: usize,
    pub external_live_failed_count: usize,
    pub phase_counts: Vec<MainChatProductMaturityV2PhaseCount>,
    pub supported_scenarios: Vec<MainChatProductMaturityV2ScenarioStatus>,
    pub blocked_scenarios: Vec<MainChatProductMaturityV2ScenarioStatus>,
    pub unsupported_scenarios: Vec<MainChatProductMaturityV2ScenarioStatus>,
    pub future_scenarios: Vec<MainChatProductMaturityV2ScenarioStatus>,
    pub blockers: Vec<String>,
    pub deterministic_blockers: Vec<String>,
    pub opt_in_live_blockers: Vec<String>,
    pub direct_writes_executed: bool,
    pub no_silent_durable_writes: bool,
    pub default_live_prod_excluded: bool,
}

pub(crate) async fn run_main_chat_agent_product_maturity_v2_final_readiness_report(
    state: &Arc<AppState>,
) -> Result<MainChatProductMaturityV2FinalReadinessReport, String> {
    run_main_chat_agent_product_maturity_v2_final_readiness_report_with_state(
        state,
        main_chat_live_provider_eval_opt_in_from_env(),
    )
    .await
}

pub(crate) async fn run_main_chat_agent_product_maturity_v2_final_readiness_report_with_state(
    state: &Arc<AppState>,
    explicit_live_eval_requested: bool,
) -> Result<MainChatProductMaturityV2FinalReadinessReport, String> {
    let memory = crate::main_chat_memory_lifecycle_eval::run_main_chat_memory_lifecycle_eval_gate();
    let event = crate::main_chat_event_stream::run_main_chat_agent_product_maturity_v2_event_gate();
    let plan =
        crate::main_chat_plan_interaction_eval::run_main_chat_agent_product_maturity_v2_plan_gate()
            .await;
    let task =
        crate::main_chat_task_continuity_eval::run_main_chat_agent_product_maturity_v2_task_continuity_gate()
            .await;
    let skills =
        crate::main_chat_skills_tools::run_main_chat_agent_product_maturity_v2_skills_gate().await;
    let live =
        crate::main_chat_live_productization_eval::run_main_chat_external_live_productization_gate_with_state(
            state,
            explicit_live_eval_requested,
        )
        .await?;

    let mut supported_scenarios = Vec::new();
    let mut blocked_scenarios = Vec::new();
    let unsupported_scenarios = Vec::new();
    let future_scenarios = Vec::new();
    let mut deterministic_blockers = Vec::new();

    let memory_phase =
        phase_count_from_memory(&memory, &mut supported_scenarios, &mut blocked_scenarios);
    push_phase_blockers(&memory_phase, &mut deterministic_blockers);

    let event_phase =
        phase_count_from_event(&event, &mut supported_scenarios, &mut blocked_scenarios);
    push_phase_blockers(&event_phase, &mut deterministic_blockers);

    let plan_phase = phase_count_from_plan(&plan, &mut supported_scenarios, &mut blocked_scenarios);
    push_phase_blockers(&plan_phase, &mut deterministic_blockers);

    let task_phase = phase_count_from_task(&task, &mut supported_scenarios, &mut blocked_scenarios);
    push_phase_blockers(&task_phase, &mut deterministic_blockers);

    let skills_phase =
        phase_count_from_skills(&skills, &mut supported_scenarios, &mut blocked_scenarios);
    push_phase_blockers(&skills_phase, &mut deterministic_blockers);

    let live_phase = phase_count_from_live(&live, &mut supported_scenarios, &mut blocked_scenarios);
    let opt_in_live_blockers = live.blockers.clone();

    let phase_counts = vec![
        memory_phase,
        event_phase,
        plan_phase,
        task_phase,
        skills_phase,
        live_phase,
    ];

    let default_deterministic_scenario_count = phase_counts
        .iter()
        .filter(|phase| phase.default_gate)
        .map(|phase| phase.scenario_count)
        .sum();
    let default_scenario_passed_count = phase_counts
        .iter()
        .filter(|phase| phase.default_gate)
        .map(|phase| phase.passed)
        .sum();
    let default_scenario_expected_blocker_count = phase_counts
        .iter()
        .filter(|phase| phase.default_gate)
        .map(|phase| phase.expected_blocker)
        .sum();
    let default_scenario_failed_count = phase_counts
        .iter()
        .filter(|phase| phase.default_gate)
        .map(|phase| phase.failed)
        .sum();
    let default_scenario_blocked_count = phase_counts
        .iter()
        .filter(|phase| phase.default_gate)
        .map(|phase| phase.blocked)
        .sum();

    let deterministic_ready = deterministic_blockers.is_empty()
        && phase_counts
            .iter()
            .filter(|phase| phase.default_gate)
            .all(|phase| phase.ready);
    let opt_in_live_ready = live.ready;
    let final_ready = deterministic_ready && opt_in_live_ready;

    let deterministic_readiness_status = if deterministic_ready {
        "ready"
    } else {
        "blocked"
    }
    .to_string();
    let opt_in_live_readiness_status = if opt_in_live_ready {
        "ready"
    } else {
        "blocked"
    }
    .to_string();
    let final_readiness_status = if final_ready {
        "ready"
    } else if !deterministic_ready {
        "blocked_deterministic_readiness_not_ready"
    } else {
        "blocked_live_productization_not_ready"
    }
    .to_string();

    let mut blockers = deterministic_blockers.clone();
    for blocker in &opt_in_live_blockers {
        push_unique(&mut blockers, blocker.clone());
    }
    if !deterministic_ready {
        push_unique(
            &mut blockers,
            "deterministic_product_maturity_v2_not_ready".into(),
        );
    }
    if !opt_in_live_ready {
        push_unique(&mut blockers, "live_productization_not_ready".into());
    }

    Ok(MainChatProductMaturityV2FinalReadinessReport {
        report_kind: "main_chat_agent_product_maturity_v2_final_readiness_gate".into(),
        readiness_semantics:
            "phase_g_final_readiness_default_deterministic_live_product_opt_in_separate".into(),
        default_readiness_scope: "MR_EV_PI_LT2_SK2_deterministic_only".into(),
        opt_in_live_readiness_scope: "LIVE_PROD_external_live_opt_in_only".into(),
        final_ready,
        deterministic_ready,
        opt_in_live_ready,
        final_readiness_status,
        deterministic_readiness_status,
        opt_in_live_readiness_status,
        default_deterministic_scenario_count,
        default_live_prod_excluded_count: live.scenario_count,
        external_live_scenario_count: live.scenario_count,
        default_scenario_passed_count,
        default_scenario_expected_blocker_count,
        default_scenario_failed_count,
        default_scenario_blocked_count,
        external_live_passed_count: live.passed_scenario_count,
        external_live_blocked_count: live.blocked_scenario_count,
        external_live_failed_count: live.failed_scenario_count,
        phase_counts,
        supported_scenarios,
        blocked_scenarios,
        unsupported_scenarios,
        future_scenarios,
        blockers,
        deterministic_blockers,
        opt_in_live_blockers,
        direct_writes_executed: live.direct_writes_executed,
        no_silent_durable_writes: !live.direct_writes_executed,
        default_live_prod_excluded: live.default_gate_scenario_count == 0,
    })
}

fn phase_count_from_memory(
    report: &crate::main_chat_memory_lifecycle_eval::MainChatMemoryLifecycleEvalGateReport,
    supported: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
    blocked: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
) -> MainChatProductMaturityV2PhaseCount {
    let phase_id = "phase_a";
    let capability_group = "memory_lifecycle";
    let mut supported_ids = Vec::new();
    let mut blocked_ids = Vec::new();
    for proof in &report.proofs {
        if proof.passed && proof.outcome == "expected_blocker" {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                "expected_blocker",
            ));
        } else if proof.passed {
            supported_ids.push(proof.scenario_id.clone());
            supported.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "supported",
                "passed",
            ));
        } else {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                first_or(&proof.diagnostics, "memory_lifecycle_scenario_failed"),
            ));
        }
    }
    deterministic_phase_count(
        phase_id,
        "Phase A Memory lifecycle",
        capability_group,
        report.scenario_count,
        report.passed_scenario_count,
        report.expected_blocker_count,
        report.ready,
        report.blockers.clone(),
        supported_ids,
        blocked_ids,
    )
}

fn phase_count_from_event(
    report: &crate::main_chat_event_stream::MainChatProductMaturityV2EventGateReport,
    supported: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
    blocked: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
) -> MainChatProductMaturityV2PhaseCount {
    let phase_id = "phase_b";
    let capability_group = "event_delta_stream";
    let mut supported_ids = Vec::new();
    let mut blocked_ids = Vec::new();
    for proof in &report.proofs {
        if proof.passed {
            supported_ids.push(proof.scenario_id.clone());
            supported.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "supported",
                "passed",
            ));
        } else {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                first_or(&proof.diagnostics, "event_delta_scenario_failed"),
            ));
        }
    }
    deterministic_phase_count(
        phase_id,
        "Phase B Event delta stream",
        capability_group,
        report.scenario_count,
        report.passed_scenario_count,
        report.expected_blocker_count,
        report.ready,
        report.blockers.clone(),
        supported_ids,
        blocked_ids,
    )
}

fn phase_count_from_plan(
    report: &crate::main_chat_plan_interaction_eval::MainChatProductMaturityV2PlanGateReport,
    supported: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
    blocked: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
) -> MainChatProductMaturityV2PhaseCount {
    let phase_id = "phase_c";
    let capability_group = "plan_interaction";
    let mut supported_ids = Vec::new();
    let mut blocked_ids = Vec::new();
    for proof in &report.proofs {
        if proof.passed && proof.expected_blocker {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                "expected_blocker",
            ));
        } else if proof.passed {
            supported_ids.push(proof.scenario_id.clone());
            supported.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "supported",
                "passed",
            ));
        } else {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                first_or(&proof.diagnostics, "plan_interaction_scenario_failed"),
            ));
        }
    }
    deterministic_phase_count(
        phase_id,
        "Phase C Plan interaction",
        capability_group,
        report.scenario_count,
        report.passed_scenario_count,
        report.expected_blocker_count,
        report.ready,
        report.blockers.clone(),
        supported_ids,
        blocked_ids,
    )
}

fn phase_count_from_task(
    report: &crate::main_chat_task_continuity_eval::MainChatProductMaturityV2TaskContinuityGateReport,
    supported: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
    blocked: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
) -> MainChatProductMaturityV2PhaseCount {
    let phase_id = "phase_d";
    let capability_group = "task_continuity";
    let mut supported_ids = Vec::new();
    let mut blocked_ids = Vec::new();
    for proof in &report.proofs {
        if proof.passed && proof.expected_blocker {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                "expected_blocker",
            ));
        } else if proof.passed {
            supported_ids.push(proof.scenario_id.clone());
            supported.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "supported",
                "passed",
            ));
        } else {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                first_or(&proof.diagnostics, "task_continuity_scenario_failed"),
            ));
        }
    }
    deterministic_phase_count(
        phase_id,
        "Phase D Task continuity",
        capability_group,
        report.scenario_count,
        report.passed_scenario_count,
        report.expected_blocker_count,
        report.ready,
        report.blockers.clone(),
        supported_ids,
        blocked_ids,
    )
}

fn phase_count_from_skills(
    report: &crate::main_chat_skills_tools::MainChatProductMaturityV2SkillsGateReport,
    supported: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
    blocked: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
) -> MainChatProductMaturityV2PhaseCount {
    let phase_id = "phase_e";
    let capability_group = "skills_tools_surface";
    let mut supported_ids = Vec::new();
    let mut blocked_ids = Vec::new();
    for proof in &report.proofs {
        if proof.passed && proof.expected_blocker {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                "expected_blocker",
            ));
        } else if proof.passed {
            supported_ids.push(proof.scenario_id.clone());
            supported.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "supported",
                "passed",
            ));
        } else {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                first_or(&proof.diagnostics, "skills_tools_scenario_failed"),
            ));
        }
    }
    deterministic_phase_count(
        phase_id,
        "Phase E Skills/tool surface",
        capability_group,
        report.scenario_count,
        report.passed_scenario_count,
        report.expected_blocker_count,
        report.ready,
        report.blockers.clone(),
        supported_ids,
        blocked_ids,
    )
}

fn phase_count_from_live(
    report: &crate::main_chat_live_productization_eval::MainChatExternalLiveProductizationGateReport,
    supported: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
    blocked: &mut Vec<MainChatProductMaturityV2ScenarioStatus>,
) -> MainChatProductMaturityV2PhaseCount {
    let phase_id = "phase_f";
    let capability_group = "external_live_productization";
    let mut supported_ids = Vec::new();
    let mut blocked_ids = Vec::new();
    for proof in &report.proofs {
        if proof.passed {
            supported_ids.push(proof.scenario_id.clone());
            supported.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "supported",
                "passed",
            ));
        } else {
            blocked_ids.push(proof.scenario_id.clone());
            blocked.push(scenario_status(
                &proof.scenario_id,
                phase_id,
                capability_group,
                "blocked",
                proof
                    .blockers
                    .first()
                    .map(String::as_str)
                    .unwrap_or(proof.status.as_str()),
            ));
        }
    }

    MainChatProductMaturityV2PhaseCount {
        phase_id: phase_id.into(),
        phase_label: "Phase F External live product evidence".into(),
        capability_group: capability_group.into(),
        scenario_count: report.scenario_count,
        passed: report.passed_scenario_count,
        expected_blocker: 0,
        failed: report.failed_scenario_count,
        blocked: report.blocked_scenario_count,
        status: if report.ready { "ready" } else { "blocked" }.into(),
        ready: report.ready,
        default_gate: false,
        opt_in_only: true,
        blockers: report.blockers.clone(),
        supported_scenarios: supported_ids,
        blocked_scenarios: blocked_ids,
        unsupported_scenarios: Vec::new(),
        future_scenarios: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn deterministic_phase_count(
    phase_id: &str,
    phase_label: &str,
    capability_group: &str,
    scenario_count: usize,
    passed_scenario_count: usize,
    expected_blocker_count: usize,
    ready: bool,
    blockers: Vec<String>,
    supported_scenarios: Vec<String>,
    blocked_scenarios: Vec<String>,
) -> MainChatProductMaturityV2PhaseCount {
    let failed = scenario_count.saturating_sub(passed_scenario_count);
    MainChatProductMaturityV2PhaseCount {
        phase_id: phase_id.into(),
        phase_label: phase_label.into(),
        capability_group: capability_group.into(),
        scenario_count,
        passed: passed_scenario_count.saturating_sub(expected_blocker_count),
        expected_blocker: expected_blocker_count,
        failed,
        blocked: 0,
        status: if ready { "ready" } else { "blocked" }.into(),
        ready,
        default_gate: true,
        opt_in_only: false,
        blockers,
        supported_scenarios,
        blocked_scenarios,
        unsupported_scenarios: Vec::new(),
        future_scenarios: Vec::new(),
    }
}

fn push_phase_blockers(
    phase: &MainChatProductMaturityV2PhaseCount,
    deterministic_blockers: &mut Vec<String>,
) {
    if phase.ready {
        return;
    }
    if phase.blockers.is_empty() {
        push_unique(
            deterministic_blockers,
            format!("{}_{}_not_ready", phase.phase_id, phase.capability_group),
        );
        return;
    }
    for blocker in &phase.blockers {
        push_unique(deterministic_blockers, blocker.clone());
    }
}

fn scenario_status(
    scenario_id: &str,
    phase_id: &str,
    capability_group: &str,
    status: &str,
    reason: impl Into<String>,
) -> MainChatProductMaturityV2ScenarioStatus {
    MainChatProductMaturityV2ScenarioStatus {
        scenario_id: scenario_id.into(),
        phase_id: phase_id.into(),
        capability_group: capability_group.into(),
        status: status.into(),
        reason: reason.into(),
    }
}

fn first_or(values: &[String], fallback: &str) -> String {
    values
        .first()
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}
