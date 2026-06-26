use crate::AppState;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

const STEP6_SCHEMA_VERSION: &str = "step6-product-acceptance-v1";
const STEP6_READINESS_SEMANTICS: &str =
    "step6_local_deterministic_required_external_live_opt_in_separate";
const STEP6_BROWSER_REPORT_PATH: &str =
    "frontend/test-results/main-chat-step6-product-acceptance-report.json";
const STEP6_BROWSER_E2E_MAX_AGE_HOURS: i64 = 24;
const STEP6_OBSERVED_EVIDENCE_SOURCE: &str = "tauri_command_surface_step6_browser_observed";
const STEP6_BLOCKED_EVIDENCE_SOURCE: &str = "tauri_command_surface_unavailable";
const STEP6_BLOCKED_LIVE_UI_STATUS: &str = "blocked_live_evidence";
const STEP6_LOCAL_JOURNEYS: [&str; 9] = [
    "S6-CLOCK",
    "S6-ROUTE",
    "S6-TOOLS",
    "S6-FILE",
    "S6-DIRECT-SELF",
    "S6-PROPOSAL",
    "S6-BLOCKED",
    "S6-PERMISSION",
    "S6-RECOVERY",
];
const STEP6_EXTERNAL_LIVE_JOURNEYS: [&str; 2] = ["S6-LIVE-WEB", "S6-LIVE-MCP"];
const STEP6_REQUIRED_JOURNEYS: [&str; 11] = [
    "S6-CLOCK",
    "S6-ROUTE",
    "S6-TOOLS",
    "S6-FILE",
    "S6-DIRECT-SELF",
    "S6-PROPOSAL",
    "S6-BLOCKED",
    "S6-PERMISSION",
    "S6-LIVE-WEB",
    "S6-LIVE-MCP",
    "S6-RECOVERY",
];
const STEP6_LIVE_EVAL_DEFAULT_PROVIDER: &str = "openai";
const STEP6_LIVE_EVAL_DEFAULT_BASE: &str = "https://api.openai.com/v1";
const STEP6_LIVE_EVAL_DEFAULT_MODEL: &str = "gpt-4o-mini";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStep6ProductAcceptanceReport {
    pub report_kind: String,
    pub schema_version: String,
    pub overall_ready: bool,
    pub local_deterministic_ready: bool,
    pub external_live_ready: bool,
    pub browser_e2e_environment_ready: bool,
    pub browser_e2e_report_path: Option<String>,
    pub required_journey_count: usize,
    pub local_journey_count: usize,
    pub external_live_journey_count: usize,
    pub passed_journey_count: usize,
    pub blocked_live_journey_count: usize,
    pub failed_journeys: Vec<String>,
    pub no_silent_durable_write: bool,
    pub no_hidden_legacy_fallback: bool,
    pub no_local_fixture_marked_external_live: bool,
    pub no_local_evidence_credited_as_external_live: bool,
    pub no_invented_unavailable_evidence: bool,
    pub ui_status_from_structured_evidence: bool,
    pub final_gate_summary: Step6FinalGateSummary,
    pub journeys: Vec<Step6JourneyReport>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStep6LiveProviderEvalStatePrepReport {
    pub report_kind: String,
    pub configured: bool,
    pub ready: bool,
    pub debug_build: bool,
    pub explicit_live_eval_requested: bool,
    pub provider: String,
    pub model: String,
    pub base_configured: bool,
    pub api_key_present: bool,
    pub network_enabled: bool,
    pub provider_endpoint_kind: String,
    pub preflight_ready: bool,
    pub preflight_blockers: Vec<String>,
    pub app_config_persisted: bool,
    pub direct_writes_executed: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step6JourneyReport {
    pub journey_id: String,
    pub status: String,
    pub credited: bool,
    pub blocked_live_evidence_report: bool,
    pub evidence_source: String,
    pub answer_evidence_count: usize,
    pub runtime_evidence_count: usize,
    pub ui_state_count: usize,
    pub final_delivery_section_count: usize,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step6FinalGateSummary {
    pub collected: bool,
    pub final_acceptance_ready: bool,
    pub final_acceptance_blockers: Vec<String>,
    pub command_surface_legacy_fallback_count: usize,
    pub command_surface_silent_write_count: usize,
    pub live_provider_attempted: bool,
    pub live_provider_ready_count: usize,
    pub live_provider_web_credit: bool,
    pub live_provider_mcp_credit: bool,
    pub live_provider_scenario_reports:
        Vec<crate::main_chat_final_gate::MainChatLiveProviderScenarioReport>,
    pub live_provider_blockers: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Step6BrowserReport {
    pub report_kind: String,
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub readiness_semantics: String,
    #[serde(default)]
    #[serde(alias = "e2eEnvironmentReady")]
    pub browser_e2e_environment_ready: bool,
    #[serde(default)]
    pub self_contained_runner: bool,
    #[serde(default)]
    pub smoke_passed: bool,
    #[serde(default)]
    pub report_path: String,
    #[serde(default)]
    pub evidence_source: String,
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub report_digest: String,
    #[serde(default)]
    pub required_journeys: Vec<String>,
    #[serde(default)]
    pub local_journeys: Vec<String>,
    #[serde(default)]
    pub local_journey_count: usize,
    #[serde(default)]
    pub external_live_journeys: Vec<String>,
    #[serde(default)]
    pub external_live_journey_count: usize,
    #[serde(default)]
    pub passed_journeys: Vec<String>,
    #[serde(default)]
    pub blocked_live_journeys: Vec<String>,
    #[serde(default)]
    pub failed_journeys: Vec<String>,
    #[serde(default)]
    pub local_deterministic_ready: bool,
    #[serde(default)]
    pub external_live_ready: bool,
    #[serde(default)]
    #[serde(alias = "acceptanceReady")]
    pub overall_ready: bool,
    #[serde(default)]
    pub no_silent_durable_write: bool,
    #[serde(default)]
    pub no_hidden_legacy_fallback: bool,
    #[serde(default)]
    #[serde(alias = "noLocalEvidenceCreditedAsExternalLive")]
    pub no_local_fixture_marked_external_live: bool,
    #[serde(default)]
    pub no_invented_unavailable_evidence: bool,
    #[serde(default)]
    pub ui_status_from_structured_evidence: bool,
    #[serde(default)]
    pub observed_journeys: Vec<Step6ObservedJourney>,
    #[serde(default)]
    pub external_live_blockers: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Step6ObservedJourney {
    pub journey_id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub observed_via: String,
    #[serde(default)]
    pub entry_point: String,
    #[serde(default)]
    pub task_session_id: String,
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub route_strategy: String,
    #[serde(default)]
    pub answer_evidence: Vec<String>,
    #[serde(default)]
    pub runtime_evidence: Vec<String>,
    #[serde(default)]
    pub visible_ui_states: Vec<String>,
    #[serde(default)]
    pub ui_status_evidence: Vec<String>,
    #[serde(default)]
    pub final_delivery_sections: Vec<String>,
    #[serde(default)]
    pub trace_evidence: Vec<String>,
    #[serde(default)]
    pub visible_blockers: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub answer_observed: bool,
    #[serde(default)]
    pub runtime_evidence_observed: bool,
    #[serde(default)]
    pub ui_state_observed: bool,
    #[serde(default)]
    pub final_delivery_observed: bool,
    #[serde(default)]
    pub non_fake_evidence_observed: bool,
    #[serde(default)]
    pub no_invented_unavailable_evidence: bool,
    #[serde(default)]
    pub unavailable_evidence_invented: bool,
    #[serde(default)]
    pub legacy_fallback_used: bool,
    #[serde(default)]
    pub silent_durable_write_detected: bool,
    #[serde(default)]
    pub fake_execution_detected: bool,
    #[serde(default)]
    pub live_evidence_kind: String,
    #[serde(default)]
    pub external_live_credit: bool,
    #[serde(default)]
    pub blocked_live_evidence_report: bool,
    #[serde(default)]
    pub local_fixture_credited_as_external_live: bool,
    #[serde(default)]
    pub external_live_status: String,
    #[serde(default)]
    pub external_live_provider_kind: Option<String>,
}

pub(crate) async fn run_main_chat_step6_product_acceptance_report(
    state: &Arc<AppState>,
) -> Result<MainChatStep6ProductAcceptanceReport, String> {
    let browser_report = read_step6_browser_report_from_default_path();
    let final_gate_summary = collect_step6_final_gate_summary(state).await;
    Ok(build_step6_product_acceptance_report(
        browser_report,
        final_gate_summary,
    ))
}

pub(crate) async fn prepare_main_chat_step6_live_provider_eval_state_with_state(
    state: &Arc<AppState>,
) -> Result<MainChatStep6LiveProviderEvalStatePrepReport, String> {
    prepare_main_chat_step6_live_provider_eval_state_with_env(
        state,
        Step6LiveProviderEvalEnv::from_process_env(),
    )
    .await
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Step6LiveProviderEvalEnv {
    pub explicit_live_eval_requested: bool,
    pub provider: String,
    pub base: String,
    pub model: String,
    pub api_key: String,
}

impl Step6LiveProviderEvalEnv {
    fn from_process_env() -> Self {
        Self {
            explicit_live_eval_requested:
                crate::main_chat_live_provider_harness::main_chat_live_provider_eval_opt_in_from_env(
                ),
            provider: std::env::var("OPENLIFE_LIVE_EVAL_PROVIDER").unwrap_or_default(),
            base: std::env::var("OPENLIFE_LIVE_EVAL_BASE").unwrap_or_default(),
            model: std::env::var("OPENLIFE_LIVE_EVAL_MODEL").unwrap_or_default(),
            api_key: std::env::var("OPENLIFE_LIVE_EVAL_API_KEY").unwrap_or_default(),
        }
    }
}

pub(crate) async fn prepare_main_chat_step6_live_provider_eval_state_with_env(
    state: &Arc<AppState>,
    env: Step6LiveProviderEvalEnv,
) -> Result<MainChatStep6LiveProviderEvalStatePrepReport, String> {
    let debug_build = cfg!(debug_assertions);
    let mut blockers = Vec::new();
    if !debug_build {
        push_unique(&mut blockers, "step6_live_provider_eval_state_debug_only");
    }
    if !env.explicit_live_eval_requested {
        push_unique(&mut blockers, "explicit_live_eval_required");
    }

    let provider = non_empty_trimmed_or(&env.provider, STEP6_LIVE_EVAL_DEFAULT_PROVIDER);
    let base = non_empty_trimmed_or(&env.base, STEP6_LIVE_EVAL_DEFAULT_BASE);
    let model = non_empty_trimmed_or(&env.model, STEP6_LIVE_EVAL_DEFAULT_MODEL);
    let api_key = env.api_key.trim().to_string();
    let base_configured = !env.base.trim().is_empty();
    let api_key_present = !api_key.is_empty();
    let provider_configured = !env.provider.trim().is_empty();
    let model_configured = !env.model.trim().is_empty();

    if !provider_configured {
        push_unique(&mut blockers, "openlife_live_eval_provider_missing");
    }
    if !base_configured {
        push_unique(&mut blockers, "openlife_live_eval_base_missing");
    }
    if !model_configured {
        push_unique(&mut blockers, "openlife_live_eval_model_missing");
    }
    if !api_key_present {
        push_unique(&mut blockers, "openlife_live_eval_api_key_missing");
    }

    let should_prepare_state = debug_build && env.explicit_live_eval_requested;
    let network_enabled = should_prepare_state
        && provider_configured
        && base_configured
        && model_configured
        && api_key_present;

    if should_prepare_state {
        let (local_model, embedding_model) = {
            let mut config = state.config.lock().await;
            config.prefer_local_model = false;
            config.llm.provider = provider.clone();
            config.llm.openai_base = base.clone();
            config.llm.openai_key = api_key.clone();
            config.llm.chat_model = model.clone();
            config.llm.embedding_enabled = false;
            config.system.network_policy.enabled = network_enabled;
            (
                config.local_model.clone(),
                config.llm.embedding_model.clone(),
            )
        };

        let mut scheduler = state.scheduler.lock().await;
        *scheduler = InferenceScheduler::new(
            local_model,
            false,
            provider.clone(),
            base.clone(),
            api_key.clone(),
            model.clone(),
            embedding_model,
            false,
        );
    }

    let scheduler = state.scheduler.lock().await.clone();
    let scripted_provider_response_present = scheduler.scripted_generation_response.is_some();
    let provider_endpoint_kind =
        crate::main_chat_generation_support::main_chat_provider_endpoint_kind(
            &scheduler,
            scripted_provider_response_present,
        )
        .to_string();
    let preflight =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_live_provider_eval_preflight(
            openlife_core::agent::main_chat_agent_v1::MainChatLiveProviderEvalPreflightInput {
                provider: provider.clone(),
                api_key_present,
                network_enabled,
                explicit_live_eval_requested: env.explicit_live_eval_requested,
                scripted_provider_response_present,
                local_only_required: false,
            },
        );
    if provider_endpoint_kind != "external_provider" {
        push_unique(&mut blockers, "external_provider_endpoint_required");
    }
    for blocker in &preflight.blockers {
        push_unique(&mut blockers, blocker);
    }
    let mut blockers = normalize_blockers(blockers);
    let preliminarily_ready = should_prepare_state
        && preflight.ready
        && provider_endpoint_kind == "external_provider"
        && blockers.is_empty();
    if preliminarily_ready {
        if let Err(error) =
            crate::main_chat_command_surface_eval::grant_builtin_echo_read_once(state).await
        {
            push_unique(
                &mut blockers,
                format!(
                    "step6_live_mcp_permission_seed_failed:{}",
                    error_code(&error)
                ),
            );
        }
    }
    let blockers = normalize_blockers(blockers);
    let ready = should_prepare_state
        && preflight.ready
        && provider_endpoint_kind == "external_provider"
        && blockers.is_empty();

    Ok(MainChatStep6LiveProviderEvalStatePrepReport {
        report_kind: "main_chat_step6_live_provider_eval_state_prep".into(),
        configured: should_prepare_state,
        ready,
        debug_build,
        explicit_live_eval_requested: env.explicit_live_eval_requested,
        provider: metadata_safe_or_redacted(&provider),
        model: metadata_safe_or_redacted(&model),
        base_configured,
        api_key_present,
        network_enabled,
        provider_endpoint_kind: metadata_safe_or_redacted(&provider_endpoint_kind),
        preflight_ready: preflight.ready,
        preflight_blockers: normalize_blockers(preflight.blockers),
        app_config_persisted: false,
        direct_writes_executed: false,
        blockers,
    })
}

async fn collect_step6_final_gate_summary(state: &Arc<AppState>) -> Step6FinalGateSummary {
    let live_opt_in =
        crate::main_chat_live_provider_harness::main_chat_live_provider_eval_opt_in_from_env();
    match crate::commands::agent_runtime::run_main_chat_agent_execution_v1_final_acceptance_gate_with_state_and_live_opt_in(
        state,
        live_opt_in,
    )
        .await
    {
        Ok(report) => {
            let live_provider_web_credit = step6_live_provider_scenario_credit(
                &report.final_gate.live_provider_scenario_reports,
                &["web-agent-loop", "web_agent_loop"],
            );
            let live_provider_mcp_credit = step6_live_provider_scenario_credit(
                &report.final_gate.live_provider_scenario_reports,
                &["registered-mcp-agent-loop", "registered_mcp_agent_loop"],
            );
            let live_provider_scenario_reports = report.final_gate.live_provider_scenario_reports;
            Step6FinalGateSummary {
                collected: true,
                final_acceptance_ready: report.final_gate.acceptance.ready,
                final_acceptance_blockers: report.final_gate.acceptance.blockers,
                command_surface_legacy_fallback_count: report
                    .command_surface_eval
                    .legacy_fallback_count,
                command_surface_silent_write_count: report.command_surface_eval.silent_write_count,
                live_provider_attempted: report.live_provider_attempted,
                live_provider_ready_count: report.final_gate.live_provider_ready_count,
                live_provider_web_credit,
                live_provider_mcp_credit,
                live_provider_scenario_reports,
                live_provider_blockers: report.final_gate.live_provider_blockers,
                blockers: Vec::new(),
            }
        },
        Err(error) => Step6FinalGateSummary {
            collected: false,
            final_acceptance_ready: false,
            final_acceptance_blockers: Vec::new(),
            command_surface_legacy_fallback_count: 0,
            command_surface_silent_write_count: 0,
            live_provider_attempted: live_opt_in,
            live_provider_ready_count: 0,
            live_provider_web_credit: false,
            live_provider_mcp_credit: false,
            live_provider_scenario_reports: Vec::new(),
            live_provider_blockers: Vec::new(),
            blockers: vec![format!("step6_final_gate_collection_failed_{}", error_code(&error))],
        },
    }
}

pub(crate) fn step6_live_provider_scenario_credit(
    reports: &[crate::main_chat_final_gate::MainChatLiveProviderScenarioReport],
    accepted_scenarios: &[&str],
) -> bool {
    reports
        .iter()
        .any(|report| report.credited && accepted_scenarios.contains(&report.scenario.as_str()))
}

fn read_step6_browser_report_from_default_path() -> Option<Step6BrowserReport> {
    let path = repo_relative_path(STEP6_BROWSER_REPORT_PATH);
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn build_step6_product_acceptance_report(
    browser_report: Option<Step6BrowserReport>,
    final_gate_summary: Step6FinalGateSummary,
) -> MainChatStep6ProductAcceptanceReport {
    let browser_audit = audit_browser_report(browser_report.as_ref());
    let journeys = build_journey_reports(browser_report.as_ref(), &final_gate_summary);
    let passed_journey_count = journeys.iter().filter(|row| row.credited).count();
    let blocked_live_journey_count = journeys
        .iter()
        .filter(|row| row.blocked_live_evidence_report && !row.credited)
        .count();
    let failed_journeys = journeys
        .iter()
        .filter(|row| !row.credited && !row.blocked_live_evidence_report)
        .map(|row| row.journey_id.clone())
        .collect::<Vec<_>>();
    let local_deterministic_ready = all_ids_credited(&journeys, &STEP6_LOCAL_JOURNEYS);
    let external_live_ready = all_ids_credited(&journeys, &STEP6_EXTERNAL_LIVE_JOURNEYS);
    let no_silent_durable_write = browser_audit.no_silent_durable_write
        && final_gate_summary.command_surface_silent_write_count == 0
        && !final_gate_summary
            .live_provider_blockers
            .iter()
            .any(|blocker| blocker == "live_provider_direct_writes_detected");
    let no_hidden_legacy_fallback = browser_audit.no_hidden_legacy_fallback
        && final_gate_summary.command_surface_legacy_fallback_count == 0
        && !final_gate_summary
            .live_provider_blockers
            .iter()
            .any(|blocker| blocker == "live_provider_legacy_fallback_detected");
    let no_local_fixture_marked_external_live = browser_audit.no_local_fixture_marked_external_live;
    let no_invented_unavailable_evidence = browser_audit.no_invented_unavailable_evidence;
    let ui_status_from_structured_evidence = browser_audit.ui_status_from_structured_evidence;

    let mut blockers = browser_audit.blockers;
    for row in &journeys {
        for blocker in &row.blockers {
            push_unique(&mut blockers, blocker);
        }
    }
    for blocker in browser_report_claim_blockers(
        browser_report.as_ref(),
        &journeys,
        local_deterministic_ready,
        external_live_ready,
        &blockers,
    ) {
        push_unique(&mut blockers, blocker);
    }
    if !local_deterministic_ready {
        push_unique(&mut blockers, "step6_local_journeys_not_all_passed");
    }
    if !external_live_ready {
        push_unique(&mut blockers, "step6_external_live_journeys_not_all_passed");
    }
    if !final_gate_summary.collected {
        push_unique(&mut blockers, "step6_final_gate_not_collected");
    }
    if !final_gate_summary.final_acceptance_ready {
        push_unique(&mut blockers, "step6_final_acceptance_not_ready");
    }
    for blocker in &final_gate_summary.final_acceptance_blockers {
        push_unique(&mut blockers, blocker);
    }
    for blocker in &final_gate_summary.live_provider_blockers {
        push_unique(&mut blockers, blocker);
    }
    for blocker in &final_gate_summary.blockers {
        push_unique(&mut blockers, blocker);
    }
    if !no_silent_durable_write {
        push_unique(&mut blockers, "step6_silent_durable_write_detected");
    }
    if !no_hidden_legacy_fallback {
        push_unique(&mut blockers, "step6_hidden_legacy_fallback_detected");
    }
    if !no_local_fixture_marked_external_live {
        push_unique(&mut blockers, "step6_local_fixture_marked_external_live");
    }
    if !no_invented_unavailable_evidence {
        push_unique(
            &mut blockers,
            "step6_invented_unavailable_evidence_detected",
        );
    }
    if !ui_status_from_structured_evidence {
        push_unique(&mut blockers, "step6_ui_status_not_structured");
    }
    let blockers = normalize_blockers(blockers);
    let overall_ready = local_deterministic_ready
        && external_live_ready
        && no_silent_durable_write
        && no_hidden_legacy_fallback
        && no_local_fixture_marked_external_live
        && final_gate_summary.final_acceptance_ready
        && blockers.is_empty();

    MainChatStep6ProductAcceptanceReport {
        report_kind: "main_chat_step6_product_acceptance_gate".into(),
        schema_version: STEP6_SCHEMA_VERSION.into(),
        overall_ready,
        local_deterministic_ready,
        external_live_ready,
        browser_e2e_environment_ready: browser_audit.environment_ready,
        browser_e2e_report_path: browser_audit.report_path,
        required_journey_count: STEP6_REQUIRED_JOURNEYS.len(),
        local_journey_count: STEP6_LOCAL_JOURNEYS.len(),
        external_live_journey_count: STEP6_EXTERNAL_LIVE_JOURNEYS.len(),
        passed_journey_count,
        blocked_live_journey_count,
        failed_journeys,
        no_silent_durable_write,
        no_hidden_legacy_fallback,
        no_local_fixture_marked_external_live,
        no_local_evidence_credited_as_external_live: no_local_fixture_marked_external_live,
        no_invented_unavailable_evidence,
        ui_status_from_structured_evidence,
        final_gate_summary,
        journeys,
        blockers,
    }
}

#[derive(Debug, Clone)]
struct BrowserAudit {
    environment_ready: bool,
    report_path: Option<String>,
    no_silent_durable_write: bool,
    no_hidden_legacy_fallback: bool,
    no_local_fixture_marked_external_live: bool,
    no_invented_unavailable_evidence: bool,
    ui_status_from_structured_evidence: bool,
    blockers: Vec<String>,
}

fn audit_browser_report(report: Option<&Step6BrowserReport>) -> BrowserAudit {
    let Some(report) = report else {
        return BrowserAudit {
            environment_ready: false,
            report_path: None,
            no_silent_durable_write: false,
            no_hidden_legacy_fallback: false,
            no_local_fixture_marked_external_live: false,
            no_invented_unavailable_evidence: false,
            ui_status_from_structured_evidence: false,
            blockers: vec!["step6_browser_report_missing".into()],
        };
    };
    let mut blockers = Vec::new();
    if !matches!(
        report.report_kind.as_str(),
        "main_chat_step6_product_acceptance" | "main_chat_step6_product_acceptance_browser_report"
    ) {
        push_unique(&mut blockers, "step6_browser_report_kind_invalid");
    }
    if report.schema_version != STEP6_SCHEMA_VERSION {
        push_unique(&mut blockers, "step6_browser_schema_invalid");
    }
    if report.readiness_semantics != STEP6_READINESS_SEMANTICS {
        push_unique(&mut blockers, "step6_browser_readiness_semantics_invalid");
    }
    if report.report_path != STEP6_BROWSER_REPORT_PATH {
        push_unique(&mut blockers, "step6_browser_report_path_invalid");
    }
    if !report.browser_e2e_environment_ready || !report.self_contained_runner {
        push_unique(&mut blockers, "step6_browser_environment_not_ready");
    }
    if !report.smoke_passed {
        push_unique(&mut blockers, "step6_browser_smoke_not_passed");
    }
    if !metadata_safe_label(&report.evidence_source) {
        push_unique(&mut blockers, "step6_browser_evidence_source_unsafe");
    }
    if !matches!(
        report.evidence_source.as_str(),
        STEP6_OBSERVED_EVIDENCE_SOURCE | STEP6_BLOCKED_EVIDENCE_SOURCE
    ) {
        push_unique(&mut blockers, "step6_browser_evidence_source_invalid");
    }
    if report.browser_e2e_environment_ready
        && report.evidence_source != STEP6_OBSERVED_EVIDENCE_SOURCE
    {
        push_unique(&mut blockers, "step6_browser_ready_source_not_observed");
    }
    if !report.browser_e2e_environment_ready
        && report.evidence_source != STEP6_BLOCKED_EVIDENCE_SOURCE
    {
        push_unique(
            &mut blockers,
            "step6_browser_blocked_source_not_unavailable",
        );
    }
    if !metadata_safe_label(&report.run_id) {
        push_unique(&mut blockers, "step6_browser_run_id_unsafe");
    }
    if !step6_browser_report_trace_is_fresh_and_bounded(report) {
        push_unique(&mut blockers, "step6_browser_report_stale_or_untraceable");
    }
    if !digest_label_shape(&report.report_digest) {
        push_unique(&mut blockers, "step6_browser_report_digest_invalid");
    }
    if !step6_browser_report_digest_matches(report) {
        push_unique(&mut blockers, "step6_browser_report_digest_mismatch");
    }
    if report.required_journeys != ids_vec(&STEP6_REQUIRED_JOURNEYS) {
        push_unique(&mut blockers, "step6_required_journeys_mismatch");
    }
    if !report.local_journeys.is_empty() && report.local_journeys != ids_vec(&STEP6_LOCAL_JOURNEYS)
    {
        push_unique(&mut blockers, "step6_local_journeys_mismatch");
    }
    if report.local_journey_count != 0 && report.local_journey_count != STEP6_LOCAL_JOURNEYS.len() {
        push_unique(&mut blockers, "step6_local_journey_count_mismatch");
    }
    if !report.external_live_journeys.is_empty()
        && report.external_live_journeys != ids_vec(&STEP6_EXTERNAL_LIVE_JOURNEYS)
    {
        push_unique(&mut blockers, "step6_external_live_journeys_mismatch");
    }
    if report.external_live_journey_count != 0
        && report.external_live_journey_count != STEP6_EXTERNAL_LIVE_JOURNEYS.len()
    {
        push_unique(&mut blockers, "step6_external_live_journey_count_mismatch");
    }
    for blocker in &report.blockers {
        if metadata_safe_blocker(blocker) {
            push_unique(&mut blockers, blocker);
        } else {
            push_unique(&mut blockers, "step6_browser_blocker_unsafe");
        }
    }
    for blocker in &report.external_live_blockers {
        if metadata_safe_blocker(blocker) {
            push_unique(&mut blockers, blocker);
        } else {
            push_unique(&mut blockers, "step6_external_live_blocker_unsafe");
        }
    }
    for blocker in observed_journey_blockers(&report.observed_journeys) {
        push_unique(&mut blockers, blocker);
    }
    let rows_no_silent_durable_write = report
        .observed_journeys
        .iter()
        .all(|row| !row.silent_durable_write_detected);
    let rows_no_hidden_legacy_fallback = report
        .observed_journeys
        .iter()
        .all(|row| !row.legacy_fallback_used);
    let rows_no_local_fixture_marked_external_live = report.observed_journeys.iter().all(|row| {
        !row_external_live_credit(row)
            || (row_live_evidence_kind(row) == "external_live_provider"
                && !row.local_fixture_credited_as_external_live)
    });
    let rows_no_invented_unavailable_evidence = report.observed_journeys.iter().all(|row| {
        (row.no_invented_unavailable_evidence || !row.unavailable_evidence_invented)
            && !row.unavailable_evidence_invented
    });
    let rows_ui_status_from_structured_evidence = report
        .observed_journeys
        .iter()
        .all(row_has_structured_ui_status);
    if report.no_silent_durable_write != rows_no_silent_durable_write {
        push_unique(
            &mut blockers,
            "step6_browser_no_silent_write_claim_mismatch",
        );
    }
    if report.no_hidden_legacy_fallback != rows_no_hidden_legacy_fallback {
        push_unique(
            &mut blockers,
            "step6_browser_no_legacy_fallback_claim_mismatch",
        );
    }
    if report.no_local_fixture_marked_external_live != rows_no_local_fixture_marked_external_live {
        push_unique(
            &mut blockers,
            "step6_browser_no_local_fixture_live_claim_mismatch",
        );
    }
    if report.no_invented_unavailable_evidence != rows_no_invented_unavailable_evidence {
        push_unique(
            &mut blockers,
            "step6_browser_no_invented_unavailable_claim_mismatch",
        );
    }
    if report.ui_status_from_structured_evidence != rows_ui_status_from_structured_evidence {
        push_unique(
            &mut blockers,
            "step6_browser_ui_status_structured_claim_mismatch",
        );
    }
    BrowserAudit {
        environment_ready: report.browser_e2e_environment_ready && report.self_contained_runner,
        report_path: Some(report.report_path.clone()),
        no_silent_durable_write: report.no_silent_durable_write && rows_no_silent_durable_write,
        no_hidden_legacy_fallback: report.no_hidden_legacy_fallback
            && rows_no_hidden_legacy_fallback,
        no_local_fixture_marked_external_live: report.no_local_fixture_marked_external_live
            && rows_no_local_fixture_marked_external_live,
        no_invented_unavailable_evidence: report.no_invented_unavailable_evidence
            && rows_no_invented_unavailable_evidence,
        ui_status_from_structured_evidence: report.ui_status_from_structured_evidence
            && rows_ui_status_from_structured_evidence,
        blockers,
    }
}

fn build_journey_reports(
    report: Option<&Step6BrowserReport>,
    final_gate_summary: &Step6FinalGateSummary,
) -> Vec<Step6JourneyReport> {
    let observed = report
        .map(|report| {
            report
                .observed_journeys
                .iter()
                .map(|row| (row.journey_id.clone(), row.clone()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    STEP6_REQUIRED_JOURNEYS
        .iter()
        .map(|id| {
            let row = observed.get(*id);
            let report_marks_blocked_live = report
                .is_some_and(|report| report.blocked_live_journeys.iter().any(|row| row == id));
            let mut blockers = Vec::new();
            if let Some(row) = row {
                for blocker in single_journey_blockers(row) {
                    push_unique(&mut blockers, blocker);
                }
                if row_external_live_credit(row) && is_step6_external_live_id(id) {
                    let final_credit = match *id {
                        "S6-LIVE-WEB" => final_gate_summary.live_provider_web_credit,
                        "S6-LIVE-MCP" => final_gate_summary.live_provider_mcp_credit,
                        _ => false,
                    };
                    if !final_credit {
                        push_unique(
                            &mut blockers,
                            format!("step6_final_gate_live_credit_missing:{id}"),
                        );
                    }
                }
            } else {
                push_unique(&mut blockers, format!("step6_journey_missing:{id}"));
            }
            let credited = row.is_some_and(observed_journey_credited) && blockers.is_empty();
            let blocked_live_evidence_report = row
                .is_some_and(|row| row_blocked_live_report(row) && !row_external_live_credit(row))
                || (is_step6_external_live_id(id) && report_marks_blocked_live);
            Step6JourneyReport {
                journey_id: (*id).into(),
                status: if credited {
                    "passed"
                } else if blocked_live_evidence_report {
                    "blocked_live"
                } else {
                    "failed"
                }
                .into(),
                credited,
                blocked_live_evidence_report,
                evidence_source: row
                    .map(row_live_evidence_kind)
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        if blocked_live_evidence_report {
                            "blocked_external_live".into()
                        } else {
                            "missing".into()
                        }
                    }),
                answer_evidence_count: row.map(|row| row.answer_evidence.len()).unwrap_or(0),
                runtime_evidence_count: row.map(|row| row.runtime_evidence.len()).unwrap_or(0),
                ui_state_count: row
                    .map(|row| {
                        row.ui_status_evidence
                            .len()
                            .max(row.visible_ui_states.len())
                    })
                    .unwrap_or(0),
                final_delivery_section_count: row.map(row_final_delivery_count).unwrap_or(0),
                blockers: normalize_blockers(blockers),
            }
        })
        .collect()
}

fn browser_report_claim_blockers(
    report: Option<&Step6BrowserReport>,
    journeys: &[Step6JourneyReport],
    local_deterministic_ready: bool,
    external_live_ready: bool,
    existing_blockers: &[String],
) -> Vec<String> {
    let Some(report) = report else {
        return Vec::new();
    };
    let mut blockers = Vec::new();
    let passed_journeys = journey_ids_matching(journeys, |row| row.credited);
    let blocked_live_journeys = journey_ids_matching(journeys, |row| {
        row.blocked_live_evidence_report && !row.credited
    });
    let failed_journeys = journey_ids_matching(journeys, |row| {
        !row.credited && !row.blocked_live_evidence_report
    });
    if report.passed_journeys != passed_journeys {
        push_unique(&mut blockers, "step6_browser_passed_journeys_mismatch");
    }
    if report.blocked_live_journeys != blocked_live_journeys {
        push_unique(
            &mut blockers,
            "step6_browser_blocked_live_journeys_mismatch",
        );
    }
    if report.failed_journeys != failed_journeys {
        push_unique(&mut blockers, "step6_browser_failed_journeys_mismatch");
    }
    if report.local_deterministic_ready != local_deterministic_ready {
        push_unique(&mut blockers, "step6_browser_local_ready_claim_mismatch");
    }
    if report.external_live_ready != external_live_ready {
        push_unique(&mut blockers, "step6_browser_external_ready_claim_mismatch");
    }
    let browser_overall_ready = local_deterministic_ready
        && external_live_ready
        && report.browser_e2e_environment_ready
        && report.self_contained_runner
        && report.no_silent_durable_write
        && report.no_hidden_legacy_fallback
        && report.no_local_fixture_marked_external_live
        && report.no_invented_unavailable_evidence
        && report.ui_status_from_structured_evidence
        && existing_blockers.is_empty()
        && blockers.is_empty();
    if report.overall_ready != browser_overall_ready {
        push_unique(&mut blockers, "step6_browser_overall_ready_claim_mismatch");
    }
    blockers
}

fn journey_ids_matching(
    journeys: &[Step6JourneyReport],
    mut predicate: impl FnMut(&Step6JourneyReport) -> bool,
) -> Vec<String> {
    journeys
        .iter()
        .filter(|row| predicate(row))
        .map(|row| row.journey_id.clone())
        .collect()
}

fn observed_journey_blockers(rows: &[Step6ObservedJourney]) -> Vec<String> {
    let mut blockers = Vec::new();
    let observed_ids = rows
        .iter()
        .map(|row| row.journey_id.clone())
        .collect::<Vec<_>>();
    let required_ids = ids_vec(&STEP6_REQUIRED_JOURNEYS);
    if observed_ids.len() != required_ids.len() {
        push_unique(&mut blockers, "step6_observed_journey_count_mismatch");
    }
    if observed_ids != required_ids {
        push_unique(&mut blockers, "step6_observed_journey_order_mismatch");
    }
    let observed_set = observed_ids.iter().cloned().collect::<BTreeSet<_>>();
    let required_set = required_ids.iter().cloned().collect::<BTreeSet<_>>();
    if observed_set != required_set {
        push_unique(&mut blockers, "step6_observed_journeys_incomplete");
    }
    for id in observed_ids.iter().filter(|id| {
        observed_ids
            .iter()
            .filter(|candidate| *candidate == *id)
            .count()
            > 1
    }) {
        push_unique(&mut blockers, format!("step6_duplicate_journey:{id}"));
    }
    let runtime_identity_rows = rows
        .iter()
        .filter(|row| !(is_step6_external_live_id(&row.journey_id) && row_blocked_live_report(row)))
        .collect::<Vec<_>>();
    let task_session_ids = runtime_identity_rows
        .iter()
        .filter_map(|row| {
            if row.task_session_id.is_empty() {
                None
            } else {
                Some(row.task_session_id.clone())
            }
        })
        .collect::<Vec<_>>();
    let distinct_task_session_ids = task_session_ids.iter().cloned().collect::<BTreeSet<_>>();
    if distinct_task_session_ids.len() != task_session_ids.len() {
        push_unique(
            &mut blockers,
            "step6_observed_task_session_ids_not_distinct",
        );
    }
    let run_ids = runtime_identity_rows
        .iter()
        .filter_map(|row| {
            if row.run_id.is_empty() {
                None
            } else {
                Some(row.run_id.clone())
            }
        })
        .collect::<Vec<_>>();
    let distinct_run_ids = run_ids.iter().cloned().collect::<BTreeSet<_>>();
    if distinct_run_ids.len() != run_ids.len() {
        push_unique(&mut blockers, "step6_observed_run_ids_not_distinct");
    }
    for row in rows {
        for blocker in single_journey_blockers(row) {
            push_unique(&mut blockers, blocker);
        }
    }
    blockers
}

fn single_journey_blockers(row: &Step6ObservedJourney) -> Vec<String> {
    let mut blockers = Vec::new();
    if !STEP6_REQUIRED_JOURNEYS.contains(&row.journey_id.as_str()) {
        push_unique(&mut blockers, "step6_unknown_journey");
        return blockers;
    }
    let external = is_step6_external_live_id(&row.journey_id);
    let expected = step6_expected_journey_evidence(&row.journey_id);
    let blocked_external_live_report =
        external && row_blocked_live_report(row) && !row_external_live_credit(row);
    if row.kind != expected.kind {
        push_unique(
            &mut blockers,
            format!("step6_kind_mismatch:{}", row.journey_id),
        );
    }
    if !metadata_safe_label(&row.observed_via) {
        push_unique(
            &mut blockers,
            format!("step6_observed_via_unsafe:{}", row.journey_id),
        );
    }
    if !metadata_safe_label(&row.entry_point) {
        push_unique(
            &mut blockers,
            format!("step6_entry_point_unsafe:{}", row.journey_id),
        );
    }
    let expected_entry_point =
        step6_expected_entry_point(&row.journey_id, blocked_external_live_report);
    if row.entry_point != expected_entry_point {
        push_unique(
            &mut blockers,
            format!("step6_entry_point_mismatch:{}", row.journey_id),
        );
    }
    if !metadata_safe_label(&row.route_strategy) {
        push_unique(
            &mut blockers,
            format!("step6_route_unsafe:{}", row.journey_id),
        );
    } else if step6_route_strategy_mentions_hidden_fallback(&row.route_strategy) {
        push_unique(
            &mut blockers,
            format!("step6_route_legacy_or_fallback:{}", row.journey_id),
        );
    }
    if blocked_external_live_report && row.route_strategy != "blocked_external_live" {
        push_unique(
            &mut blockers,
            format!("step6_route_strategy_mismatch:{}", row.journey_id),
        );
    }
    if !external {
        if row.observed_via != "real_tauri_chat_or_control_path" {
            push_unique(
                &mut blockers,
                format!("step6_local_not_real_tauri_observed:{}", row.journey_id),
            );
        }
        if row.local_fixture_credited_as_external_live {
            push_unique(
                &mut blockers,
                format!("step6_local_fixture_credited_as_live:{}", row.journey_id),
            );
        }
        if row.external_live_status != "not_applicable" {
            push_unique(
                &mut blockers,
                format!("step6_local_journey_has_live_status:{}", row.journey_id),
            );
        }
        if row.external_live_provider_kind.is_some() {
            push_unique(
                &mut blockers,
                format!("step6_local_journey_has_provider_kind:{}", row.journey_id),
            );
        }
        if !step6_metadata_safe_runtime_id(&row.task_session_id) {
            push_unique(
                &mut blockers,
                format!("step6_task_session_missing:{}", row.journey_id),
            );
        }
        if !step6_metadata_safe_runtime_id(&row.run_id) {
            push_unique(
                &mut blockers,
                format!("step6_run_missing:{}", row.journey_id),
            );
        }
    }
    if blocked_external_live_report {
        if row.observed_via != "blocked_live_evidence_report" {
            push_unique(
                &mut blockers,
                format!("step6_blocked_live_not_reported:{}", row.journey_id),
            );
        }
        if row.blockers.is_empty() {
            push_unique(
                &mut blockers,
                format!("step6_blocked_live_missing_blocker:{}", row.journey_id),
            );
        }
        if !row
            .ui_status_evidence
            .iter()
            .any(|value| value == STEP6_BLOCKED_LIVE_UI_STATUS)
        {
            push_unique(
                &mut blockers,
                format!("step6_blocked_live_ui_status_missing:{}", row.journey_id),
            );
        }
        if !row_no_invented_unavailable_evidence(row) {
            push_unique(
                &mut blockers,
                format!("step6_invented_unavailable_evidence:{}", row.journey_id),
            );
        }
    } else {
        if external && row.observed_via != "real_tauri_chat_or_control_path" {
            push_unique(
                &mut blockers,
                format!("step6_live_not_real_tauri_observed:{}", row.journey_id),
            );
        }
        for (ok, label) in [
            (row_answer_observed(row), "step6_answer_missing"),
            (row_runtime_evidence_observed(row), "step6_runtime_missing"),
            (row_has_structured_ui_status(row), "step6_ui_state_missing"),
            (
                row_final_delivery_observed(row),
                "step6_final_delivery_missing",
            ),
            (
                row_non_fake_evidence_observed(row),
                "step6_non_fake_missing",
            ),
            (
                row_no_invented_unavailable_evidence(row),
                "step6_invented_unavailable_evidence",
            ),
        ] {
            if !ok {
                push_unique(&mut blockers, format!("{label}:{}", row.journey_id));
            }
        }
        for label in expected.answer_evidence {
            if !row.answer_evidence.iter().any(|value| value == label) {
                push_unique(
                    &mut blockers,
                    format!("step6_answer_evidence_missing:{}:{label}", row.journey_id),
                );
            }
        }
        for label in expected.runtime_evidence {
            if !row.runtime_evidence.iter().any(|value| value == label) {
                push_unique(
                    &mut blockers,
                    format!("step6_runtime_evidence_missing:{}:{label}", row.journey_id),
                );
            }
        }
        if !expected
            .ui_status
            .iter()
            .any(|label| row.ui_status_evidence.iter().any(|value| value == label))
        {
            push_unique(
                &mut blockers,
                format!("step6_ui_status_missing:{}", row.journey_id),
            );
        }
        if !expected.final_delivery.iter().any(|label| {
            row.final_delivery_sections
                .iter()
                .any(|value| value == label)
        }) {
            push_unique(
                &mut blockers,
                format!("step6_final_delivery_section_missing:{}", row.journey_id),
            );
        }
    }
    if row.legacy_fallback_used {
        push_unique(
            &mut blockers,
            format!("step6_legacy_fallback:{}", row.journey_id),
        );
    }
    if row.silent_durable_write_detected {
        push_unique(
            &mut blockers,
            format!("step6_silent_write:{}", row.journey_id),
        );
    }
    if row.fake_execution_detected {
        push_unique(
            &mut blockers,
            format!("step6_fake_execution:{}", row.journey_id),
        );
    }
    for (unsafe_labels, label) in [
        (
            has_unsafe_label(&row.answer_evidence),
            "step6_answer_label_unsafe",
        ),
        (
            has_unsafe_label(&row.runtime_evidence),
            "step6_runtime_label_unsafe",
        ),
        (
            has_unsafe_label(&row.visible_ui_states),
            "step6_ui_label_unsafe",
        ),
        (
            has_unsafe_label(&row.ui_status_evidence),
            "step6_ui_status_unsafe",
        ),
        (
            has_unsafe_label(&row.final_delivery_sections),
            "step6_final_label_unsafe",
        ),
        (
            has_unsafe_label(&row.trace_evidence),
            "step6_trace_evidence_unsafe",
        ),
        (
            has_unsafe_label(&row.blockers),
            "step6_blocker_label_unsafe",
        ),
    ] {
        if unsafe_labels {
            push_unique(&mut blockers, format!("{label}:{}", row.journey_id));
        }
    }
    if external {
        let provider_kind = row.external_live_provider_kind.as_deref();
        if row.local_fixture_credited_as_external_live {
            push_unique(
                &mut blockers,
                format!("step6_local_fixture_credited_as_live:{}", row.journey_id),
            );
        }
        if row_external_live_credit(row) && row.external_live_status != "credited_external_live" {
            push_unique(
                &mut blockers,
                format!("step6_live_evidence_missing:{}", row.journey_id),
            );
        }
        if row_external_live_credit(row) && provider_kind != Some("external_provider") {
            push_unique(
                &mut blockers,
                format!("step6_external_provider_missing:{}", row.journey_id),
            );
        }
        if row_external_live_credit(row) && row_live_evidence_kind(row) != "external_live_provider"
        {
            push_unique(
                &mut blockers,
                format!("step6_fake_external_live_credit:{}", row.journey_id),
            );
        }
        if row_external_live_credit(row) {
            if !step6_metadata_safe_runtime_id(&row.task_session_id) {
                push_unique(
                    &mut blockers,
                    format!("step6_task_session_missing:{}", row.journey_id),
                );
            }
            if !step6_metadata_safe_runtime_id(&row.run_id) {
                push_unique(
                    &mut blockers,
                    format!("step6_run_missing:{}", row.journey_id),
                );
            }
        }
        let blocked_report = row_blocked_live_report(row);
        if !row_external_live_credit(row) && !blocked_report {
            push_unique(
                &mut blockers,
                format!(
                    "step6_live_journey_missing_credit_or_blocked_report:{}",
                    row.journey_id
                ),
            );
        }
    } else if row_live_evidence_kind(row) != "local_deterministic" {
        push_unique(
            &mut blockers,
            format!("step6_local_journey_live_kind_invalid:{}", row.journey_id),
        );
    }
    blockers
}

fn row_answer_observed(row: &Step6ObservedJourney) -> bool {
    row.answer_observed || !row.answer_evidence.is_empty()
}

fn step6_metadata_safe_runtime_id(value: &str) -> bool {
    metadata_safe_label(value)
        && !value.starts_with("step6_task_")
        && !value.starts_with("step6_run_")
        && !value.starts_with("stage1_task_")
        && !value.starts_with("stage1_run_")
}

fn row_runtime_evidence_observed(row: &Step6ObservedJourney) -> bool {
    row.runtime_evidence_observed || !row.runtime_evidence.is_empty()
}

fn row_has_structured_ui_status(row: &Step6ObservedJourney) -> bool {
    !row.ui_status_evidence.is_empty()
}

fn row_final_delivery_observed(row: &Step6ObservedJourney) -> bool {
    !row.final_delivery_sections.is_empty()
}

fn row_final_delivery_count(row: &Step6ObservedJourney) -> usize {
    row.final_delivery_sections.len()
}

fn row_non_fake_evidence_observed(row: &Step6ObservedJourney) -> bool {
    row.non_fake_evidence_observed
        || row_answer_observed(row)
            && row_runtime_evidence_observed(row)
            && row_has_structured_ui_status(row)
}

fn row_no_invented_unavailable_evidence(row: &Step6ObservedJourney) -> bool {
    (row.no_invented_unavailable_evidence || !row.unavailable_evidence_invented)
        && !row.unavailable_evidence_invented
}

fn row_external_live_credit(row: &Step6ObservedJourney) -> bool {
    match row.external_live_status.as_str() {
        "credited_external_live" => true,
        "blocked_live_evidence" => false,
        _ => row.external_live_credit,
    }
}

fn row_blocked_live_report(row: &Step6ObservedJourney) -> bool {
    row.blocked_live_evidence_report || row.external_live_status == "blocked_live_evidence"
}

fn row_live_evidence_kind(row: &Step6ObservedJourney) -> &str {
    if !row.live_evidence_kind.is_empty() {
        return row.live_evidence_kind.as_str();
    }
    if is_step6_external_live_id(&row.journey_id) {
        if row_external_live_credit(row) {
            "external_live_provider"
        } else if row_blocked_live_report(row) {
            "blocked_external_live"
        } else {
            "missing"
        }
    } else {
        "local_deterministic"
    }
}

fn observed_journey_credited(row: &Step6ObservedJourney) -> bool {
    if !single_journey_blockers(row).is_empty() {
        return false;
    }
    if is_step6_external_live_id(&row.journey_id) {
        row_external_live_credit(row) && row_live_evidence_kind(row) == "external_live_provider"
    } else {
        row_live_evidence_kind(row) == "local_deterministic"
    }
}

fn all_ids_credited(journeys: &[Step6JourneyReport], ids: &[&str]) -> bool {
    ids.iter().all(|id| {
        journeys
            .iter()
            .any(|row| row.journey_id == *id && row.credited)
    })
}

fn is_step6_external_live_id(id: &str) -> bool {
    STEP6_EXTERNAL_LIVE_JOURNEYS
        .iter()
        .any(|candidate| *candidate == id)
}

struct Step6ExpectedJourneyEvidence {
    kind: &'static str,
    answer_evidence: &'static [&'static str],
    runtime_evidence: &'static [&'static str],
    ui_status: &'static [&'static str],
    final_delivery: &'static [&'static str],
}

fn step6_expected_journey_evidence(id: &str) -> Step6ExpectedJourneyEvidence {
    match id {
        "S6-CLOCK" => Step6ExpectedJourneyEvidence {
            kind: "deterministic_local",
            answer_evidence: &["answer.clock_value"],
            runtime_evidence: &["source.runtime_fact", "runtime.clock"],
            ui_status: &["completed"],
            final_delivery: &["completed_work", "completed_actions"],
        },
        "S6-ROUTE" => Step6ExpectedJourneyEvidence {
            kind: "deterministic_local",
            answer_evidence: &["answer.route_summary"],
            runtime_evidence: &["source.runtime_fact", "runtime.provider_route"],
            ui_status: &["completed"],
            final_delivery: &["completed_work", "completed_actions"],
        },
        "S6-TOOLS" => Step6ExpectedJourneyEvidence {
            kind: "deterministic_local",
            answer_evidence: &["answer.tool_availability"],
            runtime_evidence: &["source.runtime_fact", "runtime.tool_availability"],
            ui_status: &["completed"],
            final_delivery: &["completed_work", "completed_actions"],
        },
        "S6-FILE" => Step6ExpectedJourneyEvidence {
            kind: "deterministic_local",
            answer_evidence: &["answer.file_summary"],
            runtime_evidence: &["tool.file_read", "observation.workspace_file"],
            ui_status: &["completed"],
            final_delivery: &["sources_used", "completed_work", "completed_actions"],
        },
        "S6-DIRECT-SELF" => Step6ExpectedJourneyEvidence {
            kind: "deterministic_local",
            answer_evidence: &["answer.direct_complete"],
            runtime_evidence: &[
                "source.model_or_direct_answer",
                "self_state.completed_response",
            ],
            ui_status: &["completed"],
            final_delivery: &["completed_work", "completed_actions"],
        },
        "S6-PROPOSAL" => Step6ExpectedJourneyEvidence {
            kind: "deterministic_local",
            answer_evidence: &["answer.proposal_pending"],
            runtime_evidence: &["proposal.created", "durable_write.not_completed"],
            ui_status: &["proposal_pending"],
            final_delivery: &["proposals_created", "pending_user_actions"],
        },
        "S6-BLOCKED" => Step6ExpectedJourneyEvidence {
            kind: "deterministic_local",
            answer_evidence: &["answer.blocked_next_action"],
            runtime_evidence: &["blocker.created", "safe_next_control"],
            ui_status: &["restricted", "blocked"],
            final_delivery: &["blocked_items", "next_steps", "pending_user_actions"],
        },
        "S6-PERMISSION" => Step6ExpectedJourneyEvidence {
            kind: "deterministic_local",
            answer_evidence: &["answer.permission_accepted"],
            runtime_evidence: &[
                "permission.pending",
                "review_action.visible",
                "permission.accepted",
                "automatic_resume_replay",
                "final_delivery.recorded",
            ],
            ui_status: &["completed"],
            final_delivery: &["completed_actions", "sources_used", "completed_work"],
        },
        "S6-LIVE-WEB" => Step6ExpectedJourneyEvidence {
            kind: "external_live",
            answer_evidence: &["answer.external_web_summary"],
            runtime_evidence: &["live_provider.external", "tool.web_read"],
            ui_status: &["completed"],
            final_delivery: &["sources_used", "completed_work", "completed_actions"],
        },
        "S6-LIVE-MCP" => Step6ExpectedJourneyEvidence {
            kind: "external_live",
            answer_evidence: &["answer.external_mcp_summary"],
            runtime_evidence: &[
                "live_provider.external",
                "tool.mcp_read",
                "provider_ranked_selection",
            ],
            ui_status: &["completed"],
            final_delivery: &["sources_used", "completed_work", "completed_actions"],
        },
        "S6-RECOVERY" => Step6ExpectedJourneyEvidence {
            kind: "deterministic_local",
            answer_evidence: &["answer.recovery_or_stop"],
            runtime_evidence: &["control.retry_or_cancel", "final_delivery.recorded"],
            ui_status: &["completed", "blocked", "cancelled"],
            final_delivery: &[
                "blocked_items",
                "next_steps",
                "skipped_work",
                "completed_actions",
                "completed_work",
            ],
        },
        _ => Step6ExpectedJourneyEvidence {
            kind: "",
            answer_evidence: &[],
            runtime_evidence: &[],
            ui_status: &[],
            final_delivery: &[],
        },
    }
}

fn step6_expected_entry_point(id: &str, blocked_external_live_report: bool) -> &'static str {
    if blocked_external_live_report {
        return "blocked_live_evidence_report";
    }
    match id {
        "S6-PERMISSION" | "S6-RECOVERY" => "task_continuity_control",
        _ => "ordinary_main_chat_input",
    }
}

fn step6_route_strategy_mentions_hidden_fallback(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    lowered.contains("legacy") || lowered.contains("fallback")
}

fn repo_relative_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(path)
}

fn ids_vec(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_string()).collect()
}

fn digest_label_shape(value: &str) -> bool {
    let Some((bytes, hash)) = value.split_once(" hash:sha256:") else {
        return false;
    };
    bytes
        .strip_prefix("bytes:")
        .and_then(|count| count.parse::<usize>().ok())
        .is_some_and(|count| count > 0)
        && hash.len() == 64
        && hash.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn step6_browser_report_trace_is_fresh_and_bounded(report: &Step6BrowserReport) -> bool {
    if !metadata_safe_label(&report.run_id) {
        return false;
    }
    let Ok(generated_at) = chrono::DateTime::parse_from_rfc3339(&report.generated_at) else {
        return false;
    };
    let generated_at = generated_at.with_timezone(&chrono::Utc);
    let now = chrono::Utc::now();
    if generated_at > now + chrono::Duration::minutes(5) {
        return false;
    }
    if now.signed_duration_since(generated_at)
        > chrono::Duration::hours(STEP6_BROWSER_E2E_MAX_AGE_HOURS)
    {
        return false;
    }
    digest_label_shape(&report.report_digest)
}

fn step6_browser_report_digest_matches(report: &Step6BrowserReport) -> bool {
    step6_browser_report_digest(report) == report.report_digest
}

fn step6_browser_report_digest(report: &Step6BrowserReport) -> String {
    digest_label(step6_browser_report_digest_input(report).as_bytes())
}

fn step6_browser_report_digest_input(report: &Step6BrowserReport) -> String {
    let rows = report
        .observed_journeys
        .iter()
        .map(|row| {
            [
                digest_part(&row.journey_id),
                digest_part(&row.kind),
                digest_part(&row.observed_via),
                digest_part(&row.entry_point),
                digest_part(&row.route_strategy),
                digest_part(&row.task_session_id),
                digest_part(&row.run_id),
                digest_part(&digest_array(&row.answer_evidence)),
                digest_part(&digest_array(&row.runtime_evidence)),
                digest_part(&digest_array(&row.ui_status_evidence)),
                digest_part(&digest_array(&row.final_delivery_sections)),
                digest_part(&digest_array(&row.trace_evidence)),
                digest_part(bool_label(row.unavailable_evidence_invented)),
                digest_part(bool_label(row.legacy_fallback_used)),
                digest_part(bool_label(row.silent_durable_write_detected)),
                digest_part(bool_label(row.local_fixture_credited_as_external_live)),
                digest_part(&row.external_live_status),
                digest_part(row.external_live_provider_kind.as_deref().unwrap_or("")),
                digest_part(&digest_array(&row.blockers)),
            ]
            .join("|")
        })
        .collect::<Vec<_>>()
        .join("\n");

    [
        "step6-product-acceptance-report-v1".into(),
        format!("reportKind={}", digest_part(&report.report_kind)),
        format!("schema={}", digest_part(&report.schema_version)),
        format!("readiness={}", digest_part(&report.readiness_semantics)),
        format!(
            "e2eEnvironmentReady={}",
            bool_label(report.browser_e2e_environment_ready)
        ),
        format!(
            "selfContainedRunner={}",
            bool_label(report.self_contained_runner)
        ),
        format!("smokePassed={}", bool_label(report.smoke_passed)),
        format!("reportPath={}", digest_part(&report.report_path)),
        format!("source={}", digest_part(&report.evidence_source)),
        format!("runId={}", digest_part(&report.run_id)),
        format!("generatedAt={}", digest_part(&report.generated_at)),
        format!("localJourneyCount={}", report.local_journey_count),
        format!(
            "externalLiveJourneyCount={}",
            report.external_live_journey_count
        ),
        format!(
            "localDeterministicReady={}",
            bool_label(report.local_deterministic_ready)
        ),
        format!(
            "externalLiveReady={}",
            bool_label(report.external_live_ready)
        ),
        format!("acceptanceReady={}", bool_label(report.overall_ready)),
        format!("required={}", digest_array(&report.required_journeys)),
        format!("passed={}", digest_array(&report.passed_journeys)),
        format!(
            "blockedLive={}",
            digest_array(&report.blocked_live_journeys)
        ),
        format!("failed={}", digest_array(&report.failed_journeys)),
        format!(
            "externalLiveBlockers={}",
            digest_array(&report.external_live_blockers)
        ),
        format!("blockers={}", digest_array(&report.blockers)),
        format!(
            "noSilentDurableWrite={}",
            bool_label(report.no_silent_durable_write)
        ),
        format!(
            "noHiddenLegacyFallback={}",
            bool_label(report.no_hidden_legacy_fallback)
        ),
        format!(
            "noLocalEvidenceCreditedAsExternalLive={}",
            bool_label(report.no_local_fixture_marked_external_live)
        ),
        format!(
            "noInventedUnavailableEvidence={}",
            bool_label(report.no_invented_unavailable_evidence)
        ),
        format!(
            "uiStatusFromStructuredEvidence={}",
            bool_label(report.ui_status_from_structured_evidence)
        ),
        "observed:".into(),
        rows,
    ]
    .join("\n")
}

fn digest_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| digest_part(value))
        .collect::<Vec<_>>()
        .join(",")
}

fn digest_part(value: &str) -> String {
    format!("{}:{value}", value.len())
}

fn bool_label(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn digest_label(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("bytes:{} hash:sha256:{:x}", bytes.len(), hasher.finalize())
}

fn metadata_safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.trim() == value
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/'))
}

fn metadata_safe_or_redacted(value: &str) -> String {
    if metadata_safe_label(value) {
        value.to_string()
    } else if value.trim().is_empty() {
        "missing".into()
    } else {
        "redacted_unsafe_label".into()
    }
}

fn metadata_safe_blocker(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value.trim() == value
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/'))
}

fn non_empty_trimmed_or(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.into()
    } else {
        trimmed.into()
    }
}

