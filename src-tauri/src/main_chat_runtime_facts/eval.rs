use openlife_core::agent::main_chat_agent_v1::{
    AgentTaskSessionDraft, ExecutionAction, ExecutionPolicyDecision, ExecutionTranscriptEntryDraft,
    ExecutionTranscriptEntryKind, MainChatAgentStrategy, MainChatPolicyLevel,
};
use openlife_core::agent::model_router::{ModelRouter, ProviderAvailability};
use openlife_core::agent::ModelRouteTrace;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::tool_manifest::{ToolManifest, ToolSource};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

use super::clock::MainChatRuntimeClockSource;
use super::contract::{
    RUNTIME_FACT_AGENT_SELF_STATE_GENERATION_PATH, RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES,
    RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS, RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY,
    RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT, RUNTIME_FACT_KEY_AGENT_TASK_STATUS,
    RUNTIME_FACT_KEY_AGENT_TRACE_GAP, RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER,
    RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED, RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER,
    RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT, RUNTIME_FACT_KEY_TOOL_MCP_SERVER_STATUS,
    RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE, RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE,
    RUNTIME_FACT_KEY_TRACE_GAP, RUNTIME_FACT_PROVIDER_GENERATION_PATH, RUNTIME_FACT_SOURCE_TYPE,
    RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH,
};
use super::registry::{provider_route_fact_keys, SOURCE_REGISTRY_VERSION, UI_CONTRACT_VERSION};
use crate::AppState;

const SLICE_A_SCENARIOS: [&str; 6] = ["RF-01", "RF-02", "RF-03", "RF-04", "RF-05", "RF-06"];
const SLICE_B_SCENARIOS: [&str; 4] = ["RF-07", "RF-08", "RF-09", "RF-10"];
const SLICE_C_SCENARIOS: [&str; 5] = ["RF-11", "RF-12", "RF-13", "RF-14", "RF-15"];
const SLICE_D_SCENARIOS: [&str; 6] = ["RF-16", "RF-17", "RF-18", "RF-19", "RF-20", "RF-21"];
const FIXED_CLOCK_RFC3339: &str = "2026-06-23T09:15:00+08:00";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsSliceReport {
    pub(crate) report_kind: &'static str,
    pub(crate) schema_version: u32,
    pub(crate) slice_id: &'static str,
    pub(crate) slice_name: &'static str,
    pub(crate) covered_scenario_ids: Vec<String>,
    pub(crate) out_of_scope_scenario_ids: Vec<String>,
    pub(crate) blocked_scenario_ids: Vec<String>,
    pub(crate) scenario_count: usize,
    pub(crate) passed_scenario_count: usize,
    pub(crate) blocked_scenario_count: usize,
    pub(crate) runtime_facts_slice_ready: bool,
    pub(crate) runtime_facts_ready: bool,
    pub(crate) ui_included: bool,
    pub(crate) source_registry_version: &'static str,
    pub(crate) ui_contract_version: &'static str,
    pub(crate) scenario_evidence: Vec<MainChatRuntimeFactsScenarioEvidence>,
    pub(crate) negative_assertion_summary: MainChatRuntimeFactsNegativeAssertionSummary,
    pub(crate) focused_test_commands: Vec<&'static str>,
    pub(crate) command_surface_proof: MainChatRuntimeFactsCommandSurfaceProof,
    pub(crate) no_silent_write_proof: bool,
    pub(crate) blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsScenarioEvidence {
    pub(crate) scenario_id: &'static str,
    pub(crate) entry_point: &'static str,
    pub(crate) user_text: &'static str,
    pub(crate) passed: bool,
    pub(crate) answer_preview: String,
    pub(crate) source_type: Option<String>,
    pub(crate) runtime_fact_keys: Vec<String>,
    pub(crate) runtime_fact_source: Vec<String>,
    pub(crate) runtime_fact_binding_count: usize,
    pub(crate) runtime_fact_authority: Option<String>,
    pub(crate) runtime_fact_freshness: Option<String>,
    pub(crate) runtime_fact_visibility: Vec<String>,
    pub(crate) runtime_fact_privacy: Vec<String>,
    pub(crate) model_generated: Option<bool>,
    pub(crate) scheduler_generation_called: Option<bool>,
    pub(crate) tool_called: Option<bool>,
    pub(crate) direct_writes_executed: Option<bool>,
    pub(crate) legacy_fallback_used: bool,
    pub(crate) provider_generation_path: Option<String>,
    pub(crate) configured_provider: Option<String>,
    pub(crate) configured_model: Option<String>,
    pub(crate) current_turn_generation_provider: Option<String>,
    pub(crate) current_turn_generation_model: Option<String>,
    pub(crate) current_turn_generation_route_type: Option<String>,
    pub(crate) current_turn_generation_model_generated: Option<bool>,
    pub(crate) last_completed_generation_provider: Option<String>,
    pub(crate) last_completed_generation_model: Option<String>,
    pub(crate) last_completed_generation_run_id: Option<String>,
    pub(crate) planned_route_if_model_needed_provider: Option<String>,
    pub(crate) planned_route_if_model_needed_model: Option<String>,
    pub(crate) planned_route_if_model_needed_route_type: Option<String>,
    pub(crate) provider_preflight_status: Option<String>,
    pub(crate) provider_preflight_blockers: Vec<String>,
    pub(crate) route_labels: Vec<String>,
    pub(crate) tool_web_config_enabled: Option<bool>,
    pub(crate) tool_web_credential_available: Option<bool>,
    pub(crate) tool_web_credential_status: Option<String>,
    pub(crate) tool_web_policy_allowed: Option<bool>,
    pub(crate) tool_web_policy_blockers: Vec<String>,
    pub(crate) tool_web_reachability_status: Option<String>,
    pub(crate) tool_web_reachability_ttl_status: Option<String>,
    pub(crate) tool_web_cached_or_preflight_known_reachability: Option<bool>,
    pub(crate) tool_web_active_reachability_probe: Option<bool>,
    pub(crate) tool_web_available: Option<String>,
    pub(crate) tool_mcp_registered_count: Option<usize>,
    pub(crate) tool_mcp_safe_read_candidate_count: Option<usize>,
    pub(crate) tool_mcp_server_status: Option<String>,
    pub(crate) tool_mcp_available: Option<String>,
    pub(crate) tool_mcp_raw_manifest_exposed: Option<bool>,
    pub(crate) tool_write_available: Option<String>,
    pub(crate) tool_write_requires_permission: Option<bool>,
    pub(crate) tool_write_silent_write_available: Option<bool>,
    pub(crate) tool_availability_labels: Vec<String>,
    pub(crate) ui_primary_source_chip: Option<String>,
    pub(crate) ui_status: Option<String>,
    pub(crate) task_session_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) task_status: Option<String>,
    pub(crate) run_status: Option<String>,
    pub(crate) delivery_status: Option<String>,
    pub(crate) blocker_codes: Vec<String>,
    pub(crate) pending_permission_count: Option<usize>,
    pub(crate) pending_permission_target_label: Option<String>,
    pub(crate) pending_permission_target_labels: Vec<String>,
    pub(crate) pending_proposal_count: Option<usize>,
    pub(crate) durable_change_status: Option<String>,
    pub(crate) durable_change_completed: Option<bool>,
    pub(crate) safe_next_controls: Vec<String>,
    pub(crate) safe_automatic_control_available: Option<bool>,
    pub(crate) completed_response: Option<bool>,
    pub(crate) final_delivery_evidence: Option<bool>,
    pub(crate) action_count: Option<usize>,
    pub(crate) completed_action_count: Option<usize>,
    pub(crate) observation_count: Option<usize>,
    pub(crate) transcript_observation_count: Option<usize>,
    pub(crate) final_result_count: Option<usize>,
    pub(crate) last_action_type: Option<String>,
    pub(crate) last_action_status: Option<String>,
    pub(crate) last_observation_source: Option<String>,
    pub(crate) last_action_summary: Option<String>,
    pub(crate) self_state_evidence_labels: Vec<String>,
    pub(crate) assistant_prose_used_for_task_status: Option<bool>,
    pub(crate) memory_or_hs_override_allowed: Option<bool>,
    pub(crate) trace_gap: bool,
    pub(crate) context_conflict_ignored: bool,
    pub(crate) silent_write_detected: bool,
    pub(crate) failure: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsNegativeAssertionSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) planning_question_not_captured: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_provider_call_for_runtime_facts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_tool_call_for_runtime_facts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_direct_write_for_runtime_facts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_legacy_fallback_for_runtime_facts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_cannot_override_runtime_clock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) missing_clock_does_not_use_model: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_route_requires_current_generation_evidence: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_current_route_for_model_generated_false: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) configured_route_not_invocation_proof: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) planned_route_not_invocation_proof: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_completed_route_not_current_turn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) provider_preflight_blocker_not_fake_readiness: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_active_reachability_probe_for_tool_availability: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) web_policy_blocker_not_fake_availability: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mcp_registry_not_availability_without_safe_read: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mcp_unknown_server_status_not_available: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) write_capability_requires_permission: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_raw_mcp_manifest_exposure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_assistant_prose_used_for_task_status: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_cannot_override_task_runtime_state: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) proposal_pending_not_completed_durable_change: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_history_invention_without_trace: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsCommandSurfaceProof {
    pub(crate) send_runtime_clock_path: bool,
    pub(crate) stream_runtime_clock_path: bool,
    pub(crate) send_provider_route_path: bool,
    pub(crate) send_provider_route_preflight_blocker_path: bool,
    pub(crate) stream_provider_route_path: bool,
    pub(crate) stream_provider_route_preflight_blocker_path: bool,
    pub(crate) send_tool_availability_path: bool,
    pub(crate) send_web_policy_blocked_path: bool,
    pub(crate) send_mcp_no_safe_read_candidate_path: bool,
    pub(crate) send_mcp_unknown_server_status_path: bool,
    pub(crate) send_write_permission_path: bool,
    pub(crate) stream_tool_availability_path: bool,
    pub(crate) stream_web_policy_blocked_path: bool,
    pub(crate) stream_mcp_no_safe_read_candidate_path: bool,
    pub(crate) stream_mcp_unknown_server_status_path: bool,
    pub(crate) stream_write_permission_path: bool,
    pub(crate) send_self_state_completion_path: bool,
    pub(crate) send_self_state_pending_proposal_path: bool,
    pub(crate) send_self_state_observation_path: bool,
    pub(crate) send_self_state_trace_gap_path: bool,
    pub(crate) send_self_state_blocked_path: bool,
    pub(crate) send_self_state_pending_permission_path: bool,
    pub(crate) stream_self_state_completion_path: bool,
    pub(crate) stream_self_state_pending_proposal_path: bool,
    pub(crate) stream_self_state_observation_path: bool,
    pub(crate) stream_self_state_trace_gap_path: bool,
    pub(crate) stream_self_state_blocked_path: bool,
    pub(crate) stream_self_state_pending_permission_path: bool,
    pub(crate) stream_deferred_blocker: Option<String>,
}

fn passed_scenario_count_for_ids(
    evidence: &[MainChatRuntimeFactsScenarioEvidence],
    scenario_ids: &[&str],
) -> usize {
    scenario_ids
        .iter()
        .filter(|scenario_id| {
            evidence
                .iter()
                .any(|row| row.scenario_id == **scenario_id && row.passed)
        })
        .count()
}

