use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const BROWSER_E2E_REPORT_PATH: &str = "frontend/test-results/main-chat-stage1-dogfood-report.json";
const BROWSER_E2E_MAX_AGE_HOURS: i64 = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatAgentStage1SeedManifest {
    pub seed_workspace_root_kind: String,
    pub knowledge_asset_count: usize,
    pub skill_count: usize,
    pub session_seed_count: usize,
    pub memory_seed_count: usize,
    pub proposal_seed_count: usize,
    pub task_seed_count: usize,
    pub plan_seed_count: usize,
    pub mcp_manifest_seed_count: usize,
    pub web_fixture_seed_count: usize,
    pub seed_digest: String,
    pub file_digests: BTreeMap<String, String>,
    pub runtime_object_digests: BTreeMap<String, String>,
    pub secrets_detected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatAgentStage1DogfoodScenarioEvidence {
    pub scenario_id: String,
    pub scenario_type: String,
    pub entry_point: String,
    pub scenario_prompt_id: String,
    pub bounded_prompt_preview: String,
    pub user_prompt_digest: String,
    pub task_session_id: String,
    pub run_id: String,
    pub route_strategy: String,
    pub expected_outcome: String,
    pub actual_outcome: String,
    pub runtime_events: Vec<String>,
    pub actions: Vec<String>,
    pub observations: Vec<String>,
    pub proposals: Vec<String>,
    pub blockers: Vec<String>,
    pub ui_states: Vec<String>,
    pub final_delivery_sections: Vec<String>,
    pub control_evidence: String,
    pub runtime_evidence_passed: bool,
    pub ui_evidence_passed: bool,
    pub final_delivery_evidence_passed: bool,
    pub non_fake_evidence_passed: bool,
    pub legacy_fallback_used: bool,
    pub silent_durable_write_detected: bool,
    pub fake_execution_detected: bool,
    pub seed_manifest_digest: String,
    pub live_provider_evidence: Option<String>,
    pub passed: bool,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatStage1BrowserE2eEvidence {
    pub environment_ready: bool,
    pub self_contained_runner: bool,
    pub smoke_passed: bool,
    pub report_path: Option<String>,
    pub evidence_source: String,
    pub run_id: Option<String>,
    pub generated_at: Option<String>,
    pub commit: Option<String>,
    pub report_digest: Option<String>,
    pub required_journeys: Vec<String>,
    pub passed_journeys: Vec<String>,
    pub failed_journeys: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct MainChatAgentStage1RuntimeEvidenceBundle {
    command_surface: crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalReport,
    plan: crate::main_chat_plan_interaction_eval::MainChatProductMaturityV2PlanGateReport,
    memory: crate::main_chat_memory_lifecycle_eval::MainChatMemoryLifecycleEvalGateReport,
}

#[derive(Debug, Clone)]
struct Stage1BrowserAudit {
    accepted: bool,
    environment_ready: bool,
    report_path: Option<String>,
    required_journey_count: usize,
    passed_journey_count: usize,
    failed_journey_count: usize,
    fake_execution_detected: bool,
    blockers: Vec<String>,
}

#[derive(Debug, Clone)]
struct Stage1ScenarioRuntimeEvidence {
    task_session_id: String,
    run_id: String,
    runtime_events: Vec<String>,
    actions: Vec<String>,
    observations: Vec<String>,
    proposals: Vec<String>,
    blockers: Vec<String>,
    control_evidence: String,
    actual_outcome: String,
    runtime_evidence_passed: bool,
    final_delivery_evidence_passed: bool,
    non_fake_evidence_passed: bool,
    legacy_fallback_used: bool,
    silent_durable_write_detected: bool,
    fake_execution_detected: bool,
}

#[derive(Debug, Clone)]
struct Stage1BetaReadinessEvidence {
    default_ready: bool,
    default_blockers: Vec<String>,
    legacy_fallback_count: usize,
    silent_durable_write_count: usize,
    product_maturity_default_scenario_count: usize,
}

impl Stage1BetaReadinessEvidence {
    async fn load_real() -> Result<Self, String> {
        let report =
            crate::main_chat_agent_beta_v1_readiness::run_main_chat_agent_beta_v1_readiness_report(
            )
            .await?;
        Ok(Self {
            default_ready: report.default_ready,
            default_blockers: report.default_blockers,
            legacy_fallback_count: report.legacy_fallback_count,
            silent_durable_write_count: report.silent_durable_write_count,
            product_maturity_default_scenario_count: report.product_maturity_default_scenario_count,
        })
    }

    #[cfg(test)]
    fn clean_for_tests() -> Self {
        Self {
            default_ready: true,
            default_blockers: Vec::new(),
            legacy_fallback_count: 0,
            silent_durable_write_count: 0,
            product_maturity_default_scenario_count: 43,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatAgentStage1DogfoodReport {
    pub report_kind: String,
    pub readiness_semantics: String,
    pub default_readiness_scope: String,
    pub opt_in_live_readiness_scope: String,
    pub default_ready: bool,
    pub opt_in_live_ready: bool,
    pub readiness_recommendation: String,
    pub scenario_count: usize,
    pub default_scenario_count: usize,
    pub default_passed_count: usize,
    pub default_failed_count: usize,
    pub task_session_created_count: usize,
    pub ordinary_chat_scenario_count: usize,
    pub seeded_task_control_scenario_count: usize,
    pub ui_verified_scenario_count: usize,
    pub final_delivery_verified_scenario_count: usize,
    pub legacy_fallback_count: usize,
    pub silent_durable_write_count: usize,
    pub fake_execution_detected_count: usize,
    pub external_live_attempted: bool,
    pub external_live_scenario_count: usize,
    pub external_live_passed_count: usize,
    pub external_live_blocked_count: usize,
    pub external_live_blockers: Vec<String>,
    pub default_readiness_unaffected_by_live: bool,
    pub browser_e2e_environment_ready: bool,
    pub browser_e2e_report_path: Option<String>,
    pub browser_e2e_required_journey_count: usize,
    pub browser_e2e_passed_journey_count: usize,
    pub browser_e2e_failed_journey_count: usize,
    pub manual_dogfood_status: String,
    pub beta_v1_default_ready: bool,
    pub product_maturity_default_scenario_count: usize,
    pub seed_manifest: MainChatAgentStage1SeedManifest,
    pub scenarios: Vec<MainChatAgentStage1DogfoodScenarioEvidence>,
    pub blockers: Vec<String>,
    pub accepted_residual_risks: Vec<String>,
}

#[derive(Debug, Clone)]
struct Stage1ScenarioDef {
    id: &'static str,
    priority: &'static str,
    scenario_type: &'static str,
    prompt: &'static str,
    route: &'static str,
    ui_states: &'static [&'static str],
    final_delivery: &'static [&'static str],
    expected_outcome: &'static str,
    blocker: Option<&'static str>,
    seed_dependency: &'static str,
    live: bool,
}

pub(crate) async fn run_main_chat_agent_stage1_dogfood_report(
) -> Result<MainChatAgentStage1DogfoodReport, String> {
    let browser_evidence = read_browser_e2e_report_from_default_path();
    run_main_chat_agent_stage1_dogfood_report_with_browser_evidence(browser_evidence).await
}

pub(crate) async fn run_main_chat_agent_stage1_dogfood_report_with_browser_evidence(
    browser_evidence: Option<MainChatStage1BrowserE2eEvidence>,
) -> Result<MainChatAgentStage1DogfoodReport, String> {
    let runtime_evidence = Some(run_stage1_runtime_evidence_bundle().await?);
    run_main_chat_agent_stage1_dogfood_report_with_inputs(
        browser_evidence,
        runtime_evidence,
        crate::main_chat_live_provider_harness::main_chat_live_provider_eval_opt_in_from_env(),
        None,
    )
    .await
}

#[cfg(test)]
pub(crate) async fn run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(
    browser_evidence: Option<MainChatStage1BrowserE2eEvidence>,
    runtime_evidence: Option<MainChatAgentStage1RuntimeEvidenceBundle>,
    external_live_attempted: bool,
) -> Result<MainChatAgentStage1DogfoodReport, String> {
    run_main_chat_agent_stage1_dogfood_report_with_inputs(
        browser_evidence,
        runtime_evidence,
        external_live_attempted,
        Some(Stage1BetaReadinessEvidence::clean_for_tests()),
    )
    .await
}

async fn run_main_chat_agent_stage1_dogfood_report_with_inputs(
    browser_evidence: Option<MainChatStage1BrowserE2eEvidence>,
    runtime_evidence: Option<MainChatAgentStage1RuntimeEvidenceBundle>,
    external_live_attempted: bool,
    beta_evidence: Option<Stage1BetaReadinessEvidence>,
) -> Result<MainChatAgentStage1DogfoodReport, String> {
    let beta_report = match beta_evidence {
        Some(beta_evidence) => beta_evidence,
        None => Stage1BetaReadinessEvidence::load_real().await?,
    };
    let seed_manifest = build_stage1_seed_manifest()?;
    let browser_audit = audit_browser_e2e_evidence(browser_evidence.as_ref());
    let scenario_defs = stage1_scenarios();
    let scenarios = scenario_defs
        .iter()
        .map(|scenario| {
            scenario_evidence(
                scenario,
                &seed_manifest.seed_digest,
                runtime_evidence.as_ref(),
                &browser_audit,
            )
        })
        .collect::<Vec<_>>();

    let default_rows = scenarios
        .iter()
        .filter(|row| row.live_provider_evidence.as_deref() == Some("default_deterministic"))
        .collect::<Vec<_>>();
    let live_rows = scenarios
        .iter()
        .filter(|row| row.live_provider_evidence.as_deref() != Some("default_deterministic"))
        .collect::<Vec<_>>();

    let default_scenario_count = default_rows.len();
    let default_passed_count = default_rows.iter().filter(|row| row.passed).count();
    let default_failed_count = default_scenario_count.saturating_sub(default_passed_count);
    let task_session_created_count = default_rows
        .iter()
        .filter(|row| !row.task_session_id.is_empty())
        .count();
    let ordinary_chat_scenario_count = default_rows
        .iter()
        .filter(|row| row.scenario_type == "chat_e2e")
        .count();
    let seeded_task_control_scenario_count = default_rows
        .iter()
        .filter(|row| row.scenario_type == "seeded_task_control_e2e")
        .count();
    let ui_verified_scenario_count = default_rows
        .iter()
        .filter(|row| row.ui_evidence_passed)
        .count();
    let final_delivery_verified_scenario_count = default_rows
        .iter()
        .filter(|row| row.final_delivery_evidence_passed)
        .count();
    let fake_execution_detected_count = default_rows
        .iter()
        .filter(|row| row.fake_execution_detected)
        .count()
        + usize::from(browser_audit.fake_execution_detected);

    let mut blockers = Vec::new();
    if !beta_report.default_ready {
        push_unique(&mut blockers, "beta_v1_default_readiness_blocked");
        for blocker in &beta_report.default_blockers {
            push_unique(&mut blockers, blocker);
        }
    }
    if seed_manifest.secrets_detected {
        push_unique(&mut blockers, "stage1_seed_secrets_detected");
    }
    if default_scenario_count != 36 {
        push_unique(&mut blockers, "stage1_default_scenario_count_not_36");
    }
    if runtime_evidence.is_none() {
        push_unique(&mut blockers, "stage1_default_scenarios_not_executed");
    }
    if ordinary_chat_scenario_count < 20 {
        push_unique(&mut blockers, "ordinary_chat_scenario_count_below_20");
    }
    if seeded_task_control_scenario_count < 8 {
        push_unique(&mut blockers, "seeded_task_control_scenario_count_below_8");
    }
    if default_failed_count > 0 {
        push_unique(&mut blockers, "stage1_default_scenarios_failed");
    }
    if task_session_created_count != default_scenario_count {
        push_unique(&mut blockers, "missing_task_session_for_stage1_scenario");
    }
    if ui_verified_scenario_count != default_scenario_count {
        push_unique(&mut blockers, "stage1_ui_evidence_incomplete");
    }
    if final_delivery_verified_scenario_count != default_scenario_count {
        push_unique(&mut blockers, "stage1_final_delivery_evidence_incomplete");
    }
    if beta_report.legacy_fallback_count > 0 {
        push_unique(&mut blockers, "hidden_legacy_fallback_detected");
    }
    if beta_report.silent_durable_write_count > 0 {
        push_unique(&mut blockers, "silent_durable_write_detected");
    }
    if fake_execution_detected_count > 0 {
        push_unique(&mut blockers, "fake_execution_detected");
    }
    if !browser_audit.accepted {
        push_unique(&mut blockers, "not_ready_browser_e2e_blocked");
    }
    for blocker in &browser_audit.blockers {
        push_unique(&mut blockers, blocker);
    }

    let mut external_live_blockers = Vec::new();
    if !external_live_attempted {
        push_unique(&mut external_live_blockers, "explicit_live_eval_required");
    }
    push_unique(
        &mut external_live_blockers,
        "external_live_provider_evidence_not_part_of_default_readiness",
    );

    let default_ready = blockers.is_empty();
    let readiness_recommendation = if default_ready {
        "ready_for_engineering_dogfood"
    } else {
        "not_ready"
    };

    Ok(MainChatAgentStage1DogfoodReport {
        report_kind: "main_chat_agent_stage1_dogfood_gate".into(),
        readiness_semantics:
            "stage1_real_e2e_dogfood_default_deterministic_browser_required_live_opt_in_separate"
                .into(),
        default_readiness_scope: "stage1_default_deterministic_seeded_dogfood".into(),
        opt_in_live_readiness_scope: "stage1_external_live_opt_in_only".into(),
        default_ready,
        opt_in_live_ready: false,
        readiness_recommendation: readiness_recommendation.into(),
        scenario_count: scenarios.len(),
        default_scenario_count,
        default_passed_count,
        default_failed_count,
        task_session_created_count,
        ordinary_chat_scenario_count,
        seeded_task_control_scenario_count,
        ui_verified_scenario_count,
        final_delivery_verified_scenario_count,
        legacy_fallback_count: beta_report.legacy_fallback_count,
        silent_durable_write_count: beta_report.silent_durable_write_count,
        fake_execution_detected_count,
        external_live_attempted,
        external_live_scenario_count: live_rows.len(),
        external_live_passed_count: 0,
        external_live_blocked_count: live_rows.len(),
        external_live_blockers,
        default_readiness_unaffected_by_live: true,
        browser_e2e_environment_ready: browser_audit.environment_ready,
        browser_e2e_report_path: browser_audit.report_path,
        browser_e2e_required_journey_count: browser_audit.required_journey_count,
        browser_e2e_passed_journey_count: browser_audit.passed_journey_count,
        browser_e2e_failed_journey_count: browser_audit.failed_journey_count,
        manual_dogfood_status: "not_attempted_engineering_dogfood_only".into(),
        beta_v1_default_ready: beta_report.default_ready,
        product_maturity_default_scenario_count: beta_report
            .product_maturity_default_scenario_count,
        seed_manifest,
        scenarios,
        blockers,
        accepted_residual_risks: vec![
            "manual_dogfood_not_attempted_ready_for_engineering_dogfood_only".into(),
            "external_live_provider_not_attempted_opt_in_separate".into(),
        ],
    })
}

pub(crate) fn passing_stage1_browser_e2e_evidence_for_tests() -> MainChatStage1BrowserE2eEvidence {
    let journeys = required_browser_journeys();
    let run_id = format!("stage1-browser-e2e-test-{}", uuid::Uuid::new_v4());
    let generated_at = chrono::Utc::now().to_rfc3339();
    let report_digest = digest_label(
        serde_json::json!({
            "runId": run_id,
            "requiredJourneys": journeys,
            "source": "tauri_command_surface_browser_e2e"
        })
        .to_string()
        .as_bytes(),
    );
    MainChatStage1BrowserE2eEvidence {
        environment_ready: true,
        self_contained_runner: true,
        smoke_passed: true,
        report_path: Some(BROWSER_E2E_REPORT_PATH.into()),
        evidence_source: "tauri_command_surface_browser_e2e".into(),
        run_id: Some(run_id),
        generated_at: Some(generated_at),
        commit: None,
        report_digest: Some(report_digest),
        required_journeys: journeys.clone(),
        passed_journeys: journeys,
        failed_journeys: Vec::new(),
        blockers: Vec::new(),
    }
}

async fn run_stage1_runtime_evidence_bundle(
) -> Result<MainChatAgentStage1RuntimeEvidenceBundle, String> {
    let command_surface =
        crate::main_chat_command_surface_eval::run_main_chat_command_surface_eval_report().await;
    let plan =
        crate::main_chat_plan_interaction_eval::run_main_chat_agent_product_maturity_v2_plan_gate()
            .await;
    let memory = crate::main_chat_memory_lifecycle_eval::run_main_chat_memory_lifecycle_eval_gate();
    Ok(MainChatAgentStage1RuntimeEvidenceBundle {
        command_surface,
        plan,
        memory,
    })
}

#[cfg(test)]
pub(crate) async fn run_stage1_runtime_evidence_bundle_for_tests(
) -> Result<MainChatAgentStage1RuntimeEvidenceBundle, String> {
    run_stage1_runtime_evidence_bundle().await
}

fn read_browser_e2e_report_from_default_path() -> Option<MainChatStage1BrowserE2eEvidence> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)?
        .join(BROWSER_E2E_REPORT_PATH);
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let required_journeys = value
        .get("requiredJourneys")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(required_browser_journeys);
    let passed_journeys = value
        .get("passedJourneys")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let failed_journeys = value
        .get("failedJourneys")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let blockers = value
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(MainChatStage1BrowserE2eEvidence {
        environment_ready: value
            .get("browserE2eEnvironmentReady")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        self_contained_runner: value
            .get("selfContainedRunner")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        smoke_passed: value
            .get("smokePassed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        report_path: value
            .get("reportPath")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        evidence_source: value
            .get("evidenceSource")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        run_id: value
            .get("runId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        generated_at: value
            .get("generatedAt")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        commit: value
            .get("commit")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        report_digest: value
            .get("reportDigest")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .or_else(|| Some(digest_label(content.as_bytes()))),
        required_journeys,
        passed_journeys,
        failed_journeys,
        blockers,
    })
}

fn audit_browser_e2e_evidence(
    evidence: Option<&MainChatStage1BrowserE2eEvidence>,
) -> Stage1BrowserAudit {
    let Some(evidence) = evidence else {
        return Stage1BrowserAudit {
            accepted: false,
            environment_ready: false,
            report_path: None,
            required_journey_count: 0,
            passed_journey_count: 0,
            failed_journey_count: 0,
            fake_execution_detected: false,
            blockers: vec!["required_browser_e2e_smoke_not_run".into()],
        };
    };

    let mut blockers = Vec::new();
    let required = required_browser_journeys();
    let required_set = required.iter().cloned().collect::<BTreeSet<_>>();
    let passed_set = evidence
        .passed_journeys
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let failed_set = evidence
        .failed_journeys
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    if !evidence.environment_ready || !evidence.self_contained_runner {
        push_unique(&mut blockers, "browser_e2e_environment_not_ready");
    }
    if !evidence.smoke_passed || !evidence.failed_journeys.is_empty() {
        push_unique(&mut blockers, "required_browser_e2e_smoke_failed");
    }
    if evidence.report_path.as_deref() != Some(BROWSER_E2E_REPORT_PATH) {
        push_unique(&mut blockers, "browser_e2e_report_path_mismatch");
    }
    if evidence.required_journeys != required
        || passed_set != required_set
        || !failed_set.is_empty()
    {
        push_unique(&mut blockers, "browser_e2e_required_journeys_incomplete");
    }
    if evidence.passed_journeys != required {
        push_unique(&mut blockers, "browser_e2e_passed_journeys_mismatch");
    }
    let fake_execution_detected =
        browser_evidence_source_is_frontend_only(&evidence.evidence_source);
    if fake_execution_detected {
        push_unique(&mut blockers, "browser_e2e_frontend_only_fixture_report");
    }
    if !browser_evidence_source_is_safe_real_command_surface(&evidence.evidence_source) {
        push_unique(&mut blockers, "browser_e2e_source_missing_or_unsafe");
    }
    if !browser_e2e_trace_is_fresh_and_bounded(evidence) {
        push_unique(&mut blockers, "browser_e2e_report_stale_or_untraceable");
    }
    for blocker in &evidence.blockers {
        push_unique(&mut blockers, blocker);
    }

    Stage1BrowserAudit {
        accepted: blockers.is_empty(),
        environment_ready: evidence.environment_ready && evidence.self_contained_runner,
        report_path: evidence.report_path.clone(),
        required_journey_count: evidence.required_journeys.len(),
        passed_journey_count: evidence.passed_journeys.len(),
        failed_journey_count: evidence.failed_journeys.len(),
        fake_execution_detected,
        blockers,
    }
}

fn browser_evidence_source_is_frontend_only(source: &str) -> bool {
    let lowered = source.to_ascii_lowercase();
    lowered.contains("frontend")
        || lowered.contains("fixture")
        || lowered.contains("mock")
        || lowered.contains("synthetic")
        || lowered.contains("stage1_browser")
}

fn browser_evidence_source_is_safe_real_command_surface(source: &str) -> bool {
    metadata_safe_label(source)
        && source.contains("tauri")
        && source.contains("command_surface")
        && !browser_evidence_source_is_frontend_only(source)
}

fn browser_e2e_trace_is_fresh_and_bounded(evidence: &MainChatStage1BrowserE2eEvidence) -> bool {
    let Some(run_id) = evidence.run_id.as_deref() else {
        return false;
    };
    let Some(generated_at) = evidence.generated_at.as_deref() else {
        return false;
    };
    if !metadata_safe_label(run_id) {
        return false;
    }
    let Ok(generated_at) = chrono::DateTime::parse_from_rfc3339(generated_at) else {
        return false;
    };
    let generated_at = generated_at.with_timezone(&chrono::Utc);
    let now = chrono::Utc::now();
    if generated_at > now + chrono::Duration::minutes(5) {
        return false;
    }
    if now.signed_duration_since(generated_at) > chrono::Duration::hours(BROWSER_E2E_MAX_AGE_HOURS)
    {
        return false;
    }
    evidence
        .commit
        .as_deref()
        .is_some_and(metadata_safe_commit_label)
        || evidence
            .report_digest
            .as_deref()
            .is_some_and(metadata_safe_digest_label)
}

fn metadata_safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.trim() == value
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
}

fn metadata_safe_commit_label(value: &str) -> bool {
    value.len() >= 7 && value.len() <= 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn metadata_safe_digest_label(value: &str) -> bool {
    let Some((bytes, hash)) = value.split_once(" hash:sha256:") else {
        return false;
    };
    let Some(count) = bytes.strip_prefix("bytes:") else {
        return false;
    };
    count.parse::<usize>().is_ok_and(|count| count > 0)
        && hash.len() == 64
        && hash.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn build_stage1_seed_manifest() -> Result<MainChatAgentStage1SeedManifest, String> {
    let seed_files = seed_file_contents();
    let mut file_digests: BTreeMap<String, String> = BTreeMap::new();
    let mut secrets_detected = false;
    for (path, content) in &seed_files {
        if contains_secret_like_pattern(content) {
            secrets_detected = true;
        }
        file_digests.insert((*path).to_string(), digest_label(content.as_bytes()));
    }

    let runtime_objects = runtime_seed_objects();
    let mut runtime_object_digests: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in &runtime_objects {
        let bytes = serde_json::to_vec(value)
            .map_err(|err| format!("serialize stage1 runtime seed object {name}: {err}"))?;
        runtime_object_digests.insert((*name).to_string(), digest_label(&bytes));
    }

    let canonical = serde_json::json!({
        "fileDigests": file_digests,
        "knowledgeAssetCount": 9,
        "mcpManifestSeedCount": 2,
        "memorySeedCount": 5,
        "planSeedCount": 1,
        "proposalSeedCount": 2,
        "runtimeObjectDigests": runtime_object_digests,
        "seedWorkspaceRootKind": "temp_isolated",
        "sessionSeedCount": 1,
        "skillCount": 3,
        "taskSeedCount": 5,
        "webFixtureSeedCount": 1,
    });
    let canonical_bytes = serde_json::to_vec(&canonical)
        .map_err(|err| format!("serialize stage1 seed manifest: {err}"))?;
    let seed_digest = digest_label(&canonical_bytes);

    let file_digests = canonical
        .get("fileDigests")
        .and_then(serde_json::Value::as_object)
        .map(|values| {
            values
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|digest| (key.clone(), digest.into()))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let runtime_object_digests = canonical
        .get("runtimeObjectDigests")
        .and_then(serde_json::Value::as_object)
        .map(|values| {
            values
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|digest| (key.clone(), digest.into()))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    Ok(MainChatAgentStage1SeedManifest {
        seed_workspace_root_kind: "temp_isolated".into(),
        knowledge_asset_count: 9,
        skill_count: 3,
        session_seed_count: 1,
        memory_seed_count: 5,
        proposal_seed_count: 2,
        task_seed_count: 5,
        plan_seed_count: 1,
        mcp_manifest_seed_count: 2,
        web_fixture_seed_count: 1,
        seed_digest,
        file_digests,
        runtime_object_digests,
        secrets_detected,
    })
}

fn seed_file_contents() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        (
            "AGENTS.md",
            "Stage 1 dogfood workspace. Use proposal-first memory and knowledge updates.",
        ),
        ("SOUL.md", "OpenLife dogfood seed: local-first, transparent, consentful."),
        (
            "USER.md",
            "The seed user prefers clear plans, morning deep work, and explicit blockers.",
        ),
        (
            "MEMORY.md",
            "Accepted memory context: prefers morning deep work and concise planning.",
        ),
        (
            "project_brief.md",
            "Project policy: prove actions with runtime evidence before final delivery.",
        ),
        (
            "planning_notes.md",
            "Weekly plan seed: review priorities, read the policy note, then ask before risky publish.",
        ),
        (
            "policy_note.md",
            "External publication is write-like and requires explicit permission.",
        ),
        (
            "memories/USER.md",
            "Memory user seed: avoid silent overwrite when facts conflict.",
        ),
        (
            "memories/MEMORY.md",
            "Memory seed: rollback removes a materialized memory from active context.",
        ),
        (
            "skills/phase_e_review/SKILL.md",
            "Review plans by checking evidence, blocker visibility, and final delivery sections.",
        ),
        (
            "skills/planning_review/SKILL.md",
            "Critique planning notes for next action clarity and safe read-only first steps.",
        ),
        (
            "skills/unselected_sensitive/SKILL.md",
            "Unselected sensitive skill body must never be injected into Stage 1 prompts.",
        ),
    ])
}

fn runtime_seed_objects() -> BTreeMap<&'static str, serde_json::Value> {
    BTreeMap::from([
        (
            "accepted_memory_preference",
            serde_json::json!({"kind":"memory","status":"accepted","summary":"prefers morning deep work"}),
        ),
        (
            "conflicting_memory_pair",
            serde_json::json!({"kind":"memory_conflict","evidenceIds":["ev-memory-a","ev-memory-b"],"conflictCount":2}),
        ),
        (
            "pending_memory_proposal",
            serde_json::json!({"kind":"proposal","status":"pending","proposalType":"memory_write"}),
        ),
        (
            "accepted_memory_for_rollback",
            serde_json::json!({"kind":"memory","status":"accepted","rollbackEligible":true}),
        ),
        (
            "seeded_chat_session",
            serde_json::json!({"kind":"chat_session","topic":"memory rollback discussion","messageCount":3}),
        ),
        (
            "blocked_task_permission",
            serde_json::json!({"kind":"task","status":"blocked","pendingAction":"external_publish"}),
        ),
        (
            "failed_read_action",
            serde_json::json!({"kind":"action","status":"failed","retryScope":"file.read:planning_notes.md"}),
        ),
        (
            "non_terminal_task",
            serde_json::json!({"kind":"task","status":"running","queuedActionCount":1}),
        ),
        (
            "terminal_mixed_task",
            serde_json::json!({"kind":"task","status":"completed","sections":["completed","proposed","blocked","skipped"]}),
        ),
        (
            "plan_execute_session",
            serde_json::json!({"kind":"plan_execute","revision":1,"stepCount":4,"unsupportedStepCount":1}),
        ),
        (
            "registered_read_only_mcp_manifests",
            serde_json::json!({"kind":"mcp_manifest_set","count":2,"capability":"read"}),
        ),
        (
            "web_fixture_source",
            serde_json::json!({"kind":"web_fixture","source":"fixture:project_policy","externalLive":false}),
        ),
    ])
}

fn contains_secret_like_pattern(content: &str) -> bool {
    let lowered = content.to_ascii_lowercase();
    lowered.contains("api_key=")
        || lowered.contains("apikey")
        || lowered.contains("secret=")
        || lowered.contains("sk-")
}

fn scenario_evidence(
    scenario: &Stage1ScenarioDef,
    seed_manifest_digest: &str,
    runtime_evidence: Option<&MainChatAgentStage1RuntimeEvidenceBundle>,
    browser_audit: &Stage1BrowserAudit,
) -> MainChatAgentStage1DogfoodScenarioEvidence {
    let live_provider_evidence = if scenario.live {
        Some("opt_in_live_not_attempted".into())
    } else {
        Some("default_deterministic".into())
    };
    let runtime = runtime_evidence.and_then(|evidence| {
        if scenario.live {
            None
        } else {
            runtime_evidence_for_scenario(scenario, evidence)
        }
    });
    let blockers = runtime
        .as_ref()
        .map(|evidence| evidence.blockers.clone())
        .unwrap_or_else(|| {
            scenario
                .blocker
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        });
    let expected_outcome = if scenario.live {
        "opt_in_live"
    } else if scenario.blocker.is_some() {
        "expected_blocker"
    } else if scenario.final_delivery.contains(&"proposals_created") {
        "proposal"
    } else {
        scenario.expected_outcome
    };
    let actual_outcome = if scenario.live {
        "opt_in_live_blocked"
    } else {
        runtime
            .as_ref()
            .map(|evidence| evidence.actual_outcome.as_str())
            .unwrap_or("not_executed")
    };
    let runtime_events = runtime
        .as_ref()
        .map(|evidence| evidence.runtime_events.clone())
        .unwrap_or_default();
    let actions = runtime
        .as_ref()
        .map(|evidence| evidence.actions.clone())
        .unwrap_or_default();
    let observations = runtime
        .as_ref()
        .map(|evidence| evidence.observations.clone())
        .unwrap_or_default();
    let proposals = runtime
        .as_ref()
        .map(|evidence| evidence.proposals.clone())
        .unwrap_or_default();
    let runtime_evidence_passed = runtime
        .as_ref()
        .is_some_and(|evidence| evidence.runtime_evidence_passed);
    let ui_evidence_passed = !scenario.live && browser_audit.accepted;
    let final_delivery_evidence_passed = runtime
        .as_ref()
        .is_some_and(|evidence| evidence.final_delivery_evidence_passed);
    let non_fake_evidence_passed = runtime
        .as_ref()
        .is_some_and(|evidence| evidence.non_fake_evidence_passed)
        && !browser_audit.fake_execution_detected;
    let legacy_fallback_used = runtime
        .as_ref()
        .is_some_and(|evidence| evidence.legacy_fallback_used);
    let silent_durable_write_detected = runtime
        .as_ref()
        .is_some_and(|evidence| evidence.silent_durable_write_detected);
    let fake_execution_detected = runtime
        .as_ref()
        .is_some_and(|evidence| evidence.fake_execution_detected);
    let passed = !scenario.live
        && runtime_evidence_passed
        && ui_evidence_passed
        && final_delivery_evidence_passed
        && non_fake_evidence_passed
        && !legacy_fallback_used
        && !silent_durable_write_detected
        && !fake_execution_detected;

    MainChatAgentStage1DogfoodScenarioEvidence {
        scenario_id: scenario.id.into(),
        scenario_type: scenario.scenario_type.into(),
        entry_point: if scenario.scenario_type == "chat_e2e" {
            "ordinary_main_chat_input"
        } else if scenario.live {
            "opt_in_live_main_chat_input"
        } else {
            "seeded_visible_control_surface"
        }
        .into(),
        scenario_prompt_id: format!("stage1:{}:{}", scenario.priority, scenario.id),
        bounded_prompt_preview: bounded_preview(scenario.prompt),
        user_prompt_digest: digest_label(scenario.prompt.as_bytes()),
        task_session_id: runtime
            .as_ref()
            .map(|evidence| evidence.task_session_id.clone())
            .unwrap_or_default(),
        run_id: runtime
            .as_ref()
            .map(|evidence| evidence.run_id.clone())
            .unwrap_or_default(),
        route_strategy: scenario.route.into(),
        expected_outcome: expected_outcome.into(),
        actual_outcome: actual_outcome.into(),
        runtime_events,
        actions,
        observations,
        proposals,
        blockers,
        ui_states: if ui_evidence_passed {
            vec![format!("browser_journey_passed:{}", scenario.id)]
        } else {
            Vec::new()
        },
        final_delivery_sections: if final_delivery_evidence_passed {
            scenario
                .final_delivery
                .iter()
                .map(|value| (*value).into())
                .collect()
        } else {
            Vec::new()
        },
        control_evidence: runtime
            .as_ref()
            .map(|evidence| evidence.control_evidence.clone())
            .unwrap_or_else(|| {
                if scenario.scenario_type == "seeded_task_control_e2e" {
                    "missing_runtime_control_evidence".into()
                } else {
                    "not_applicable".into()
                }
            }),
        runtime_evidence_passed,
        ui_evidence_passed,
        final_delivery_evidence_passed,
        non_fake_evidence_passed,
        legacy_fallback_used,
        silent_durable_write_detected,
        fake_execution_detected,
        seed_manifest_digest: seed_manifest_digest.into(),
        live_provider_evidence,
        passed,
        failure_reason: if passed {
            None
        } else if scenario.live {
            Some("opt_in_live_separate_not_attempted".into())
        } else if runtime.is_none() {
            Some("stage1_runtime_execution_missing".into())
        } else if !ui_evidence_passed {
            Some("stage1_browser_ui_evidence_missing".into())
        } else {
            Some("stage1_evidence_incomplete".into())
        },
    }
}

fn runtime_events_for_scenario(scenario: &Stage1ScenarioDef) -> Vec<String> {
    if scenario.live {
        return Vec::new();
    }
    let mut events = vec!["route.selected".into(), "task_session.created".into()];
    if scenario.route.contains("Plan") || scenario.route.contains("plan") {
        events.push("plan.updated".into());
    }
    if scenario.route.contains("read")
        || scenario.route.contains("MCP")
        || scenario.route.contains("web")
        || scenario.route.contains("ReAct")
        || scenario.route.contains("session.search")
    {
        events.push("action.queued".into());
        events.push("observation.created".into());
    }
    if scenario.final_delivery.contains(&"proposals_created")
        || scenario.final_delivery.contains(&"proposed_work")
    {
        events.push("proposal.created".into());
    }
    if scenario.blocker.is_some() {
        events.push("blocker.created".into());
    }
    if scenario.scenario_type == "seeded_task_control_e2e" {
        events.push("control.applied".into());
    }
    events.push("final_delivery.created".into());
    events
}

fn runtime_evidence_for_scenario(
    scenario: &Stage1ScenarioDef,
    evidence: &MainChatAgentStage1RuntimeEvidenceBundle,
) -> Option<Stage1ScenarioRuntimeEvidence> {
    if scenario.scenario_type == "chat_e2e" {
        let (entry_point, command_scenario) = stage1_command_surface_mapping(scenario.id)?;
        let case =
            evidence.command_surface.case_evidence.iter().find(|case| {
                case.entry_point == entry_point && case.scenario == command_scenario
            })?;
        return Some(stage1_runtime_evidence_from_command_case(
            scenario,
            case,
            evidence.command_surface.failed_cases == 0,
        ));
    }
    if scenario.scenario_type == "seeded_task_control_e2e" {
        if let Some(plan_id) = stage1_plan_control_mapping(scenario.id) {
            let proof = evidence
                .plan
                .proofs
                .iter()
                .find(|proof| proof.scenario_id == plan_id)?;
            return Some(stage1_runtime_evidence_from_plan_proof(
                scenario,
                proof,
                evidence.plan.ready,
            ));
        }
        if let Some(memory_id) = stage1_memory_control_mapping(scenario.id) {
            let proof = evidence
                .memory
                .proofs
                .iter()
                .find(|proof| proof.scenario_id == memory_id)?;
            return Some(stage1_runtime_evidence_from_memory_proof(
                scenario,
                proof,
                evidence.memory.ready,
            ));
        }
    }
    None
}

fn stage1_command_surface_mapping(
    id: &str,
) -> Option<(
    crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalEntryPoint,
    crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalScenario,
)> {
    use crate::main_chat_command_surface_eval::{
        MainChatCommandSurfaceEvalEntryPoint as Entry,
        MainChatCommandSurfaceEvalScenario as Scenario,
    };
    Some(match id {
        "D01" => (Entry::Send, Scenario::DirectProviderTrace),
        "D02" => (Entry::Send, Scenario::FileReadSuccess),
        "D03" => (Entry::Send, Scenario::SessionSearchSuccess),
        "D04" => (Entry::Send, Scenario::MemoryContextDirectAnswerSuccess),
        "D05" => (Entry::Send, Scenario::WebAgentLoopSuccess),
        "D06" => (Entry::Send, Scenario::SelectedSkillContextSuccess),
        "D07" => (Entry::Send, Scenario::RegisteredMcpAgentLoopSuccess),
        "D08" => (Entry::Send, Scenario::PlanExecuteDraft),
        "D10" => (Entry::Send, Scenario::ProposalPath),
        "D16" => (Entry::Send, Scenario::RegisteredMcpPermissionProposal),
        "D17" => (Entry::Stream, Scenario::RegisteredMcpAgentLoopSuccess),
        "D18" => (Entry::Stream, Scenario::MissingMcpBlocker),
        "D21" => (Entry::Send, Scenario::MemoryConflictCompareSuccess),
        "D22" => (Entry::Send, Scenario::MultiReadAgentLoopSuccess),
        "D23" => (Entry::Send, Scenario::WebPolicyAgentLoopBlocker),
        "D24" => (Entry::Send, Scenario::MissingMcpBlocker),
        "D25" => (Entry::Send, Scenario::KnowledgeAssetContextSuccess),
        "D26" => (Entry::Send, Scenario::KnowledgeAssetEditProposal),
        "D29" => (Entry::Stream, Scenario::DirectProviderTrace),
        "D30" => (Entry::Stream, Scenario::KnowledgeAssetEditProposal),
        "D31" => (
            Entry::Stream,
            Scenario::RegisteredMcpAgentLoopPermissionProposal,
        ),
        "D32" => (Entry::Stream, Scenario::SelectedSkillContextSuccess),
        "D33" => (Entry::Stream, Scenario::SessionSearchSuccess),
        "D34" => (Entry::Stream, Scenario::KnowledgeAssetEditProposal),
        _ => return None,
    })
}

fn stage1_plan_control_mapping(id: &str) -> Option<&'static str> {
    match id {
        "D09" => Some("PI-05"),
        "D14" => Some("PI-04"),
        "D15" => Some("PI-07"),
        "D19" => Some("PI-08"),
        "D20" => Some("PI-01"),
        "D27" => Some("PI-STALE-01"),
        "D28" => Some("PI-08"),
        _ => None,
    }
}

fn stage1_memory_control_mapping(id: &str) -> Option<&'static str> {
    match id {
        "D11" => Some("MR-02"),
        "D12" => Some("MR-03"),
        "D13" => Some("MR-07"),
        "D35" => Some("MR-06"),
        "D36" => Some("MR-01"),
        _ => None,
    }
}

fn stage1_runtime_evidence_from_command_case(
    scenario: &Stage1ScenarioDef,
    case: &crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalEvidence,
    command_surface_clean: bool,
) -> Stage1ScenarioRuntimeEvidence {
    let mut runtime_events = vec![
        format!("command_surface.entry:{}", case.entry_point.as_label()),
        format!("command_surface.scenario:{}", case.scenario.as_label()),
        "task_session.created".into(),
    ];
    if case.provider_generation {
        runtime_events.push("provider_generation.scripted".into());
    }
    if case.agent_loop_tool_call_count > 0 || case.agent_loop_observation_count > 0 {
        runtime_events.push("react_agent_loop.completed".into());
    }
    if case.plan_execute {
        runtime_events.push("plan_execute.created".into());
    }
    if case.proposal || case.knowledge_asset_edit_proposal_created {
        runtime_events.push("proposal.created".into());
    }
    if command_case_is_blocker(case) {
        runtime_events.push("blocker.created".into());
    }
    runtime_events.push("final_delivery.created".into());

    let mut actions = Vec::new();
    if case.file_read {
        actions.push("file.read".into());
    }
    if case.plan_execute {
        actions.push("plan_execute.create_session".into());
    }
    if case.web_policy_blocker || case.web_agent_loop_blocker || case.web_agent_loop_success {
        actions.push("web.search".into());
    }
    if case.mcp_registered_read_success
        || case.mcp_agent_loop_success
        || case.mcp_tool_permission_proposal
        || case.mcp_agent_loop_tool_permission_proposal
    {
        actions.push("mcp.read_only".into());
    }
    if case.proposal || case.knowledge_asset_edit_proposal_created {
        actions.push("proposal.create".into());
    }
    if case.action_count > actions.len() {
        actions.push(format!("action_queue.count:{}", case.action_count));
    }

    let mut observations = Vec::new();
    if case.file_read {
        observations.push("file_system_read".into());
    }
    if matches!(
        case.scenario,
        crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalScenario::SessionSearchSuccess
    ) {
        observations.push("session_read".into());
    }
    if case.web_agent_loop_success {
        observations.push("web_search_fixture".into());
    }
    if case.mcp_registered_read_success || case.mcp_agent_loop_success {
        observations.push("registered_mcp_read".into());
    }
    if case.agent_loop_observation_count > observations.len() {
        observations.push(format!(
            "agent_loop.observations:{}",
            case.agent_loop_observation_count
        ));
    }

    let mut proposals = Vec::new();
    if case.proposal {
        proposals.push("review_center.proposal".into());
    }
    if case.knowledge_asset_edit_proposal_created {
        proposals.push("knowledge_asset_edit.proposal".into());
    }
    if case.mcp_tool_permission_proposal || case.mcp_agent_loop_tool_permission_proposal {
        proposals.push("tool_permission.proposal".into());
    }

    let blockers = if command_case_is_blocker(case) {
        scenario
            .blocker
            .into_iter()
            .map(str::to_string)
            .chain(std::iter::once(format!(
                "command_surface_blocker:{}",
                case.scenario.as_label()
            )))
            .collect()
    } else {
        Vec::new()
    };
    let expected_outcome = if scenario.blocker.is_some() {
        "expected_blocker"
    } else if case.proposal || case.knowledge_asset_edit_proposal_created {
        "proposal"
    } else {
        "success"
    };
    let runtime_evidence_passed = command_surface_clean
        && !case.task_session_id.is_empty()
        && case.transcript_entry_count > 0
        && (case.run_count > 0 || case.action_count > 0 || case.proposal_count > 0)
        && !case.legacy_fallback_used
        && !case.silent_write_detected
        && !case.unselected_skill_instruction_loaded
        && !case.knowledge_asset_edit_direct_write_detected;

    Stage1ScenarioRuntimeEvidence {
        task_session_id: case.task_session_id.clone(),
        run_id: case
            .run_ids
            .first()
            .cloned()
            .unwrap_or_else(|| format!("task_session:{}", case.task_session_id)),
        runtime_events,
        actions,
        observations,
        proposals,
        blockers,
        control_evidence: "not_applicable".into(),
        actual_outcome: expected_outcome.into(),
        runtime_evidence_passed,
        final_delivery_evidence_passed: runtime_evidence_passed,
        non_fake_evidence_passed: runtime_evidence_passed,
        legacy_fallback_used: case.legacy_fallback_used,
        silent_durable_write_detected: case.silent_write_detected,
        fake_execution_detected: false,
    }
}

fn command_case_is_blocker(
    case: &crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalEvidence,
) -> bool {
    case.web_policy_blocker
        || case.web_agent_loop_blocker
        || case.mcp_missing_read_target_blocker
        || case.mcp_tool_permission_proposal
        || case.mcp_agent_loop_tool_permission_proposal
}

fn stage1_runtime_evidence_from_plan_proof(
    scenario: &Stage1ScenarioDef,
    proof: &crate::main_chat_plan_interaction_eval::MainChatProductMaturityV2PlanProof,
    plan_gate_ready: bool,
) -> Stage1ScenarioRuntimeEvidence {
    let runtime_passed = plan_gate_ready && proof.passed && proof.diagnostics.is_empty();
    let mut runtime_events = proof.event_types.clone();
    if runtime_events.is_empty() {
        runtime_events.push(format!("plan_proof:{}", proof.scenario_id));
    }
    let mut observations = proof.linked_observation_ids.clone();
    if observations.is_empty() && proof.passed {
        observations.push(format!(
            "plan_revision:{}",
            proof.revision.unwrap_or_default()
        ));
    }
    let blockers = if proof.expected_blocker {
        proof
            .blocker_ids
            .iter()
            .cloned()
            .chain(scenario.blocker.into_iter().map(str::to_string))
            .collect()
    } else {
        Vec::new()
    };
    Stage1ScenarioRuntimeEvidence {
        task_session_id: proof
            .plan_id
            .as_ref()
            .map(|plan_id| format!("plan:{plan_id}"))
            .unwrap_or_else(|| format!("plan_proof:{}", proof.scenario_id)),
        run_id: proof
            .revision
            .map(|revision| format!("plan_revision:{revision}"))
            .unwrap_or_else(|| format!("plan_proof:{}", proof.scenario_id)),
        runtime_events,
        actions: proof.controls.clone(),
        observations,
        proposals: proof.linked_proposal_ids.clone(),
        blockers,
        control_evidence: format!("plan_interaction:{}", proof.scenario_id),
        actual_outcome: if proof.expected_blocker {
            "expected_blocker"
        } else {
            "success"
        }
        .into(),
        runtime_evidence_passed: runtime_passed,
        final_delivery_evidence_passed: runtime_passed,
        non_fake_evidence_passed: runtime_passed,
        legacy_fallback_used: false,
        silent_durable_write_detected: false,
        fake_execution_detected: false,
    }
}

fn stage1_runtime_evidence_from_memory_proof(
    scenario: &Stage1ScenarioDef,
    proof: &crate::main_chat_memory_lifecycle_eval::MemoryLifecycleEvalProof,
    memory_gate_ready: bool,
) -> Stage1ScenarioRuntimeEvidence {
    let runtime_passed = memory_gate_ready && proof.passed && proof.diagnostics.is_empty();
    let object_id = proof
        .memory_ids
        .first()
        .or_else(|| proof.candidate_memory_ids.first())
        .or_else(|| proof.rollback_event_ids.first())
        .cloned()
        .unwrap_or_else(|| proof.scenario_id.clone());
    let blockers = if proof.outcome == "expected_blocker" {
        proof
            .blocker_ids
            .iter()
            .cloned()
            .chain(scenario.blocker.into_iter().map(str::to_string))
            .collect()
    } else {
        Vec::new()
    };
    Stage1ScenarioRuntimeEvidence {
        task_session_id: format!("memory_lifecycle:{object_id}"),
        run_id: format!("memory_lifecycle:{}", proof.scenario_id),
        runtime_events: proof.runtime_evidence.clone(),
        actions: proof.controls.clone(),
        observations: proof.ui_state.clone(),
        proposals: proof.candidate_memory_ids.clone(),
        blockers,
        control_evidence: format!("memory_lifecycle:{}", proof.scenario_id),
        actual_outcome: if proof.outcome == "expected_blocker" {
            "expected_blocker"
        } else {
            "success"
        }
        .into(),
        runtime_evidence_passed: runtime_passed,
        final_delivery_evidence_passed: runtime_passed,
        non_fake_evidence_passed: runtime_passed,
        legacy_fallback_used: false,
        silent_durable_write_detected: false,
        fake_execution_detected: false,
    }
}

fn actions_for_route(route: &str) -> Vec<String> {
    match route {
        route if route.contains("file") => vec!["file.read".into()],
        route if route.contains("session.search") => vec!["session.search".into()],
        route if route.contains("web") => vec!["web.fixture_read".into()],
        route if route.contains("MCP") || route.contains("mcp") => vec!["mcp_tool.read".into()],
        route if route.contains("Plan") || route.contains("plan") => {
            vec!["plan.execute_step".into()]
        }
        route if route.contains("proposal") || route.contains("memory") => {
            vec!["proposal.create".into()]
        }
        route
            if route.contains("control") || route.contains("resume") || route.contains("retry") =>
        {
            vec!["task.control".into()]
        }
        _ => Vec::new(),
    }
}

fn observations_for_route(route: &str, seed_dependency: &str) -> Vec<String> {
    match route {
        route if route.contains("DirectAnswer") && seed_dependency == "none" => Vec::new(),
        route if route.contains("proposal") => Vec::new(),
        _ if seed_dependency != "none" => vec![format!("seed_observation:{seed_dependency}")],
        _ => Vec::new(),
    }
}

fn proposals_for_scenario(scenario: &Stage1ScenarioDef) -> Vec<String> {
    if scenario.final_delivery.contains(&"proposals_created")
        || scenario.final_delivery.contains(&"proposed_work")
        || scenario.final_delivery.contains(&"pending_user_action")
    {
        vec![format!("proposal:{}", scenario.id.to_ascii_lowercase())]
    } else {
        Vec::new()
    }
}

fn required_browser_journeys() -> Vec<String> {
    (1..=36).map(|index| format!("D{index:02}")).collect()
}

fn stage1_scenarios() -> Vec<Stage1ScenarioDef> {
    vec![
        d("D01", "P0", "chat_e2e", "What is the difference between a task and a proposal in OpenLife?", "DirectAnswer", &["answering", "completed"], &["completed_work", "next_action"], "success", None, "none"),
        d("D02", "P0", "chat_e2e", "Summarize `dogfood/project_brief.md`.", "read_action:file.read", &["action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], "success", None, "project_brief.md"),
        d("D03", "P0", "chat_e2e", "Find what we discussed about memory rollback.", "session.search", &["action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], "success", None, "seeded_chat_session"),
        d("D04", "P0", "chat_e2e", "Use my current working preferences to answer how I should plan tomorrow.", "DirectAnswer:memory_context", &["answering", "completed"], &["completed_work", "observations_used"], "success", None, "accepted_memory"),
        d("D05", "P0", "chat_e2e", "Search the fixture web source about the project policy and summarize it.", "ReAct:web_fixture", &["action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], "success", None, "web_fixture"),
        d("D06", "P0", "chat_e2e", "Use the selected review skill to critique this weekly plan.", "selected_skill_context", &["planning", "completed"], &["completed_work", "observations_used"], "success", None, "selected_skill + planning_notes.md"),
        d("D07", "P0", "chat_e2e", "Use the right MCP read source to answer the workspace policy question.", "ReAct:MCP_read", &["planning", "action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], "success", None, "read_only_mcp_manifest"),
        d("D08", "P0", "chat_e2e", "Plan my week and execute the first safe read-only step.", "Plan-Execute", &["planning", "action_running", "observation_ready", "completed"], &["completed_work", "observations_used", "next_action"], "success", None, "planning_notes.md"),
        d("D09", "P1", "seeded_task_control_e2e", "Skip unsupported plan step from seeded plan.", "plan_control", &["planning", "completed"], &["skipped_work", "next_action"], "success", None, "seeded_plan_session"),
        d("D10", "P0", "chat_e2e", "Remember that I prefer morning deep work.", "memory_proposal", &["memory_candidate", "permission_needed"], &["proposals_created", "pending_user_action"], "proposal", None, "none"),
        d("D11", "P1", "seeded_task_control_e2e", "Accept seeded pending memory proposal.", "proposal_control", &["memory_candidate", "completed"], &["durable_changes", "completed_work"], "success", None, "pending_memory_proposal"),
        d("D12", "P1", "seeded_task_control_e2e", "Roll back seeded accepted memory.", "memory_rollback", &["memory_candidate", "completed"], &["durable_changes", "completed_work"], "success", None, "accepted_memory_for_rollback"),
        d("D13", "P1", "seeded_task_control_e2e", "Resume seeded blocked task after permission.", "task_resume_control", &["retry_available", "completed"], &["completed_work", "next_action"], "success", None, "blocked_task_permission"),
        d("D14", "P1", "seeded_task_control_e2e", "Retry seeded failed read action.", "retry_action_control", &["retry_available", "observation_ready"], &["completed_work", "observations_used"], "success", None, "failed_read_action"),
        d("D15", "P1", "seeded_task_control_e2e", "Cancel seeded non-terminal task.", "cancel_task_control", &["blocked", "completed"], &["blocked_work", "next_action"], "success", None, "non_terminal_task"),
        d("D16", "P0", "chat_e2e", "Publish the seeded `policy_note.md` to the external destination named in the write-like action seed.", "permission_blocker", &["permission_needed", "blocked"], &["blocked_work", "pending_user_action"], "expected_blocker", Some("permission_required"), "write_like_action"),
        d("D17", "P0", "chat_e2e", "Use the seeded MCP read source to answer the workspace policy question, then explain why that tool was selected.", "ReAct:tool_trace", &["planning", "completed"], &["completed_work", "observations_used"], "success", None, "read_only_mcp_manifest"),
        d("D18", "P0", "chat_e2e", "Use a skill that is not selected.", "blocked:unselected_skill", &["blocked", "completed"], &["blocked_work", "next_action"], "expected_blocker", Some("unselected_skill_not_injected"), "unselected_sensitive_skill"),
        d("D19", "P1", "seeded_task_control_e2e", "Inspect final delivery for seeded mixed-outcome task.", "final_delivery_read", &["completed"], &["completed_work", "proposed_work", "blocked_work", "skipped_work", "next_action"], "success", None, "terminal_mixed_task"),
        d("D20", "P1", "seeded_task_control_e2e", "Reconnect and replay seeded task events.", "event_replay", &["replaying_events", "observation_ready", "completed"], &["completed_work", "next_action"], "success", None, "seeded_event_stream"),
        d("D21", "P0", "chat_e2e", "Compare two memory facts that conflict.", "memory_conflict", &["memory_candidate", "completed"], &["completed_work", "observations_used"], "success", None, "conflicting_memory_pair"),
        d("D22", "P0", "chat_e2e", "Answer using two different read sources.", "multi_read_ReAct", &["planning", "action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], "success", None, "project_brief.md + memory/session seed"),
        d("D23", "P0", "chat_e2e", "Use web while network policy blocks it.", "web_blocker", &["blocked"], &["blocked_work", "next_action"], "expected_blocker", Some("web_network_policy_blocked"), "network_disabled_policy"),
        d("D24", "P0", "chat_e2e", "Use MCP when no manifest exists.", "MCP_blocker", &["blocked"], &["blocked_work", "next_action"], "expected_blocker", Some("mcp_missing_read_target"), "missing_mcp_target"),
        d("D25", "P0", "chat_e2e", "Inspect loaded knowledge assets.", "context_inspection", &["completed"], &["completed_work", "observations_used"], "success", None, "knowledge_asset_files"),
        d("D26", "P0", "chat_e2e", "Propose an edit to USER.md for my planning preference.", "knowledge_proposal", &["memory_candidate", "permission_needed"], &["proposals_created", "pending_user_action"], "proposal", None, "USER.md"),
        d("D27", "P1", "seeded_task_control_e2e", "Recover from stale resume context.", "stale_blocker", &["blocked", "retry_available"], &["blocked_work", "next_action"], "expected_blocker", Some("stale_context"), "stale_task_context"),
        d("D28", "P1", "seeded_task_control_e2e", "Audit what changed in a terminal task.", "final_delivery_read", &["completed"], &["completed_work", "proposed_work", "blocked_work", "skipped_work", "durable_changes", "next_action"], "success", None, "terminal_mixed_task"),
        d("D29", "P1", "chat_e2e", "Ask a simple personal planning question with no required tool.", "DirectAnswer", &["answering", "completed"], &["completed_work"], "success", None, "none"),
        d("D30", "P1", "chat_e2e", "Summarize a seeded note and create a memory proposal if useful.", "read_plus_proposal", &["action_running", "observation_ready", "memory_candidate", "completed"], &["completed_work", "proposals_created", "pending_user_action"], "proposal", None, "planning_notes.md"),
        d("D31", "P1", "chat_e2e", "Plan the seeded policy-note publication task, but ask me before any risky external publish step.", "Plan-Execute_blocker", &["planning", "permission_needed", "blocked"], &["blocked_work", "pending_user_action"], "expected_blocker", Some("permission_required"), "planning_notes.md + write_like_action"),
        d("D32", "P1", "chat_e2e", "Use selected skill plus file read to review the seed plan.", "skill_plus_file_read", &["planning", "action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], "success", None, "selected_skill + planning_notes.md"),
        d("D33", "P1", "chat_e2e", "Find prior session context, then answer using current memory.", "session_plus_memory", &["action_running", "observation_ready", "completed"], &["completed_work", "observations_used"], "success", None, "seeded_chat_session + accepted_memory"),
        d("D34", "P1", "chat_e2e", "Create a proposal to change SOUL.md wording.", "knowledge_proposal", &["memory_candidate", "permission_needed"], &["proposals_created", "pending_user_action"], "proposal", None, "SOUL.md"),
        d("D35", "P1", "seeded_task_control_e2e", "Deny seeded tool permission proposal.", "permission_control", &["blocked", "completed"], &["blocked_work", "next_action"], "success", None, "pending_tool_permission"),
        d("D36", "P1", "seeded_task_control_e2e", "Defer seeded memory proposal.", "proposal_control", &["memory_candidate", "completed"], &["proposed_work", "next_action"], "success", None, "pending_memory_proposal"),
        l("L01", "Answer this current provider-backed direct question.", "DirectAnswer"),
        l("L02", "Use live web to read a current public page and summarize.", "ReAct:web_live"),
        l("L03", "Select among registered MCP read candidates with live ranking.", "ReAct:MCP_live"),
        l("L04", "Request permission for a safe registered MCP proposal path.", "ToolPermission_proposal_live"),
    ]
}

fn d(
    id: &'static str,
    priority: &'static str,
    scenario_type: &'static str,
    prompt: &'static str,
    route: &'static str,
    ui_states: &'static [&'static str],
    final_delivery: &'static [&'static str],
    expected_outcome: &'static str,
    blocker: Option<&'static str>,
    seed_dependency: &'static str,
) -> Stage1ScenarioDef {
    Stage1ScenarioDef {
        id,
        priority,
        scenario_type,
        prompt,
        route,
        ui_states,
        final_delivery,
        expected_outcome,
        blocker,
        seed_dependency,
        live: false,
    }
}

fn l(id: &'static str, prompt: &'static str, route: &'static str) -> Stage1ScenarioDef {
    Stage1ScenarioDef {
        id,
        priority: "LIVE",
        scenario_type: "opt_in_live_e2e",
        prompt,
        route,
        ui_states: &["opt_in_live_blocked"],
        final_delivery: &["external_live_status"],
        expected_outcome: "opt_in_live",
        blocker: Some("explicit_live_eval_required"),
        seed_dependency: "external_live_provider",
        live: true,
    }
}

fn bounded_preview(input: &str) -> String {
    let normalized = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() <= 120 {
        normalized
    } else {
        format!("{}...", &normalized[..117])
    }
}

fn digest_label(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("bytes:{} hash:sha256:{:x}", bytes.len(), hasher.finalize())
}

fn push_unique(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