fn has_unsafe_label(values: &[String]) -> bool {
    values.iter().any(|value| !metadata_safe_label(value))
}

fn normalize_blockers(values: Vec<String>) -> Vec<String> {
    values.into_iter().fold(Vec::new(), |mut acc, value| {
        let normalized = value
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/') {
                    ch
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .chars()
            .take(160)
            .collect::<String>();
        if !normalized.is_empty() {
            push_unique(&mut acc, normalized);
        }
        acc
    })
}

fn push_unique(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn error_code(error: &str) -> String {
    let lowered = error.to_ascii_lowercase();
    if lowered.contains("provider") {
        "provider".into()
    } else if lowered.contains("scheduler") {
        "scheduler".into()
    } else if lowered.contains("state") {
        "state".into()
    } else {
        "unknown".into()
    }
}

#[cfg(test)]
pub(crate) fn build_step6_product_acceptance_report_for_tests(
    browser_report: Option<Step6BrowserReport>,
    final_gate_summary: Step6FinalGateSummary,
) -> MainChatStep6ProductAcceptanceReport {
    build_step6_product_acceptance_report(browser_report, final_gate_summary)
}

#[cfg(test)]
pub(crate) fn clean_step6_final_gate_summary_for_tests() -> Step6FinalGateSummary {
    Step6FinalGateSummary {
        collected: true,
        final_acceptance_ready: true,
        final_acceptance_blockers: Vec::new(),
        command_surface_legacy_fallback_count: 0,
        command_surface_silent_write_count: 0,
        live_provider_attempted: true,
        live_provider_ready_count: 2,
        live_provider_web_credit: true,
        live_provider_mcp_credit: true,
        live_provider_scenario_reports: Vec::new(),
        live_provider_blockers: Vec::new(),
        blockers: Vec::new(),
    }
}

#[cfg(test)]
pub(crate) fn step6_observed_journey_for_tests(id: &str) -> Step6ObservedJourney {
    let external = is_step6_external_live_id(id);
    let expected = step6_expected_journey_evidence(id);
    Step6ObservedJourney {
        journey_id: id.into(),
        kind: expected.kind.into(),
        observed_via: "real_tauri_chat_or_control_path".into(),
        entry_point: step6_expected_entry_point(id, false).into(),
        task_session_id: format!("step6-task-{id}"),
        run_id: format!("step6-run-{id}"),
        route_strategy: if matches!(id, "S6-PERMISSION" | "S6-RECOVERY") {
            "task_continuity_control"
        } else if external {
            "external_live_provider"
        } else {
            "main_chat_kernel"
        }
        .into(),
        answer_evidence: expected
            .answer_evidence
            .iter()
            .map(|label| (*label).to_string())
            .collect(),
        runtime_evidence: expected
            .runtime_evidence
            .iter()
            .map(|label| (*label).to_string())
            .collect(),
        visible_ui_states: vec![expected
            .ui_status
            .first()
            .copied()
            .unwrap_or("completed")
            .into()],
        ui_status_evidence: expected
            .ui_status
            .iter()
            .map(|label| (*label).to_string())
            .collect(),
        final_delivery_sections: vec![expected
            .final_delivery
            .first()
            .copied()
            .unwrap_or("completed_work")
            .into()],
        trace_evidence: vec!["structured_trace".into()],
        visible_blockers: Vec::new(),
        blockers: Vec::new(),
        answer_observed: true,
        runtime_evidence_observed: true,
        ui_state_observed: true,
        final_delivery_observed: true,
        non_fake_evidence_observed: true,
        no_invented_unavailable_evidence: true,
        unavailable_evidence_invented: false,
        legacy_fallback_used: false,
        silent_durable_write_detected: false,
        fake_execution_detected: false,
        live_evidence_kind: if external {
            "external_live_provider"
        } else {
            "local_deterministic"
        }
        .into(),
        external_live_credit: external,
        blocked_live_evidence_report: false,
        local_fixture_credited_as_external_live: false,
        external_live_status: if external {
            "credited_external_live"
        } else {
            "not_applicable"
        }
        .into(),
        external_live_provider_kind: external.then(|| "external_provider".into()),
    }
}

#[cfg(test)]
pub(crate) fn step6_browser_report_for_tests(
    observed_journeys: Vec<Step6ObservedJourney>,
) -> Step6BrowserReport {
    let generated_at = chrono::Utc::now().to_rfc3339();
    let mut report = Step6BrowserReport {
        report_kind: "main_chat_step6_product_acceptance_browser_report".into(),
        schema_version: STEP6_SCHEMA_VERSION.into(),
        readiness_semantics: STEP6_READINESS_SEMANTICS.into(),
        browser_e2e_environment_ready: true,
        self_contained_runner: true,
        smoke_passed: true,
        report_path: STEP6_BROWSER_REPORT_PATH.into(),
        evidence_source: STEP6_OBSERVED_EVIDENCE_SOURCE.into(),
        run_id: "step6-browser-e2e-real-test".into(),
        generated_at,
        report_digest: String::new(),
        required_journeys: ids_vec(&STEP6_REQUIRED_JOURNEYS),
        local_journey_count: STEP6_LOCAL_JOURNEYS.len(),
        local_journeys: ids_vec(&STEP6_LOCAL_JOURNEYS),
        external_live_journey_count: STEP6_EXTERNAL_LIVE_JOURNEYS.len(),
        external_live_journeys: ids_vec(&STEP6_EXTERNAL_LIVE_JOURNEYS),
        passed_journeys: ids_vec(&STEP6_REQUIRED_JOURNEYS),
        blocked_live_journeys: Vec::new(),
        failed_journeys: Vec::new(),
        local_deterministic_ready: true,
        external_live_ready: true,
        overall_ready: true,
        no_silent_durable_write: true,
        no_hidden_legacy_fallback: true,
        no_local_fixture_marked_external_live: true,
        no_invented_unavailable_evidence: true,
        ui_status_from_structured_evidence: true,
        observed_journeys,
        external_live_blockers: Vec::new(),
        blockers: Vec::new(),
    };
    report.report_digest = step6_browser_report_digest(&report);
    report
}

#[cfg(test)]
pub(crate) fn refresh_step6_browser_report_digest_for_tests(report: &mut Step6BrowserReport) {
    report.report_digest = step6_browser_report_digest(report);
}