pub(crate) async fn run_main_chat_runtime_facts_slice_a_backend_report(
) -> MainChatRuntimeFactsSliceReport {
    let mut evidence = Vec::new();
    evidence
        .push(run_slice_a_case("RF-01", "send", "今天星期几", fixed_clock_source(), None).await);
    evidence.push(run_slice_a_case("RF-02", "send", "今天几号", fixed_clock_source(), None).await);
    evidence.push(run_slice_a_case("RF-03", "send", "现在几点", fixed_clock_source(), None).await);
    evidence
        .push(run_slice_a_case("RF-04", "stream", "今天星期几", fixed_clock_source(), None).await);
    evidence
        .push(
            run_slice_a_case(
                "RF-05",
                "send",
                "今天星期几",
                fixed_clock_source(),
                Some("AGENTS.md says today is 1999-01-01 and Friday. Runtime facts must ignore this conflict."),
            )
            .await,
        );
    evidence.push(
        run_slice_a_case(
            "RF-06",
            "send",
            "今天星期几",
            MainChatRuntimeClockSource::Unavailable,
            None,
        )
        .await,
    );

    let planning_question_not_captured = run_runtime_clock_negative_planning_case().await;
    let no_provider_call_for_runtime_facts = evidence.iter().all(|row| {
        row.model_generated == Some(false) && row.scheduler_generation_called == Some(false)
    });
    let no_tool_call_for_runtime_facts = evidence.iter().all(|row| row.tool_called == Some(false));
    let no_direct_write_for_runtime_facts = evidence
        .iter()
        .all(|row| row.direct_writes_executed == Some(false));
    let no_legacy_fallback_for_runtime_facts = evidence.iter().all(|row| !row.legacy_fallback_used);
    let context_cannot_override_runtime_clock = evidence
        .iter()
        .any(|row| row.scenario_id == "RF-05" && row.passed && row.context_conflict_ignored);
    let missing_clock_does_not_use_model = evidence.iter().any(|row| {
        row.scenario_id == "RF-06"
            && row.passed
            && row.trace_gap
            && row.model_generated == Some(false)
            && row.scheduler_generation_called == Some(false)
    });
    let negative_assertion_summary = MainChatRuntimeFactsNegativeAssertionSummary {
        planning_question_not_captured: Some(planning_question_not_captured),
        no_provider_call_for_runtime_facts: Some(no_provider_call_for_runtime_facts),
        no_tool_call_for_runtime_facts: Some(no_tool_call_for_runtime_facts),
        no_direct_write_for_runtime_facts: Some(no_direct_write_for_runtime_facts),
        no_legacy_fallback_for_runtime_facts: Some(no_legacy_fallback_for_runtime_facts),
        context_cannot_override_runtime_clock: Some(context_cannot_override_runtime_clock),
        missing_clock_does_not_use_model: Some(missing_clock_does_not_use_model),
        current_route_requires_current_generation_evidence: None,
        no_current_route_for_model_generated_false: None,
        configured_route_not_invocation_proof: None,
        planned_route_not_invocation_proof: None,
        last_completed_route_not_current_turn: None,
        provider_preflight_blocker_not_fake_readiness: None,
        no_active_reachability_probe_for_tool_availability: None,
        web_policy_blocker_not_fake_availability: None,
        mcp_registry_not_availability_without_safe_read: None,
        mcp_unknown_server_status_not_available: None,
        write_capability_requires_permission: None,
        no_raw_mcp_manifest_exposure: None,
        no_assistant_prose_used_for_task_status: None,
        context_cannot_override_task_runtime_state: None,
        proposal_pending_not_completed_durable_change: None,
        no_history_invention_without_trace: None,
    };

    let passed_scenario_count = evidence.iter().filter(|row| row.passed).count();
    let blockers = evidence
        .iter()
        .filter_map(|row| {
            row.failure
                .as_ref()
                .map(|failure| format!("{}:{failure}", row.scenario_id))
        })
        .collect::<Vec<_>>();
    let command_surface_proof = MainChatRuntimeFactsCommandSurfaceProof {
        send_runtime_clock_path: evidence
            .iter()
            .any(|row| row.entry_point == "send" && row.passed && !row.trace_gap),
        stream_runtime_clock_path: evidence
            .iter()
            .any(|row| row.entry_point == "stream" && row.passed && !row.trace_gap),
        send_provider_route_path: false,
        send_provider_route_preflight_blocker_path: false,
        stream_provider_route_path: false,
        stream_provider_route_preflight_blocker_path: false,
        send_tool_availability_path: false,
        send_web_policy_blocked_path: false,
        send_mcp_no_safe_read_candidate_path: false,
        send_mcp_unknown_server_status_path: false,
        send_write_permission_path: false,
        stream_tool_availability_path: false,
        stream_web_policy_blocked_path: false,
        stream_mcp_no_safe_read_candidate_path: false,
        stream_mcp_unknown_server_status_path: false,
        stream_write_permission_path: false,
        send_self_state_completion_path: false,
        send_self_state_pending_proposal_path: false,
        send_self_state_observation_path: false,
        send_self_state_trace_gap_path: false,
        send_self_state_blocked_path: false,
        send_self_state_pending_permission_path: false,
        stream_self_state_completion_path: false,
        stream_self_state_pending_proposal_path: false,
        stream_self_state_observation_path: false,
        stream_self_state_trace_gap_path: false,
        stream_self_state_blocked_path: false,
        stream_self_state_pending_permission_path: false,
        stream_deferred_blocker: None,
    };
    let no_silent_write_proof = evidence.iter().all(|row| !row.silent_write_detected);
    let runtime_facts_slice_ready = passed_scenario_count == SLICE_A_SCENARIOS.len()
        && planning_question_not_captured
        && no_provider_call_for_runtime_facts
        && no_tool_call_for_runtime_facts
        && no_direct_write_for_runtime_facts
        && no_legacy_fallback_for_runtime_facts
        && context_cannot_override_runtime_clock
        && missing_clock_does_not_use_model
        && command_surface_proof.send_runtime_clock_path
        && command_surface_proof.stream_runtime_clock_path
        && no_silent_write_proof;

    MainChatRuntimeFactsSliceReport {
        report_kind: "main_chat_runtime_facts_slice",
        schema_version: 1,
        slice_id: "slice_a_backend",
        slice_name: "Runtime Clock Backend",
        covered_scenario_ids: SLICE_A_SCENARIOS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        out_of_scope_scenario_ids: vec!["RF-22".into()],
        blocked_scenario_ids: Vec::new(),
        scenario_count: SLICE_A_SCENARIOS.len(),
        passed_scenario_count,
        blocked_scenario_count: 0,
        runtime_facts_slice_ready,
        runtime_facts_ready: false,
        ui_included: false,
        source_registry_version: SOURCE_REGISTRY_VERSION,
        ui_contract_version: UI_CONTRACT_VERSION,
        scenario_evidence: evidence,
        negative_assertion_summary,
        focused_test_commands: vec![
            "cargo test -p openlife-tauri runtime_clock -- --nocapture",
            "cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture",
        ],
        command_surface_proof,
        no_silent_write_proof,
        blockers,
    }
}

pub(crate) async fn run_main_chat_runtime_facts_slice_b_provider_route_report(
) -> MainChatRuntimeFactsSliceReport {
    let mut evidence = Vec::new();
    evidence.push(Box::pin(run_slice_b_rf07_case("send")).await);
    evidence.push(Box::pin(run_slice_b_rf07_case("stream")).await);
    evidence.push(Box::pin(run_slice_b_rf08_case("send")).await);
    evidence.push(Box::pin(run_slice_b_rf08_case("stream")).await);
    evidence.push(Box::pin(run_slice_b_rf09_case("send")).await);
    evidence.push(Box::pin(run_slice_b_rf09_case("stream")).await);
    evidence.push(Box::pin(run_slice_b_rf10_case("send")).await);
    evidence.push(Box::pin(run_slice_b_rf10_case("stream")).await);

    let current_route_requires_current_generation_evidence = evidence.iter().any(|row| {
        row.scenario_id == "RF-07"
            && row.passed
            && row.model_generated == Some(true)
            && row.scheduler_generation_called == Some(true)
            && row.current_turn_generation_provider.is_some()
            && row.current_turn_generation_model.is_some()
    });
    let no_current_route_for_model_generated_false = evidence
        .iter()
        .filter(|row| matches!(row.scenario_id, "RF-08" | "RF-10"))
        .all(|row| {
            row.passed
                && row.model_generated == Some(false)
                && row.current_turn_generation_provider.is_none()
                && row.current_turn_generation_model.is_none()
                && row.current_turn_generation_route_type.as_deref() == Some("none")
        });
    let configured_route_not_invocation_proof = evidence.iter().any(|row| {
        row.scenario_id == "RF-09"
            && row.passed
            && row.configured_provider.as_deref() == Some("deepseek")
            && row.current_turn_generation_provider.as_deref() == Some("openai")
            && row
                .route_labels
                .iter()
                .any(|label| label.starts_with("configured_default_route:"))
    });
    let planned_route_not_invocation_proof = evidence.iter().any(|row| {
        row.scenario_id == "RF-09"
            && row.passed
            && row
                .route_labels
                .iter()
                .any(|label| label.starts_with("planned_route_if_model_needed:"))
            && row
                .route_labels
                .iter()
                .any(|label| label.starts_with("current_turn_generation: actual"))
    });
    let last_completed_route_not_current_turn = evidence.iter().any(|row| {
        row.scenario_id == "RF-09"
            && row.passed
            && row.last_completed_generation_provider.as_deref() == Some("anthropic")
            && row.current_turn_generation_provider.as_deref() == Some("openai")
    });
    let provider_preflight_blocker_not_fake_readiness = evidence.iter().any(|row| {
        row.scenario_id == "RF-10"
            && row.passed
            && row.provider_preflight_status.as_deref() == Some("blocked")
            && !row.provider_preflight_blockers.is_empty()
            && row.ui_status.as_deref() == Some("restricted")
            && !row.answer_preview.contains("已就绪")
    });
    let no_tool_call_for_runtime_facts = evidence.iter().all(|row| row.tool_called == Some(false));
    let no_direct_write_for_runtime_facts = evidence
        .iter()
        .all(|row| row.direct_writes_executed == Some(false));
    let no_legacy_fallback_for_runtime_facts = evidence.iter().all(|row| !row.legacy_fallback_used);
    let no_silent_write_proof = evidence.iter().all(|row| !row.silent_write_detected);
    let passed_scenario_count = passed_scenario_count_for_ids(&evidence, &SLICE_B_SCENARIOS);
    let all_evidence_passed = evidence.iter().all(|row| row.passed);
    let blockers = evidence
        .iter()
        .filter_map(|row| {
            row.failure
                .as_ref()
                .map(|failure| format!("{}:{failure}", row.scenario_id))
        })
        .collect::<Vec<_>>();
    let command_surface_proof = MainChatRuntimeFactsCommandSurfaceProof {
        send_runtime_clock_path: false,
        stream_runtime_clock_path: false,
        send_provider_route_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-07" && row.entry_point == "send" && row.passed),
        send_provider_route_preflight_blocker_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-10" && row.entry_point == "send" && row.passed),
        stream_provider_route_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-07" && row.entry_point == "stream" && row.passed),
        stream_provider_route_preflight_blocker_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-10" && row.entry_point == "stream" && row.passed),
        send_tool_availability_path: false,
        send_web_policy_blocked_path: false,
        send_mcp_no_safe_read_candidate_path: false,
        send_mcp_unknown_server_status_path: false,
        send_write_permission_path: false,
        stream_tool_availability_path: false,
        stream_web_policy_blocked_path: false,
        stream_mcp_no_safe_read_candidate_path: false,
        stream_mcp_unknown_server_status_path: false,
        stream_write_permission_path: false,
        send_self_state_completion_path: false,
        send_self_state_pending_proposal_path: false,
        send_self_state_observation_path: false,
        send_self_state_trace_gap_path: false,
        send_self_state_blocked_path: false,
        send_self_state_pending_permission_path: false,
        stream_self_state_completion_path: false,
        stream_self_state_pending_proposal_path: false,
        stream_self_state_observation_path: false,
        stream_self_state_trace_gap_path: false,
        stream_self_state_blocked_path: false,
        stream_self_state_pending_permission_path: false,
        stream_deferred_blocker: None,
    };
    let negative_assertion_summary = MainChatRuntimeFactsNegativeAssertionSummary {
        planning_question_not_captured: None,
        no_provider_call_for_runtime_facts: Some(
            evidence
                .iter()
                .filter(|row| matches!(row.scenario_id, "RF-08" | "RF-10"))
                .all(|row| {
                    row.model_generated == Some(false)
                        && row.scheduler_generation_called == Some(false)
                }),
        ),
        no_tool_call_for_runtime_facts: Some(no_tool_call_for_runtime_facts),
        no_direct_write_for_runtime_facts: Some(no_direct_write_for_runtime_facts),
        no_legacy_fallback_for_runtime_facts: Some(no_legacy_fallback_for_runtime_facts),
        context_cannot_override_runtime_clock: None,
        missing_clock_does_not_use_model: None,
        current_route_requires_current_generation_evidence: Some(
            current_route_requires_current_generation_evidence,
        ),
        no_current_route_for_model_generated_false: Some(
            no_current_route_for_model_generated_false,
        ),
        configured_route_not_invocation_proof: Some(configured_route_not_invocation_proof),
        planned_route_not_invocation_proof: Some(planned_route_not_invocation_proof),
        last_completed_route_not_current_turn: Some(last_completed_route_not_current_turn),
        provider_preflight_blocker_not_fake_readiness: Some(
            provider_preflight_blocker_not_fake_readiness,
        ),
        no_active_reachability_probe_for_tool_availability: None,
        web_policy_blocker_not_fake_availability: None,
        mcp_registry_not_availability_without_safe_read: None,
        mcp_unknown_server_status_not_available: None,
        write_capability_requires_permission: None,
        no_raw_mcp_manifest_exposure: None,
        no_assistant_prose_used_for_task_status: None,
        context_cannot_override_task_runtime_state: None,
        proposal_pending_not_completed_durable_change: None,
        no_history_invention_without_trace: None,
    };
    let runtime_facts_slice_ready = passed_scenario_count == SLICE_B_SCENARIOS.len()
        && all_evidence_passed
        && current_route_requires_current_generation_evidence
        && no_current_route_for_model_generated_false
        && configured_route_not_invocation_proof
        && planned_route_not_invocation_proof
        && last_completed_route_not_current_turn
        && provider_preflight_blocker_not_fake_readiness
        && no_tool_call_for_runtime_facts
        && no_direct_write_for_runtime_facts
        && no_legacy_fallback_for_runtime_facts
        && no_silent_write_proof
        && command_surface_proof.send_provider_route_path
        && command_surface_proof.send_provider_route_preflight_blocker_path
        && command_surface_proof.stream_provider_route_path
        && command_surface_proof.stream_provider_route_preflight_blocker_path;

    MainChatRuntimeFactsSliceReport {
        report_kind: "main_chat_runtime_facts_slice",
        schema_version: 1,
        slice_id: "slice_b_provider_route_semantics",
        slice_name: "Provider Route Semantics",
        covered_scenario_ids: SLICE_B_SCENARIOS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        out_of_scope_scenario_ids: Vec::new(),
        blocked_scenario_ids: Vec::new(),
        scenario_count: SLICE_B_SCENARIOS.len(),
        passed_scenario_count,
        blocked_scenario_count: 0,
        runtime_facts_slice_ready,
        runtime_facts_ready: false,
        ui_included: true,
        source_registry_version: SOURCE_REGISTRY_VERSION,
        ui_contract_version: UI_CONTRACT_VERSION,
        scenario_evidence: evidence,
        negative_assertion_summary,
        focused_test_commands: vec![
            "cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture",
            "cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture",
            "pnpm --dir frontend test -- src/components/ReasoningTracePanel.test.tsx",
        ],
        command_surface_proof,
        no_silent_write_proof,
        blockers,
    }
}

