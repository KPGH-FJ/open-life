use crate::main_chat_command_surface_eval::{
    configure_main_chat_command_surface_eval_state, json_contains_direct_write_true,
    main_chat_command_surface_eval_has_silent_write, MainChatCommandSurfaceEvalScenario,
};
use crate::AppState;
use openlife_core::agent::main_chat_agent_v1::{
    AgentTaskSession, AgentTaskSessionStatus, ExecutionQueueStatus, ExecutionTranscriptEntry,
    ExecutionTranscriptEntryKind, MainChatAgentStrategy, QueuedExecutionAction,
};
use openlife_core::llm::ChatMessage;
use serde::Serialize;
use std::sync::Arc;

const CAPABILITY_EVAL_SCHEMA_VERSION: &str = "main-chat-capability-eval-v1";
const CAPABILITY_EVAL_REPORT_KIND: &str = "main_chat_capability_eval";
const CAPABILITY_EVAL_READINESS: &str =
    "deterministic_real_capability_send_path_live_provider_excluded";
const CAPABILITY_EVAL_STREAM_GATE: &str =
    "main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MainChatCapabilityEvalScenario {
    CfDirect01,
    CfFile01,
    CfWeb01,
    CfMcp01,
}

impl MainChatCapabilityEvalScenario {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::CfDirect01 => "CF-DIRECT-01",
            Self::CfFile01 => "CF-FILE-01",
            Self::CfWeb01 => "CF-WEB-01",
            Self::CfMcp01 => "CF-MCP-01",
        }
    }

    fn prompt(self) -> &'static str {
        match self {
            Self::CfDirect01 => "Explain focused work in one concise paragraph for a teammate.",
            Self::CfFile01 => "Read Cargo.toml as a governed workspace file observation.",
            Self::CfWeb01 => "Please web search OpenLife release notes.",
            Self::CfMcp01 => "Use mcp builtin_echo read-only now.",
        }
    }

    fn capability_group(self) -> &'static str {
        match self {
            Self::CfDirect01 => "ordinary_answer",
            Self::CfFile01 => "workspace_file_read",
            Self::CfWeb01 => "fixture_web_read",
            Self::CfMcp01 => "registered_mcp_read",
        }
    }

    fn expected_route(self) -> MainChatAgentStrategy {
        match self {
            Self::CfDirect01 => MainChatAgentStrategy::DirectAnswer,
            Self::CfFile01 | Self::CfWeb01 | Self::CfMcp01 => {
                MainChatAgentStrategy::ReActToolExecution
            }
        }
    }
}

