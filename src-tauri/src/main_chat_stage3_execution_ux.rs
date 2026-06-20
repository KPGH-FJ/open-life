use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage3ExecutionUxCoverageRow {
    pub scenario_id: String,
    pub scenario: String,
    pub status: String,
    pub evidence: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage3ExecutionUxReport {
    pub report_kind: String,
    pub schema_version: String,
    pub data_path: String,
    pub total_scenario_count: usize,
    pub passed_scenario_count: usize,
    pub failed_scenario_count: usize,
    pub blocked_scenario_count: usize,
    pub execution_first_required_ids: Vec<String>,
    pub execution_first_passed_ids: Vec<String>,
    pub execution_first_claim_valid: bool,
    pub ready_for_limited_internal_trial: bool,
    pub readiness_recommendation: String,
    pub stage2_readiness_preserved: String,
    pub non_goals: Vec<String>,
    pub coverage: Vec<MainChatStage3ExecutionUxCoverageRow>,
    pub blockers: Vec<String>,
}

struct Stage3UxScenarioSpec {
    id: &'static str,
    scenario: &'static str,
    productization_proofs: &'static [&'static str],
    extra_evidence: &'static [&'static str],
}

const EXECUTION_FIRST_REQUIRED_IDS: &[&str] = &[
    "UX3-02", "UX3-03", "UX3-04", "UX3-06", "UX3-09", "UX3-11", "UX3-12",
];

pub(crate) fn run_main_chat_stage3_execution_ux_report() -> MainChatStage3ExecutionUxReport {
    let productization =
        crate::main_chat_agent_productization_eval::run_main_chat_agent_productization_v1_gate_report(
        );
    let event_gate =
        crate::main_chat_event_stream::run_main_chat_agent_product_maturity_v2_event_gate();
    let passed_productization_ids = productization
        .runtime_required_group_evidence
        .iter()
        .filter(|proof| proof.passed)
        .map(|proof| proof.scenario_id.as_str())
        .collect::<BTreeSet<_>>();
    let productization_ready = productization.full_productization_v1_complete
        && productization.failed_scenario_count == 0
        && productization.runtime_required_group_count
            == productization.runtime_required_group_passed_count;
    let event_recovery_ready = event_gate.ready
        && event_gate.proofs.iter().any(|proof| {
            proof.ui_state.iter().any(|state| {
                matches!(
                    state.as_str(),
                    "replaying_events"
                        | "stream_recovered"
                        | "snapshot_backfill_excluded_from_live_credit"
                )
            })
        });

    let coverage = stage3_ux_scenarios()
        .into_iter()
        .map(|scenario| {
            let missing = scenario
                .productization_proofs
                .iter()
                .filter(|proof_id| !passed_productization_ids.contains(**proof_id))
                .map(|proof_id| format!("missing_productization_runtime_proof:{proof_id}"))
                .collect::<Vec<_>>();
            let mut blockers = Vec::new();
            if !productization_ready {
                blockers.push("productization_runtime_gate_not_complete".into());
            }
            blockers.extend(missing);
            if scenario.id == "UX3-12" && !event_recovery_ready {
                blockers.push("event_stream_reload_recovery_not_proven".into());
            }
            let mut evidence = scenario
                .productization_proofs
                .iter()
                .map(|proof_id| format!("productization_runtime_proof:{proof_id}"))
                .collect::<Vec<_>>();
            evidence.extend(
                scenario
                    .extra_evidence
                    .iter()
                    .map(|item| (*item).to_string()),
            );
            MainChatStage3ExecutionUxCoverageRow {
                scenario_id: scenario.id.into(),
                scenario: scenario.scenario.into(),
                status: if blockers.is_empty() {
                    "passed".into()
                } else {
                    "blocked".into()
                },
                evidence,
                blockers,
            }
        })
        .collect::<Vec<_>>();

    let passed_scenario_count = coverage.iter().filter(|row| row.status == "passed").count();
    let failed_scenario_count = coverage.iter().filter(|row| row.status == "failed").count();
    let blocked_scenario_count = coverage
        .iter()
        .filter(|row| row.status == "blocked")
        .count();
    let execution_first_passed_ids = EXECUTION_FIRST_REQUIRED_IDS
        .iter()
        .filter(|id| {
            coverage
                .iter()
                .any(|row| row.scenario_id == **id && row.status == "passed")
        })
        .map(|id| (*id).to_string())
        .collect::<Vec<_>>();
    let execution_first_claim_valid =
        execution_first_passed_ids.len() == EXECUTION_FIRST_REQUIRED_IDS.len();
    let blockers = coverage
        .iter()
        .filter(|row| row.status != "passed")
        .flat_map(|row| {
            row.blockers
                .iter()
                .map(move |blocker| format!("{}:{blocker}", row.scenario_id))
        })
        .collect::<Vec<_>>();

    MainChatStage3ExecutionUxReport {
        report_kind: "main_chat_stage3_execution_ux".into(),
        schema_version: "stage3-execution-ux-v1".into(),
        data_path: "Main Chat send/stream -> AgentIngress / strategy route -> AgentTaskSession / ActionQueue / ExecutionTranscript / Main Chat event stream -> MainChatAgentStateSnapshot -> AgentControlPlane".into(),
        total_scenario_count: coverage.len(),
        passed_scenario_count,
        failed_scenario_count,
        blocked_scenario_count,
        execution_first_required_ids: EXECUTION_FIRST_REQUIRED_IDS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        execution_first_passed_ids,
        execution_first_claim_valid,
        ready_for_limited_internal_trial: false,
        readiness_recommendation: "not_ready_for_limited_internal_trial".into(),
        stage2_readiness_preserved: "stage2_readiness_remains_fail_closed_without_manual_dogfood_and_current_commit_live_evidence".into(),
        non_goals: vec![
            "manual_dogfood_rows_not_run_or_fabricated".into(),
            "ready_for_limited_internal_trial_not_claimed".into(),
            "no_parallel_runtime_task_event_proposal_or_memory_system".into(),
            "stage2_readiness_gate_not_replaced".into(),
        ],
        coverage,
        blockers,
    }
}