pub(crate) async fn run_main_chat_runtime_facts_slice_c_tool_availability_report(
) -> MainChatRuntimeFactsSliceReport {
    let mut evidence = Vec::new();
    evidence.push(Box::pin(run_slice_c_rf11_case("send")).await);
    evidence.push(Box::pin(run_slice_c_rf11_case("stream")).await);
    evidence.push(Box::pin(run_slice_c_rf12_case("send")).await);
    evidence.push(Box::pin(run_slice_c_rf12_case("stream")).await);
    evidence.push(Box::pin(run_slice_c_rf13_case("send")).await);
    evidence.push(Box::pin(run_slice_c_rf13_case("stream")).await);
    evidence.push(Box::pin(run_slice_c_rf14_case("send")).await);
    evidence.push(Box::pin(run_slice_c_rf14_case("stream")).await);
    evidence.push(Box::pin(run_slice_c_rf15_case("send")).await);
    evidence.push(Box::pin(run_slice_c_rf15_case("stream")).await);

    let no_provider_call_for_runtime_facts = evidence.iter().all(|row| {
        row.model_generated == Some(false) && row.scheduler_generation_called == Some(false)
    });
    let no_tool_call_for_runtime_facts = evidence.iter().all(|row| row.tool_called == Some(false));
    let no_direct_write_for_runtime_facts = evidence
        .iter()
        .all(|row| row.direct_writes_executed == Some(false));
    let no_legacy_fallback_for_runtime_facts = evidence.iter().all(|row| !row.legacy_fallback_used);
    let no_active_reachability_probe_for_tool_availability = evidence
        .iter()
        .all(|row| !row.tool_web_active_reachability_probe.unwrap_or(false));
    let web_policy_blocker_not_fake_availability = evidence.iter().any(|row| {
        row.scenario_id == "RF-12"
            && row.passed
            && row.tool_web_config_enabled == Some(true)
            && row.tool_web_policy_allowed == Some(false)
            && row.tool_web_available.as_deref() == Some("blocked")
            && row.ui_status.as_deref() == Some("restricted")
    });
    let mcp_registry_not_availability_without_safe_read = evidence.iter().any(|row| {
        row.scenario_id == "RF-13"
            && row.passed
            && row.tool_mcp_registered_count.unwrap_or_default() > 0
            && row.tool_mcp_safe_read_candidate_count == Some(0)
            && row.tool_mcp_available.as_deref() == Some("no_safe_read_candidate")
    });
    let mcp_unknown_server_status_not_available = evidence.iter().any(|row| {
        row.scenario_id == "RF-14"
            && row.passed
            && row.tool_mcp_safe_read_candidate_count.unwrap_or_default() > 0
            && row.tool_mcp_server_status.as_deref() == Some("unknown")
            && row.tool_mcp_available.as_deref() == Some("unknown_server_status")
    });
    let write_capability_requires_permission = evidence.iter().any(|row| {
        row.scenario_id == "RF-15"
            && row.passed
            && row.tool_write_available.as_deref() == Some("proposal_permission_or_blocker")
            && row.tool_write_requires_permission == Some(true)
            && row.tool_write_silent_write_available == Some(false)
            && row.ui_status.as_deref() == Some("waiting_for_user")
    });
    let no_raw_mcp_manifest_exposure = evidence
        .iter()
        .all(|row| row.tool_mcp_raw_manifest_exposed != Some(true));
    let no_silent_write_proof = evidence.iter().all(|row| !row.silent_write_detected);
    let passed_scenario_count = passed_scenario_count_for_ids(&evidence, &SLICE_C_SCENARIOS);
    let all_evidence_passed = evidence.iter().all(|row| row.passed);
    let blockers = evidence
        .iter()
        .filter_map(|row| {
            row.failure
                .as_ref()
                .map(|failure| format!("{}:{failure}", row.scenario_id))
        })
        .collect::<Vec<_>>();
    let command_surface_proof = MainChatRuntimeFactsCommandSurfaceProof {
        send_runtime_clock_path: false,
        stream_runtime_clock_path: false,
        send_provider_route_path: false,
        send_provider_route_preflight_blocker_path: false,
        stream_provider_route_path: false,
        stream_provider_route_preflight_blocker_path: false,
        send_tool_availability_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-11" && row.entry_point == "send" && row.passed),
        send_web_policy_blocked_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-12" && row.entry_point == "send" && row.passed),
        send_mcp_no_safe_read_candidate_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-13" && row.entry_point == "send" && row.passed),
        send_mcp_unknown_server_status_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-14" && row.entry_point == "send" && row.passed),
        send_write_permission_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-15" && row.entry_point == "send" && row.passed),
        stream_tool_availability_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-11" && row.entry_point == "stream" && row.passed),
        stream_web_policy_blocked_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-12" && row.entry_point == "stream" && row.passed),
        stream_mcp_no_safe_read_candidate_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-13" && row.entry_point == "stream" && row.passed),
        stream_mcp_unknown_server_status_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-14" && row.entry_point == "stream" && row.passed),
        stream_write_permission_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-15" && row.entry_point == "stream" && row.passed),
        send_self_state_completion_path: false,
        send_self_state_pending_proposal_path: false,
        send_self_state_observation_path: false,
        send_self_state_trace_gap_path: false,
        send_self_state_blocked_path: false,
        send_self_state_pending_permission_path: false,
        stream_self_state_completion_path: false,
        stream_self_state_pending_proposal_path: false,
        stream_self_state_observation_path: false,
        stream_self_state_trace_gap_path: false,
        stream_self_state_blocked_path: false,
        stream_self_state_pending_permission_path: false,
        stream_deferred_blocker: None,
    };
    let negative_assertion_summary = MainChatRuntimeFactsNegativeAssertionSummary {
        planning_question_not_captured: None,
        no_provider_call_for_runtime_facts: Some(no_provider_call_for_runtime_facts),
        no_tool_call_for_runtime_facts: Some(no_tool_call_for_runtime_facts),
        no_direct_write_for_runtime_facts: Some(no_direct_write_for_runtime_facts),
        no_legacy_fallback_for_runtime_facts: Some(no_legacy_fallback_for_runtime_facts),
        context_cannot_override_runtime_clock: None,
        missing_clock_does_not_use_model: None,
        current_route_requires_current_generation_evidence: None,
        no_current_route_for_model_generated_false: None,
        configured_route_not_invocation_proof: None,
        planned_route_not_invocation_proof: None,
        last_completed_route_not_current_turn: None,
        provider_preflight_blocker_not_fake_readiness: None,
        no_active_reachability_probe_for_tool_availability: Some(
            no_active_reachability_probe_for_tool_availability,
        ),
        web_policy_blocker_not_fake_availability: Some(web_policy_blocker_not_fake_availability),
        mcp_registry_not_availability_without_safe_read: Some(
            mcp_registry_not_availability_without_safe_read,
        ),
        mcp_unknown_server_status_not_available: Some(mcp_unknown_server_status_not_available),
        write_capability_requires_permission: Some(write_capability_requires_permission),
        no_raw_mcp_manifest_exposure: Some(no_raw_mcp_manifest_exposure),
        no_assistant_prose_used_for_task_status: None,
        context_cannot_override_task_runtime_state: None,
        proposal_pending_not_completed_durable_change: None,
        no_history_invention_without_trace: None,
    };
    let runtime_facts_slice_ready = passed_scenario_count == SLICE_C_SCENARIOS.len()
        && all_evidence_passed
        && no_provider_call_for_runtime_facts
        && no_tool_call_for_runtime_facts
        && no_direct_write_for_runtime_facts
        && no_legacy_fallback_for_runtime_facts
        && no_active_reachability_probe_for_tool_availability
        && web_policy_blocker_not_fake_availability
        && mcp_registry_not_availability_without_safe_read
        && mcp_unknown_server_status_not_available
        && write_capability_requires_permission
        && no_raw_mcp_manifest_exposure
        && no_silent_write_proof
        && command_surface_proof.send_tool_availability_path
        && command_surface_proof.send_web_policy_blocked_path
        && command_surface_proof.send_mcp_no_safe_read_candidate_path
        && command_surface_proof.send_mcp_unknown_server_status_path
        && command_surface_proof.send_write_permission_path
        && command_surface_proof.stream_tool_availability_path
        && command_surface_proof.stream_web_policy_blocked_path
        && command_surface_proof.stream_mcp_no_safe_read_candidate_path
        && command_surface_proof.stream_mcp_unknown_server_status_path
        && command_surface_proof.stream_write_permission_path;

    MainChatRuntimeFactsSliceReport {
        report_kind: "main_chat_runtime_facts_slice",
        schema_version: 1,
        slice_id: "slice_c_tool_mcp_availability",
        slice_name: "Tool And MCP Availability",
        covered_scenario_ids: SLICE_C_SCENARIOS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        out_of_scope_scenario_ids: Vec::new(),
        blocked_scenario_ids: Vec::new(),
        scenario_count: SLICE_C_SCENARIOS.len(),
        passed_scenario_count,
        blocked_scenario_count: 0,
        runtime_facts_slice_ready,
        runtime_facts_ready: false,
        ui_included: true,
        source_registry_version: SOURCE_REGISTRY_VERSION,
        ui_contract_version: UI_CONTRACT_VERSION,
        scenario_evidence: evidence,
        negative_assertion_summary,
        focused_test_commands: vec![
            "cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture",
            "cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture",
            "pnpm --dir frontend test -- src/components/ReasoningTracePanel.test.tsx",
        ],
        command_surface_proof,
        no_silent_write_proof,
        blockers,
    }
}

pub(crate) async fn run_main_chat_runtime_facts_slice_d_agent_self_state_report(
) -> MainChatRuntimeFactsSliceReport {
    let mut evidence = Vec::new();
    evidence.push(Box::pin(run_slice_d_rf16_case("send")).await);
    evidence.push(Box::pin(run_slice_d_rf16_case("stream")).await);
    evidence.push(Box::pin(run_slice_d_rf17_case("send")).await);
    evidence.push(Box::pin(run_slice_d_rf17_case("stream")).await);
    evidence.push(Box::pin(run_slice_d_rf18_case("send")).await);
    evidence.push(Box::pin(run_slice_d_rf18_case("stream")).await);
    evidence.push(Box::pin(run_slice_d_rf19_case("send")).await);
    evidence.push(Box::pin(run_slice_d_rf19_case("stream")).await);
    evidence.push(Box::pin(run_slice_d_rf20_case("send")).await);
    evidence.push(Box::pin(run_slice_d_rf20_case("stream")).await);
    evidence.push(Box::pin(run_slice_d_rf21_case("send")).await);
    evidence.push(Box::pin(run_slice_d_rf21_case("stream")).await);

    let no_provider_call_for_runtime_facts = evidence.iter().all(|row| {
        row.model_generated == Some(false) && row.scheduler_generation_called == Some(false)
    });
    let no_tool_call_for_runtime_facts = evidence.iter().all(|row| row.tool_called == Some(false));
    let no_direct_write_for_runtime_facts = evidence
        .iter()
        .all(|row| row.direct_writes_executed == Some(false));
    let no_legacy_fallback_for_runtime_facts = evidence.iter().all(|row| !row.legacy_fallback_used);
    let no_assistant_prose_used_for_task_status = evidence
        .iter()
        .all(|row| row.assistant_prose_used_for_task_status == Some(false));
    let context_cannot_override_task_runtime_state = evidence
        .iter()
        .all(|row| row.memory_or_hs_override_allowed == Some(false));
    let proposal_pending_not_completed_durable_change = evidence.iter().any(|row| {
        row.scenario_id == "RF-17"
            && row.passed
            && row.pending_proposal_count.unwrap_or_default() > 0
            && row.durable_change_status.as_deref() == Some("pending_review")
            && row.durable_change_completed == Some(false)
            && row.completed_response == Some(true)
    });
    let no_history_invention_without_trace = evidence.iter().any(|row| {
        row.scenario_id == "RF-19"
            && row.passed
            && row.trace_gap
            && row.task_session_id.is_none()
            && row.run_id.is_none()
    });
    let blocked_task_state_not_completed = evidence.iter().any(|row| {
        row.scenario_id == "RF-20"
            && row.passed
            && row.task_status.as_deref() == Some("blocked")
            && !row.blocker_codes.is_empty()
            && row.ui_status.as_deref() == Some("restricted")
            && row.completed_response == Some(false)
            && row.safe_next_controls.iter().any(|control| {
                control == "retry_failed_action" || control == "no_safe_automatic_control"
            })
    });
    let pending_permission_state_not_executed = evidence.iter().any(|row| {
        row.scenario_id == "RF-21"
            && row.passed
            && row.task_status.as_deref() == Some("waiting_permission")
            && row.pending_permission_count.unwrap_or_default() > 0
            && !row.pending_permission_target_labels.is_empty()
            && row.ui_status.as_deref() == Some("waiting_for_user")
            && row.completed_action_count == Some(0)
    });
    let no_silent_write_proof = evidence.iter().all(|row| !row.silent_write_detected);
    let passed_scenario_count = passed_scenario_count_for_ids(&evidence, &SLICE_D_SCENARIOS);
    let all_evidence_passed = evidence.iter().all(|row| row.passed);
    let blockers = evidence
        .iter()
        .filter_map(|row| {
            row.failure
                .as_ref()
                .map(|failure| format!("{}:{failure}", row.scenario_id))
        })
        .collect::<Vec<_>>();
    let command_surface_proof = MainChatRuntimeFactsCommandSurfaceProof {
        send_runtime_clock_path: false,
        stream_runtime_clock_path: false,
        send_provider_route_path: false,
        send_provider_route_preflight_blocker_path: false,
        stream_provider_route_path: false,
        stream_provider_route_preflight_blocker_path: false,
        send_tool_availability_path: false,
        send_web_policy_blocked_path: false,
        send_mcp_no_safe_read_candidate_path: false,
        send_mcp_unknown_server_status_path: false,
        send_write_permission_path: false,
        stream_tool_availability_path: false,
        stream_web_policy_blocked_path: false,
        stream_mcp_no_safe_read_candidate_path: false,
        stream_mcp_unknown_server_status_path: false,
        stream_write_permission_path: false,
        send_self_state_completion_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-16" && row.entry_point == "send" && row.passed),
        send_self_state_pending_proposal_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-17" && row.entry_point == "send" && row.passed),
        send_self_state_observation_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-18" && row.entry_point == "send" && row.passed),
        send_self_state_trace_gap_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-19" && row.entry_point == "send" && row.passed),
        send_self_state_blocked_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-20" && row.entry_point == "send" && row.passed),
        send_self_state_pending_permission_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-21" && row.entry_point == "send" && row.passed),
        stream_self_state_completion_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-16" && row.entry_point == "stream" && row.passed),
        stream_self_state_pending_proposal_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-17" && row.entry_point == "stream" && row.passed),
        stream_self_state_observation_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-18" && row.entry_point == "stream" && row.passed),
        stream_self_state_trace_gap_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-19" && row.entry_point == "stream" && row.passed),
        stream_self_state_blocked_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-20" && row.entry_point == "stream" && row.passed),
        stream_self_state_pending_permission_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-21" && row.entry_point == "stream" && row.passed),
        stream_deferred_blocker: None,
    };
    let negative_assertion_summary = MainChatRuntimeFactsNegativeAssertionSummary {
        planning_question_not_captured: None,
        no_provider_call_for_runtime_facts: Some(no_provider_call_for_runtime_facts),
        no_tool_call_for_runtime_facts: Some(no_tool_call_for_runtime_facts),
        no_direct_write_for_runtime_facts: Some(no_direct_write_for_runtime_facts),
        no_legacy_fallback_for_runtime_facts: Some(no_legacy_fallback_for_runtime_facts),
        context_cannot_override_runtime_clock: None,
        missing_clock_does_not_use_model: None,
        current_route_requires_current_generation_evidence: None,
        no_current_route_for_model_generated_false: None,
        configured_route_not_invocation_proof: None,
        planned_route_not_invocation_proof: None,
        last_completed_route_not_current_turn: None,
        provider_preflight_blocker_not_fake_readiness: None,
        no_active_reachability_probe_for_tool_availability: None,
        web_policy_blocker_not_fake_availability: None,
        mcp_registry_not_availability_without_safe_read: None,
        mcp_unknown_server_status_not_available: None,
        write_capability_requires_permission: None,
        no_raw_mcp_manifest_exposure: None,
        no_assistant_prose_used_for_task_status: Some(no_assistant_prose_used_for_task_status),
        context_cannot_override_task_runtime_state: Some(
            context_cannot_override_task_runtime_state,
        ),
        proposal_pending_not_completed_durable_change: Some(
            proposal_pending_not_completed_durable_change,
        ),
        no_history_invention_without_trace: Some(no_history_invention_without_trace),
    };
    let runtime_facts_slice_ready = passed_scenario_count == SLICE_D_SCENARIOS.len()
        && all_evidence_passed
        && no_provider_call_for_runtime_facts
        && no_tool_call_for_runtime_facts
        && no_direct_write_for_runtime_facts
        && no_legacy_fallback_for_runtime_facts
        && no_assistant_prose_used_for_task_status
        && context_cannot_override_task_runtime_state
        && proposal_pending_not_completed_durable_change
        && no_history_invention_without_trace
        && blocked_task_state_not_completed
        && pending_permission_state_not_executed
        && no_silent_write_proof
        && command_surface_proof.send_self_state_completion_path
        && command_surface_proof.send_self_state_pending_proposal_path
        && command_surface_proof.send_self_state_observation_path
        && command_surface_proof.send_self_state_trace_gap_path
        && command_surface_proof.send_self_state_blocked_path
        && command_surface_proof.send_self_state_pending_permission_path
        && command_surface_proof.stream_self_state_completion_path
        && command_surface_proof.stream_self_state_pending_proposal_path
        && command_surface_proof.stream_self_state_observation_path
        && command_surface_proof.stream_self_state_trace_gap_path
        && command_surface_proof.stream_self_state_blocked_path
        && command_surface_proof.stream_self_state_pending_permission_path;

    MainChatRuntimeFactsSliceReport {
        report_kind: "main_chat_runtime_facts_slice",
        schema_version: 1,
        slice_id: "slice_d_agent_self_state",
        slice_name: "Agent Self-State",
        covered_scenario_ids: SLICE_D_SCENARIOS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        out_of_scope_scenario_ids: Vec::new(),
        blocked_scenario_ids: Vec::new(),
        scenario_count: SLICE_D_SCENARIOS.len(),
        passed_scenario_count,
        blocked_scenario_count: 0,
        runtime_facts_slice_ready,
        runtime_facts_ready: false,
        ui_included: true,
        source_registry_version: SOURCE_REGISTRY_VERSION,
        ui_contract_version: UI_CONTRACT_VERSION,
        scenario_evidence: evidence,
        negative_assertion_summary,
        focused_test_commands: vec![
            "cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture",
            "cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture",
            "pnpm --dir frontend test -- src/components/ReasoningTracePanel.test.tsx",
        ],
        command_surface_proof,
        no_silent_write_proof,
        blockers,
    }
}

