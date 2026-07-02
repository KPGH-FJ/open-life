use openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceReport;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

use crate::main_chat_turn_pipeline::{MainChatTurnRouteDecision, MainChatTurnStreamMode};
use crate::state::{MainChatFinalGateReadinessSnapshot, MainChatTurnRouteEvidenceSnapshot};
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatRuntimeStatus {
    pub status_version: u8,
    pub authoritative_runtime: &'static str,
    pub default_send_path: &'static str,
    pub start_stream_path: &'static str,
    pub source_of_truth: &'static str,
    pub kernel_evidence: MainChatKernelEvidence,
    pub latest_route_evidence: MainChatLatestRouteEvidence,
    pub legacy_fallback: MainChatLegacyFallbackStatus,
    pub final_gate_readiness: MainChatFinalGateReadinessStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatKernelEvidence {
    pub kernel_backed_default: bool,
    pub final_gate_evidence_present: bool,
    pub final_gate_ready: bool,
    pub latest_kernel_route_observed: bool,
    pub legacy_fallback_free_since_startup: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatLatestRouteEvidence {
    pub status: &'static str,
    pub direct_answer_observed: bool,
    pub governed_blocker_observed: bool,
    pub agent_loop_observed: bool,
    pub kernel_backed_default_observed: bool,
    pub legacy_fallback_used: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_kernel_event_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_route_reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_kernel_support_disposition: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatLegacyFallbackStatus {
    pub mode: &'static str,
    pub allowed_by_default: bool,
    pub used_count_since_startup: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reason_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatFinalGateReadinessStatus {
    pub authority: &'static str,
    pub status: String,
    pub blockers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_report_run_id: Option<String>,
}

#[tauri::command]
pub async fn get_main_chat_runtime_status(
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatRuntimeStatus, String> {
    Ok(get_main_chat_runtime_status_with_state(state.inner()).await)
}

pub(crate) async fn get_main_chat_runtime_status_with_state(
    state: &Arc<AppState>,
) -> MainChatRuntimeStatus {
    let runtime = state.main_chat_runtime_state.lock().await.clone();
    let final_gate_readiness = runtime
        .latest_final_gate_readiness
        .clone()
        .map(|snapshot| {
            final_gate_readiness_with_current_runtime(snapshot, runtime.legacy_fallback_used_count)
        })
        .unwrap_or_else(|| MainChatFinalGateReadinessStatus {
            authority: "main_chat_final_acceptance_gate",
            status: "not_run".into(),
            blockers: Vec::new(),
            last_report_run_id: None,
        });

    let latest_route = runtime.latest_turn_route_evidence.clone();
    let latest_route_evidence = latest_route_evidence_status(latest_route.as_ref());
    let final_gate_evidence_present = runtime.latest_final_gate_readiness.is_some();
    let final_gate_ready = final_gate_readiness.status == "ready";
    let legacy_fallback_free_since_startup = runtime.legacy_fallback_used_count == 0;
    let kernel_evidence = MainChatKernelEvidence {
        kernel_backed_default: final_gate_ready
            && latest_route_evidence.kernel_backed_default_observed
            && legacy_fallback_free_since_startup,
        final_gate_evidence_present,
        final_gate_ready,
        latest_kernel_route_observed: latest_route_evidence.kernel_backed_default_observed,
        legacy_fallback_free_since_startup,
    };

    MainChatRuntimeStatus {
        status_version: 2,
        authoritative_runtime: "main_chat_kernel",
        default_send_path: "main_chat_kernel",
        start_stream_path: "main_chat_kernel",
        source_of_truth: "main_chat_turn_pipeline",
        kernel_evidence,
        latest_route_evidence,
        legacy_fallback: MainChatLegacyFallbackStatus {
            mode: "explicit_only",
            allowed_by_default: false,
            used_count_since_startup: runtime.legacy_fallback_used_count,
            last_used_at: runtime.last_legacy_fallback_at,
            last_reason_code: runtime.last_legacy_fallback_reason_code,
        },
        final_gate_readiness,
    }
}

fn latest_route_evidence_status(
    latest_route: Option<&MainChatTurnRouteEvidenceSnapshot>,
) -> MainChatLatestRouteEvidence {
    let Some(route) = latest_route else {
        return MainChatLatestRouteEvidence {
            status: "not_observed",
            direct_answer_observed: false,
            governed_blocker_observed: false,
            agent_loop_observed: false,
            kernel_backed_default_observed: false,
            legacy_fallback_used: false,
            last_kernel_event_count: None,
            last_route_reason_code: None,
            last_kernel_support_disposition: None,
        };
    };

    let kernel_backed_default = route.kernel_supported
        && matches!(
            route.execution_path.as_str(),
            "KernelDirect"
                | "KernelReadTool"
                | "KernelWriteOutcome"
                | "PlanExecute"
                | "GovernedBlocker"
        )
        && !route.legacy_fallback_used;
    let direct_answer_observed = kernel_backed_default
        && route.execution_path == "KernelDirect"
        && route.reason_code == "kernel_supported_direct_answer"
        && route.kernel_event_count.is_some();
    let governed_blocker_observed = kernel_backed_default
        && route.execution_path == "GovernedBlocker"
        && route.reason_code == "kernel_governed_blocker";
    let agent_loop_observed = route.execution_path == "ToolLoop"
        && route.observed_agent_loop
        && route.observed_agent_loop_without_fallback
        && !route.legacy_fallback_used;

    MainChatLatestRouteEvidence {
        status: "observed",
        direct_answer_observed,
        governed_blocker_observed,
        agent_loop_observed,
        kernel_backed_default_observed: kernel_backed_default,
        legacy_fallback_used: route.legacy_fallback_used,
        last_kernel_event_count: route.kernel_event_count,
        last_route_reason_code: Some(route.reason_code.clone()),
        last_kernel_support_disposition: Some(route.kernel_support_disposition.clone()),
    }
}

pub(crate) async fn record_main_chat_turn_route_evidence(
    state: &Arc<AppState>,
    route_decision: &MainChatTurnRouteDecision,
    stream_mode: MainChatTurnStreamMode,
    observed_agent_loop: bool,
    legacy_fallback_used: bool,
    kernel_event_count: Option<usize>,
) {
    let mut runtime = state.main_chat_runtime_state.lock().await;
    if let Some(count) = kernel_event_count {
        runtime.last_kernel_event_count = Some(count);
    }
    runtime.latest_turn_route_evidence = Some(MainChatTurnRouteEvidenceSnapshot {
        stream_mode: stream_mode.as_str().into(),
        execution_path: route_decision.path.as_str().into(),
        strategy_label: route_decision.strategy_label.clone(),
        reason_code: metadata_safe_reason_code(route_decision.reason_code.clone()),
        kernel_supported: route_decision.kernel_supported,
        kernel_support_disposition: route_decision.kernel_support_disposition.clone(),
        fallback_allowed: route_decision.fallback_allowed,
        requires_tool_loop: route_decision.requires_tool_loop,
        observed_agent_loop,
        observed_agent_loop_without_fallback: observed_agent_loop && !legacy_fallback_used,
        legacy_fallback_used,
        kernel_event_count,
        recorded_at: chrono::Utc::now().to_rfc3339(),
    });
}

pub(crate) async fn record_main_chat_kernel_event_count(
    state: &Arc<AppState>,
    kernel_event_count: usize,
) {
    let mut runtime = state.main_chat_runtime_state.lock().await;
    runtime.last_kernel_event_count = Some(kernel_event_count);
}

pub(crate) async fn record_main_chat_legacy_fallback(
    state: &Arc<AppState>,
    reason_code: impl Into<String>,
) {
    let mut runtime = state.main_chat_runtime_state.lock().await;
    runtime.legacy_fallback_used_count = runtime.legacy_fallback_used_count.saturating_add(1);
    runtime.last_legacy_fallback_reason_code = Some(metadata_safe_reason_code(reason_code.into()));
    runtime.last_legacy_fallback_at = Some(chrono::Utc::now().to_rfc3339());
}

pub(crate) async fn main_chat_legacy_fallback_used_count(state: &Arc<AppState>) -> u64 {
    state
        .main_chat_runtime_state
        .lock()
        .await
        .legacy_fallback_used_count
}

pub(crate) async fn record_main_chat_final_gate_readiness(
    state: &Arc<AppState>,
    acceptance: &MainChatAgentExecutionV1AcceptanceReport,
    report_run_id: String,
) {
    let mut runtime = state.main_chat_runtime_state.lock().await;
    runtime.latest_final_gate_readiness = Some(MainChatFinalGateReadinessSnapshot {
        status: if acceptance.ready {
            "ready".into()
        } else {
            "blocked".into()
        },
        blockers: acceptance.blockers.clone(),
        last_report_run_id: Some(metadata_safe_reason_code(report_run_id)),
    });
}

pub(crate) fn apply_startup_legacy_fallback_blocker(
    acceptance: &mut MainChatAgentExecutionV1AcceptanceReport,
    legacy_fallback_used_count: u64,
) {
    if legacy_fallback_used_count == 0 {
        return;
    }
    acceptance.ready = false;
    acceptance.status = "blocked".into();
    acceptance.command_surface_gate_ready = false;
    push_unique_blocker(
        &mut acceptance.blockers,
        "legacy_fallback_used_since_startup",
    );
}

fn final_gate_readiness_with_current_runtime(
    snapshot: MainChatFinalGateReadinessSnapshot,
    legacy_fallback_used_count: u64,
) -> MainChatFinalGateReadinessStatus {
    let mut status = snapshot.status;
    let mut blockers = snapshot.blockers;
    if legacy_fallback_used_count > 0 {
        status = "blocked".into();
        push_unique_blocker(&mut blockers, "legacy_fallback_used_since_startup");
    }
    MainChatFinalGateReadinessStatus {
        authority: "main_chat_final_acceptance_gate",
        status,
        blockers,
        last_report_run_id: snapshot.last_report_run_id,
    }
}

fn metadata_safe_reason_code(reason_code: String) -> String {
    let safe = reason_code
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        .take(96)
        .collect::<String>();
    if safe.is_empty() {
        "legacy_fallback_used".into()
    } else {
        safe
    }
}

fn push_unique_blocker(blockers: &mut Vec<String>, blocker: &str) {
    if !blockers.iter().any(|existing| existing == blocker) {
        blockers.push(blocker.to_string());
    }
}
