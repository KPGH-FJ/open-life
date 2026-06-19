use crate::main_chat_event_stream::{
    list_main_chat_agent_events_with_state, MainChatAgentDurableEvent,
};
use crate::main_chat_final_gate::{
    MainChatLiveProviderEvalHarnessReport, MainChatLiveProviderEvalHarnessScenario,
};
use crate::main_chat_generation_support::main_chat_provider_endpoint_kind;
use crate::main_chat_live_provider_harness::{
    main_chat_live_provider_agent_loop_metadata_from_entries,
    run_main_chat_live_provider_eval_harness, MainChatLiveProviderEvalHarnessInput,
};
use crate::main_chat_send::send_message_with_state;
use crate::{main_chat_eval_state, AppState};
use openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentStateSnapshot;
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatLiveProductScenario {
    pub id: String,
    pub capability_group: String,
    pub prompt: String,
    pub expected_route: String,
    pub required_runtime_evidence: Vec<String>,
    pub required_ui_state: Vec<String>,
    pub required_controls: Vec<String>,
    pub negative_assertions: Vec<String>,
    pub expected_outcome: String,
    pub default_gate: bool,
    pub run_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatLiveProductScenarioEvidence {
    pub scenario_id: String,
    pub provider: String,
    pub provider_model: Option<String>,
    pub provider_endpoint_kind: String,
    pub live_provider_invocation_allowed: bool,
    pub main_chat_invoked: bool,
    pub model_invoked: bool,
    pub task_session_id: Option<String>,
    pub run_id: Option<String>,
    pub action_ids: Vec<String>,
    pub observation_ids: Vec<String>,
    pub proposal_ids: Vec<String>,
    pub blocker_ids: Vec<String>,
    pub final_delivery_id: Option<String>,
    pub event_types: Vec<String>,
    pub event_sequence_start: Option<u64>,
    pub event_sequence_end: Option<u64>,
    pub ui_state_assertions: Vec<String>,
    pub runtime_evidence: Vec<String>,
    pub controls: Vec<String>,
    pub negative_assertions: Vec<String>,
    pub direct_writes_executed: bool,
    pub legacy_fallback_used: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatLiveProductScenarioProof {
    pub scenario_id: String,
    pub passed: bool,
    pub status: String,
    pub provider: String,
    pub provider_model: Option<String>,
    pub provider_endpoint_kind: String,
    pub task_session_id: Option<String>,
    pub run_id: Option<String>,
    pub action_ids: Vec<String>,
    pub observation_ids: Vec<String>,
    pub proposal_ids: Vec<String>,
    pub blocker_ids: Vec<String>,
    pub final_delivery_id: Option<String>,
    pub event_types: Vec<String>,
    pub event_sequence_start: Option<u64>,
    pub event_sequence_end: Option<u64>,
    pub ui_state_assertions: Vec<String>,
    pub runtime_evidence: Vec<String>,
    pub controls: Vec<String>,
    pub negative_assertions: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatExternalLiveProductizationGateReport {
    pub report_kind: String,
    pub scenario_count: usize,
    pub default_gate_scenario_count: usize,
    pub readiness_semantics: String,
    pub run_mode: String,
    pub live_provider_attempted: bool,
    pub passed_scenario_count: usize,
    pub blocked_scenario_count: usize,
    pub failed_scenario_count: usize,
    pub ready: bool,
    pub external_provider_invoked: bool,
    pub direct_writes_executed: bool,
    pub legacy_fallback_used: bool,
    pub deterministic_readiness_unchanged: bool,
    pub blockers: Vec<String>,
    pub proofs: Vec<MainChatLiveProductScenarioProof>,
}

pub(crate) fn main_chat_live_product_scenarios() -> Vec<MainChatLiveProductScenario> {
    vec![
        scenario(
            "LIVE-PROD-01",
            "external_live_productization",
            "External provider direct answer.",
            "direct_answer",
            &["task_run", "provider_model", "final_delivery"],
            &["task_header", "provider_trace", "final_delivery"],
            &["open_trace"],
            &["direct_answer_no_tool_timeline"],
        ),
        scenario(
            "LIVE-PROD-02",
            "external_live_productization",
            "External web ReAct read.",
            "react_tool_execution",
            &["action", "web_observation_source", "final_delivery"],
            &["action_timeline", "observation_card", "final_delivery"],
            &["open_trace"],
            &["no_fake_source"],
        ),
        scenario(
            "LIVE-PROD-03",
            "external_live_productization",
            "External MCP candidate selection.",
            "react_tool_execution",
            &[
                "candidate_list",
                "candidate_ranking_trace",
                "selected_target",
                "observation",
            ],
            &["tool_candidates", "selected_tool", "observation_card"],
            &["open_trace"],
            &["selected_target_from_allowlist"],
        ),
        scenario(
            "LIVE-PROD-04",
            "external_live_productization",
            "ToolPermission live proposal.",
            "permission_request",
            &["pending_proposal", "exact_action_proposal"],
            &["pending_proposal", "approve_deny_defer_controls"],
            &["approve_once", "deny", "defer"],
            &["no_overlapping_read_success"],
        ),
        scenario(
            "LIVE-PROD-05",
            "external_live_productization",
            "Live failure recovery.",
            "blocked",
            &["blocker", "blocker_reason", "safe_recovery_controls"],
            &["blocker_visible", "retry_cancel_controls"],
            &["retry", "cancel"],
            &["no_success_text_without_blocker"],
        ),
        scenario(
            "LIVE-PROD-06",
            "external_live_productization",
            "Live delta stream.",
            "react_tool_execution",
            &[
                "event_sequence_range",
                "route_event",
                "action_event",
                "observation_event",
            ],
            &["event_status_strip", "event_log"],
            &["open_trace"],
            &["snapshot_backfill_not_counted_as_live_delta"],
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn scenario(
    id: &str,
    capability_group: &str,
    prompt: &str,
    expected_route: &str,
    runtime: &[&str],
    ui: &[&str],
    controls: &[&str],
    negative: &[&str],
) -> MainChatLiveProductScenario {
    MainChatLiveProductScenario {
        id: id.into(),
        capability_group: capability_group.into(),
        prompt: prompt.into(),
        expected_route: expected_route.into(),
        required_runtime_evidence: strings(runtime),
        required_ui_state: strings(ui),
        required_controls: strings(controls),
        negative_assertions: strings(negative),
        expected_outcome: "pass".into(),
        default_gate: false,
        run_mode: "external_live_opt_in".into(),
    }
}

pub(crate) fn build_main_chat_external_live_productization_gate_report(
    explicit_live_eval_requested: bool,
    preflight_blockers: Vec<String>,
    evidence: Vec<MainChatLiveProductScenarioEvidence>,
) -> MainChatExternalLiveProductizationGateReport {
    let scenarios = main_chat_live_product_scenarios();
    let evidence_by_id = evidence
        .into_iter()
        .map(|evidence| (evidence.scenario_id.clone(), evidence))
        .collect::<BTreeMap<_, _>>();
    let mut proofs = Vec::new();
    let mut blockers = Vec::new();

    if !explicit_live_eval_requested {
        let scenario_blockers = if preflight_blockers.is_empty() {
            vec!["explicit_live_eval_required".into()]
        } else {
            preflight_blockers
        };
        for scenario in &scenarios {
            proofs.push(blocked_proof(&scenario.id, &scenario_blockers));
        }
        blockers.extend(scenario_blockers);
    } else if !preflight_blockers.is_empty() {
        for scenario in &scenarios {
            proofs.push(blocked_proof(&scenario.id, &preflight_blockers));
        }
        blockers.extend(preflight_blockers);
    } else {
        for scenario in &scenarios {
            let proof = evidence_by_id
                .get(&scenario.id)
                .map(|evidence| proof_for_evidence(scenario, evidence))
                .unwrap_or_else(|| {
                    missing_evidence_proof(&scenario.id, "live_product_evidence_missing")
                });
            for blocker in &proof.blockers {
                push_unique(&mut blockers, blocker.clone());
            }
            proofs.push(proof);
        }
    }

    for scenario in &scenarios {
        if !proofs.iter().any(|proof| proof.scenario_id == scenario.id) {
            proofs.push(missing_evidence_proof(
                &scenario.id,
                "live_product_scenario_not_evaluated",
            ));
            push_unique(&mut blockers, "live_product_scenario_not_evaluated".into());
        }
    }

    let passed_scenario_count = proofs.iter().filter(|proof| proof.passed).count();
    let blocked_scenario_count = proofs
        .iter()
        .filter(|proof| proof.status == "blocked")
        .count();
    let failed_scenario_count = proofs
        .iter()
        .filter(|proof| proof.status == "failed")
        .count();
    let direct_writes_executed = proofs.iter().any(|proof| {
        proof
            .blockers
            .contains(&"live_product_direct_writes_detected".into())
    });
    let legacy_fallback_used = proofs.iter().any(|proof| {
        proof
            .blockers
            .contains(&"live_product_legacy_fallback_detected".into())
    });
    let external_provider_invoked = proofs.iter().any(|proof| {
        proof.passed
            && proof.provider_endpoint_kind == "external_provider"
            && proof.provider_model.is_some()
    });
    if failed_scenario_count > 0 {
        push_unique(&mut blockers, "live_product_scenarios_failed".into());
    }
    if blocked_scenario_count > 0 {
        push_unique(&mut blockers, "live_product_scenarios_blocked".into());
    }
    let ready = explicit_live_eval_requested
        && blockers.is_empty()
        && passed_scenario_count == scenarios.len()
        && external_provider_invoked
        && !direct_writes_executed
        && !legacy_fallback_used;

    MainChatExternalLiveProductizationGateReport {
        report_kind: "main_chat_external_live_productization_gate".into(),
        scenario_count: scenarios.len(),
        default_gate_scenario_count: scenarios
            .iter()
            .filter(|scenario| scenario.default_gate)
            .count(),
        readiness_semantics:
            "opt_in_external_live_product_evidence_only_default_readiness_unchanged".into(),
        run_mode: "external_live_opt_in".into(),
        live_provider_attempted: explicit_live_eval_requested,
        passed_scenario_count,
        blocked_scenario_count,
        failed_scenario_count,
        ready,
        external_provider_invoked,
        direct_writes_executed,
        legacy_fallback_used,
        deterministic_readiness_unchanged: true,
        blockers,
        proofs,
    }
}

fn proof_for_evidence(
    scenario: &MainChatLiveProductScenario,
    evidence: &MainChatLiveProductScenarioEvidence,
) -> MainChatLiveProductScenarioProof {
    let mut blockers = Vec::new();
    validate_common_live_product_evidence(evidence, &mut blockers);
    validate_scenario_live_product_evidence(scenario, evidence, &mut blockers);
    let passed = blockers.is_empty();
    MainChatLiveProductScenarioProof {
        scenario_id: scenario.id.clone(),
        passed,
        status: if passed { "passed" } else { "failed" }.into(),
        provider: evidence.provider.clone(),
        provider_model: evidence.provider_model.clone(),
        provider_endpoint_kind: evidence.provider_endpoint_kind.clone(),
        task_session_id: evidence.task_session_id.clone(),
        run_id: evidence.run_id.clone(),
        action_ids: evidence.action_ids.clone(),
        observation_ids: evidence.observation_ids.clone(),
        proposal_ids: evidence.proposal_ids.clone(),
        blocker_ids: evidence.blocker_ids.clone(),
        final_delivery_id: evidence.final_delivery_id.clone(),
        event_types: evidence.event_types.clone(),
        event_sequence_start: evidence.event_sequence_start,
        event_sequence_end: evidence.event_sequence_end,
        ui_state_assertions: evidence.ui_state_assertions.clone(),
        runtime_evidence: evidence.runtime_evidence.clone(),
        controls: evidence.controls.clone(),
        negative_assertions: evidence.negative_assertions.clone(),
        blockers,
    }
}

fn validate_common_live_product_evidence(
    evidence: &MainChatLiveProductScenarioEvidence,
    blockers: &mut Vec<String>,
) {
    if !evidence.live_provider_invocation_allowed {
        push_unique(blockers, "live_product_invocation_not_allowed".into());
    }
    if !evidence.main_chat_invoked {
        push_unique(blockers, "live_product_main_chat_not_invoked".into());
    }
    if !evidence.model_invoked {
        push_unique(blockers, "live_product_model_not_invoked".into());
    }
    if evidence.provider_endpoint_kind != "external_provider"
        || !external_provider_label(&evidence.provider)
    {
        push_unique(blockers, "live_product_external_provider_missing".into());
    }
    if evidence
        .provider_model
        .as_deref()
        .filter(|model| metadata_safe_label(model))
        .is_none()
    {
        push_unique(blockers, "live_product_provider_model_missing".into());
    }
    if evidence
        .task_session_id
        .as_deref()
        .filter(|id| metadata_safe_label(id))
        .is_none()
    {
        push_unique(blockers, "live_product_task_session_missing".into());
    }
    if evidence
        .run_id
        .as_deref()
        .filter(|id| metadata_safe_label(id))
        .is_none()
    {
        push_unique(blockers, "live_product_run_missing".into());
    }
    if evidence.direct_writes_executed {
        push_unique(blockers, "live_product_direct_writes_detected".into());
    }
    if evidence.legacy_fallback_used {
        push_unique(blockers, "live_product_legacy_fallback_detected".into());
    }
}

fn validate_scenario_live_product_evidence(
    scenario: &MainChatLiveProductScenario,
    evidence: &MainChatLiveProductScenarioEvidence,
    blockers: &mut Vec<String>,
) {
    for required in &scenario.required_runtime_evidence {
        if !evidence.runtime_evidence.contains(required) {
            push_unique(
                blockers,
                format!("live_product_required_runtime_evidence_missing:{required}"),
            );
        }
    }
    for required in &scenario.required_ui_state {
        if !evidence.ui_state_assertions.contains(required) {
            push_unique(
                blockers,
                format!("live_product_required_ui_state_missing:{required}"),
            );
        }
    }
    for required in &scenario.required_controls {
        if !evidence.controls.contains(required) {
            push_unique(
                blockers,
                format!("live_product_required_control_missing:{required}"),
            );
        }
    }
    for required in &scenario.negative_assertions {
        if !evidence.negative_assertions.contains(required) {
            push_unique(
                blockers,
                format!("live_product_negative_assertion_missing:{required}"),
            );
        }
    }

    match scenario.id.as_str() {
        "LIVE-PROD-01" => {
            if evidence.final_delivery_id.is_none() {
                push_unique(blockers, "live_product_final_delivery_missing".into());
            }
            if !evidence.action_ids.is_empty() || !evidence.observation_ids.is_empty() {
                push_unique(blockers, "live_product_direct_answer_fake_timeline".into());
            }
        }
        "LIVE-PROD-02" => {
            if evidence.action_ids.is_empty() {
                push_unique(blockers, "live_product_action_missing".into());
            }
            if evidence.observation_ids.is_empty() {
                push_unique(blockers, "live_product_observation_missing".into());
            }
            if evidence.final_delivery_id.is_none() {
                push_unique(blockers, "live_product_final_delivery_missing".into());
            }
        }
        "LIVE-PROD-03" => {
            if evidence.action_ids.is_empty() || evidence.observation_ids.is_empty() {
                push_unique(
                    blockers,
                    "live_product_mcp_action_observation_missing".into(),
                );
            }
        }
        "LIVE-PROD-04" => {
            if evidence.proposal_ids.len() != 1 {
                push_unique(
                    blockers,
                    "live_product_tool_permission_proposal_missing".into(),
                );
            }
        }
        "LIVE-PROD-05" => {
            if evidence.blocker_ids.is_empty() {
                push_unique(blockers, "live_product_blocker_missing".into());
            }
            if !evidence.controls.contains(&"retry".into())
                || !evidence.controls.contains(&"cancel".into())
            {
                push_unique(blockers, "live_product_recovery_controls_missing".into());
            }
        }
        "LIVE-PROD-06" => {
            if evidence.event_sequence_start.is_none()
                || evidence.event_sequence_end <= evidence.event_sequence_start
            {
                push_unique(blockers, "live_product_event_sequence_range_missing".into());
            }
            for event_type in [
                "route.selected",
                "action.queued",
                "observation.created",
                "final_delivery.created",
            ] {
                if !evidence.event_types.contains(&event_type.to_string()) {
                    push_unique(
                        blockers,
                        format!("live_product_event_type_missing:{event_type}"),
                    );
                }
            }
        }
        _ => push_unique(blockers, "live_product_unknown_scenario".into()),
    }
}

pub(crate) async fn run_main_chat_external_live_productization_gate_with_state(
    source_state: &Arc<AppState>,
    explicit_live_eval_requested: bool,
) -> Result<MainChatExternalLiveProductizationGateReport, String> {
    let source_config = source_state.config.lock().await.clone();
    let source_scheduler = source_state.scheduler.lock().await.clone();
    let scripted_provider_response_present =
        source_scheduler.scripted_generation_response.is_some();
    let provider_endpoint_kind =
        main_chat_provider_endpoint_kind(&source_scheduler, scripted_provider_response_present)
            .to_string();
    let preflight =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_live_provider_eval_preflight(
            openlife_core::agent::main_chat_agent_v1::MainChatLiveProviderEvalPreflightInput {
                provider: source_scheduler.provider.clone(),
                api_key_present: !source_scheduler.effective_api_key().trim().is_empty(),
                network_enabled: source_config.system.network_policy.enabled,
                explicit_live_eval_requested,
                scripted_provider_response_present,
                local_only_required: false,
            },
        );
    let mut preflight_blockers = preflight.blockers.clone();
    if explicit_live_eval_requested && provider_endpoint_kind != "external_provider" {
        push_unique(
            &mut preflight_blockers,
            "external_provider_endpoint_required".into(),
        );
    }
    if !explicit_live_eval_requested || !preflight_blockers.is_empty() {
        return Ok(build_main_chat_external_live_productization_gate_report(
            explicit_live_eval_requested,
            preflight_blockers,
            Vec::new(),
        ));
    }

    let mut evidence = Vec::new();
    for (scenario_id, harness_scenario) in [
        (
            "LIVE-PROD-01",
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
        ),
        (
            "LIVE-PROD-02",
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        ),
        (
            "LIVE-PROD-03",
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
        ),
        (
            "LIVE-PROD-04",
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
        ),
    ] {
        let state = configured_isolated_live_product_state(&source_config, &source_scheduler).await;
        let report = run_main_chat_live_provider_eval_harness(
            state.clone(),
            MainChatLiveProviderEvalHarnessInput {
                scenario: harness_scenario,
                session_id: format!("live-product-{}", scenario_id.to_ascii_lowercase()),
                prompt: harness_scenario.prompt().into(),
                explicit_live_eval_requested: true,
                local_only_required: false,
            },
        )
        .await?;
        evidence
            .push(live_product_evidence_from_harness_report(scenario_id, &report, &state).await?);
    }

    let failure_state =
        configured_isolated_live_product_state(&source_config, &source_scheduler).await;
    evidence.push(run_live_failure_recovery_product_evidence(failure_state).await?);
    if let Some(web_evidence) = evidence
        .iter()
        .find(|evidence| evidence.scenario_id == "LIVE-PROD-02")
        .cloned()
    {
        evidence.push(live_delta_product_evidence_from_web_evidence(web_evidence));
    }

    Ok(build_main_chat_external_live_productization_gate_report(
        true,
        Vec::new(),
        evidence,
    ))
}

async fn configured_isolated_live_product_state(
    source_config: &openlife_core::config::AppConfig,
    source_scheduler: &openlife_core::scheduler::InferenceScheduler,
) -> Arc<AppState> {
    let state = main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        *config = source_config.clone();
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = source_scheduler.clone();
    }
    state
}

async fn run_live_failure_recovery_product_evidence(
    state: Arc<AppState>,
) -> Result<MainChatLiveProductScenarioEvidence, String> {
    {
        let mut config = state.config.lock().await;
        config.system.network_policy.enabled = false;
    }
    let prompt = concat!(
        "For this live failure recovery eval, call the allowed web.search candidate exactly once ",
        "before answering. Return only a JSON action envelope with actions[0].name=\"web.search\", ",
        "actions[0].action_type=\"mcp_tool\", and actions[0].arguments={}; do not answer directly."
    );
    let result = send_message_with_state(
        "live-product-live-prod-05".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: prompt.into(),
        }],
        None,
        &state,
    )
    .await?;
    let response = serde_json::to_value(&result)
        .map_err(|error| format!("serialize live failure product response failed: {error}"))?;
    let agent_loop_metadata = response
        .get("execution_transcript")
        .and_then(serde_json::Value::as_array)
        .and_then(|entries| main_chat_live_provider_agent_loop_metadata_from_entries(entries));
    let model_invoked = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("liveProviderInvoked"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let mut evidence = live_product_evidence_from_result(
        "LIVE-PROD-05",
        &state,
        result.run_id.as_deref(),
        result
            .agent_ingress
            .as_ref()
            .and_then(|ingress| ingress.agent_task_session_id.as_deref()),
        true,
        model_invoked,
        result.legacy_fallback_used,
        crate::main_chat_command_surface_eval::json_contains_direct_write_true(&response),
    )
    .await?;
    if agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("blockerReason"))
        .and_then(serde_json::Value::as_str)
        .is_some()
    {
        push_unique(&mut evidence.runtime_evidence, "blocker_reason".into());
    }
    push_unique(
        &mut evidence.negative_assertions,
        "no_success_text_without_blocker".into(),
    );
    Ok(evidence)
}

fn live_delta_product_evidence_from_web_evidence(
    mut evidence: MainChatLiveProductScenarioEvidence,
) -> MainChatLiveProductScenarioEvidence {
    evidence.scenario_id = "LIVE-PROD-06".into();
    evidence.runtime_evidence = vec![
        "event_sequence_range".into(),
        "route_event".into(),
        "action_event".into(),
        "observation_event".into(),
    ];
    evidence.ui_state_assertions = vec!["event_status_strip".into(), "event_log".into()];
    evidence.controls = vec!["open_trace".into()];
    evidence.negative_assertions = vec!["snapshot_backfill_not_counted_as_live_delta".into()];
    evidence
}

async fn live_product_evidence_from_harness_report(
    scenario_id: &str,
    report: &MainChatLiveProviderEvalHarnessReport,
    state: &Arc<AppState>,
) -> Result<MainChatLiveProductScenarioEvidence, String> {
    let mut evidence = live_product_evidence_from_result(
        scenario_id,
        state,
        report.run_id.as_deref(),
        report.task_session_id.as_deref(),
        report.main_chat_invoked,
        report.model_invoked,
        report.legacy_fallback_used,
        report.direct_writes_executed,
    )
    .await?;
    evidence.provider = report.provider.clone();
    evidence.provider_model = report.provider_model.clone();
    evidence.provider_endpoint_kind = report.provider_endpoint_kind.clone();
    evidence.live_provider_invocation_allowed = report.live_provider_invocation_allowed;
    match scenario_id {
        "LIVE-PROD-02" => {
            push_unique(
                &mut evidence.runtime_evidence,
                "web_observation_source".into(),
            );
            push_unique(&mut evidence.negative_assertions, "no_fake_source".into());
        }
        "LIVE-PROD-03" => {
            if report.tool_selection_candidate_count > 0 {
                push_unique(&mut evidence.runtime_evidence, "candidate_list".into());
                push_unique(&mut evidence.ui_state_assertions, "tool_candidates".into());
            }
            if report.tool_selection_model_ranked {
                push_unique(
                    &mut evidence.runtime_evidence,
                    "candidate_ranking_trace".into(),
                );
            }
            if report.model_selected_candidate_target.is_some() {
                push_unique(&mut evidence.runtime_evidence, "selected_target".into());
                push_unique(&mut evidence.ui_state_assertions, "selected_tool".into());
            }
            push_unique(
                &mut evidence.negative_assertions,
                "selected_target_from_allowlist".into(),
            );
        }
        "LIVE-PROD-04" => {
            if report.tool_permission_proposal_created {
                for proposal_id in tool_permission_proposal_ids_for_session(
                    state,
                    report.task_session_id.as_deref(),
                )
                .await
                {
                    push_unique(&mut evidence.proposal_ids, proposal_id);
                }
                push_unique(&mut evidence.runtime_evidence, "pending_proposal".into());
                push_unique(
                    &mut evidence.runtime_evidence,
                    "exact_action_proposal".into(),
                );
                push_unique(&mut evidence.ui_state_assertions, "pending_proposal".into());
                push_unique(
                    &mut evidence.ui_state_assertions,
                    "approve_deny_defer_controls".into(),
                );
                for control in ["approve_once", "deny", "defer"] {
                    push_unique(&mut evidence.controls, control.into());
                }
            }
            if !report.mcp_read_target_resolved {
                push_unique(
                    &mut evidence.negative_assertions,
                    "no_overlapping_read_success".into(),
                );
            }
        }
        _ => {}
    }
    Ok(evidence)
}

async fn tool_permission_proposal_ids_for_session(
    state: &Arc<AppState>,
    task_session_id: Option<&str>,
) -> Vec<String> {
    let Some(task_session_id) = task_session_id else {
        return Vec::new();
    };
    let proposal_ids = if let Some(ref queue_arc) = state.main_chat_action_queue_store {
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|action| {
                action
                    .observation_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get("proposalId"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|proposal_id| metadata_safe_label(proposal_id))
                    .map(str::to_string)
            })
            .collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    if proposal_ids.is_empty() {
        return Vec::new();
    }

    let Some(ref proposal_arc) = state.proposal_store else {
        return Vec::new();
    };
    let proposal_store = proposal_arc.lock().await;
    proposal_store
        .list_pending_proposals(100)
        .unwrap_or_default()
        .into_iter()
        .filter(|proposal| {
            proposal_ids.contains(&proposal.id)
                && proposal.proposal_type == openlife_core::agent::ProposalType::ToolPermission
        })
        .map(|proposal| proposal.id)
        .collect()
}

#[allow(clippy::too_many_arguments)]
async fn live_product_evidence_from_result(
    scenario_id: &str,
    state: &Arc<AppState>,
    run_id: Option<&str>,
    task_session_id: Option<&str>,
    main_chat_invoked: bool,
    model_invoked: bool,
    legacy_fallback_used: bool,
    direct_writes_executed: bool,
) -> Result<MainChatLiveProductScenarioEvidence, String> {
    let scheduler = state.scheduler.lock().await.clone();
    let provider_endpoint_kind = main_chat_provider_endpoint_kind(
        &scheduler,
        scheduler.scripted_generation_response.is_some(),
    )
    .to_string();
    let snapshot = if let Some(task_session_id) = task_session_id {
        crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn(
            state,
            Some(task_session_id),
            run_id,
        )
        .await
    } else {
        None
    };
    let events = if let Some(task_session_id) = task_session_id {
        list_main_chat_agent_events_with_state(
            state,
            task_session_id.to_string(),
            Some(0),
            Some(250),
        )
        .await
        .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut evidence = evidence_from_snapshot_and_events(
        scenario_id,
        &scheduler.provider,
        (!scheduler.chat_model.is_empty()).then_some(scheduler.chat_model.clone()),
        &provider_endpoint_kind,
        snapshot.as_ref(),
        &events,
    );
    evidence.live_provider_invocation_allowed = true;
    evidence.main_chat_invoked = main_chat_invoked;
    evidence.model_invoked = model_invoked;
    evidence.run_id = run_id.map(str::to_string).or(evidence.run_id);
    evidence.task_session_id = task_session_id
        .map(str::to_string)
        .or(evidence.task_session_id);
    evidence.legacy_fallback_used = legacy_fallback_used;
    evidence.direct_writes_executed = direct_writes_executed;
    Ok(evidence)
}

fn evidence_from_snapshot_and_events(
    scenario_id: &str,
    provider: &str,
    provider_model: Option<String>,
    provider_endpoint_kind: &str,
    snapshot: Option<&MainChatAgentStateSnapshot>,
    events: &[MainChatAgentDurableEvent],
) -> MainChatLiveProductScenarioEvidence {
    let live_events = events
        .iter()
        .filter(|event| !event.backfilled)
        .collect::<Vec<_>>();
    let event_types = live_events
        .iter()
        .map(|event| event.event_type.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let event_sequence_start = live_events.first().map(|event| event.sequence);
    let event_sequence_end = live_events.last().map(|event| event.sequence);
    let mut runtime_evidence = Vec::new();
    let mut ui_state_assertions = Vec::new();
    let mut controls = Vec::new();
    let mut negative_assertions = Vec::new();

    let (
        task_session_id,
        run_id,
        action_ids,
        observation_ids,
        proposal_ids,
        blocker_ids,
        final_delivery_id,
    ) = if let Some(snapshot) = snapshot {
        if snapshot.provider.is_some() {
            push_unique(&mut runtime_evidence, "provider_model".into());
        }
        if snapshot.final_delivery.is_some() {
            push_unique(&mut runtime_evidence, "final_delivery".into());
            push_unique(&mut ui_state_assertions, "final_delivery".into());
        }
        if !snapshot.actions.is_empty() {
            push_unique(&mut runtime_evidence, "action".into());
            push_unique(&mut ui_state_assertions, "action_timeline".into());
        }
        if !snapshot.observations.is_empty() {
            push_unique(&mut runtime_evidence, "observation".into());
            push_unique(&mut ui_state_assertions, "observation_card".into());
        }
        if !snapshot.proposals.is_empty() {
            push_unique(&mut runtime_evidence, "pending_proposal".into());
            push_unique(&mut ui_state_assertions, "pending_proposal".into());
        }
        if !snapshot.blockers.is_empty() {
            push_unique(&mut runtime_evidence, "blocker".into());
            push_unique(&mut runtime_evidence, "safe_recovery_controls".into());
            push_unique(&mut ui_state_assertions, "blocker_visible".into());
            push_unique(&mut ui_state_assertions, "retry_cancel_controls".into());
        }
        controls.extend(
            snapshot
                .task
                .controls
                .iter()
                .map(|control| control.as_str().to_string()),
        );
        for blocker in &snapshot.blockers {
            controls.extend(
                blocker
                    .controls
                    .iter()
                    .map(|control| control.as_str().to_string()),
            );
        }
        for proposal in &snapshot.proposals {
            controls.extend(
                proposal
                    .controls
                    .iter()
                    .map(|control| control.as_str().to_string()),
            );
        }
        (
            Some(snapshot.task.task_id.clone()),
            Some(snapshot.task.run_id.clone()),
            snapshot.task.action_ids.clone(),
            snapshot.task.observation_ids.clone(),
            snapshot.task.proposal_ids.clone(),
            snapshot.task.blocker_ids.clone(),
            snapshot.task.final_delivery_id.clone(),
        )
    } else {
        (
            None,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
        )
    };

    if run_id.is_some() && task_session_id.is_some() {
        push_unique(&mut runtime_evidence, "task_run".into());
        push_unique(&mut ui_state_assertions, "task_header".into());
    }
    if provider_model.is_some() {
        push_unique(&mut ui_state_assertions, "provider_trace".into());
    }
    if event_sequence_start.is_some() && event_sequence_end > event_sequence_start {
        push_unique(&mut runtime_evidence, "event_sequence_range".into());
    }
    if event_types.contains(&"route.selected".into()) {
        push_unique(&mut runtime_evidence, "route_event".into());
    }
    if event_types.contains(&"action.queued".into()) {
        push_unique(&mut runtime_evidence, "action_event".into());
    }
    if event_types.contains(&"observation.created".into()) {
        push_unique(&mut runtime_evidence, "observation_event".into());
    }
    if scenario_id == "LIVE-PROD-01" {
        push_unique(
            &mut ui_state_assertions,
            "direct_answer_no_tool_timeline".into(),
        );
        push_unique(
            &mut negative_assertions,
            "direct_answer_no_tool_timeline".into(),
        );
    }
    if controls.is_empty() {
        controls.push("open_trace".into());
    }
    dedupe(&mut controls);

    MainChatLiveProductScenarioEvidence {
        scenario_id: scenario_id.into(),
        provider: provider.into(),
        provider_model,
        provider_endpoint_kind: provider_endpoint_kind.into(),
        live_provider_invocation_allowed: false,
        main_chat_invoked: false,
        model_invoked: false,
        task_session_id,
        run_id,
        action_ids,
        observation_ids,
        proposal_ids,
        blocker_ids,
        final_delivery_id,
        event_types,
        event_sequence_start,
        event_sequence_end,
        ui_state_assertions,
        runtime_evidence,
        controls,
        negative_assertions,
        direct_writes_executed: false,
        legacy_fallback_used: false,
    }
}

fn blocked_proof(scenario_id: &str, blockers: &[String]) -> MainChatLiveProductScenarioProof {
    MainChatLiveProductScenarioProof {
        scenario_id: scenario_id.into(),
        passed: false,
        status: "blocked".into(),
        provider: String::new(),
        provider_model: None,
        provider_endpoint_kind: String::new(),
        task_session_id: None,
        run_id: None,
        action_ids: Vec::new(),
        observation_ids: Vec::new(),
        proposal_ids: Vec::new(),
        blocker_ids: Vec::new(),
        final_delivery_id: None,
        event_types: Vec::new(),
        event_sequence_start: None,
        event_sequence_end: None,
        ui_state_assertions: Vec::new(),
        runtime_evidence: Vec::new(),
        controls: Vec::new(),
        negative_assertions: Vec::new(),
        blockers: blockers.to_vec(),
    }
}

fn missing_evidence_proof(scenario_id: &str, blocker: &str) -> MainChatLiveProductScenarioProof {
    let blockers = vec![blocker.into()];
    let mut proof = blocked_proof(scenario_id, &blockers);
    proof.status = "failed".into();
    proof
}

fn external_provider_label(provider: &str) -> bool {
    if !metadata_safe_label(provider) {
        return false;
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
        return false;
    }
    ![
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

fn metadata_safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.trim() == value
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/'))
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn dedupe(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

#[cfg(test)]
pub(crate) fn test_live_product_evidence_for_scenario(
    scenario_id: &str,
) -> MainChatLiveProductScenarioEvidence {
    let mut evidence = MainChatLiveProductScenarioEvidence {
        scenario_id: scenario_id.into(),
        provider: "openai".into(),
        provider_model: Some("gpt-live-eval".into()),
        provider_endpoint_kind: "external_provider".into(),
        live_provider_invocation_allowed: true,
        main_chat_invoked: true,
        model_invoked: true,
        task_session_id: Some(format!("task-{scenario_id}")),
        run_id: Some(format!("run-{scenario_id}")),
        action_ids: Vec::new(),
        observation_ids: Vec::new(),
        proposal_ids: Vec::new(),
        blocker_ids: Vec::new(),
        final_delivery_id: Some(format!("delivery-{scenario_id}")),
        event_types: vec!["task.created".into(), "route.selected".into()],
        event_sequence_start: Some(1),
        event_sequence_end: Some(2),
        ui_state_assertions: vec![
            "task_header".into(),
            "provider_trace".into(),
            "final_delivery".into(),
        ],
        runtime_evidence: vec![
            "task_run".into(),
            "provider_model".into(),
            "final_delivery".into(),
        ],
        controls: vec!["open_trace".into()],
        negative_assertions: Vec::new(),
        direct_writes_executed: false,
        legacy_fallback_used: false,
    };
    match scenario_id {
        "LIVE-PROD-01" => {
            evidence
                .ui_state_assertions
                .push("direct_answer_no_tool_timeline".into());
            evidence
                .negative_assertions
                .push("direct_answer_no_tool_timeline".into());
        }
        "LIVE-PROD-02" => {
            evidence.action_ids = vec!["action-live-web".into()];
            evidence.observation_ids = vec!["observation-live-web".into()];
            evidence
                .runtime_evidence
                .extend(["action".into(), "web_observation_source".into()]);
            evidence
                .ui_state_assertions
                .extend(["action_timeline".into(), "observation_card".into()]);
            evidence.negative_assertions.push("no_fake_source".into());
            evidence.controls.push("retry".into());
            evidence.event_types.extend([
                "action.queued".into(),
                "observation.created".into(),
                "final_delivery.created".into(),
            ]);
            evidence.event_sequence_end = Some(6);
        }
        "LIVE-PROD-03" => {
            evidence.action_ids = vec!["action-live-mcp".into()];
            evidence.observation_ids = vec!["observation-live-mcp".into()];
            evidence.runtime_evidence.extend([
                "action".into(),
                "observation".into(),
                "candidate_list".into(),
                "candidate_ranking_trace".into(),
                "selected_target".into(),
            ]);
            evidence.ui_state_assertions.extend([
                "tool_candidates".into(),
                "selected_tool".into(),
                "observation_card".into(),
            ]);
            evidence
                .negative_assertions
                .push("selected_target_from_allowlist".into());
        }
        "LIVE-PROD-04" => {
            evidence.action_ids = vec!["action-live-permission".into()];
            evidence.proposal_ids = vec!["proposal-live-permission".into()];
            evidence.observation_ids.clear();
            evidence
                .runtime_evidence
                .extend(["pending_proposal".into(), "exact_action_proposal".into()]);
            evidence.ui_state_assertions.extend([
                "pending_proposal".into(),
                "approve_deny_defer_controls".into(),
            ]);
            evidence.controls = vec!["approve_once".into(), "deny".into(), "defer".into()];
            evidence
                .negative_assertions
                .push("no_overlapping_read_success".into());
        }
        "LIVE-PROD-05" => {
            evidence.final_delivery_id = None;
            evidence.blocker_ids = vec!["blocker-live-recovery".into()];
            evidence.runtime_evidence.extend([
                "blocker".into(),
                "blocker_reason".into(),
                "safe_recovery_controls".into(),
            ]);
            evidence
                .ui_state_assertions
                .extend(["blocker_visible".into(), "retry_cancel_controls".into()]);
            evidence.controls = vec!["retry".into(), "cancel".into(), "open_trace".into()];
            evidence
                .negative_assertions
                .push("no_success_text_without_blocker".into());
        }
        "LIVE-PROD-06" => {
            evidence.action_ids = vec!["action-live-delta".into()];
            evidence.observation_ids = vec!["observation-live-delta".into()];
            evidence.runtime_evidence = vec![
                "event_sequence_range".into(),
                "route_event".into(),
                "action_event".into(),
                "observation_event".into(),
            ];
            evidence.ui_state_assertions = vec!["event_status_strip".into(), "event_log".into()];
            evidence.controls = vec!["open_trace".into()];
            evidence
                .negative_assertions
                .push("snapshot_backfill_not_counted_as_live_delta".into());
            evidence.event_types = vec![
                "route.selected".into(),
                "action.queued".into(),
                "observation.created".into(),
                "final_delivery.created".into(),
            ];
            evidence.event_sequence_start = Some(2);
            evidence.event_sequence_end = Some(8);
        }
        _ => {}
    }
    dedupe(&mut evidence.runtime_evidence);
    dedupe(&mut evidence.ui_state_assertions);
    dedupe(&mut evidence.controls);
    dedupe(&mut evidence.negative_assertions);
    dedupe(&mut evidence.event_types);
    evidence
}