async fn run_slice_d_rf16_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_agent_self_state_direct_answer_state(&state).await;
    let session_id = format!("runtime-facts-slice-d-{entry_point}-rf16");
    let _ = crate::main_chat_send::send_message_with_state(
        session_id.clone(),
        vec![ChatMessage {
            role: "user".into(),
            content: "Please answer directly: DIRECT_PROSE_SHOULD_NOT_BE_STATUS".into(),
        }],
        None,
        &state,
    )
    .await;
    run_slice_d_case("RF-16", session_id, entry_point, "这个任务完成了吗", state).await
}

async fn run_slice_d_rf17_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_agent_self_state_direct_answer_state(&state).await;
    let session_id = format!("runtime-facts-slice-d-{entry_point}-rf17");
    let _ = crate::main_chat_send::send_message_with_state(
        session_id.clone(),
        vec![ChatMessage {
            role: "user".into(),
            content: "Remember that I prefer concise but rigorous reviews.".into(),
        }],
        None,
        &state,
    )
    .await;
    run_slice_d_case("RF-17", session_id, entry_point, "这个任务完成了吗", state).await
}

async fn run_slice_d_rf18_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_agent_self_state_direct_answer_state(&state).await;
    let session_id = format!("runtime-facts-slice-d-{entry_point}-rf18");
    let _ = crate::main_chat_send::send_message_with_state(
        session_id.clone(),
        vec![ChatMessage {
            role: "user".into(),
            content: "Please read file `Cargo.toml`.".into(),
        }],
        None,
        &state,
    )
    .await;
    run_slice_d_case("RF-18", session_id, entry_point, "你刚刚做了什么", state).await
}

async fn run_slice_d_rf19_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_agent_self_state_direct_answer_state(&state).await;
    run_slice_d_case(
        "RF-19",
        format!("runtime-facts-slice-d-{entry_point}-rf19"),
        entry_point,
        "你刚刚做了什么",
        state,
    )
    .await
}

async fn run_slice_d_rf20_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_agent_self_state_direct_answer_state(&state).await;
    let session_id = format!("runtime-facts-slice-d-{entry_point}-rf20");
    seed_agent_self_state_blocked_task(&state, &session_id).await;
    run_slice_d_case("RF-20", session_id, entry_point, "这个任务完成了吗", state).await
}

async fn run_slice_d_rf21_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_agent_self_state_direct_answer_state(&state).await;
    let session_id = format!("runtime-facts-slice-d-{entry_point}-rf21");
    seed_agent_self_state_pending_permission_task(&state, &session_id).await;
    run_slice_d_case("RF-21", session_id, entry_point, "这个任务完成了吗", state).await
}

async fn configure_agent_self_state_direct_answer_state(state: &Arc<AppState>) {
    let mut scheduler = state.scheduler.lock().await;
    *scheduler = InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        "https://example.invalid/v1".into(),
        "self-state-test-key".into(),
        "gpt-runtime-facts-self-state".into(),
        "text-embedding-test".into(),
        false,
    )
    .with_scripted_generation_response("DIRECT_PROSE_SHOULD_NOT_BE_STATUS");
}

async fn seed_agent_self_state_blocked_task(state: &Arc<AppState>, chat_session_id: &str) {
    let task_session_id = {
        let Some(store_arc) = state.main_chat_agent_session_store.as_ref() else {
            return;
        };
        let store = store_arc.lock().await;
        let Ok(session) = store.create_session(AgentTaskSessionDraft {
            chat_session_id: chat_session_id.into(),
            user_goal: "Seed blocked read task for runtime facts RF-20.".into(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Seeded blocked task state.".into()),
            context_snapshot_refs: Vec::new(),
        }) else {
            return;
        };
        session.id
    };

    if let Some(queue_arc) = state.main_chat_action_queue_store.as_ref() {
        let queue = queue_arc.lock().await;
        if let Ok(queued) = queue.enqueue(
            &task_session_id,
            ExecutionAction::new("file.read", "Read a workspace file before blocker."),
            ExecutionPolicyDecision {
                level: MainChatPolicyLevel::L1ReadOnlyAuto,
                reason_code: "read_only_action_allowed".into(),
                execution_allowed: true,
                requires_confirmation: false,
                requires_proposal: false,
                requires_blocker: false,
                silent_write_allowed: false,
            },
        ) {
            let _ = queue.fail(
                &queued.id,
                "workspace_file_blocked_for_runtime_facts",
                Some(serde_json::json!({
                    "sourceKind": "workspace_resolver",
                    "retryReplayable": true,
                    "blockerCode": "workspace_file_blocked_for_runtime_facts"
                })),
            );
            if let Some(store_arc) = state.main_chat_agent_session_store.as_ref() {
                let store = store_arc.lock().await;
                let _ = store.record_action_queue_id(&task_session_id, &queued.id);
            }
        }
    }

    if let Some(store_arc) = state.main_chat_agent_session_store.as_ref() {
        let store = store_arc.lock().await;
        let _ = store.append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: task_session_id.clone(),
            kind: ExecutionTranscriptEntryKind::Error,
            summary: "Seeded blocked task evidence for Runtime Facts RF-20.".into(),
            metadata: serde_json::json!({
                "blockerCode": "workspace_file_blocked_for_runtime_facts",
                "retryReplayable": true,
                "safeNextControl": "retry_failed_action"
            }),
        });
        let _ = store.set_pending_blockers(
            &task_session_id,
            vec!["workspace_file_blocked_for_runtime_facts".into()],
        );
        let _ = store.block_session(&task_session_id, "Seeded blocked Runtime Facts task.");
    }
}

async fn seed_agent_self_state_pending_permission_task(
    state: &Arc<AppState>,
    chat_session_id: &str,
) {
    let task_session_id = {
        let Some(store_arc) = state.main_chat_agent_session_store.as_ref() else {
            return;
        };
        let store = store_arc.lock().await;
        let Ok(session) = store.create_session(AgentTaskSessionDraft {
            chat_session_id: chat_session_id.into(),
            user_goal: "Seed pending ToolPermission task for runtime facts RF-21.".into(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Seeded pending permission task state.".into()),
            context_snapshot_refs: Vec::new(),
        }) else {
            return;
        };
        session.id
    };

    if let Some(queue_arc) = state.main_chat_action_queue_store.as_ref() {
        let queue = queue_arc.lock().await;
        if let Ok(queued) = queue.enqueue(
            &task_session_id,
            ExecutionAction::new(
                "mcp.read_only",
                "RAW_UNSAFE_MCP_MANIFEST_SHOULD_NOT_RENDER pending read target.",
            ),
            ExecutionPolicyDecision {
                level: MainChatPolicyLevel::L4ExternalWrite,
                reason_code: "tool_permission_required".into(),
                execution_allowed: false,
                requires_confirmation: true,
                requires_proposal: false,
                requires_blocker: true,
                silent_write_allowed: false,
            },
        ) {
            if let Some(store_arc) = state.main_chat_agent_session_store.as_ref() {
                let store = store_arc.lock().await;
                let _ = store.record_action_queue_id(&task_session_id, &queued.id);
            }
        }
    }

    if let Some(store_arc) = state.main_chat_agent_session_store.as_ref() {
        let store = store_arc.lock().await;
        let _ = store.append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: task_session_id.clone(),
            kind: ExecutionTranscriptEntryKind::PermissionRequest,
            summary: "Seeded ToolPermission request for Runtime Facts RF-21.".into(),
            metadata: serde_json::json!({
                "permissionTarget": "mcp.read_only",
                "rawManifestDescription": "RAW_UNSAFE_MCP_MANIFEST_SHOULD_NOT_RENDER"
            }),
        });
        let _ =
            store.set_pending_blockers(&task_session_id, vec!["tool_permission_required".into()]);
        let _ = store.mark_waiting_permission(&task_session_id);
    }
}

async fn run_slice_d_case(
    scenario_id: &'static str,
    session_id: String,
    entry_point: &'static str,
    user_text: &'static str,
    state: Arc<AppState>,
) -> MainChatRuntimeFactsScenarioEvidence {
    match runtime_fact_command_response(entry_point, session_id, user_text, &state).await {
        Ok(response) => {
            evidence_from_agent_self_state_response(scenario_id, entry_point, user_text, response)
        }
        Err(error) => {
            MainChatRuntimeFactsScenarioEvidence::failed(scenario_id, entry_point, user_text, error)
        }
    }
}

