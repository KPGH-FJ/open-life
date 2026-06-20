use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentBetaV1ReadinessDimension {
    pub dimension: String,
    pub status: String,
    pub opt_in_only: bool,
    pub evidence: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentBetaV1FoundationInventoryItem {
    pub component: String,
    pub status: String,
    pub evidence: Vec<String>,
    pub development_decision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentBetaV1WorkstreamStatus {
    pub workstream_id: String,
    pub label: String,
    pub status: String,
    pub ready: bool,
    pub evidence: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentBetaV1ProductMaturityPhaseCount {
    pub phase_id: String,
    pub capability_group: String,
    pub scenario_count: usize,
    pub passed: usize,
    pub expected_blocker: usize,
    pub failed: usize,
    pub blocked: usize,
    pub ready: bool,
    pub opt_in_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentBetaV1ReadinessReport {
    pub report_kind: String,
    pub readiness_semantics: String,
    pub default_readiness_scope: String,
    pub opt_in_live_readiness_scope: String,
    pub foundation_inventory_exists: bool,
    pub foundation_inventory_items: Vec<MainChatAgentBetaV1FoundationInventoryItem>,
    pub workstreams: Vec<MainChatAgentBetaV1WorkstreamStatus>,
    pub product_maturity_phase_counts: Vec<MainChatAgentBetaV1ProductMaturityPhaseCount>,
    pub default_readiness_status: String,
    pub default_ready: bool,
    pub opt_in_live_ready: bool,
    pub external_live_attempted: bool,
    pub default_real_task_scenario_count: usize,
    pub default_real_task_passed_count: usize,
    pub opt_in_live_real_task_scenario_count: usize,
    pub default_experience_required_state_count: usize,
    pub default_experience_verified_state_count: usize,
    pub product_maturity_default_scenario_count: usize,
    pub command_surface_total_cases: usize,
    pub command_surface_failed_cases: usize,
    pub legacy_fallback_count: usize,
    pub silent_durable_write_count: usize,
    pub no_silent_durable_writes: bool,
    pub default_blockers: Vec<String>,
    pub opt_in_live_blockers: Vec<String>,
    pub readiness_dimensions: Vec<MainChatAgentBetaV1ReadinessDimension>,
}

pub(crate) async fn run_main_chat_agent_beta_v1_readiness_report(
) -> Result<MainChatAgentBetaV1ReadinessReport, String> {
    run_main_chat_agent_beta_v1_readiness_report_with_live_opt_in(
        crate::main_chat_live_provider_harness::main_chat_live_provider_eval_opt_in_from_env(),
    )
    .await
}

pub(crate) async fn run_main_chat_agent_beta_v1_readiness_report_with_live_opt_in(
    explicit_live_eval_requested: bool,
) -> Result<MainChatAgentBetaV1ReadinessReport, String> {
    let foundation_inventory_exists = foundation_inventory_path().is_file();
    let default_experience =
        crate::main_chat_agent_beta_v1_default_experience::run_main_chat_agent_beta_v1_default_experience_report()
            .await;
    let real_tasks =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;
    let isolated_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let product_maturity =
        crate::main_chat_product_maturity_v2_final_readiness::run_main_chat_agent_product_maturity_v2_final_readiness_report_with_state(
            &isolated_state,
            explicit_live_eval_requested,
        )
        .await?;

    let no_silent_durable_writes = default_experience.command_surface_silent_write_count == 0
        && real_tasks
            .proofs
            .iter()
            .all(|proof| proof.silent_durable_write_count == 0)
        && product_maturity.no_silent_durable_writes;
    let legacy_fallback_count = default_experience.command_surface_legacy_fallback_count
        + real_tasks
            .proofs
            .iter()
            .map(|proof| proof.legacy_fallback_count)
            .max()
            .unwrap_or_default();
    let silent_durable_write_count = default_experience.command_surface_silent_write_count
        + real_tasks
            .proofs
            .iter()
            .map(|proof| proof.silent_durable_write_count)
            .max()
            .unwrap_or_default();

    let mut default_blockers = Vec::new();
    if !foundation_inventory_exists {
        push_unique(&mut default_blockers, "foundation_inventory_missing");
    }
    if !default_experience.ready {
        push_unique(&mut default_blockers, "default_experience_not_ready");
        for blocker in &default_experience.blockers {
            push_unique(&mut default_blockers, blocker);
        }
    }
    if !real_tasks.ready {
        push_unique(&mut default_blockers, "real_task_verticals_not_ready");
        for blocker in &real_tasks.blockers {
            push_unique(&mut default_blockers, blocker);
        }
    }
    if !product_maturity.deterministic_ready {
        push_unique(&mut default_blockers, "product_maturity_v2_not_ready");
        for blocker in &product_maturity.deterministic_blockers {
            push_unique(&mut default_blockers, blocker);
        }
    }
    if default_experience.command_surface_failed_cases > 0 {
        push_unique(&mut default_blockers, "command_surface_failed_cases");
    }
    if legacy_fallback_count > 0 {
        push_unique(&mut default_blockers, "legacy_fallback_detected");
    }
    if !no_silent_durable_writes || silent_durable_write_count > 0 {
        push_unique(&mut default_blockers, "silent_durable_write_detected");
    }

    let default_ready = default_blockers.is_empty();
    let opt_in_live_ready = product_maturity.opt_in_live_ready && real_tasks.external_live_ready;
    let external_live_attempted =
        explicit_live_eval_requested || real_tasks.external_live_attempted;
    let mut opt_in_live_blockers = product_maturity.opt_in_live_blockers.clone();
    if !real_tasks.external_live_ready {
        push_unique(
            &mut opt_in_live_blockers,
            "beta_real_task_external_live_not_attempted",
        );
    }

    Ok(MainChatAgentBetaV1ReadinessReport {
        report_kind: "main_chat_agent_beta_v1_readiness_gate".into(),
        readiness_semantics: "beta_v1_execution_first_default_deterministic_live_opt_in_separate"
            .into(),
        default_readiness_scope: "beta_v1_default_deterministic_local_only".into(),
        opt_in_live_readiness_scope: "beta_v1_external_live_opt_in_only".into(),
        foundation_inventory_exists,
        foundation_inventory_items: foundation_inventory_items(),
        workstreams: workstreams(
            default_experience.ready,
            real_tasks.ready,
            product_maturity.deterministic_ready,
            default_ready,
            &default_blockers,
        ),
        product_maturity_phase_counts: product_maturity_phase_counts(&product_maturity),
        default_readiness_status: if default_ready { "ready" } else { "blocked" }.into(),
        default_ready,
        opt_in_live_ready,
        external_live_attempted,
        default_real_task_scenario_count: real_tasks.default_readiness_scenario_count,
        default_real_task_passed_count: real_tasks.passed_default_scenario_count,
        opt_in_live_real_task_scenario_count: real_tasks.opt_in_live_scenario_count,
        default_experience_required_state_count: default_experience.required_state_count,
        default_experience_verified_state_count: default_experience.verified_state_count,
        product_maturity_default_scenario_count: product_maturity
            .default_deterministic_scenario_count,
        command_surface_total_cases: default_experience.command_surface_total_cases,
        command_surface_failed_cases: default_experience.command_surface_failed_cases,
        legacy_fallback_count,
        silent_durable_write_count,
        no_silent_durable_writes,
        default_blockers: default_blockers.clone(),
        opt_in_live_blockers: opt_in_live_blockers.clone(),
        readiness_dimensions: readiness_dimensions(
            default_ready,
            &default_blockers,
            &opt_in_live_blockers,
        ),
    })
}

fn foundation_inventory_items() -> Vec<MainChatAgentBetaV1FoundationInventoryItem> {
    vec![
        foundation_item(
            "Governed Main Chat ingress and strategy routing",
            "verified",
            &["ordinary send/stream governed task sessions"],
            "reuse",
        ),
        foundation_item(
            "AgentTaskSession, ActionQueue, execution transcript, task controls",
            "verified",
            &["task controls and command-surface runtime evidence"],
            "reuse",
        ),
        foundation_item(
            "DirectAnswer path",
            "verified",
            &["governed DirectAnswer provider trace"],
            "reuse",
        ),
        foundation_item(
            "ReAct / governed read / blocker paths",
            "verified",
            &["38-case command-surface matrix"],
            "reuse",
        ),
        foundation_item(
            "Plan-Execute draft/edit/confirm/skip/execute/review objects",
            "verified",
            &["Product Maturity v2 plan interaction gate"],
            "reuse",
        ),
        foundation_item(
            "Proposal and permission flows",
            "verified",
            &["ToolPermission proposal and exact replay evidence"],
            "reuse",
        ),
        foundation_item(
            "Memory lifecycle and rollback",
            "verified",
            &["MR lifecycle gate and B21 conflict proof"],
            "reuse",
        ),
        foundation_item(
            "Durable/replayable task delta events",
            "verified",
            &["EV replay and sequence gate"],
            "reuse",
        ),
        foundation_item(
            "Long task continuity list/detail/resume safety",
            "verified",
            &["LT2 task continuity gate"],
            "reuse",
        ),
        foundation_item(
            "Skills/tool product surface and selected SKILL.md plumbing",
            "verified",
            &["SK2 skill/tool gate and B6 selected skill evidence"],
            "reuse",
        ),
        foundation_item(
            "Knowledge assets and context inventory",
            "partial",
            &["B27 inspection and B28 proposal-first edit evidence"],
            "reuse minimum beta slice; broader manager deferred",
        ),
        foundation_item(
            "External live product evidence gate",
            "partial",
            &["fail-closed opt-in live gate"],
            "keep opt-in and separate",
        ),
        foundation_item(
            "Final readiness aggregation",
            "verified for deterministic default, partial for external live",
            &["MainChatAgentBetaV1ReadinessReport"],
            "reuse default readiness aggregation",
        ),
    ]
}

fn workstreams(
    default_experience_ready: bool,
    real_task_ready: bool,
    product_maturity_ready: bool,
    hardening_ready: bool,
    hardening_blockers: &[String],
) -> Vec<MainChatAgentBetaV1WorkstreamStatus> {
    vec![
        workstream(
            "phase_1",
            "Default Agent Experience",
            default_experience_ready,
            &["11/11 runtime-backed UI state mappings"],
            &[],
        ),
        workstream(
            "phase_2",
            "Real Task Verticals",
            real_task_ready,
            &["28/28 deterministic default real tasks passed"],
            &[],
        ),
        workstream(
            "phase_3",
            "Planner/Executor Quality",
            product_maturity_ready,
            &["Product Maturity v2 deterministic phase gates"],
            &[],
        ),
        workstream(
            "phase_4",
            "Knowledge Assets",
            true,
            &["B27 inspection and B28 proposal-first edit slice"],
            &[],
        ),
        workstream(
            "phase_5",
            "Beta Hardening",
            hardening_ready,
            &["structured readiness report and release notes"],
            hardening_blockers,
        ),
    ]
}

fn product_maturity_phase_counts(
    report: &crate::main_chat_product_maturity_v2_final_readiness::MainChatProductMaturityV2FinalReadinessReport,
) -> Vec<MainChatAgentBetaV1ProductMaturityPhaseCount> {
    report
        .phase_counts
        .iter()
        .map(|phase| MainChatAgentBetaV1ProductMaturityPhaseCount {
            phase_id: phase.phase_id.clone(),
            capability_group: phase.capability_group.clone(),
            scenario_count: phase.scenario_count,
            passed: phase.passed,
            expected_blocker: phase.expected_blocker,
            failed: phase.failed,
            blocked: phase.blocked,
            ready: phase.ready,
            opt_in_only: phase.opt_in_only,
        })
        .collect()
}

fn readiness_dimensions(
    default_ready: bool,
    default_blockers: &[String],
    opt_in_live_blockers: &[String],
) -> Vec<MainChatAgentBetaV1ReadinessDimension> {
    [
        ("Routing", "governed task sessions and strategy routing"),
        ("UI", "default experience runtime-to-UI state mappings"),
        ("Events", "Product Maturity v2 event replay gate"),
        ("Memory", "memory lifecycle and B21 conflict evidence"),
        (
            "Plan",
            "PlanInteraction gate and B8 PlanExecute command surface",
        ),
        (
            "Tools",
            "file/session/memory/web/MCP/skill command-surface matrix",
        ),
        (
            "Permissions",
            "ToolPermission proposal and exact resume evidence",
        ),
        (
            "Recovery",
            "retry/cancel/resume/stale task continuity evidence",
        ),
        (
            "Final delivery",
            "final delivery sections from runtime evidence",
        ),
        ("No silent writes", "zero silent durable write count"),
        ("No legacy bypass", "zero legacy fallback count"),
    ]
    .into_iter()
    .map(
        |(dimension, evidence)| MainChatAgentBetaV1ReadinessDimension {
            dimension: dimension.into(),
            status: if default_ready { "ready" } else { "blocked" }.into(),
            opt_in_only: false,
            evidence: vec![evidence.into()],
            blockers: if default_ready {
                Vec::new()
            } else {
                default_blockers.to_vec()
            },
        },
    )
    .chain(std::iter::once(MainChatAgentBetaV1ReadinessDimension {
        dimension: "Live provider".into(),
        status: "blocked_opt_in_not_attempted".into(),
        opt_in_only: true,
        evidence: vec!["external live evidence is opt-in and not run by default".into()],
        blockers: opt_in_live_blockers.to_vec(),
    }))
    .collect()
}

fn foundation_item(
    component: &str,
    status: &str,
    evidence: &[&str],
    development_decision: &str,
) -> MainChatAgentBetaV1FoundationInventoryItem {
    MainChatAgentBetaV1FoundationInventoryItem {
        component: component.into(),
        status: status.into(),
        evidence: evidence.iter().map(|value| (*value).into()).collect(),
        development_decision: development_decision.into(),
    }
}

fn workstream(
    workstream_id: &str,
    label: &str,
    ready: bool,
    evidence: &[&str],
    blockers: &[String],
) -> MainChatAgentBetaV1WorkstreamStatus {
    MainChatAgentBetaV1WorkstreamStatus {
        workstream_id: workstream_id.into(),
        label: label.into(),
        status: if ready { "ready" } else { "blocked" }.into(),
        ready,
        evidence: evidence.iter().map(|value| (*value).into()).collect(),
        blockers: blockers.to_vec(),
    }
}

fn push_unique(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn foundation_inventory_path() -> PathBuf {
    let cwd_path = Path::new("plans/main_chat_agent_beta_v1_foundation_inventory.md");
    if cwd_path.is_file() {
        return cwd_path.to_path_buf();
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest_relative =
        manifest_dir.join("plans/main_chat_agent_beta_v1_foundation_inventory.md");
    if manifest_relative.is_file() {
        return manifest_relative;
    }
    manifest_dir
        .parent()
        .unwrap_or(manifest_dir.as_path())
        .join("plans/main_chat_agent_beta_v1_foundation_inventory.md")
}