fn stage3_ux_scenarios() -> Vec<Stage3UxScenarioSpec> {
    vec![
        Stage3UxScenarioSpec {
            id: "UX3-01",
            scenario: "Direct answer",
            productization_proofs: &["OA-02"],
            extra_evidence: &[
                "AgentControlPlane compact direct-answer state",
                "no fake action observations for DirectAnswer",
            ],
        },
        Stage3UxScenarioSpec {
            id: "UX3-02",
            scenario: "File read success",
            productization_proofs: &["FR-01"],
            extra_evidence: &[
                "ActionQueue file.read evidence",
                "ExecutionTranscript observation",
            ],
        },
        Stage3UxScenarioSpec {
            id: "UX3-03",
            scenario: "Missing file",
            productization_proofs: &["FR-03"],
            extra_evidence: &["blocker_id", "missing source reason"],
        },
        Stage3UxScenarioSpec {
            id: "UX3-04",
            scenario: "Web policy blocker",
            productization_proofs: &["WR-02"],
            extra_evidence: &["network policy blocker", "no fake web observation"],
        },
        Stage3UxScenarioSpec {
            id: "UX3-05",
            scenario: "Registered MCP read",
            productization_proofs: &["MCP-01", "MCP-03"],
            extra_evidence: &["selected MCP target evidence", "observation source label"],
        },
        Stage3UxScenarioSpec {
            id: "UX3-06",
            scenario: "Tool permission proposal",
            productization_proofs: &["MCP-04"],
            extra_evidence: &["ToolPermission proposal", "exact action target scope"],
        },
        Stage3UxScenarioSpec {
            id: "UX3-07",
            scenario: "Plan draft",
            productization_proofs: &["PE-01"],
            extra_evidence: &["PlanExecute session/revision controls"],
        },
        Stage3UxScenarioSpec {
            id: "UX3-08",
            scenario: "Memory proposal after read",
            productization_proofs: &["MP-01"],
            extra_evidence: &[
                "ProposalStore pending memory proposal",
                "no materialized claim",
            ],
        },
        Stage3UxScenarioSpec {
            id: "UX3-09",
            scenario: "Retry failed read",
            productization_proofs: &["LT-03", "RA-08"],
            extra_evidence: &["retry control targets failed action"],
        },
        Stage3UxScenarioSpec {
            id: "UX3-10",
            scenario: "Cancel task",
            productization_proofs: &["LT-05", "PB-08"],
            extra_evidence: &["cancelled terminal task state"],
        },
        Stage3UxScenarioSpec {
            id: "UX3-11",
            scenario: "Final delivery",
            productization_proofs: &["FD-02", "FD-03", "FD-04", "FD-08"],
            extra_evidence: &["completed/proposed/blocked/skipped/pending sections"],
        },
        Stage3UxScenarioSpec {
            id: "UX3-12",
            scenario: "Reload recovery",
            productization_proofs: &["LT-07", "LT-08"],
            extra_evidence: &[
                "Main Chat durable event stream replay",
                "conversation-linked task snapshot recovery",
            ],
        },
        Stage3UxScenarioSpec {
            id: "UX3-13",
            scenario: "Reviewer trace",
            productization_proofs: &["FD-08"],
            extra_evidence: &["bounded one-line JSON reviewer trace keys"],
        },
    ]
}