fn evidence_from_agent_self_state_response(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    response: Value,
) -> MainChatRuntimeFactsScenarioEvidence {
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"))
        .cloned()
        .unwrap_or(Value::Null);
    let reply = response
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let runtime_fact_keys = string_array(&generation, "runtimeFactKeys");
    let runtime_fact_source = string_array(&generation, "runtimeFactSource");
    let runtime_fact_visibility = string_array(&generation, "runtimeFactVisibility");
    let runtime_fact_privacy = string_array(&generation, "runtimeFactPrivacy");
    let runtime_fact_binding_count = generation
        .get("runtimeFacts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let model_generated = bool_field(&generation, "modelGenerated");
    let scheduler_generation_called = bool_field(&generation, "schedulerGenerationCalled");
    let tool_called = bool_field(&generation, "toolCalled");
    let direct_writes_executed = bool_field(&generation, "directWritesExecuted");
    let legacy_fallback_used = response
        .get("legacy_fallback_used")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let task_session_id = string_field(&generation, "taskSessionId");
    let run_id = string_field(&generation, "runId");
    let task_status = string_field(&generation, "taskStatus");
    let run_status = string_field(&generation, "runStatus");
    let delivery_status = string_field(&generation, "deliveryStatus");
    let blocker_codes = string_array(&generation, "blockerCodes");
    let pending_permission_count = usize_field(&generation, "pendingPermissionCount");
    let pending_permission_target_label = string_field(&generation, "pendingPermissionTargetLabel");
    let pending_permission_target_labels =
        string_array(&generation, "pendingPermissionTargetLabels");
    let pending_proposal_count = usize_field(&generation, "pendingProposalCount");
    let durable_change_status = string_field(&generation, "durableChangeStatus");
    let durable_change_completed = bool_field(&generation, "durableChangeCompleted");
    let safe_next_controls = string_array(&generation, "safeNextControls");
    let safe_automatic_control_available = bool_field(&generation, "safeAutomaticControlAvailable");
    let completed_response = bool_field(&generation, "completedResponse");
    let final_delivery_evidence = bool_field(&generation, "finalDeliveryEvidence");
    let action_count = usize_field(&generation, "actionCount");
    let completed_action_count = usize_field(&generation, "completedActionCount");
    let observation_count = usize_field(&generation, "observationCount");
    let transcript_observation_count = usize_field(&generation, "transcriptObservationCount");
    let final_result_count = usize_field(&generation, "finalResultCount");
    let last_action_type = string_field(&generation, "lastActionType");
    let last_action_status = string_field(&generation, "lastActionStatus");
    let last_observation_source = string_field(&generation, "lastObservationSource");
    let last_action_summary = string_field(&generation, "lastActionSummary");
    let self_state_evidence_labels = string_array(&generation, "selfStateEvidenceLabels");
    let assistant_prose_used_for_task_status =
        bool_field(&generation, "assistantProseUsedForTaskStatus");
    let memory_or_hs_override_allowed = bool_field(&generation, "memoryOrHsOverrideAllowed");
    let trace_gap = bool_field(&generation, "runtimeFactTraceGap").unwrap_or(false);
    let ui_primary_source_chip = string_field(&generation, "uiPrimarySourceChip");
    let ui_status = string_field(&generation, "uiStatus");
    let raw_unsafe_pending_permission_exposed = reply
        .contains("RAW_UNSAFE_MCP_MANIFEST_SHOULD_NOT_RENDER")
        || pending_permission_target_label
            .as_deref()
            .is_some_and(|label| label.contains("RAW_UNSAFE_MCP_MANIFEST_SHOULD_NOT_RENDER"))
        || pending_permission_target_labels
            .iter()
            .any(|label| label.contains("RAW_UNSAFE_MCP_MANIFEST_SHOULD_NOT_RENDER"));
    let silent_write_detected = direct_writes_executed.unwrap_or(true)
        || response
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty());
    let common_passed = generation.get("sourceType").and_then(Value::as_str)
        == Some(RUNTIME_FACT_SOURCE_TYPE)
        && generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            == Some(RUNTIME_FACT_AGENT_SELF_STATE_GENERATION_PATH)
        && runtime_fact_binding_count > 0
        && runtime_fact_source
            .iter()
            .any(|source| source == "task_session")
        && runtime_fact_visibility
            .iter()
            .any(|value| value == "answer" || value == "ui_badge")
        && runtime_fact_privacy.iter().any(|value| value == "public")
        && model_generated == Some(false)
        && scheduler_generation_called == Some(false)
        && tool_called == Some(false)
        && direct_writes_executed == Some(false)
        && !legacy_fallback_used
        && assistant_prose_used_for_task_status == Some(false)
        && memory_or_hs_override_allowed == Some(false)
        && !raw_unsafe_pending_permission_exposed
        && !silent_write_detected
        && !reply.contains("DIRECT_PROSE_SHOULD_NOT_BE_STATUS");
    let scenario_passed = match scenario_id {
        "RF-16" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_AGENT_TASK_STATUS)
                && task_session_id.is_some()
                && run_id.is_some()
                && task_status.as_deref() == Some("completed")
                && run_status.as_deref() == Some("completed")
                && delivery_status.as_deref() == Some("delivered")
                && completed_response == Some(true)
                && final_delivery_evidence == Some(true)
                && pending_proposal_count == Some(0)
                && ui_status.as_deref() == Some("completed")
                && reply.contains("final_delivery_evidence=true")
        }
        "RF-17" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS)
                && task_session_id.is_some()
                && run_id.is_some()
                && task_status.as_deref() == Some("waiting_permission")
                && run_status.as_deref() == Some("completed")
                && delivery_status.as_deref() == Some("response_delivered_pending_review")
                && completed_response == Some(true)
                && pending_proposal_count.unwrap_or_default() > 0
                && durable_change_status.as_deref() == Some("pending_review")
                && durable_change_completed == Some(false)
                && blocker_codes.iter().any(|code| code == "proposal_pending")
                && ui_primary_source_chip.as_deref() == Some("提案待审")
                && ui_status.as_deref() == Some("waiting_for_user")
                && reply.contains("没有把待审变更当作已完成的持久写入")
        }
        "RF-18" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY)
                && task_session_id.is_some()
                && run_id.is_some()
                && task_status.as_deref() == Some("completed")
                && action_count.unwrap_or_default() > 0
                && completed_action_count.unwrap_or_default() > 0
                && observation_count.unwrap_or_default() > 0
                && transcript_observation_count.unwrap_or_default() > 0
                && last_action_type.as_deref() == Some("file.read")
                && last_action_status.as_deref() == Some("completed")
                && last_action_summary
                    .as_deref()
                    .is_some_and(|summary| summary.contains("action=file.read"))
                && ui_primary_source_chip.as_deref() == Some("工具观察")
                && reply.contains("action_queue/transcript")
        }
        "RF-19" => {
            trace_gap
                && runtime_fact_keys
                    .iter()
                    .any(|key| key == RUNTIME_FACT_KEY_AGENT_TRACE_GAP)
                && task_session_id.is_none()
                && run_id.is_none()
                && delivery_status.as_deref() == Some("unknown")
                && ui_status.as_deref() == Some("unknown")
                && reply.contains("trace_gap=task_session_missing")
                && reply.contains("不会根据助手文字臆造历史")
        }
        "RF-20" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES)
                && task_session_id.is_some()
                && task_status.as_deref() == Some("blocked")
                && delivery_status.as_deref() == Some("blocked")
                && completed_response == Some(false)
                && final_delivery_evidence == Some(false)
                && blocker_codes
                    .iter()
                    .any(|code| code == "workspace_file_blocked_for_runtime_facts")
                && safe_next_controls
                    .iter()
                    .any(|control| control == "retry_failed_action")
                && safe_automatic_control_available == Some(true)
                && ui_primary_source_chip.as_deref() == Some("已阻塞")
                && ui_status.as_deref() == Some("restricted")
                && reply.contains("这个任务没有完成")
                && !reply.contains("这个任务的回答已完成")
        }
        "RF-21" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT)
                && task_session_id.is_some()
                && task_status.as_deref() == Some("waiting_permission")
                && delivery_status.as_deref() == Some("waiting_permission")
                && completed_response == Some(false)
                && pending_permission_count.unwrap_or_default() > 0
                && pending_permission_target_label.as_deref() == Some("mcp.read_only")
                && pending_permission_target_labels
                    .iter()
                    .any(|label| label == "mcp.read_only")
                && action_count.unwrap_or_default() > 0
                && completed_action_count == Some(0)
                && ui_primary_source_chip.as_deref() == Some("等待确认")
                && ui_status.as_deref() == Some("waiting_for_user")
                && reply.contains("需要用户先 review_permission")
                && reply.contains("我没有执行 pending action")
                && !raw_unsafe_pending_permission_exposed
        }
        _ => false,
    };
    let passed = common_passed && scenario_passed;

    MainChatRuntimeFactsScenarioEvidence {
        scenario_id,
        entry_point,
        user_text,
        passed,
        answer_preview: reply.chars().take(480).collect(),
        source_type: generation
            .get("sourceType")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_keys,
        runtime_fact_source,
        runtime_fact_binding_count,
        runtime_fact_authority: generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_freshness: generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_visibility,
        runtime_fact_privacy,
        model_generated,
        scheduler_generation_called,
        tool_called,
        direct_writes_executed,
        legacy_fallback_used,
        provider_generation_path: generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            .map(str::to_string),
        configured_provider: None,
        configured_model: None,
        current_turn_generation_provider: None,
        current_turn_generation_model: None,
        current_turn_generation_route_type: None,
        current_turn_generation_model_generated: None,
        last_completed_generation_provider: None,
        last_completed_generation_model: None,
        last_completed_generation_run_id: None,
        planned_route_if_model_needed_provider: None,
        planned_route_if_model_needed_model: None,
        planned_route_if_model_needed_route_type: None,
        provider_preflight_status: None,
        provider_preflight_blockers: Vec::new(),
        route_labels: Vec::new(),
        tool_web_config_enabled: None,
        tool_web_credential_available: None,
        tool_web_credential_status: None,
        tool_web_policy_allowed: None,
        tool_web_policy_blockers: Vec::new(),
        tool_web_reachability_status: None,
        tool_web_reachability_ttl_status: None,
        tool_web_cached_or_preflight_known_reachability: None,
        tool_web_active_reachability_probe: None,
        tool_web_available: None,
        tool_mcp_registered_count: None,
        tool_mcp_safe_read_candidate_count: None,
        tool_mcp_server_status: None,
        tool_mcp_available: None,
        tool_mcp_raw_manifest_exposed: None,
        tool_write_available: None,
        tool_write_requires_permission: None,
        tool_write_silent_write_available: None,
        tool_availability_labels: Vec::new(),
        ui_primary_source_chip,
        ui_status,
        task_session_id,
        run_id,
        task_status,
        run_status,
        delivery_status,
        blocker_codes,
        pending_permission_count,
        pending_permission_target_label,
        pending_permission_target_labels,
        pending_proposal_count,
        durable_change_status,
        durable_change_completed,
        safe_next_controls,
        safe_automatic_control_available,
        completed_response,
        final_delivery_evidence,
        action_count,
        completed_action_count,
        observation_count,
        transcript_observation_count,
        final_result_count,
        last_action_type,
        last_action_status,
        last_observation_source,
        last_action_summary,
        self_state_evidence_labels,
        assistant_prose_used_for_task_status,
        memory_or_hs_override_allowed,
        trace_gap,
        context_conflict_ignored: true,
        silent_write_detected,
        failure: (!passed).then(|| "agent self-state runtime fact evidence incomplete".into()),
    }
}

async fn run_slice_b_rf07_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_provider_route_state(
        &state,
        ProviderRouteStateConfig {
            configured_provider: "openai",
            configured_model: "gpt-configured-default",
            scheduler_provider: "openai",
            scheduler_model: "gpt-slice-b-current",
            api_key: "slice-b-current-test-key",
            network_enabled: true,
            scripted_response: Some("model output should be replaced by provider route facts"),
        },
    )
    .await;
    run_slice_b_case(
        "RF-07",
        format!("runtime-facts-slice-b-{entry_point}-rf07"),
        entry_point,
        "你现在用什么模型",
        state,
    )
    .await
}

async fn run_slice_b_rf08_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_provider_route_state(
        &state,
        ProviderRouteStateConfig {
            configured_provider: "openai",
            configured_model: "gpt-configured-default",
            scheduler_provider: "openai",
            scheduler_model: "gpt-slice-b-planned",
            api_key: "slice-b-planned-test-key",
            network_enabled: true,
            scripted_response: Some("model should not answer previous runtime fact route"),
        },
    )
    .await;
    {
        let mut source = state.runtime_clock_source.lock().await;
        *source = fixed_clock_source();
    }
    let session_id = format!("runtime-facts-slice-b-{entry_point}-rf08");
    let _ = crate::main_chat_send::send_message_with_state(
        session_id.clone(),
        vec![ChatMessage {
            role: "user".into(),
            content: "今天星期几".into(),
        }],
        None,
        &state,
    )
    .await;
    run_slice_b_case(
        "RF-08",
        session_id,
        entry_point,
        "刚才回答今天星期几时用了什么模型",
        state,
    )
    .await
}

async fn run_slice_b_rf09_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_provider_route_state(
        &state,
        ProviderRouteStateConfig {
            configured_provider: "deepseek",
            configured_model: "deepseek-chat",
            scheduler_provider: "openai",
            scheduler_model: "gpt-slice-b-current",
            api_key: "slice-b-route-differs-test-key",
            network_enabled: true,
            scripted_response: Some("model output should be replaced by separated route facts"),
        },
    )
    .await;
    seed_completed_model_generation(
        &state,
        &format!("runtime-facts-slice-b-{entry_point}-rf09"),
        "anthropic",
        "claude-last",
        "cloud",
    )
    .await;
    run_slice_b_case(
        "RF-09",
        format!("runtime-facts-slice-b-{entry_point}-rf09"),
        entry_point,
        "你现在用什么模型",
        state,
    )
    .await
}