pub(crate) const MAIN_CHAT_CAPABILITY_EVAL_SCENARIOS: [MainChatCapabilityEvalScenario; 4] = [
    MainChatCapabilityEvalScenario::CfDirect01,
    MainChatCapabilityEvalScenario::CfFile01,
    MainChatCapabilityEvalScenario::CfWeb01,
    MainChatCapabilityEvalScenario::CfMcp01,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatCapabilityEvalFixtureMode {
    Default,
    MissingMcpFixture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MainChatCapabilityEvalCaseStatus {
    Passed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatCapabilityEvalReport {
    pub report_kind: String,
    pub schema_version: String,
    pub readiness_semantics: String,
    pub local_deterministic_ready: bool,
    pub allow_writes: bool,
    pub live_provider_required: bool,
    pub stream_coverage_reused_from_command_surface_gate: bool,
    pub stream_coverage_gate: String,
    pub total_case_count: usize,
    pub passed_case_count: usize,
    pub blocked_case_count: usize,
    pub failed_case_count: usize,
    pub legacy_fallback_count: usize,
    pub silent_write_count: usize,
    pub direct_durable_write_count: usize,
    pub fake_observation_count: usize,
    pub live_only_proof_count: usize,
    pub cases: Vec<MainChatCapabilityEvalCaseReport>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatCapabilityEvalCaseReport {
    pub scenario_id: String,
    pub status: MainChatCapabilityEvalCaseStatus,
    pub capability_group: String,
    pub prompt: String,
    pub entry_point: String,
    pub expected_route: String,
    pub actual_route: Option<String>,
    pub task_session_id: Option<String>,
    pub run_ids: Vec<String>,
    pub route_decision_observed: bool,
    pub deterministic_route_used: bool,
    pub route_preview_advisory_only: bool,
    pub generation_result_observed: bool,
    pub provider_scheduler_trace_observed: bool,
    pub final_assistant_delivery_observed: bool,
    pub final_transcript_observed: bool,
    pub tool_action_count: usize,
    pub observation_count: usize,
    pub proposal_record_count: usize,
    pub permission_record_count: usize,
    pub legacy_fallback_used: bool,
    pub silent_write_detected: bool,
    pub direct_durable_write_detected: bool,
    pub fake_observation_detected: bool,
    pub live_only_proof_used: bool,
    pub read_execution_kind: Option<String>,
    pub read_source_kind: Option<String>,
    pub read_real_read_only_execution: Option<bool>,
    pub read_fixture_backed: Option<bool>,
    pub network_policy_enabled: Option<bool>,
    pub structured_blocker: Option<String>,
    pub evidence: Vec<String>,
    pub blockers: Vec<String>,
}

struct MainChatCapabilityEvalArtifacts {
    scenario: MainChatCapabilityEvalScenario,
    response_value: serde_json::Value,
    task_session_id: String,
    session: AgentTaskSession,
    transcript: Vec<ExecutionTranscriptEntry>,
    actions: Vec<QueuedExecutionAction>,
    proposal_record_count: usize,
    permission_record_count: usize,
    runs: Vec<openlife_core::agent::AgentRun>,
    legacy_fallback_used: bool,
    reply_non_empty: bool,
    tool_call_count: usize,
    actual_route: Option<MainChatAgentStrategy>,
    network_policy_enabled: bool,
}

pub(crate) async fn run_main_chat_capability_eval_report() -> MainChatCapabilityEvalReport {
    let mut cases = Vec::new();
    for scenario in MAIN_CHAT_CAPABILITY_EVAL_SCENARIOS {
        cases.push(
            run_main_chat_capability_eval_case(
                scenario,
                MainChatCapabilityEvalFixtureMode::Default,
            )
            .await,
        );
    }
    MainChatCapabilityEvalReport::from_cases(cases)
}

pub(crate) async fn run_main_chat_capability_eval_case(
    scenario: MainChatCapabilityEvalScenario,
    fixture_mode: MainChatCapabilityEvalFixtureMode,
) -> MainChatCapabilityEvalCaseReport {
    match collect_main_chat_capability_eval_artifacts(scenario, fixture_mode).await {
        Ok(artifacts) => evaluate_main_chat_capability_eval_artifacts(&artifacts),
        Err(error) => MainChatCapabilityEvalCaseReport::failed_before_artifacts(scenario, error),
    }
}

impl MainChatCapabilityEvalReport {
    fn from_cases(cases: Vec<MainChatCapabilityEvalCaseReport>) -> Self {
        let passed_case_count = cases
            .iter()
            .filter(|case| case.status == MainChatCapabilityEvalCaseStatus::Passed)
            .count();
        let blocked_case_count = cases
            .iter()
            .filter(|case| case.status == MainChatCapabilityEvalCaseStatus::Blocked)
            .count();
        let failed_case_count = cases
            .iter()
            .filter(|case| case.status == MainChatCapabilityEvalCaseStatus::Failed)
            .count();
        let legacy_fallback_count = cases
            .iter()
            .filter(|case| case.legacy_fallback_used)
            .count();
        let silent_write_count = cases
            .iter()
            .filter(|case| case.silent_write_detected)
            .count();
        let direct_durable_write_count = cases
            .iter()
            .filter(|case| case.direct_durable_write_detected)
            .count();
        let fake_observation_count = cases
            .iter()
            .filter(|case| case.fake_observation_detected)
            .count();
        let live_only_proof_count = cases
            .iter()
            .filter(|case| case.live_only_proof_used)
            .count();
        let mut blockers = Vec::new();
        if failed_case_count > 0 {
            blockers.push("capability_eval_failed_cases".into());
        }
        if blocked_case_count > 0 {
            blockers.push("capability_eval_blocked_cases".into());
        }
        if legacy_fallback_count > 0 {
            blockers.push("capability_eval_legacy_fallback_used".into());
        }
        if silent_write_count > 0 {
            blockers.push("capability_eval_silent_write_detected".into());
        }
        if direct_durable_write_count > 0 {
            blockers.push("capability_eval_direct_durable_write_detected".into());
        }
        if fake_observation_count > 0 {
            blockers.push("capability_eval_fake_observation_detected".into());
        }
        if live_only_proof_count > 0 {
            blockers.push("capability_eval_live_only_proof_used".into());
        }

        Self {
            report_kind: CAPABILITY_EVAL_REPORT_KIND.into(),
            schema_version: CAPABILITY_EVAL_SCHEMA_VERSION.into(),
            readiness_semantics: CAPABILITY_EVAL_READINESS.into(),
            local_deterministic_ready: blockers.is_empty(),
            allow_writes: false,
            live_provider_required: false,
            stream_coverage_reused_from_command_surface_gate: true,
            stream_coverage_gate: CAPABILITY_EVAL_STREAM_GATE.into(),
            total_case_count: cases.len(),
            passed_case_count,
            blocked_case_count,
            failed_case_count,
            legacy_fallback_count,
            silent_write_count,
            direct_durable_write_count,
            fake_observation_count,
            live_only_proof_count,
            cases,
            blockers,
        }
    }
}

impl MainChatCapabilityEvalCaseReport {
    fn failed_before_artifacts(scenario: MainChatCapabilityEvalScenario, error: String) -> Self {
        Self {
            scenario_id: scenario.id().into(),
            status: MainChatCapabilityEvalCaseStatus::Failed,
            capability_group: scenario.capability_group().into(),
            prompt: scenario.prompt().into(),
            entry_point: "send".into(),
            expected_route: scenario.expected_route().as_str().into(),
            actual_route: None,
            task_session_id: None,
            run_ids: Vec::new(),
            route_decision_observed: false,
            deterministic_route_used: false,
            route_preview_advisory_only: true,
            generation_result_observed: false,
            provider_scheduler_trace_observed: false,
            final_assistant_delivery_observed: false,
            final_transcript_observed: false,
            tool_action_count: 0,
            observation_count: 0,
            proposal_record_count: 0,
            permission_record_count: 0,
            legacy_fallback_used: false,
            silent_write_detected: false,
            direct_durable_write_detected: false,
            fake_observation_detected: false,
            live_only_proof_used: false,
            read_execution_kind: None,
            read_source_kind: None,
            read_real_read_only_execution: None,
            read_fixture_backed: None,
            network_policy_enabled: None,
            structured_blocker: None,
            evidence: Vec::new(),
            blockers: vec![error],
        }
    }
}

async fn collect_main_chat_capability_eval_artifacts(
    scenario: MainChatCapabilityEvalScenario,
    fixture_mode: MainChatCapabilityEvalFixtureMode,
) -> Result<MainChatCapabilityEvalArtifacts, String> {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_main_chat_capability_eval_state(&state, scenario, fixture_mode).await?;
    let network_policy_enabled = state.config.lock().await.system.network_policy.enabled;
    let session_id = main_chat_capability_eval_session_id(scenario, fixture_mode);
    let prompt = main_chat_capability_eval_prompt(scenario, fixture_mode);
    let response = crate::main_chat_send::send_message_with_state(
        session_id.clone(),
        vec![ChatMessage {
            role: "user".into(),
            content: prompt.into(),
        }],
        None,
        &state,
    )
    .await?;
    let task_session_id = response
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .ok_or_else(|| "capability eval missing task session id".to_string())?
        .to_string();
    let actual_route = response
        .agent_ingress
        .as_ref()
        .map(|decision| decision.selected_strategy);
    let legacy_fallback_used = response.legacy_fallback_used;
    let reply_non_empty = !response.reply.trim().is_empty();
    let tool_call_count = response.tool_calls.len();
    let response_value = serde_json::to_value(&response)
        .map_err(|error| format!("serialize capability eval response failed: {error}"))?;
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "capability eval missing main chat session store".to_string())?;
    let (session, transcript) = {
        let store = store_arc.lock().await;
        let session = store
            .load_session(&task_session_id)
            .map_err(|error| format!("load capability eval task session failed: {error}"))?
            .ok_or_else(|| "capability eval task session missing after execution".to_string())?;
        let transcript = store
            .list_transcript_entries(&task_session_id)
            .map_err(|error| format!("list capability eval transcript failed: {error}"))?;
        (session, transcript)
    };
    let actions = if let Some(ref queue_arc) = state.main_chat_action_queue_store {
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(&task_session_id)
            .map_err(|error| format!("list capability eval actions failed: {error}"))?
    } else {
        Vec::new()
    };
    let proposal_record_count = if let Some(ref proposal_arc) = state.proposal_store {
        let proposal_store = proposal_arc.lock().await;
        proposal_store
            .list_pending_proposals(50)
            .map_err(|error| format!("list capability eval proposals failed: {error}"))?
            .len()
    } else {
        0
    };
    let permission_record_count = state
        .tool_permission_store
        .lock()
        .await
        .list()
        .map_err(|error| format!("list capability eval permissions failed: {error}"))?
        .len();
    let runs = if let Some(ref run_store_arc) = state.agent_run_store {
        let run_store = run_store_arc.lock().await;
        run_store
            .list_runs_for_session(&session_id, 20)
            .map_err(|error| format!("list capability eval runs failed: {error}"))?
    } else {
        Vec::new()
    };

    Ok(MainChatCapabilityEvalArtifacts {
        scenario,
        response_value,
        task_session_id,
        session,
        transcript,
        actions,
        proposal_record_count,
        permission_record_count,
        runs,
        legacy_fallback_used,
        reply_non_empty,
        tool_call_count,
        actual_route,
        network_policy_enabled,
    })
}

async fn configure_main_chat_capability_eval_state(
    state: &Arc<AppState>,
    scenario: MainChatCapabilityEvalScenario,
    fixture_mode: MainChatCapabilityEvalFixtureMode,
) -> Result<(), String> {
    match (scenario, fixture_mode) {
        (MainChatCapabilityEvalScenario::CfDirect01, _) => {
            configure_main_chat_command_surface_eval_state(
                state,
                MainChatCommandSurfaceEvalScenario::DirectProviderTrace,
            )
            .await
        }
        (MainChatCapabilityEvalScenario::CfFile01, _) => {
            configure_main_chat_command_surface_eval_state(
                state,
                MainChatCommandSurfaceEvalScenario::FileReadSuccess,
            )
            .await
        }
        (MainChatCapabilityEvalScenario::CfWeb01, _) => {
            configure_main_chat_command_surface_eval_state(
                state,
                MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess,
            )
            .await
        }
        (MainChatCapabilityEvalScenario::CfMcp01, MainChatCapabilityEvalFixtureMode::Default) => {
            configure_main_chat_command_surface_eval_state(
                state,
                MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess,
            )
            .await
        }
        (
            MainChatCapabilityEvalScenario::CfMcp01,
            MainChatCapabilityEvalFixtureMode::MissingMcpFixture,
        ) => {
            configure_main_chat_command_surface_eval_state(
                state,
                MainChatCommandSurfaceEvalScenario::MissingMcpBlocker,
            )
            .await
        }
    }
}

fn main_chat_capability_eval_prompt(
    scenario: MainChatCapabilityEvalScenario,
    fixture_mode: MainChatCapabilityEvalFixtureMode,
) -> &'static str {
    if scenario == MainChatCapabilityEvalScenario::CfMcp01
        && fixture_mode == MainChatCapabilityEvalFixtureMode::MissingMcpFixture
    {
        "Use mcp missing.status read-only now."
    } else {
        scenario.prompt()
    }
}

fn main_chat_capability_eval_session_id(
    scenario: MainChatCapabilityEvalScenario,
    fixture_mode: MainChatCapabilityEvalFixtureMode,
) -> String {
    let suffix = match fixture_mode {
        MainChatCapabilityEvalFixtureMode::Default => "default",
        MainChatCapabilityEvalFixtureMode::MissingMcpFixture => "missing-mcp-fixture",
    };
    format!(
        "capability-eval-{}-{suffix}",
        scenario.id().to_ascii_lowercase()
    )
}

fn evaluate_main_chat_capability_eval_artifacts(
    artifacts: &MainChatCapabilityEvalArtifacts,
) -> MainChatCapabilityEvalCaseReport {
    let generation = artifacts
        .response_value
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"));
    let route_decision_observed = artifacts.actual_route.is_some()
        && artifacts
            .transcript
            .iter()
            .any(|entry| entry.kind == ExecutionTranscriptEntryKind::RouteDecision);
    let generation_result_observed = generation.is_some();
    let final_transcript_observed = artifacts
        .transcript
        .iter()
        .any(|entry| entry.kind == ExecutionTranscriptEntryKind::FinalResult)
        || artifacts.session.final_summary.is_some();
    let final_assistant_delivery_observed = artifacts.reply_non_empty
        && final_transcript_observed
        && matches!(
            artifacts.session.status,
            AgentTaskSessionStatus::Completed
                | AgentTaskSessionStatus::Blocked
                | AgentTaskSessionStatus::WaitingPermission
        );
    let read_evidence = first_read_execution_evidence(&artifacts.actions);
    let observation_count = artifacts
        .actions
        .iter()
        .filter(|action| action.observation_metadata.is_some())
        .count()
        + artifacts
            .transcript
            .iter()
            .filter(|entry| entry.kind == ExecutionTranscriptEntryKind::Observation)
            .count();
    let fake_observation_detected = artifacts.actions.iter().any(|action| {
        matches!(
            action.status,
            ExecutionQueueStatus::Completed
                | ExecutionQueueStatus::Observed
                | ExecutionQueueStatus::Failed
        ) && action.observation_metadata.is_none()
    });
    let silent_write_detected = main_chat_command_surface_eval_has_silent_write(
        Some(&artifacts.response_value),
        &artifacts.transcript,
        &artifacts.actions,
        &artifacts.runs,
    );
    let direct_durable_write_detected =
        silent_write_detected || artifacts.actions.iter().any(is_direct_durable_write_action);
    let legacy_fallback_used = artifacts.legacy_fallback_used
        || artifacts
            .transcript
            .iter()
            .any(|entry| entry.kind == ExecutionTranscriptEntryKind::Fallback)
        || json_contains_bool_at_key(&artifacts.response_value, "legacyFallbackUsed", true);
    let live_only_proof_used =
        json_contains_bool_at_key(&artifacts.response_value, "liveProviderInvoked", true)
            || artifacts.transcript.iter().any(|entry| {
                json_contains_bool_at_key(&entry.metadata, "liveProviderInvoked", true)
            })
            || artifacts.actions.iter().any(|action| {
                action
                    .observation_metadata
                    .as_ref()
                    .is_some_and(|metadata| {
                        json_contains_bool_at_key(metadata, "liveProviderInvoked", true)
                    })
            });
    let actual_route = artifacts
        .actual_route
        .map(|route| route.as_str().to_string());
    let deterministic_route_used = artifacts.actual_route
        == Some(artifacts.scenario.expected_route())
        && artifacts.session.selected_strategy == artifacts.scenario.expected_route();
    let route_preview_advisory_only = deterministic_route_used;
    let provider_scheduler_trace_observed = generation.is_some_and(|generation| {
        generation
            .get("providerGenerationPath")
            .and_then(serde_json::Value::as_str)
            == Some("main_chat_direct_answer_scheduler")
            && generation
                .get("schedulerGenerationCalled")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            && generation
                .get("modelGenerated")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
    });
    let structured_blocker = structured_capability_blocker(artifacts);
    let mut report = MainChatCapabilityEvalCaseReport {
        scenario_id: artifacts.scenario.id().into(),
        status: MainChatCapabilityEvalCaseStatus::Failed,
        capability_group: artifacts.scenario.capability_group().into(),
        prompt: artifacts.scenario.prompt().into(),
        entry_point: "send".into(),
        expected_route: artifacts.scenario.expected_route().as_str().into(),
        actual_route,
        task_session_id: Some(artifacts.task_session_id.clone()),
        run_ids: artifacts.runs.iter().map(|run| run.id.clone()).collect(),
        route_decision_observed,
        deterministic_route_used,
        route_preview_advisory_only,
        generation_result_observed,
        provider_scheduler_trace_observed,
        final_assistant_delivery_observed,
        final_transcript_observed,
        tool_action_count: artifacts.actions.len(),
        observation_count,
        proposal_record_count: artifacts.proposal_record_count,
        permission_record_count: artifacts.permission_record_count,
        legacy_fallback_used,
        silent_write_detected,
        direct_durable_write_detected,
        fake_observation_detected,
        live_only_proof_used,
        read_execution_kind: read_evidence
            .and_then(|evidence| evidence.get("kind"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        read_source_kind: read_evidence
            .and_then(|evidence| evidence.get("sourceKind"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        read_real_read_only_execution: read_evidence
            .and_then(|evidence| evidence.get("realReadOnlyExecution"))
            .and_then(serde_json::Value::as_bool),
        read_fixture_backed: read_evidence
            .and_then(|evidence| evidence.get("fixtureBacked"))
            .and_then(serde_json::Value::as_bool),
        network_policy_enabled: Some(artifacts.network_policy_enabled),
        structured_blocker,
        evidence: Vec::new(),
        blockers: Vec::new(),
    };

    push_common_capability_blockers(&mut report);
    match artifacts.scenario {
        MainChatCapabilityEvalScenario::CfDirect01 => {
            assert_cf_direct_01(artifacts, &mut report);
        }
        MainChatCapabilityEvalScenario::CfFile01 => {
            assert_read_capability(
                artifacts,
                &mut report,
                "file.read",
                "file_system_read",
                "file",
                Some(true),
                Some(false),
            );
        }
        MainChatCapabilityEvalScenario::CfWeb01 => {
            assert_cf_web_01(artifacts, &mut report);
        }
        MainChatCapabilityEvalScenario::CfMcp01 => {
            assert_cf_mcp_01(artifacts, &mut report);
        }
    }

    if report.structured_blocker.is_some() && report.blockers.is_empty() {
        report.status = MainChatCapabilityEvalCaseStatus::Blocked;
    } else if report.blockers.is_empty() {
        report.status = MainChatCapabilityEvalCaseStatus::Passed;
    } else if report.structured_blocker.is_some()
        && report
            .blockers
            .iter()
            .all(|blocker| blocker.starts_with("expected_blocker:"))
    {
        report.status = MainChatCapabilityEvalCaseStatus::Blocked;
    } else {
        report.status = MainChatCapabilityEvalCaseStatus::Failed;
    }

    report
}

fn push_common_capability_blockers(report: &mut MainChatCapabilityEvalCaseReport) {
    if !report.route_decision_observed {
        report.blockers.push("route_decision_missing".into());
    } else {
        report.evidence.push("route_decision_observed".into());
    }
    if !report.deterministic_route_used {
        report.blockers.push("deterministic_route_mismatch".into());
    } else {
        report.evidence.push("deterministic_route_used".into());
    }
    if !report.route_preview_advisory_only {
        report.blockers.push("route_preview_used_as_route".into());
    }
    if !report.generation_result_observed {
        report.blockers.push("generation_result_missing".into());
    } else {
        report.evidence.push("generation_result_observed".into());
    }
    if !report.final_assistant_delivery_observed {
        report
            .blockers
            .push("final_assistant_delivery_missing".into());
    } else {
        report
            .evidence
            .push("final_assistant_delivery_observed".into());
    }
    if report.legacy_fallback_used {
        report.blockers.push("legacy_fallback_used".into());
    }
    if report.silent_write_detected {
        report.blockers.push("silent_write_detected".into());
    }
    if report.direct_durable_write_detected {
        report.blockers.push("direct_durable_write_detected".into());
    }
    if report.fake_observation_detected {
        report.blockers.push("fake_observation_detected".into());
    }
    if report.live_only_proof_used {
        report.blockers.push("live_only_proof_used".into());
    }
}

fn assert_cf_direct_01(
    artifacts: &MainChatCapabilityEvalArtifacts,
    report: &mut MainChatCapabilityEvalCaseReport,
) {
    if artifacts.session.status != AgentTaskSessionStatus::Completed {
        report.blockers.push(format!(
            "direct_session_not_completed:{:?}",
            artifacts.session.status
        ));
    }
    if !artifacts.session.pending_blockers.is_empty() {
        report.blockers.push("direct_session_kept_blockers".into());
    }
    if !artifacts.actions.is_empty() || artifacts.tool_call_count != 0 {
        report
            .blockers
            .push("direct_answer_created_tool_action".into());
    } else {
        report.evidence.push("no_tool_action_or_tool_call".into());
    }
    if artifacts.proposal_record_count != 0 {
        report
            .blockers
            .push("direct_answer_created_proposal_record".into());
    }
    if !report.provider_scheduler_trace_observed {
        report
            .blockers
            .push("direct_provider_scheduler_trace_missing".into());
    } else {
        report
            .evidence
            .push("provider_scheduler_trace_observed".into());
    }
    if !artifacts.runs.iter().any(|run| {
        run.reasoning_strategy.as_deref() == Some("main_chat_agent_v1_direct_answer")
            && run.model_route.is_some()
            && run.tool_call_count == 0
    }) {
        report
            .blockers
            .push("direct_agent_run_route_or_tool_count_missing".into());
    } else {
        report.evidence.push("direct_agent_run_observed".into());
    }
}

fn assert_cf_web_01(
    artifacts: &MainChatCapabilityEvalArtifacts,
    report: &mut MainChatCapabilityEvalCaseReport,
) {
    if !artifacts.network_policy_enabled {
        report.blockers.push("web_network_policy_disabled".into());
    } else {
        report.evidence.push("web_network_policy_enabled".into());
    }
    assert_read_capability(
        artifacts,
        report,
        "web.search",
        "web_search_fixture",
        "web",
        Some(false),
        Some(true),
    );
    if report.live_only_proof_used
        || report.read_execution_kind.as_deref() == Some("web_search_network")
    {
        report.blockers.push("web_fixture_credited_as_live".into());
    }
}

fn assert_cf_mcp_01(
    artifacts: &MainChatCapabilityEvalArtifacts,
    report: &mut MainChatCapabilityEvalCaseReport,
) {
    if report.structured_blocker.as_deref() == Some("cf_mcp_fixture_unavailable") {
        report
            .evidence
            .push("structured_mcp_fixture_blocker_observed".into());
        report
            .blockers
            .push("expected_blocker:cf_mcp_fixture_unavailable".into());
        return;
    }
    assert_read_capability(
        artifacts,
        report,
        "mcp.read_only",
        "registered_mcp_read",
        "mcp",
        Some(true),
        Some(false),
    );
    let Some(mcp_action) = artifacts
        .actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
    else {
        return;
    };
    let Some(metadata) = mcp_action.observation_metadata.as_ref() else {
        return;
    };
    if metadata
        .get("mcpReadTargetResolved")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
        || metadata.get("target").and_then(serde_json::Value::as_str) != Some("builtin_echo")
    {
        report
            .blockers
            .push("registered_mcp_target_resolution_missing".into());
    } else {
        report
            .evidence
            .push("registered_mcp_target_resolved".into());
    }
    if metadata
        .get("strictManifestIdentity")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
        || metadata
            .get("kernelBackedReadOnlyToolLoop")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
    {
        report
            .evidence
            .push("registered_mcp_manifest_identity_observed".into());
    }
}

fn assert_read_capability(
    artifacts: &MainChatCapabilityEvalArtifacts,
    report: &mut MainChatCapabilityEvalCaseReport,
    action_type: &str,
    expected_kind: &str,
    expected_source_kind: &str,
    expected_real_read_only: Option<bool>,
    expected_fixture_backed: Option<bool>,
) {
    if artifacts.session.status != AgentTaskSessionStatus::Completed {
        report.blockers.push(format!(
            "{action_type}_session_not_completed:{:?}",
            artifacts.session.status
        ));
    }
    if !artifacts.session.pending_blockers.is_empty() {
        report
            .blockers
            .push(format!("{action_type}_session_kept_blockers"));
    }
    if artifacts.proposal_record_count != 0 {
        report
            .blockers
            .push(format!("{action_type}_unexpected_proposal_record"));
    }
    let action = artifacts
        .actions
        .iter()
        .find(|action| action.action.action_type == action_type);
    let Some(action) = action else {
        report
            .blockers
            .push(format!("{action_type}_action_missing"));
        return;
    };
    if action.status != ExecutionQueueStatus::Completed {
        report.blockers.push(format!(
            "{action_type}_action_not_completed:{:?}",
            action.status
        ));
    }
    let Some(metadata) = action.observation_metadata.as_ref() else {
        report
            .blockers
            .push(format!("{action_type}_observation_metadata_missing"));
        return;
    };
    if metadata
        .get("sourceKind")
        .and_then(serde_json::Value::as_str)
        != Some(expected_source_kind)
    {
        report
            .blockers
            .push(format!("{action_type}_source_kind_mismatch"));
    }
    if metadata
        .get("preview")
        .and_then(serde_json::Value::as_str)
        .map_or(true, str::is_empty)
    {
        report
            .blockers
            .push(format!("{action_type}_observation_preview_missing"));
    }
    let read_evidence = metadata
        .get("structuredResult")
        .and_then(|value| value.get("readExecutionEvidence"));
    let Some(read_evidence) = read_evidence else {
        report
            .blockers
            .push(format!("{action_type}_read_execution_evidence_missing"));
        return;
    };
    if read_evidence
        .get("kind")
        .and_then(serde_json::Value::as_str)
        != Some(expected_kind)
    {
        report
            .blockers
            .push(format!("{action_type}_read_execution_kind_mismatch"));
    }
    if read_evidence
        .get("sourceKind")
        .and_then(serde_json::Value::as_str)
        != Some(expected_source_kind)
    {
        report
            .blockers
            .push(format!("{action_type}_read_source_kind_mismatch"));
    }
    if let Some(expected_real_read_only) = expected_real_read_only {
        if read_evidence
            .get("realReadOnlyExecution")
            .and_then(serde_json::Value::as_bool)
            != Some(expected_real_read_only)
        {
            report
                .blockers
                .push(format!("{action_type}_real_read_only_flag_mismatch"));
        }
    }
    if let Some(expected_fixture_backed) = expected_fixture_backed {
        if read_evidence
            .get("fixtureBacked")
            .and_then(serde_json::Value::as_bool)
            != Some(expected_fixture_backed)
        {
            report
                .blockers
                .push(format!("{action_type}_fixture_flag_mismatch"));
        }
    }
    if read_evidence
        .get("directWritesExecuted")
        .and_then(serde_json::Value::as_bool)
        != Some(false)
        || metadata
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
    {
        report
            .blockers
            .push(format!("{action_type}_direct_write_flag_missing"));
    }
    if !artifacts.transcript.iter().any(|entry| {
        entry
            .summary
            .contains("MainChatKernel read-only tool loop completed")
            || entry.summary.contains("Governed ReAct AgentLoop completed")
    }) {
        report.blockers.push(format!(
            "{action_type}_read_loop_completion_transcript_missing"
        ));
    } else {
        report
            .evidence
            .push(format!("{action_type}_read_loop_completion_transcript"));
    }
    if generation_provider_path(&artifacts.response_value)
        != Some("main_chat_kernel_read_tool_synthesis")
    {
        report
            .blockers
            .push(format!("{action_type}_final_synthesis_generation_missing"));
    } else {
        report
            .evidence
            .push(format!("{action_type}_final_synthesis_generation"));
    }
    report
        .evidence
        .push(format!("{action_type}_{expected_kind}_observation"));
}

fn structured_capability_blocker(artifacts: &MainChatCapabilityEvalArtifacts) -> Option<String> {
    if artifacts.scenario == MainChatCapabilityEvalScenario::CfMcp01
        && artifacts.session.status == AgentTaskSessionStatus::Blocked
        && artifacts
            .session
            .pending_blockers
            .iter()
            .any(|blocker| blocker.contains("mcp_read_tool_not_registered"))
        && artifacts.actions.iter().any(|action| {
            action.action.action_type == "mcp.read_only"
                && action.status == ExecutionQueueStatus::Failed
                && action
                    .observation_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get("blockerReason"))
                    .and_then(serde_json::Value::as_str)
                    == Some("mcp_read_tool_not_registered")
        })
    {
        return Some("cf_mcp_fixture_unavailable".into());
    }
    None
}

fn first_read_execution_evidence(actions: &[QueuedExecutionAction]) -> Option<&serde_json::Value> {
    actions.iter().find_map(|action| {
        action
            .observation_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("structuredResult"))
            .and_then(|structured| structured.get("readExecutionEvidence"))
    })
}

fn generation_provider_path(response: &serde_json::Value) -> Option<&str> {
    response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"))
        .and_then(|generation| generation.get("providerGenerationPath"))
        .and_then(serde_json::Value::as_str)
}

fn json_contains_bool_at_key(value: &serde_json::Value, key: &str, expected: bool) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(candidate_key, value)| {
            (candidate_key == key && value.as_bool() == Some(expected))
                || json_contains_bool_at_key(value, key, expected)
        }),
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| json_contains_bool_at_key(value, key, expected)),
        _ => false,
    }
}

fn is_direct_durable_write_action(action: &QueuedExecutionAction) -> bool {
    matches!(
        action.action.action_type.as_str(),
        "file.write"
            | "file.update"
            | "knowledge.write"
            | "memory.write"
            | "life_model.write"
            | "calendar.write"
            | "email.send"
            | "external.write"
    ) || action
        .observation_metadata
        .as_ref()
        .is_some_and(json_contains_direct_write_true)
}