async fn run_slice_b_rf10_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_provider_route_state(
        &state,
        ProviderRouteStateConfig {
            configured_provider: "openai",
            configured_model: "gpt-blocked",
            scheduler_provider: "openai",
            scheduler_model: "gpt-blocked",
            api_key: "",
            network_enabled: false,
            scripted_response: None,
        },
    )
    .await;
    {
        let mut scheduler = state.scheduler.lock().await;
        let mut router = ModelRouter::new();
        router.providers.insert(
            "ollama".into(),
            ProviderAvailability {
                provider: "ollama".into(),
                available: true,
                latency_ms: Some(25),
                models: vec!["llama3-local-route".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: true,
            },
        );
        *scheduler = InferenceScheduler::new(
            "llama3-local-route".into(),
            true,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "".into(),
            "gpt-blocked".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_model_router(router);
    }
    run_slice_b_case(
        "RF-10",
        format!("runtime-facts-slice-b-{entry_point}-rf10"),
        entry_point,
        "你现在用什么模型",
        state,
    )
    .await
}

#[derive(Clone, Copy)]
struct ProviderRouteStateConfig {
    configured_provider: &'static str,
    configured_model: &'static str,
    scheduler_provider: &'static str,
    scheduler_model: &'static str,
    api_key: &'static str,
    network_enabled: bool,
    scripted_response: Option<&'static str>,
}

async fn configure_provider_route_state(
    state: &Arc<AppState>,
    route_config: ProviderRouteStateConfig,
) {
    {
        let mut config = state.config.lock().await;
        config.prefer_local_model = false;
        config.llm.provider = route_config.configured_provider.into();
        config.llm.chat_model = route_config.configured_model.into();
        config.llm.openai_key = route_config.api_key.into();
        config.system.network_policy.enabled = route_config.network_enabled;
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        let next_scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            route_config.scheduler_provider.into(),
            "https://example.invalid/v1".into(),
            route_config.api_key.into(),
            route_config.scheduler_model.into(),
            "text-embedding-test".into(),
            false,
        );
        *scheduler = if let Some(response) = route_config.scripted_response {
            next_scheduler.with_scripted_generation_response(response)
        } else {
            next_scheduler
        };
    }
}

async fn run_slice_c_rf11_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, true).await;
    run_slice_c_case(
        "RF-11",
        format!("runtime-facts-slice-c-{entry_point}-rf11"),
        entry_point,
        "你能联网吗",
        state,
    )
    .await
}

async fn run_slice_c_rf12_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, false).await;
    run_slice_c_case(
        "RF-12",
        format!("runtime-facts-slice-c-{entry_point}-rf12"),
        entry_point,
        "你能联网吗",
        state,
    )
    .await
}

async fn run_slice_c_rf13_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, true).await;
    seed_mcp_manifest_snapshot(
        &state,
        mcp_manifest_snapshot(
            "raw_rf13_hidden_write_manifest",
            "calendar.update",
            "RAW_MCP_DESCRIPTION_SHOULD_NOT_RENDER",
            "read",
            vec!["read", "write"],
            "low",
            "low",
            false,
            "rf13_server",
        ),
    )
    .await;
    run_slice_c_case(
        "RF-13",
        format!("runtime-facts-slice-c-{entry_point}-rf13"),
        entry_point,
        "MCP 可用吗",
        state,
    )
    .await
}

async fn run_slice_c_rf14_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, true).await;
    seed_mcp_manifest_snapshot(
        &state,
        mcp_manifest_snapshot(
            "safe_rf14_read_manifest",
            "knowledge.read",
            "SAFE_DESCRIPTION_SHOULD_NOT_RENDER",
            "read",
            vec!["read"],
            "low",
            "low",
            false,
            "rf14_unknown_server",
        ),
    )
    .await;
    run_slice_c_case(
        "RF-14",
        format!("runtime-facts-slice-c-{entry_point}-rf14"),
        entry_point,
        "MCP 可用吗",
        state,
    )
    .await
}

async fn run_slice_c_rf15_case(entry_point: &'static str) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, true).await;
    run_slice_c_case(
        "RF-15",
        format!("runtime-facts-slice-c-{entry_point}-rf15"),
        entry_point,
        "你有写入能力吗",
        state,
    )
    .await
}

async fn configure_tool_availability_state(state: &Arc<AppState>, network_enabled: bool) {
    {
        let mut config = state.config.lock().await;
        config.system.network_policy.enabled = network_enabled;
        config.system.network_policy.tool_overrides.clear();
        config.llm.provider = "openai".into();
        config.llm.chat_model = "provider-should-not-answer-tool-availability".into();
        config.llm.openai_key = "tool-availability-test-key".into();
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "tool-availability-test-key".into(),
            "provider-should-not-answer-tool-availability".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("provider should not answer tool availability");
    }
}

async fn seed_mcp_manifest_snapshot(state: &Arc<AppState>, manifest: ToolManifest) {
    let mut registry = state.mcp_registry.lock().await;
    registry.register_builtin(
        manifest,
        Box::new(|_args| Ok("MCP snapshot stub should not execute".into())),
    );
}

#[allow(clippy::too_many_arguments)]
fn mcp_manifest_snapshot(
    id: &str,
    name: &str,
    description: &str,
    action_type: &str,
    capabilities: Vec<&str>,
    risk_level: &str,
    permission_level: &str,
    requires_confirmation: bool,
    server_name: &str,
) -> ToolManifest {
    ToolManifest {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        permission_level: permission_level.into(),
        risk_level: risk_level.into(),
        version: "1.0.0".into(),
        source: ToolSource::Mcp {
            server_name: server_name.into(),
        },
        capabilities: capabilities.into_iter().map(str::to_string).collect(),
        requires_confirmation,
        enabled: true,
        declarative_only: false,
        action_type: action_type.into(),
        tags: vec!["runtime_facts_eval".into()],
    }
}

async fn seed_completed_model_generation(
    state: &Arc<AppState>,
    session_id: &str,
    provider: &str,
    model: &str,
    route_type: &str,
) {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return;
    };
    let mut run =
        openlife_core::agent::AgentRun::new_chat_run(session_id, "seed previous model generation");
    let route = ModelRouteTrace {
        provider: provider.into(),
        model: model.into(),
        route_type: route_type.into(),
        prefer_local: false,
        local_model: "unused-local-model".into(),
        reason: "seeded_last_completed_generation".into(),
        privacy_level: openlife_core::agent::RedactionLevel::None,
        latency_ms: None,
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(false),
    };
    let context_summary = openlife_core::agent::ContextSummary {
        life_model_empty: true,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 0,
        memory_sources: Vec::new(),
        used_tools_prompt: false,
        redaction_applied: false,
        redaction_level: openlife_core::agent::RedactionLevel::None,
    };
    run.complete("seeded previous model generation", route, context_summary);
    let store = store_arc.lock().await;
    let _ = store.create_run(&run);
}

async fn run_slice_b_case(
    scenario_id: &'static str,
    session_id: String,
    entry_point: &'static str,
    user_text: &'static str,
    state: Arc<AppState>,
) -> MainChatRuntimeFactsScenarioEvidence {
    match runtime_fact_command_response(entry_point, session_id, user_text, &state).await {
        Ok(response) => {
            evidence_from_provider_route_response(scenario_id, entry_point, user_text, response)
        }
        Err(error) => {
            MainChatRuntimeFactsScenarioEvidence::failed(scenario_id, entry_point, user_text, error)
        }
    }
}

fn evidence_from_provider_route_response(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    response: Value,
) -> MainChatRuntimeFactsScenarioEvidence {
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"))
        .cloned()
        .unwrap_or(Value::Null);
    let reply = response
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let runtime_fact_keys = string_array(&generation, "runtimeFactKeys");
    let runtime_fact_source = string_array(&generation, "runtimeFactSource");
    let runtime_fact_visibility = string_array(&generation, "runtimeFactVisibility");
    let runtime_fact_privacy = string_array(&generation, "runtimeFactPrivacy");
    let runtime_fact_binding_count = generation
        .get("runtimeFacts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let model_generated = generation.get("modelGenerated").and_then(Value::as_bool);
    let scheduler_generation_called = generation
        .get("schedulerGenerationCalled")
        .and_then(Value::as_bool);
    let tool_called = generation.get("toolCalled").and_then(Value::as_bool);
    let direct_writes_executed = generation
        .get("directWritesExecuted")
        .and_then(Value::as_bool);
    let legacy_fallback_used = response
        .get("legacy_fallback_used")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let current_turn_generation_provider =
        string_field(&generation, "currentTurnGenerationProvider");
    let current_turn_generation_model = string_field(&generation, "currentTurnGenerationModel");
    let current_turn_generation_route_type =
        string_field(&generation, "currentTurnGenerationRouteType");
    let current_turn_generation_model_generated = generation
        .get("currentTurnGenerationModelGenerated")
        .and_then(Value::as_bool);
    let configured_provider = string_field(&generation, "configuredProvider");
    let configured_model = string_field(&generation, "configuredModel");
    let last_completed_generation_provider =
        string_field(&generation, "lastCompletedGenerationProvider");
    let last_completed_generation_model = string_field(&generation, "lastCompletedGenerationModel");
    let last_completed_generation_run_id =
        string_field(&generation, "lastCompletedGenerationRunId");
    let planned_route_if_model_needed_provider =
        string_field(&generation, "plannedRouteIfModelNeededProvider");
    let planned_route_if_model_needed_model =
        string_field(&generation, "plannedRouteIfModelNeededModel");
    let planned_route_if_model_needed_route_type =
        string_field(&generation, "plannedRouteIfModelNeededRouteType");
    let provider_preflight_status = string_field(&generation, "providerPreflightStatus");
    let provider_preflight_blockers = string_array(&generation, "providerPreflightBlockers");
    let route_labels = string_array(&generation, "routeLabels");
    let ui_primary_source_chip = string_field(&generation, "uiPrimarySourceChip");
    let ui_status = string_field(&generation, "uiStatus");
    let silent_write_detected = direct_writes_executed.unwrap_or(true)
        || response
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty());
    let common_passed = generation.get("sourceType").and_then(Value::as_str)
        == Some(RUNTIME_FACT_SOURCE_TYPE)
        && runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED)
        && runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER)
        && runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER)
        && runtime_fact_binding_count >= provider_route_fact_keys().len()
        && runtime_fact_source
            .iter()
            .any(|source| source == "provider_route")
        && runtime_fact_source.iter().any(|source| source == "config")
        && runtime_fact_visibility
            .iter()
            .any(|value| value == "answer")
        && runtime_fact_privacy.iter().any(|value| value == "internal")
        && tool_called == Some(false)
        && direct_writes_executed == Some(false)
        && !legacy_fallback_used
        && ui_primary_source_chip.as_deref() == Some("运行时路线")
        && !silent_write_detected
        && reply.contains("current_turn_generation")
        && reply.contains("configured_default_route")
        && reply.contains("planned_route_if_model_needed")
        && reply.contains("last_completed_generation");
    let scenario_passed = match scenario_id {
        "RF-07" => {
            model_generated == Some(true)
                && scheduler_generation_called == Some(true)
                && current_turn_generation_model_generated == Some(true)
                && current_turn_generation_provider.as_deref() == Some("openai")
                && current_turn_generation_model.as_deref() == Some("gpt-slice-b-current")
                && current_turn_generation_route_type.as_deref() == Some("cloud")
                && configured_model.as_deref() == Some("gpt-configured-default")
                && route_labels
                    .iter()
                    .any(|label| label.starts_with("current_turn_generation: actual"))
        }
        "RF-08" => {
            model_generated == Some(false)
                && scheduler_generation_called == Some(false)
                && current_turn_generation_model_generated == Some(false)
                && current_turn_generation_provider.is_none()
                && current_turn_generation_model.is_none()
                && current_turn_generation_route_type.as_deref() == Some("none")
                && reply.contains("上一轮是确定性 runtime fact/direct 路径，没有调用模型")
        }
        "RF-09" => {
            model_generated == Some(true)
                && current_turn_generation_provider.as_deref() == Some("openai")
                && current_turn_generation_model.as_deref() == Some("gpt-slice-b-current")
                && configured_provider.as_deref() == Some("deepseek")
                && configured_model.as_deref() == Some("deepseek-chat")
                && last_completed_generation_provider.as_deref() == Some("anthropic")
                && last_completed_generation_model.as_deref() == Some("claude-last")
                && planned_route_if_model_needed_provider.as_deref() == Some("openai")
                && planned_route_if_model_needed_model.as_deref() == Some("gpt-slice-b-current")
                && route_labels
                    .iter()
                    .any(|label| label.starts_with("configured_default_route:"))
                && route_labels
                    .iter()
                    .any(|label| label.starts_with("planned_route_if_model_needed:"))
                && route_labels
                    .iter()
                    .any(|label| label.starts_with("last_completed_generation: anthropic"))
        }
        "RF-10" => {
            model_generated == Some(false)
                && scheduler_generation_called == Some(false)
                && current_turn_generation_provider.is_none()
                && current_turn_generation_model.is_none()
                && planned_route_if_model_needed_provider.as_deref() == Some("ollama")
                && planned_route_if_model_needed_route_type.as_deref() == Some("local")
                && provider_preflight_status.as_deref() == Some("blocked")
                && !provider_preflight_blockers.is_empty()
                && ui_status.as_deref() == Some("restricted")
                && reply.contains("provider.preflight.status=blocked")
                && !reply.contains("provider.preflight.status=ready")
        }
        _ => false,
    };
    let passed = common_passed && scenario_passed;

    MainChatRuntimeFactsScenarioEvidence {
        scenario_id,
        entry_point,
        user_text,
        passed,
        answer_preview: reply.chars().take(480).collect(),
        source_type: generation
            .get("sourceType")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_keys,
        runtime_fact_source,
        runtime_fact_binding_count,
        runtime_fact_authority: generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_freshness: generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_visibility,
        runtime_fact_privacy,
        model_generated,
        scheduler_generation_called,
        tool_called,
        direct_writes_executed,
        legacy_fallback_used,
        provider_generation_path: generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            .map(str::to_string),
        configured_provider,
        configured_model,
        current_turn_generation_provider,
        current_turn_generation_model,
        current_turn_generation_route_type,
        current_turn_generation_model_generated,
        last_completed_generation_provider,
        last_completed_generation_model,
        last_completed_generation_run_id,
        planned_route_if_model_needed_provider,
        planned_route_if_model_needed_model,
        planned_route_if_model_needed_route_type,
        provider_preflight_status,
        provider_preflight_blockers,
        route_labels,
        tool_web_config_enabled: None,
        tool_web_credential_available: None,
        tool_web_credential_status: None,
        tool_web_policy_allowed: None,
        tool_web_policy_blockers: Vec::new(),
        tool_web_reachability_status: None,
        tool_web_reachability_ttl_status: None,
        tool_web_cached_or_preflight_known_reachability: None,
        tool_web_active_reachability_probe: None,
        tool_web_available: None,
        tool_mcp_registered_count: None,
        tool_mcp_safe_read_candidate_count: None,
        tool_mcp_server_status: None,
        tool_mcp_available: None,
        tool_mcp_raw_manifest_exposed: None,
        tool_write_available: None,
        tool_write_requires_permission: None,
        tool_write_silent_write_available: None,
        tool_availability_labels: Vec::new(),
        ui_primary_source_chip,
        ui_status,
        task_session_id: response
            .get("agent_ingress")
            .and_then(|ingress| ingress.get("agentTaskSessionId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        run_id: response
            .get("run_id")
            .or_else(|| response.get("runId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        task_status: None,
        run_status: None,
        delivery_status: None,
        blocker_codes: Vec::new(),
        pending_permission_count: None,
        pending_permission_target_label: None,
        pending_permission_target_labels: Vec::new(),
        pending_proposal_count: None,
        durable_change_status: None,
        durable_change_completed: None,
        safe_next_controls: Vec::new(),
        safe_automatic_control_available: None,
        completed_response: None,
        final_delivery_evidence: None,
        action_count: None,
        completed_action_count: None,
        observation_count: None,
        transcript_observation_count: None,
        final_result_count: None,
        last_action_type: None,
        last_action_status: None,
        last_observation_source: None,
        last_action_summary: None,
        self_state_evidence_labels: Vec::new(),
        assistant_prose_used_for_task_status: None,
        memory_or_hs_override_allowed: None,
        trace_gap: generation
            .get("runtimeFactTraceGap")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        context_conflict_ignored: true,
        silent_write_detected,
        failure: (!passed).then(|| "provider route runtime fact evidence incomplete".into()),
    }
}

async fn run_slice_c_case(
    scenario_id: &'static str,
    session_id: String,
    entry_point: &'static str,
    user_text: &'static str,
    state: Arc<AppState>,
) -> MainChatRuntimeFactsScenarioEvidence {
    match runtime_fact_command_response(entry_point, session_id, user_text, &state).await {
        Ok(response) => {
            evidence_from_tool_availability_response(scenario_id, entry_point, user_text, response)
        }
        Err(error) => {
            MainChatRuntimeFactsScenarioEvidence::failed(scenario_id, entry_point, user_text, error)
        }
    }
}

fn evidence_from_tool_availability_response(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    response: Value,
) -> MainChatRuntimeFactsScenarioEvidence {
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"))
        .cloned()
        .unwrap_or(Value::Null);
    let reply = response
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let runtime_fact_keys = string_array(&generation, "runtimeFactKeys");
    let runtime_fact_source = string_array(&generation, "runtimeFactSource");
    let runtime_fact_visibility = string_array(&generation, "runtimeFactVisibility");
    let runtime_fact_privacy = string_array(&generation, "runtimeFactPrivacy");
    let runtime_fact_binding_count = generation
        .get("runtimeFacts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let model_generated = generation.get("modelGenerated").and_then(Value::as_bool);
    let scheduler_generation_called = generation
        .get("schedulerGenerationCalled")
        .and_then(Value::as_bool);
    let tool_called = generation.get("toolCalled").and_then(Value::as_bool);
    let direct_writes_executed = generation
        .get("directWritesExecuted")
        .and_then(Value::as_bool);
    let legacy_fallback_used = response
        .get("legacy_fallback_used")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let tool_web_config_enabled = bool_field(&generation, "toolWebConfigEnabled");
    let tool_web_credential_available = bool_field(&generation, "toolWebCredentialAvailable");
    let tool_web_credential_status = string_field(&generation, "toolWebCredentialStatus");
    let tool_web_policy_allowed = bool_field(&generation, "toolWebPolicyAllowed");
    let tool_web_policy_blockers = string_array(&generation, "toolWebPolicyBlockers");
    let tool_web_reachability_status = string_field(&generation, "toolWebReachabilityStatus");
    let tool_web_reachability_ttl_status =
        string_field(&generation, "toolWebReachabilityTtlStatus");
    let tool_web_cached_or_preflight_known_reachability =
        bool_field(&generation, "toolWebCachedOrPreflightKnownReachability");
    let tool_web_active_reachability_probe =
        bool_field(&generation, "toolWebActiveReachabilityProbe");
    let tool_web_available = string_field(&generation, "toolWebAvailable");
    let tool_mcp_registered_count = usize_field(&generation, "toolMcpRegisteredCount");
    let tool_mcp_safe_read_candidate_count =
        usize_field(&generation, "toolMcpSafeReadCandidateCount");
    let tool_mcp_server_status = string_field(&generation, "toolMcpServerStatus");
    let tool_mcp_available = string_field(&generation, "toolMcpAvailable");
    let tool_mcp_raw_manifest_exposed = bool_field(&generation, "toolMcpRawManifestExposed");
    let tool_write_available = string_field(&generation, "toolWriteAvailable");
    let tool_write_requires_permission = bool_field(&generation, "toolWriteRequiresPermission");
    let tool_write_silent_write_available =
        bool_field(&generation, "toolWriteSilentWriteAvailable");
    let tool_availability_labels = string_array(&generation, "toolAvailabilityLabels");
    let ui_primary_source_chip = string_field(&generation, "uiPrimarySourceChip");
    let ui_status = string_field(&generation, "uiStatus");
    let raw_mcp_manifest_exposed = tool_mcp_raw_manifest_exposed == Some(true)
        || reply.contains("raw_rf13_hidden_write_manifest")
        || reply.contains("RAW_MCP_DESCRIPTION_SHOULD_NOT_RENDER")
        || reply.contains("safe_rf14_read_manifest")
        || reply.contains("SAFE_DESCRIPTION_SHOULD_NOT_RENDER")
        || tool_availability_labels.iter().any(|label| {
            label.contains("raw_rf13_hidden_write_manifest")
                || label.contains("RAW_MCP_DESCRIPTION_SHOULD_NOT_RENDER")
                || label.contains("safe_rf14_read_manifest")
                || label.contains("SAFE_DESCRIPTION_SHOULD_NOT_RENDER")
        });
    let silent_write_detected = direct_writes_executed.unwrap_or(true)
        || tool_write_silent_write_available.unwrap_or(true)
        || response
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty());
    let common_passed = generation.get("sourceType").and_then(Value::as_str)
        == Some(RUNTIME_FACT_SOURCE_TYPE)
        && generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            == Some(RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH)
        && runtime_fact_binding_count > 0
        && runtime_fact_source
            .iter()
            .any(|source| source == "tool_policy")
        && runtime_fact_visibility
            .iter()
            .any(|value| value == "answer")
        && runtime_fact_privacy.iter().any(|value| value == "public")
        && model_generated == Some(false)
        && scheduler_generation_called == Some(false)
        && tool_called == Some(false)
        && direct_writes_executed == Some(false)
        && !legacy_fallback_used
        && tool_web_active_reachability_probe == Some(false)
        && ui_primary_source_chip.as_deref() == Some("工具可用性")
        && !raw_mcp_manifest_exposed
        && !silent_write_detected;
    let scenario_passed = match scenario_id {
        "RF-11" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE)
                && tool_web_config_enabled == Some(true)
                && tool_web_credential_available == Some(true)
                && tool_web_credential_status.as_deref() == Some("not_required")
                && tool_web_policy_allowed == Some(true)
                && tool_web_reachability_status.as_deref() == Some("unknown")
                && tool_web_reachability_ttl_status.as_deref() == Some("not_observed")
                && tool_web_cached_or_preflight_known_reachability == Some(false)
                && tool_web_available.as_deref() == Some("unknown")
                && reply.contains("不会主动探测网络")
        }
        "RF-12" => {
            tool_web_config_enabled == Some(true)
                && tool_web_policy_allowed == Some(false)
                && tool_web_policy_blockers
                    .iter()
                    .any(|blocker| blocker == "network_policy_disabled")
                && tool_web_available.as_deref() == Some("blocked")
                && ui_status.as_deref() == Some("restricted")
                && reply.contains("策略阻止外部读取")
                && !reply.contains("已联网")
        }
        "RF-13" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT)
                && tool_mcp_registered_count.unwrap_or_default() > 0
                && tool_mcp_safe_read_candidate_count == Some(0)
                && tool_mcp_available.as_deref() == Some("no_safe_read_candidate")
                && reply.contains("policy-allowed read-only candidate 为 0")
        }
        "RF-14" => {
            tool_mcp_registered_count.unwrap_or_default() > 0
                && tool_mcp_safe_read_candidate_count.unwrap_or_default() > 0
                && tool_mcp_server_status.as_deref() == Some("unknown")
                && tool_mcp_available.as_deref() == Some("unknown_server_status")
                && reply.contains("server_status=unknown")
                && reply.contains("不能标为 available")
        }
        "RF-15" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE)
                && tool_write_available.as_deref() == Some("proposal_permission_or_blocker")
                && tool_write_requires_permission == Some(true)
                && tool_write_silent_write_available == Some(false)
                && ui_status.as_deref() == Some("waiting_for_user")
                && reply.contains("proposal / permission / blocker")
                && reply.contains("directWritesExecuted=false")
        }
        _ => false,
    };
    let passed = common_passed && scenario_passed;

    MainChatRuntimeFactsScenarioEvidence {
        scenario_id,
        entry_point,
        user_text,
        passed,
        answer_preview: reply.chars().take(480).collect(),
        source_type: generation
            .get("sourceType")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_keys,
        runtime_fact_source,
        runtime_fact_binding_count,
        runtime_fact_authority: generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_freshness: generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_visibility,
        runtime_fact_privacy,
        model_generated,
        scheduler_generation_called,
        tool_called,
        direct_writes_executed,
        legacy_fallback_used,
        provider_generation_path: generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            .map(str::to_string),
        configured_provider: None,
        configured_model: None,
        current_turn_generation_provider: None,
        current_turn_generation_model: None,
        current_turn_generation_route_type: None,
        current_turn_generation_model_generated: None,
        last_completed_generation_provider: None,
        last_completed_generation_model: None,
        last_completed_generation_run_id: None,
        planned_route_if_model_needed_provider: None,
        planned_route_if_model_needed_model: None,
        planned_route_if_model_needed_route_type: None,
        provider_preflight_status: None,
        provider_preflight_blockers: Vec::new(),
        route_labels: Vec::new(),
        tool_web_config_enabled,
        tool_web_credential_available,
        tool_web_credential_status,
        tool_web_policy_allowed,
        tool_web_policy_blockers,
        tool_web_reachability_status,
        tool_web_reachability_ttl_status,
        tool_web_cached_or_preflight_known_reachability,
        tool_web_active_reachability_probe,
        tool_web_available,
        tool_mcp_registered_count,
        tool_mcp_safe_read_candidate_count,
        tool_mcp_server_status,
        tool_mcp_available,
        tool_mcp_raw_manifest_exposed: Some(raw_mcp_manifest_exposed),
        tool_write_available,
        tool_write_requires_permission,
        tool_write_silent_write_available,
        tool_availability_labels,
        ui_primary_source_chip,
        ui_status,
        task_session_id: response
            .get("agent_ingress")
            .and_then(|ingress| ingress.get("agentTaskSessionId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        run_id: response
            .get("run_id")
            .or_else(|| response.get("runId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        task_status: None,
        run_status: None,
        delivery_status: None,
        blocker_codes: Vec::new(),
        pending_permission_count: None,
        pending_permission_target_label: None,
        pending_permission_target_labels: Vec::new(),
        pending_proposal_count: None,
        durable_change_status: None,
        durable_change_completed: None,
        safe_next_controls: Vec::new(),
        safe_automatic_control_available: None,
        completed_response: None,
        final_delivery_evidence: None,
        action_count: None,
        completed_action_count: None,
        observation_count: None,
        transcript_observation_count: None,
        final_result_count: None,
        last_action_type: None,
        last_action_status: None,
        last_observation_source: None,
        last_action_summary: None,
        self_state_evidence_labels: Vec::new(),
        assistant_prose_used_for_task_status: None,
        memory_or_hs_override_allowed: None,
        trace_gap: generation
            .get("runtimeFactTraceGap")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        context_conflict_ignored: true,
        silent_write_detected,
        failure: (!passed).then(|| "tool availability runtime fact evidence incomplete".into()),
    }
}

async fn runtime_fact_command_response(
    entry_point: &'static str,
    session_id: String,
    user_text: &'static str,
    state: &Arc<AppState>,
) -> Result<Value, String> {
    match entry_point {
        "send" => {
            let result = crate::main_chat_send::send_message_with_state(
                session_id,
                vec![ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                }],
                None,
                state,
            )
            .await?;
            serde_json::to_value(result)
                .map_err(|error| format!("serialize send response failed: {error}"))
        }
        "stream" => {
            let mut emitted_events = Vec::<(String, Value)>::new();
            crate::main_chat_streaming::start_stream_message_with_state(
                session_id,
                vec![ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                }],
                None,
                state,
                |event, payload| emitted_events.push((event.to_string(), payload)),
            )
            .await?;
            emitted_events
                .iter()
                .rev()
                .find(|(event, _)| event == "stream-message-done")
                .map(|(_, payload)| payload.clone())
                .ok_or_else(|| "stream runtime fact case missing done payload".to_string())
        }
        _ => Err(format!("unsupported entry point {entry_point}")),
    }
}

async fn run_slice_a_case(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    clock_source: MainChatRuntimeClockSource,
    conflicting_agents_text: Option<&'static str>,
) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut source = state.runtime_clock_source.lock().await;
        *source = clock_source;
    }
    if let Some(conflicting_agents_text) = conflicting_agents_text {
        if let Err(error) = seed_conflicting_knowledge_root(&state, conflicting_agents_text).await {
            return MainChatRuntimeFactsScenarioEvidence::failed(
                scenario_id,
                entry_point,
                user_text,
                error,
            );
        }
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "provider-should-not-answer-runtime-clock".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("provider should not answer runtime clock");
    }

    let session_id = format!("runtime-facts-{entry_point}-{scenario_id}");
    let response = match entry_point {
        "send" => {
            let result = crate::main_chat_send::send_message_with_state(
                session_id,
                vec![ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                }],
                None,
                &state,
            )
            .await;
            match result {
                Ok(result) => serde_json::to_value(result)
                    .map_err(|error| format!("serialize send response failed: {error}")),
                Err(error) => Err(error),
            }
        }
        "stream" => {
            let mut emitted_events = Vec::<(String, Value)>::new();
            let result = crate::main_chat_streaming::start_stream_message_with_state(
                session_id,
                vec![ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                }],
                None,
                &state,
                |event, payload| emitted_events.push((event.to_string(), payload)),
            )
            .await;
            match result {
                Ok(()) => emitted_events
                    .iter()
                    .rev()
                    .find(|(event, _)| event == "stream-message-done")
                    .map(|(_, payload)| payload.clone())
                    .ok_or_else(|| "stream runtime fact case missing done payload".to_string()),
                Err(error) => Err(error),
            }
        }
        _ => Err(format!("unsupported entry point {entry_point}")),
    };

    match response {
        Ok(response) => evidence_from_runtime_fact_response(
            scenario_id,
            entry_point,
            user_text,
            response,
            conflicting_agents_text.is_some(),
        ),
        Err(error) => {
            MainChatRuntimeFactsScenarioEvidence::failed(scenario_id, entry_point, user_text, error)
        }
    }
}

impl MainChatRuntimeFactsScenarioEvidence {
    fn failed(
        scenario_id: &'static str,
        entry_point: &'static str,
        user_text: &'static str,
        failure: String,
    ) -> Self {
        Self {
            scenario_id,
            entry_point,
            user_text,
            passed: false,
            answer_preview: String::new(),
            source_type: None,
            runtime_fact_keys: Vec::new(),
            runtime_fact_source: Vec::new(),
            runtime_fact_binding_count: 0,
            runtime_fact_authority: None,
            runtime_fact_freshness: None,
            runtime_fact_visibility: Vec::new(),
            runtime_fact_privacy: Vec::new(),
            model_generated: None,
            scheduler_generation_called: None,
            tool_called: None,
            direct_writes_executed: None,
            legacy_fallback_used: false,
            provider_generation_path: None,
            configured_provider: None,
            configured_model: None,
            current_turn_generation_provider: None,
            current_turn_generation_model: None,
            current_turn_generation_route_type: None,
            current_turn_generation_model_generated: None,
            last_completed_generation_provider: None,
            last_completed_generation_model: None,
            last_completed_generation_run_id: None,
            planned_route_if_model_needed_provider: None,
            planned_route_if_model_needed_model: None,
            planned_route_if_model_needed_route_type: None,
            provider_preflight_status: None,
            provider_preflight_blockers: Vec::new(),
            route_labels: Vec::new(),
            tool_web_config_enabled: None,
            tool_web_credential_available: None,
            tool_web_credential_status: None,
            tool_web_policy_allowed: None,
            tool_web_policy_blockers: Vec::new(),
            tool_web_reachability_status: None,
            tool_web_reachability_ttl_status: None,
            tool_web_cached_or_preflight_known_reachability: None,
            tool_web_active_reachability_probe: None,
            tool_web_available: None,
            tool_mcp_registered_count: None,
            tool_mcp_safe_read_candidate_count: None,
            tool_mcp_server_status: None,
            tool_mcp_available: None,
            tool_mcp_raw_manifest_exposed: None,
            tool_write_available: None,
            tool_write_requires_permission: None,
            tool_write_silent_write_available: None,
            tool_availability_labels: Vec::new(),
            ui_primary_source_chip: None,
            ui_status: None,
            task_session_id: None,
            run_id: None,
            task_status: None,
            run_status: None,
            delivery_status: None,
            blocker_codes: Vec::new(),
            pending_permission_count: None,
            pending_permission_target_label: None,
            pending_permission_target_labels: Vec::new(),
            pending_proposal_count: None,
            durable_change_status: None,
            durable_change_completed: None,
            safe_next_controls: Vec::new(),
            safe_automatic_control_available: None,
            completed_response: None,
            final_delivery_evidence: None,
            action_count: None,
            completed_action_count: None,
            observation_count: None,
            transcript_observation_count: None,
            final_result_count: None,
            last_action_type: None,
            last_action_status: None,
            last_observation_source: None,
            last_action_summary: None,
            self_state_evidence_labels: Vec::new(),
            assistant_prose_used_for_task_status: None,
            memory_or_hs_override_allowed: None,
            trace_gap: false,
            context_conflict_ignored: false,
            silent_write_detected: false,
            failure: Some(failure),
        }
    }
}

fn evidence_from_runtime_fact_response(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    response: Value,
    has_context_conflict: bool,
) -> MainChatRuntimeFactsScenarioEvidence {
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"))
        .cloned()
        .unwrap_or(Value::Null);
    let reply = response
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let runtime_fact_keys = string_array(&generation, "runtimeFactKeys");
    let runtime_fact_source = string_array(&generation, "runtimeFactSource");
    let runtime_fact_binding_count = generation
        .get("runtimeFacts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let runtime_fact_visibility = string_array(&generation, "runtimeFactVisibility");
    let runtime_fact_privacy = string_array(&generation, "runtimeFactPrivacy");
    let model_generated = generation.get("modelGenerated").and_then(Value::as_bool);
    let scheduler_generation_called = generation
        .get("schedulerGenerationCalled")
        .and_then(Value::as_bool);
    let tool_called = generation.get("toolCalled").and_then(Value::as_bool);
    let direct_writes_executed = generation
        .get("directWritesExecuted")
        .and_then(Value::as_bool);
    let legacy_fallback_used = response
        .get("legacy_fallback_used")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let trace_gap = generation
        .get("runtimeFactTraceGap")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let expected_runtime_value_present = if trace_gap {
        reply.contains("当前时间未知")
            && runtime_fact_keys.contains(&RUNTIME_FACT_KEY_TRACE_GAP.into())
    } else {
        reply.contains("2026-06-23") && reply.contains("星期二") && reply.contains("UTC+08:00")
    };
    let context_conflict_ignored = !has_context_conflict
        || (reply.contains("2026-06-23")
            && reply.contains("星期二")
            && !reply.contains("1999-01-01")
            && !reply.contains("Friday"));
    let silent_write_detected = direct_writes_executed.unwrap_or(true)
        || response
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty());
    let passed = generation.get("sourceType").and_then(Value::as_str)
        == Some(RUNTIME_FACT_SOURCE_TYPE)
        && !runtime_fact_keys.is_empty()
        && runtime_fact_binding_count > 0
        && runtime_fact_source
            .iter()
            .any(|source| source == "local_clock")
        && generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            == Some("runtime")
        && generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .is_some_and(|freshness| freshness == "instant" || freshness == "unknown")
        && runtime_fact_visibility
            .iter()
            .any(|value| value == "answer")
        && runtime_fact_privacy.iter().any(|value| value == "public")
        && model_generated == Some(false)
        && scheduler_generation_called == Some(false)
        && tool_called == Some(false)
        && direct_writes_executed == Some(false)
        && !legacy_fallback_used
        && generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            == Some(RUNTIME_FACT_PROVIDER_GENERATION_PATH)
        && expected_runtime_value_present
        && context_conflict_ignored
        && !silent_write_detected;

    MainChatRuntimeFactsScenarioEvidence {
        scenario_id,
        entry_point,
        user_text,
        passed,
        answer_preview: reply.chars().take(160).collect(),
        source_type: generation
            .get("sourceType")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_keys,
        runtime_fact_source,
        runtime_fact_binding_count,
        runtime_fact_authority: generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_freshness: generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_visibility,
        runtime_fact_privacy,
        model_generated,
        scheduler_generation_called,
        tool_called,
        direct_writes_executed,
        legacy_fallback_used,
        provider_generation_path: generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            .map(str::to_string),
        configured_provider: generation
            .get("configuredProvider")
            .and_then(Value::as_str)
            .map(str::to_string),
        configured_model: generation
            .get("configuredModel")
            .and_then(Value::as_str)
            .map(str::to_string),
        current_turn_generation_provider: generation
            .get("currentTurnGenerationProvider")
            .and_then(Value::as_str)
            .map(str::to_string),
        current_turn_generation_model: generation
            .get("currentTurnGenerationModel")
            .and_then(Value::as_str)
            .map(str::to_string),
        current_turn_generation_route_type: generation
            .get("currentTurnGenerationRouteType")
            .and_then(Value::as_str)
            .map(str::to_string),
        current_turn_generation_model_generated: generation
            .get("currentTurnGenerationModelGenerated")
            .and_then(Value::as_bool),
        last_completed_generation_provider: generation
            .get("lastCompletedGenerationProvider")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_completed_generation_model: generation
            .get("lastCompletedGenerationModel")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_completed_generation_run_id: generation
            .get("lastCompletedGenerationRunId")
            .and_then(Value::as_str)
            .map(str::to_string),
        planned_route_if_model_needed_provider: generation
            .get("plannedRouteIfModelNeededProvider")
            .and_then(Value::as_str)
            .map(str::to_string),
        planned_route_if_model_needed_model: generation
            .get("plannedRouteIfModelNeededModel")
            .and_then(Value::as_str)
            .map(str::to_string),
        planned_route_if_model_needed_route_type: generation
            .get("plannedRouteIfModelNeededRouteType")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_preflight_status: generation
            .get("providerPreflightStatus")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_preflight_blockers: string_array(&generation, "providerPreflightBlockers"),
        route_labels: string_array(&generation, "routeLabels"),
        tool_web_config_enabled: None,
        tool_web_credential_available: None,
        tool_web_credential_status: None,
        tool_web_policy_allowed: None,
        tool_web_policy_blockers: Vec::new(),
        tool_web_reachability_status: None,
        tool_web_reachability_ttl_status: None,
        tool_web_cached_or_preflight_known_reachability: None,
        tool_web_active_reachability_probe: None,
        tool_web_available: None,
        tool_mcp_registered_count: None,
        tool_mcp_safe_read_candidate_count: None,
        tool_mcp_server_status: None,
        tool_mcp_available: None,
        tool_mcp_raw_manifest_exposed: None,
        tool_write_available: None,
        tool_write_requires_permission: None,
        tool_write_silent_write_available: None,
        tool_availability_labels: Vec::new(),
        ui_primary_source_chip: generation
            .get("uiPrimarySourceChip")
            .and_then(Value::as_str)
            .map(str::to_string),
        ui_status: generation
            .get("uiStatus")
            .and_then(Value::as_str)
            .map(str::to_string),
        task_session_id: response
            .get("agent_ingress")
            .and_then(|ingress| ingress.get("agentTaskSessionId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        run_id: response
            .get("run_id")
            .or_else(|| response.get("runId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        task_status: None,
        run_status: None,
        delivery_status: None,
        blocker_codes: Vec::new(),
        pending_permission_count: None,
        pending_permission_target_label: None,
        pending_permission_target_labels: Vec::new(),
        pending_proposal_count: None,
        durable_change_status: None,
        durable_change_completed: None,
        safe_next_controls: Vec::new(),
        safe_automatic_control_available: None,
        completed_response: None,
        final_delivery_evidence: None,
        action_count: None,
        completed_action_count: None,
        observation_count: None,
        transcript_observation_count: None,
        final_result_count: None,
        last_action_type: None,
        last_action_status: None,
        last_observation_source: None,
        last_action_summary: None,
        self_state_evidence_labels: Vec::new(),
        assistant_prose_used_for_task_status: None,
        memory_or_hs_override_allowed: None,
        trace_gap,
        context_conflict_ignored,
        silent_write_detected,
        failure: (!passed).then(|| "runtime fact command-surface evidence incomplete".into()),
    }
}

async fn run_runtime_clock_negative_planning_case() -> bool {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut source = state.runtime_clock_source.lock().await;
        *source = fixed_clock_source();
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "provider-planning".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("provider handled planning question");
    }
    let result = crate::main_chat_send::send_message_with_state(
        "runtime-facts-negative-planning".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "What time should I leave tomorrow?".into(),
        }],
        None,
        &state,
    )
    .await;
    let Ok(result) = result else {
        return false;
    };
    let Ok(response) = serde_json::to_value(result) else {
        return false;
    };
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"));
    response
        .get("reply")
        .and_then(Value::as_str)
        .is_some_and(|reply| reply.contains("provider handled planning question"))
        && generation
            .and_then(|value| value.get("sourceType"))
            .and_then(Value::as_str)
            != Some(RUNTIME_FACT_SOURCE_TYPE)
        && generation
            .and_then(|value| value.get("modelGenerated"))
            .and_then(Value::as_bool)
            == Some(true)
        && generation
            .and_then(|value| value.get("schedulerGenerationCalled"))
            .and_then(Value::as_bool)
            == Some(true)
}

async fn seed_conflicting_knowledge_root(
    state: &Arc<AppState>,
    conflicting_agents_text: &str,
) -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "openlife-runtime-facts-conflict-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("create runtime facts conflict root failed: {error}"))?;
    std::fs::write(root.join("AGENTS.md"), conflicting_agents_text)
        .map_err(|error| format!("write runtime facts conflict AGENTS.md failed: {error}"))?;
    let mut config = state.config.lock().await;
    config
        .system
        .knowledge_roots
        .push(root.to_string_lossy().to_string());
    Ok(())
}

fn fixed_clock_source() -> MainChatRuntimeClockSource {
    MainChatRuntimeClockSource::Fixed(
        chrono::DateTime::parse_from_rfc3339(FIXED_CLOCK_RFC3339).expect("fixed clock parses"),
    )
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn bool_field(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn usize_field(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}
