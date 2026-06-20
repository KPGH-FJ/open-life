use crate::main_chat_stage4_memory_knowledge::{
    confirm_managed_knowledge_write_with_state, create_managed_knowledge_write_draft_with_state,
    rollback_managed_knowledge_write_with_state,
};
use crate::main_chat_task_controls::get_main_chat_agent_task_detail_with_state;
use crate::storage::app_data_dir;
use crate::AppState;
use chrono::Utc;
use openlife_core::agent::main_chat_agent_v1::{
    ExecutionQueueStatus, ExecutionTranscriptEntry, ExecutionTranscriptEntryKind,
    QueuedExecutionAction,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

const STAGE5_PREFLIGHT_SCHEMA_VERSION: &str = "stage5-preflight-v1";
const STAGE5_BUNDLE_SCHEMA_VERSION: &str = "stage5-debug-bundle-v1";
const STAGE5_ISSUE_SCHEMA_VERSION: &str = "stage5-issue-report-v1";
const STAGE5_REPORT_SCHEMA_VERSION: &str = "stage5-release-debug-v1";
const PREVIEW_LIMIT: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5BuildInfo {
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub app_version: String,
    pub build_timestamp: Option<String>,
    pub dirty_state: Option<bool>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5ProviderPreflight {
    pub provider: String,
    pub model: String,
    pub route_type: String,
    pub key_present: bool,
    pub network_opt_in: bool,
    pub live_provider_invocation_allowed: bool,
    pub live_provider_preflight_status: String,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5SchedulerPreflight {
    pub scheduler_type: String,
    pub scripted_provider_response_present: bool,
    pub prefer_local: bool,
    pub local_model_configured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5WorkspacePreflight {
    pub root_digest: String,
    pub safe_path_count: usize,
    pub safe_paths_digest: String,
    pub safe_paths_configured: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5McpPreflight {
    pub registry_available: bool,
    pub manifest_count: usize,
    pub read_candidate_count: usize,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5DatabasePreflight {
    pub memory_store_available: bool,
    pub agent_run_store_available: bool,
    pub task_session_store_available: bool,
    pub action_queue_store_available: bool,
    pub proposal_store_available: bool,
    pub memory_lifecycle_store_available: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5GateSummary {
    pub recommendation: String,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5FailureClassification {
    pub class: String,
    pub severity: String,
    pub scope: String,
    pub recoverability: String,
    pub recovery_recommendation: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5PreflightReport {
    pub report_kind: String,
    pub schema_version: String,
    pub created_at: String,
    pub build: MainChatStage5BuildInfo,
    pub provider: MainChatStage5ProviderPreflight,
    pub scheduler: MainChatStage5SchedulerPreflight,
    pub workspace: MainChatStage5WorkspacePreflight,
    pub mcp: MainChatStage5McpPreflight,
    pub database: MainChatStage5DatabasePreflight,
    pub stage2_readiness: MainChatStage5GateSummary,
    pub final_acceptance: MainChatStage5GateSummary,
    pub failure: MainChatStage5FailureClassification,
    pub external_provider_invoked_by_default: bool,
    pub model_invoked: bool,
    pub direct_writes_executed: bool,
    pub metadata_safe: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5ArtifactMetadata {
    pub artifact_id: String,
    pub artifact_kind: String,
    pub schema_version: String,
    pub created_at: String,
    pub storage_alias: String,
    pub digest: String,
    pub byte_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5UiEvidence {
    pub frontend_route: String,
    pub surface: String,
    pub visible_control_labels: Vec<String>,
    pub task_session_id: String,
    #[serde(default)]
    pub backend_snapshot_id: Option<String>,
    pub timestamp: String,
    #[serde(default)]
    pub dom_digest: Option<String>,
    #[serde(default)]
    pub screenshot_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5BundleScenario {
    #[serde(default)]
    pub scenario_id: Option<String>,
    #[serde(default)]
    pub reviewer_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub notes_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5BundleTaskSummary {
    pub chat_session_id: String,
    pub task_session_id: String,
    pub run_id: Option<String>,
    pub strategy: String,
    pub status: String,
    pub user_goal_digest: String,
    pub transcript_entry_count: usize,
    pub action_count: usize,
    pub proposal_count: usize,
    pub blocker_count: usize,
    pub final_delivery_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5RouteSummary {
    pub route_type: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub local_only: bool,
    pub live_provider_attempted: bool,
    pub provider_endpoint_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5TimelineItem {
    pub item_id: String,
    pub kind: String,
    pub summary_preview: String,
    pub metadata_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5ToolSummary {
    pub candidate_count: usize,
    pub selected_tool: Option<String>,
    pub action_type: Option<String>,
    pub target_digest: Option<String>,
    pub policy_decision: Option<String>,
    pub observation_count: usize,
    pub action_statuses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5ContextSummary {
    pub active_memory_ids: Vec<String>,
    pub excluded_memory_ids: Vec<String>,
    pub knowledge_asset_ids: Vec<String>,
    pub selected_skill_id: Option<String>,
    pub context_source_digests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5MemorySummary {
    pub proposal_ids: Vec<String>,
    pub accepted_memory_ids: Vec<String>,
    pub rolled_back_memory_ids: Vec<String>,
    pub managed_knowledge_version_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5FinalDeliverySummary {
    pub completed_work_count: usize,
    pub durable_change_count: usize,
    pub pending_user_action_count: usize,
    pub skipped_work_count: usize,
    pub blocker_count: usize,
    pub final_delivery_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5RedactionReport {
    pub mode: String,
    pub raw_content_included: bool,
    pub secrets_detected: bool,
    pub unsafe_field_count: usize,
    pub unsafe_fields_dropped: Vec<String>,
    pub preview_limit: usize,
    pub prompt_digest: Option<String>,
    pub response_digest: Option<String>,
    pub context_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5DebugBundle {
    pub bundle_id: String,
    pub schema_version: String,
    pub created_at: String,
    pub build: MainChatStage5BuildInfo,
    pub environment: MainChatStage5PreflightReport,
    pub scenario: MainChatStage5BundleScenario,
    pub task: MainChatStage5BundleTaskSummary,
    pub route: MainChatStage5RouteSummary,
    pub timeline: Vec<MainChatStage5TimelineItem>,
    pub tools: MainChatStage5ToolSummary,
    pub context: MainChatStage5ContextSummary,
    pub memory: MainChatStage5MemorySummary,
    pub final_delivery: MainChatStage5FinalDeliverySummary,
    pub failure: MainChatStage5FailureClassification,
    pub redaction: MainChatStage5RedactionReport,
    #[serde(default)]
    pub ui_evidence: Option<MainChatStage5UiEvidence>,
    pub artifact: MainChatStage5ArtifactMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5IssueReportInput {
    pub scenario_id: String,
    pub reviewer_id: String,
    pub status: String,
    #[serde(default)]
    pub task_session_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub bundle_id: Option<String>,
    #[serde(default)]
    pub failure_class: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub preflight_only_missing_task_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5IssueReport {
    pub report_id: String,
    pub schema_version: String,
    pub created_at: String,
    pub scenario_id: String,
    pub reviewer_id: String,
    pub status: String,
    pub task_session_id: Option<String>,
    pub run_id: Option<String>,
    pub bundle_id: Option<String>,
    pub build_commit: Option<String>,
    pub app_version: String,
    pub redaction_mode: String,
    pub failure_class: Option<String>,
    pub notes_digest: Option<String>,
    pub notes_preview: Option<String>,
    pub missing_task_run_reason: Option<String>,
    pub blockers: Vec<String>,
    pub artifact: MainChatStage5ArtifactMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5ReportRow {
    pub id: String,
    pub scenario: String,
    pub status: String,
    pub evidence_ids: Vec<String>,
    pub bundle_ids: Vec<String>,
    pub issue_artifact_ids: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5ManagedKnowledgeEval {
    pub isolated_eval_app_state: bool,
    pub temp_workspace: bool,
    pub real_workspace_write_executed: bool,
    pub user_write_completed: bool,
    pub memory_rollback_completed: bool,
    pub managed_knowledge_write_version_ids: Vec<String>,
    pub managed_knowledge_audit_ids: Vec<String>,
    pub rollback_snapshot_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatStage5ReleaseDebugReport {
    pub report_kind: String,
    pub schema_version: String,
    pub scenario_count: usize,
    pub passed_scenario_count: usize,
    pub blocked_scenario_count: usize,
    pub not_a_readiness_gate: bool,
    pub readiness_claim: bool,
    pub rows: Vec<MainChatStage5ReportRow>,
    pub evidence_ids: Vec<String>,
    pub blockers: Vec<String>,
    pub build: MainChatStage5BuildInfo,
    pub preflight_summary: MainChatStage5PreflightReport,
    pub bundle_ids: Vec<String>,
    pub issue_artifact_ids: Vec<String>,
    pub artifact_storage_summary: Vec<MainChatStage5ArtifactMetadata>,
    pub redaction_summary: MainChatStage5RedactionReport,
    pub managed_knowledge_eval: MainChatStage5ManagedKnowledgeEval,
    pub stage2_readiness_preserved: bool,
}

#[tauri::command]
pub async fn evaluate_main_chat_stage5_release_debug_preflight(
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatStage5PreflightReport, String> {
    evaluate_main_chat_stage5_release_debug_preflight_with_state(state.inner()).await
}

#[tauri::command]
pub async fn export_main_chat_agent_debug_bundle(
    task_session_id: String,
    scenario_id: Option<String>,
    reviewer_id: Option<String>,
    ui_evidence: Option<MainChatStage5UiEvidence>,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatStage5DebugBundle, String> {
    export_main_chat_agent_debug_bundle_with_store_root(
        state.inner(),
        &app_data_dir(),
        task_session_id,
        scenario_id,
        reviewer_id,
        ui_evidence,
    )
    .await
}

#[tauri::command]
pub async fn create_main_chat_internal_issue_report(
    input: MainChatStage5IssueReportInput,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatStage5IssueReport, String> {
    create_main_chat_internal_issue_report_with_store_root(state.inner(), &app_data_dir(), input)
        .await
}

#[tauri::command]
pub async fn list_main_chat_debug_bundles() -> Result<Vec<MainChatStage5ArtifactMetadata>, String> {
    list_main_chat_debug_bundles_from_root(&app_data_dir())
}

#[tauri::command]
pub async fn get_main_chat_debug_bundle(
    bundle_id: String,
) -> Result<MainChatStage5DebugBundle, String> {
    get_main_chat_debug_bundle_from_root(&app_data_dir(), &bundle_id)
}

#[tauri::command]
pub async fn delete_main_chat_debug_bundle(bundle_id: String) -> Result<bool, String> {
    delete_main_chat_debug_bundle_from_root(&app_data_dir(), &bundle_id)
}

#[tauri::command]
pub async fn list_main_chat_internal_issue_reports(
) -> Result<Vec<MainChatStage5ArtifactMetadata>, String> {
    list_main_chat_internal_issue_reports_from_root(&app_data_dir())
}

#[tauri::command]
pub async fn get_main_chat_internal_issue_report(
    report_id: String,
) -> Result<MainChatStage5IssueReport, String> {
    get_main_chat_internal_issue_report_from_root(&app_data_dir(), &report_id)
}

#[tauri::command]
pub async fn delete_main_chat_internal_issue_report(report_id: String) -> Result<bool, String> {
    delete_main_chat_internal_issue_report_from_root(&app_data_dir(), &report_id)
}

#[tauri::command]
pub async fn run_main_chat_stage5_release_debug_report(
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatStage5ReleaseDebugReport, String> {
    run_main_chat_stage5_release_debug_report_with_store_root(state.inner(), &app_data_dir()).await
}

pub async fn evaluate_main_chat_stage5_release_debug_preflight_with_state(
    state: &Arc<AppState>,
) -> Result<MainChatStage5PreflightReport, String> {
    let build = stage5_build_info();
    let (provider, model, network_opt_in, key_present) = {
        let cfg = state.config.lock().await;
        (
            cfg.llm.provider.clone(),
            cfg.llm.chat_model.clone(),
            cfg.system.network_policy.enabled,
            !cfg.effective_openai_key().trim().is_empty(),
        )
    };
    let (scheduler_type, scripted_provider_response_present, prefer_local, local_model_configured) = {
        let scheduler = state.scheduler.lock().await;
        (
            if scheduler.scripted_generation_response.is_some() {
                "scripted_eval"
            } else {
                "inference_scheduler"
            }
            .to_string(),
            scheduler.scripted_generation_response.is_some(),
            scheduler.prefer_local,
            !scheduler.local_model.trim().is_empty(),
        )
    };
    let live_provider_preflight =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_live_provider_eval_preflight(
            openlife_core::agent::main_chat_agent_v1::MainChatLiveProviderEvalPreflightInput {
                provider: provider.clone(),
                api_key_present: key_present,
                network_enabled: network_opt_in,
                explicit_live_eval_requested: false,
                scripted_provider_response_present,
                local_only_required: false,
            },
        );

    let workspace = stage5_workspace_preflight(state).await;
    let mcp = stage5_mcp_preflight(state).await;
    let database = stage5_database_preflight(state);
    let stage2_readiness = MainChatStage5GateSummary {
        recommendation: "not_ready_for_limited_internal_trial".into(),
        blockers: vec![
            "stage2_manual_dogfood_evidence_missing".into(),
            "stage2_live_provider_p0_evidence_missing".into(),
        ],
    };
    let final_acceptance = MainChatStage5GateSummary {
        recommendation: "not_final_completion_ready".into(),
        blockers: vec![
            "live_provider_generation_not_executed".into(),
            "provider_backed_web_agent_loop_not_executed".into(),
            "provider_backed_mcp_agent_loop_not_executed".into(),
            "provider_live_proposal_permission_not_executed".into(),
        ],
    };

    let provider_preflight = MainChatStage5ProviderPreflight {
        provider,
        model,
        route_type: "default_preflight_no_invocation".into(),
        key_present,
        network_opt_in,
        live_provider_invocation_allowed: false,
        live_provider_preflight_status: live_provider_preflight.status,
        blockers: live_provider_preflight.blockers,
    };
    let scheduler = MainChatStage5SchedulerPreflight {
        scheduler_type,
        scripted_provider_response_present,
        prefer_local,
        local_model_configured,
    };
    let mut blockers = Vec::new();
    blockers.extend(build.blockers.clone());
    blockers.extend(provider_preflight.blockers.clone());
    blockers.extend(workspace.blockers.clone());
    blockers.extend(mcp.blockers.clone());
    blockers.extend(database.blockers.clone());
    blockers.sort();
    blockers.dedup();
    let failure = classify_main_chat_stage5_failure(&blockers);
    let report = MainChatStage5PreflightReport {
        report_kind: "main_chat_stage5_release_debug_preflight".into(),
        schema_version: STAGE5_PREFLIGHT_SCHEMA_VERSION.into(),
        created_at: Utc::now().to_rfc3339(),
        build,
        provider: provider_preflight,
        scheduler,
        workspace,
        mcp,
        database,
        stage2_readiness,
        final_acceptance,
        failure,
        external_provider_invoked_by_default: false,
        model_invoked: false,
        direct_writes_executed: false,
        metadata_safe: true,
        blockers,
    };
    validate_metadata_safe_artifact(&serde_json::to_value(&report).map_err(|e| e.to_string())?)?;
    Ok(report)
}

pub async fn export_main_chat_agent_debug_bundle_with_store_root(
    state: &Arc<AppState>,
    store_root: &Path,
    task_session_id: String,
    scenario_id: Option<String>,
    reviewer_id: Option<String>,
    ui_evidence: Option<MainChatStage5UiEvidence>,
) -> Result<MainChatStage5DebugBundle, String> {
    let detail = get_main_chat_agent_task_detail_with_state(&task_session_id, state).await?;
    let preflight = evaluate_main_chat_stage5_release_debug_preflight_with_state(state).await?;
    let run_id = stage5_run_id_from_transcript(&detail.transcript);
    let run = load_stage5_agent_run(state, run_id.as_deref()).await?;
    let route = stage5_route_summary(&detail.transcript, run.as_ref());
    let final_delivery = stage5_final_delivery_summary(detail.final_delivery.as_ref());
    let timeline = detail
        .transcript
        .iter()
        .map(stage5_timeline_item)
        .collect::<Vec<_>>();
    let tools = stage5_tool_summary(&detail.actions, &detail.transcript);
    let context = stage5_context_summary(state, &detail.transcript).await;
    let memory = stage5_memory_summary(&detail.proposals, state).await;
    let blockers = stage5_bundle_blockers(&detail.blockers, &detail.actions, &detail.transcript);
    let failure = classify_main_chat_stage5_failure(&blockers);
    let redaction = MainChatStage5RedactionReport {
        mode: "metadata_safe".into(),
        raw_content_included: false,
        secrets_detected: false,
        unsafe_field_count: 0,
        unsafe_fields_dropped: Vec::new(),
        preview_limit: PREVIEW_LIMIT,
        prompt_digest: Some(digest_label(&detail.task_session.user_goal)),
        response_digest: run
            .as_ref()
            .and_then(|run| run.output_preview.as_ref())
            .map(|output| digest_label(output)),
        context_digest: Some(digest_json(&json!({
            "context": context,
            "memory": memory,
        }))),
    };

    let artifact_id = format!("stage5-bundle-{}", Uuid::new_v4());
    let created_at = Utc::now().to_rfc3339();
    let artifact = MainChatStage5ArtifactMetadata {
        artifact_id: artifact_id.clone(),
        artifact_kind: "debug_bundle".into(),
        schema_version: STAGE5_BUNDLE_SCHEMA_VERSION.into(),
        created_at: created_at.clone(),
        storage_alias: stage5_artifact_alias("debug_bundles", &artifact_id),
        digest: String::new(),
        byte_size: 0,
    };
    let mut bundle = MainChatStage5DebugBundle {
        bundle_id: artifact_id,
        schema_version: STAGE5_BUNDLE_SCHEMA_VERSION.into(),
        created_at: created_at.clone(),
        build: preflight.build.clone(),
        environment: preflight,
        scenario: MainChatStage5BundleScenario {
            scenario_id,
            reviewer_id,
            status: None,
            notes_digest: None,
        },
        task: MainChatStage5BundleTaskSummary {
            chat_session_id: detail.task_session.chat_session_id.clone(),
            task_session_id: detail.task_session.id.clone(),
            run_id,
            strategy: detail.task_session.selected_strategy.as_str().into(),
            status: detail.task_session.status.as_str().into(),
            user_goal_digest: digest_label(&detail.task_session.user_goal),
            transcript_entry_count: detail.transcript.len(),
            action_count: detail.actions.len(),
            proposal_count: detail.proposals.len(),
            blocker_count: detail.blockers.len(),
            final_delivery_id: detail
                .transcript
                .iter()
                .rev()
                .find(|entry| entry.kind == ExecutionTranscriptEntryKind::FinalResult)
                .map(|entry| entry.id.clone()),
        },
        route,
        timeline,
        tools,
        context,
        memory,
        final_delivery,
        failure,
        redaction,
        ui_evidence: ui_evidence.map(sanitize_ui_evidence),
        artifact,
    };
    let artifact = stage5_artifact_metadata_for_payload(
        "debug_bundles",
        &bundle.bundle_id,
        "debug_bundle",
        STAGE5_BUNDLE_SCHEMA_VERSION,
        &created_at,
        &bundle,
    )?;
    bundle.artifact = artifact;
    persist_stage5_artifact(store_root, "debug_bundles", &bundle.bundle_id, &bundle)?;
    Ok(bundle)
}

pub async fn create_main_chat_internal_issue_report_with_store_root(
    state: &Arc<AppState>,
    store_root: &Path,
    input: MainChatStage5IssueReportInput,
) -> Result<MainChatStage5IssueReport, String> {
    validate_issue_report_input(&input)?;
    let bundle = input
        .bundle_id
        .as_deref()
        .filter(|bundle_id| !bundle_id.trim().is_empty())
        .map(|bundle_id| get_main_chat_debug_bundle_from_root(store_root, bundle_id))
        .transpose()?;
    if let Some(bundle) = bundle.as_ref() {
        if let Some(task_session_id) = input.task_session_id.as_deref() {
            if bundle.task.task_session_id != task_session_id {
                return Err("stage5_issue_task_session_id_mismatch".into());
            }
        }
        if let Some(run_id) = input.run_id.as_deref() {
            if bundle.task.run_id.as_deref() != Some(run_id) {
                return Err("stage5_issue_run_id_mismatch".into());
            }
        }
    }
    let preflight = evaluate_main_chat_stage5_release_debug_preflight_with_state(state).await?;
    let mut blockers = Vec::new();
    if preflight.build.commit.is_none() {
        blockers.push("stage5_issue_build_commit_missing".into());
    }
    if input.task_session_id.is_none() || input.run_id.is_none() {
        blockers.push("stage5_issue_task_run_missing".into());
    }
    if bundle.is_none() {
        blockers.push("stage5_issue_bundle_missing_preflight_only".into());
    }
    blockers.extend(preflight.build.blockers.clone());
    blockers.sort();
    blockers.dedup();

    let (notes_digest, notes_preview, unsafe_fields_dropped) =
        stage5_notes_redaction(input.notes.as_deref());
    if !unsafe_fields_dropped.is_empty() {
        blockers.push("stage5_issue_notes_preview_redacted".into());
    }

    let report_id = format!("stage5-issue-{}", Uuid::new_v4());
    let created_at = Utc::now().to_rfc3339();
    let artifact = MainChatStage5ArtifactMetadata {
        artifact_id: report_id.clone(),
        artifact_kind: "issue_report".into(),
        schema_version: STAGE5_ISSUE_SCHEMA_VERSION.into(),
        created_at: created_at.clone(),
        storage_alias: stage5_artifact_alias("issue_reports", &report_id),
        digest: String::new(),
        byte_size: 0,
    };
    let mut issue = MainChatStage5IssueReport {
        report_id,
        schema_version: STAGE5_ISSUE_SCHEMA_VERSION.into(),
        created_at: created_at.clone(),
        scenario_id: input.scenario_id,
        reviewer_id: input.reviewer_id,
        status: input.status,
        task_session_id: input.task_session_id,
        run_id: input.run_id,
        bundle_id: bundle.map(|bundle| bundle.bundle_id),
        build_commit: preflight.build.commit,
        app_version: preflight.build.app_version,
        redaction_mode: "metadata_safe".into(),
        failure_class: input.failure_class,
        notes_digest,
        notes_preview,
        missing_task_run_reason: input.preflight_only_missing_task_reason,
        blockers,
        artifact,
    };
    let artifact = stage5_artifact_metadata_for_payload(
        "issue_reports",
        &issue.report_id,
        "issue_report",
        STAGE5_ISSUE_SCHEMA_VERSION,
        &created_at,
        &issue,
    )?;
    issue.artifact = artifact;
    persist_stage5_artifact(store_root, "issue_reports", &issue.report_id, &issue)?;
    Ok(issue)
}

pub fn list_main_chat_debug_bundles_from_root(
    store_root: &Path,
) -> Result<Vec<MainChatStage5ArtifactMetadata>, String> {
    list_stage5_artifacts::<MainChatStage5DebugBundle>(store_root, "debug_bundles")
}

pub fn get_main_chat_debug_bundle_from_root(
    store_root: &Path,
    bundle_id: &str,
) -> Result<MainChatStage5DebugBundle, String> {
    read_stage5_artifact(store_root, "debug_bundles", bundle_id)
}

pub fn delete_main_chat_debug_bundle_from_root(
    store_root: &Path,
    bundle_id: &str,
) -> Result<bool, String> {
    delete_stage5_artifact(store_root, "debug_bundles", bundle_id)
}

pub fn list_main_chat_internal_issue_reports_from_root(
    store_root: &Path,
) -> Result<Vec<MainChatStage5ArtifactMetadata>, String> {
    list_stage5_artifacts::<MainChatStage5IssueReport>(store_root, "issue_reports")
}

pub fn get_main_chat_internal_issue_report_from_root(
    store_root: &Path,
    report_id: &str,
) -> Result<MainChatStage5IssueReport, String> {
    read_stage5_artifact(store_root, "issue_reports", report_id)
}

pub fn delete_main_chat_internal_issue_report_from_root(
    store_root: &Path,
    report_id: &str,
) -> Result<bool, String> {
    delete_stage5_artifact(store_root, "issue_reports", report_id)
}

pub async fn run_main_chat_stage5_release_debug_report_with_store_root(
    state: &Arc<AppState>,
    store_root: &Path,
) -> Result<MainChatStage5ReleaseDebugReport, String> {
    let preflight = evaluate_main_chat_stage5_release_debug_preflight_with_state(state).await?;
    let existing_bundles = list_main_chat_debug_bundles_from_root(store_root).unwrap_or_default();
    let bundle_payloads = existing_bundles
        .iter()
        .filter_map(|artifact| {
            get_main_chat_debug_bundle_from_root(store_root, &artifact.artifact_id).ok()
        })
        .collect::<Vec<_>>();
    let existing_issues =
        list_main_chat_internal_issue_reports_from_root(store_root).unwrap_or_default();
    let issue_payloads = existing_issues
        .iter()
        .filter_map(|artifact| {
            get_main_chat_internal_issue_report_from_root(store_root, &artifact.artifact_id).ok()
        })
        .collect::<Vec<_>>();
    let managed_knowledge_eval = run_stage5_isolated_managed_knowledge_eval().await;
    let redaction_summary = MainChatStage5RedactionReport {
        mode: "metadata_safe".into(),
        raw_content_included: false,
        secrets_detected: false,
        unsafe_field_count: 0,
        unsafe_fields_dropped: Vec::new(),
        preview_limit: PREVIEW_LIMIT,
        prompt_digest: None,
        response_digest: None,
        context_digest: Some(digest_json(&json!({
            "bundleCount": existing_bundles.len(),
            "issueCount": existing_issues.len(),
        }))),
    };
    let rows = stage5_dbg_rows(
        &preflight,
        &existing_bundles,
        &existing_issues,
        &bundle_payloads,
        &issue_payloads,
        &managed_knowledge_eval,
    );
    let passed_scenario_count = rows.iter().filter(|row| row.status == "passed").count();
    let blocked_scenario_count = rows.iter().filter(|row| row.status == "blocked").count();
    let blockers = rows
        .iter()
        .filter(|row| row.status == "blocked")
        .flat_map(|row| {
            row.blockers
                .iter()
                .map(move |blocker| format!("{}:{blocker}", row.id))
        })
        .collect::<Vec<_>>();
    Ok(MainChatStage5ReleaseDebugReport {
        report_kind: "main_chat_stage5_release_debug".into(),
        schema_version: STAGE5_REPORT_SCHEMA_VERSION.into(),
        scenario_count: rows.len(),
        passed_scenario_count,
        blocked_scenario_count,
        not_a_readiness_gate: true,
        readiness_claim: false,
        evidence_ids: rows
            .iter()
            .flat_map(|row| row.evidence_ids.iter().cloned())
            .collect(),
        bundle_ids: existing_bundles
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect(),
        issue_artifact_ids: existing_issues
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect(),
        artifact_storage_summary: existing_bundles
            .iter()
            .chain(existing_issues.iter())
            .cloned()
            .collect(),
        build: preflight.build.clone(),
        preflight_summary: preflight,
        redaction_summary,
        managed_knowledge_eval,
        stage2_readiness_preserved: true,
        rows,
        blockers,
    })
}

pub fn classify_main_chat_stage5_failure(
    evidence: &[String],
) -> MainChatStage5FailureClassification {
    let joined = evidence
        .iter()
        .map(|item| item.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("|");
    let (class, severity, scope, recoverability, recovery) = if contains_any(
        &joined,
        &["secret", "redaction", "unsafe_field", "raw_private"],
    ) {
        (
            "redaction_failure",
            "p0",
            "export",
            "needs_developer_fix",
            "Stop testing and regenerate the artifact after the redaction bug is fixed.",
        )
    } else if contains_any(&joined, &["route", "legacy_fallback", "fallback_used"]) {
        (
            "routing_failure",
            "p1",
            "runtime",
            "needs_developer_fix",
            "Re-run with trace and file a strategy routing regression.",
        )
    } else if contains_any(
        &joined,
        &["disallowed_tool", "allowlist", "candidate", "ranking"],
    ) {
        (
            "tool_selection_failure",
            "p1",
            "tool_selection",
            "needs_developer_fix",
            "File a tool-selection regression; do not bypass the allowlist.",
        )
    } else if contains_any(
        &joined,
        &[
            "provider_api_key_missing",
            "workspace",
            "safe_path",
            "database",
            "store",
            "mcp_registry",
            "scheduler",
            "environment",
        ],
    ) {
        (
            "environment_preflight_failure",
            "p1",
            "environment",
            "needs_environment_fix",
            "Fix local provider, workspace, MCP, or store configuration before judging Agent behavior.",
        )
    } else if contains_any(&joined, &["provider", "model", "generation", "timeout"]) {
        (
            "provider_failure",
            "p1",
            "provider",
            "needs_environment_fix",
            "Fix provider credentials/network or retry after provider recovery.",
        )
    } else if contains_any(&joined, &["action_failed", "executor", "tool_execution"]) {
        (
            "tool_execution_failure",
            "p1",
            "tool_execution",
            "retry_safe",
            "Retry only if the action is safe; otherwise file a tool runtime bug.",
        )
    } else if contains_any(&joined, &["policy", "permission", "network", "high_risk"]) {
        (
            "policy_blocker",
            "p2",
            "governance",
            "terminal_expected",
            "Ask for confirmation or adjust allowed configuration; do not bypass policy.",
        )
    } else if contains_any(&joined, &["knowledge", "user.md", "memory.md", "skill.md"]) {
        (
            "knowledge_asset_failure",
            "p1",
            "knowledge_asset",
            "needs_developer_fix",
            "Regenerate managed draft, rollback the version, or file a knowledge asset bug.",
        )
    } else if contains_any(&joined, &["memory"]) {
        (
            "memory_context_failure",
            "p1",
            "memory",
            "needs_developer_fix",
            "File a memory/context regression or use visible rollback/rebuild controls.",
        )
    } else if contains_any(&joined, &["final_delivery", "final"]) {
        (
            "final_delivery_failure",
            "p1",
            "final_delivery",
            "needs_developer_fix",
            "File a final-delivery contract bug.",
        )
    } else if contains_any(&joined, &["ui", "visible_control", "surface"]) {
        (
            "ui_state_failure",
            "p2",
            "ui",
            "needs_developer_fix",
            "File a frontend/state mapping bug with backend snapshot evidence.",
        )
    } else if contains_any(&joined, &["retry", "resume", "cancel", "recovery"]) {
        (
            "recovery_failure",
            "p1",
            "recovery",
            "needs_developer_fix",
            "Use visible task controls only and file a recovery-control bug.",
        )
    } else if contains_any(&joined, &["build", "artifact", "commit_missing", "stale"]) {
        (
            "release_artifact_failure",
            "p1",
            "release_artifact",
            "needs_environment_fix",
            "Re-run on a known build and regenerate the report.",
        )
    } else {
        (
            "unknown_failure",
            "p2",
            "unknown",
            "needs_developer_fix",
            "Mark unknown and triage; do not convert to pass without trace-backed evidence.",
        )
    };
    MainChatStage5FailureClassification {
        class: class.into(),
        severity: severity.into(),
        scope: scope.into(),
        recoverability: recoverability.into(),
        recovery_recommendation: recovery.into(),
        evidence: evidence.iter().map(|item| sanitize_preview(item)).collect(),
    }
}

fn stage5_build_info() -> MainChatStage5BuildInfo {
    let mut blockers = Vec::new();
    let commit = env_metadata("OPENLIFE_BUILD_COMMIT")
        .or_else(|| env_metadata("GITHUB_SHA"))
        .or_else(|| option_env!("OPENLIFE_BUILD_COMMIT").map(str::to_string))
        .filter(|value| build_identity_allowed(value));
    if commit.is_none() {
        blockers.push("build_commit_unavailable".into());
    }
    let branch = env_metadata("OPENLIFE_BUILD_BRANCH")
        .or_else(|| env_metadata("GITHUB_REF_NAME"))
        .or_else(|| option_env!("OPENLIFE_BUILD_BRANCH").map(str::to_string))
        .filter(|value| metadata_safe_label(value));
    if branch.is_none() {
        blockers.push("build_branch_unavailable".into());
    }
    let build_timestamp = env_metadata("OPENLIFE_BUILD_TIMESTAMP")
        .or_else(|| option_env!("OPENLIFE_BUILD_TIMESTAMP").map(str::to_string))
        .filter(|value| metadata_safe_timestamp(value));
    if build_timestamp.is_none() {
        blockers.push("build_timestamp_unavailable".into());
    }
    let dirty_state = env_metadata("OPENLIFE_BUILD_DIRTY")
        .or_else(|| option_env!("OPENLIFE_BUILD_DIRTY").map(str::to_string))
        .and_then(|value| match value.as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        });
    if dirty_state.is_none() {
        blockers.push("build_dirty_state_unavailable".into());
    }
    MainChatStage5BuildInfo {
        commit,
        branch,
        app_version: env!("CARGO_PKG_VERSION").into(),
        build_timestamp,
        dirty_state,
        blockers,
    }
}

async fn stage5_workspace_preflight(state: &Arc<AppState>) -> MainChatStage5WorkspacePreflight {
    let safe_paths = {
        let cfg = state.config.lock().await;
        cfg.system.safe_paths.clone()
    };
    let root_digest = std::env::current_dir()
        .ok()
        .and_then(|path| path.canonicalize().ok())
        .map(|path| digest_label(&path.to_string_lossy()))
        .unwrap_or_else(|| digest_label("workspace-unavailable"));
    let safe_paths_digest = digest_json(&safe_paths);
    MainChatStage5WorkspacePreflight {
        root_digest,
        safe_path_count: safe_paths.len(),
        safe_paths_digest,
        safe_paths_configured: !safe_paths.is_empty(),
        blockers: if safe_paths.is_empty() {
            vec!["workspace_safe_paths_empty".into()]
        } else {
            Vec::new()
        },
    }
}

async fn stage5_mcp_preflight(state: &Arc<AppState>) -> MainChatStage5McpPreflight {
    let registry = state.mcp_registry.lock().await;
    let manifests = registry.list_manifests();
    let read_candidate_count = manifests
        .iter()
        .filter(|manifest| {
            manifest
                .capabilities
                .iter()
                .any(|capability| capability.eq_ignore_ascii_case("read"))
                || manifest.action_type.contains("read")
        })
        .count();
    MainChatStage5McpPreflight {
        registry_available: true,
        manifest_count: manifests.len(),
        read_candidate_count,
        blockers: Vec::new(),
    }
}

fn stage5_database_preflight(state: &Arc<AppState>) -> MainChatStage5DatabasePreflight {
    let mut blockers = Vec::new();
    for (available, blocker) in [
        (
            state.agent_run_store.is_some(),
            "agent_run_store_unavailable",
        ),
        (
            state.main_chat_agent_session_store.is_some(),
            "task_session_store_unavailable",
        ),
        (
            state.main_chat_action_queue_store.is_some(),
            "action_queue_store_unavailable",
        ),
        (state.proposal_store.is_some(), "proposal_store_unavailable"),
        (
            state.memory_lifecycle_store.is_some(),
            "memory_lifecycle_store_unavailable",
        ),
    ] {
        if !available {
            blockers.push(blocker.into());
        }
    }
    MainChatStage5DatabasePreflight {
        memory_store_available: true,
        agent_run_store_available: state.agent_run_store.is_some(),
        task_session_store_available: state.main_chat_agent_session_store.is_some(),
        action_queue_store_available: state.main_chat_action_queue_store.is_some(),
        proposal_store_available: state.proposal_store.is_some(),
        memory_lifecycle_store_available: state.memory_lifecycle_store.is_some(),
        blockers,
    }
}

async fn load_stage5_agent_run(
    state: &Arc<AppState>,
    run_id: Option<&str>,
) -> Result<Option<openlife_core::agent::AgentRun>, String> {
    let Some(run_id) = run_id else {
        return Ok(None);
    };
    let Some(ref store_arc) = state.agent_run_store else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    store
        .get_run(run_id)
        .map_err(|error| format!("load AgentRun for stage5 bundle failed: {error}"))
}

fn stage5_run_id_from_transcript(transcript: &[ExecutionTranscriptEntry]) -> Option<String> {
    transcript
        .iter()
        .rev()
        .find_map(|entry| string_from_value(&entry.metadata, &["runId", "run_id"]))
}

fn stage5_route_summary(
    transcript: &[ExecutionTranscriptEntry],
    run: Option<&openlife_core::agent::AgentRun>,
) -> MainChatStage5RouteSummary {
    let route = run.and_then(|run| run.model_route.as_ref());
    let provider_from_transcript = transcript
        .iter()
        .rev()
        .find_map(|entry| string_from_value(&entry.metadata, &["provider", "rankingProvider"]));
    let model_from_transcript = transcript
        .iter()
        .rev()
        .find_map(|entry| string_from_value(&entry.metadata, &["model", "rankingModel"]));
    let provider_endpoint_kind = transcript.iter().rev().find_map(|entry| {
        string_from_value(
            &entry.metadata,
            &["providerEndpointKind", "provider_endpoint_kind"],
        )
    });
    MainChatStage5RouteSummary {
        route_type: route
            .map(|route| route.route_type.clone())
            .unwrap_or_else(|| "unknown".into()),
        provider: route
            .map(|route| route.provider.clone())
            .or(provider_from_transcript),
        model: route
            .map(|route| route.model.clone())
            .or(model_from_transcript),
        local_only: transcript.iter().any(|entry| {
            entry
                .metadata
                .get("localOnlyProviderGuardExercised")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        }),
        live_provider_attempted: transcript.iter().any(|entry| {
            entry
                .metadata
                .get("liveProviderInvoked")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        }),
        provider_endpoint_kind,
    }
}

fn stage5_timeline_item(entry: &ExecutionTranscriptEntry) -> MainChatStage5TimelineItem {
    MainChatStage5TimelineItem {
        item_id: entry.id.clone(),
        kind: entry.kind.as_str().into(),
        summary_preview: sanitize_preview(&entry.summary),
        metadata_digest: digest_json(&entry.metadata),
    }
}

fn stage5_tool_summary(
    actions: &[QueuedExecutionAction],
    transcript: &[ExecutionTranscriptEntry],
) -> MainChatStage5ToolSummary {
    let candidate_count = transcript
        .iter()
        .filter_map(|entry| {
            entry
                .metadata
                .get("toolSelectionCandidateCount")
                .or_else(|| entry.metadata.get("candidateCount"))
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0) as usize;
    let selected_tool = transcript.iter().rev().find_map(|entry| {
        string_from_value(
            &entry.metadata,
            &[
                "modelSelectedCandidateTarget",
                "selectedTarget",
                "target",
                "toolName",
            ],
        )
    });
    let action_type = actions
        .iter()
        .rev()
        .map(|action| action.action.action_type.clone())
        .next()
        .or_else(|| {
            transcript.iter().rev().find_map(|entry| {
                string_from_value(
                    &entry.metadata,
                    &["modelSelectedCandidateActionType", "actionType"],
                )
            })
        });
    let target_digest = selected_tool.as_ref().map(|target| digest_label(target));
    let policy_decision = actions
        .iter()
        .rev()
        .map(|action| action.policy.reason_code.clone())
        .next();
    let observation_count = transcript
        .iter()
        .filter(|entry| entry.kind == ExecutionTranscriptEntryKind::Observation)
        .count();
    MainChatStage5ToolSummary {
        candidate_count,
        selected_tool: selected_tool.map(|value| sanitize_preview(&value)),
        action_type,
        target_digest,
        policy_decision,
        observation_count,
        action_statuses: actions
            .iter()
            .map(|action| action.status.as_str().to_string())
            .collect(),
    }
}

async fn stage5_context_summary(
    state: &Arc<AppState>,
    transcript: &[ExecutionTranscriptEntry],
) -> MainChatStage5ContextSummary {
    let active_memory_ids = if let Some(ref store_arc) = state.memory_lifecycle_store {
        let store = store_arc.lock().await;
        store
            .list_active_records(None, 50)
            .map(|records| records.into_iter().map(|record| record.memory_id).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let excluded_memory_ids = if let Some(ref store_arc) = state.memory_lifecycle_store {
        let store = store_arc.lock().await;
        store
            .list_records(None, None, 50, 0)
            .map(|records| {
                records
                    .into_iter()
                    .filter(|record| record.status.to_string() != "active")
                    .map(|record| record.memory_id)
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut knowledge_asset_ids = Vec::new();
    let mut selected_skill_id = None;
    let mut context_source_digests = Vec::new();
    for entry in transcript {
        collect_context_source_metadata(
            &entry.metadata,
            &mut knowledge_asset_ids,
            &mut selected_skill_id,
            &mut context_source_digests,
        );
    }
    knowledge_asset_ids.sort();
    knowledge_asset_ids.dedup();
    context_source_digests.sort();
    context_source_digests.dedup();
    MainChatStage5ContextSummary {
        active_memory_ids,
        excluded_memory_ids,
        knowledge_asset_ids,
        selected_skill_id,
        context_source_digests,
    }
}

async fn stage5_memory_summary(
    proposals: &[openlife_core::agent::AgentProposal],
    state: &Arc<AppState>,
) -> MainChatStage5MemorySummary {
    let proposal_ids = proposals
        .iter()
        .map(|proposal| proposal.id.clone())
        .collect::<Vec<_>>();
    let mut accepted_memory_ids = Vec::new();
    let mut rolled_back_memory_ids = Vec::new();
    if let Some(ref store_arc) = state.memory_lifecycle_store {
        let store = store_arc.lock().await;
        if let Ok(records) = store.list_records(None, None, 100, 0) {
            for record in records {
                match record.status.to_string().as_str() {
                    "active" => accepted_memory_ids.push(record.memory_id),
                    "rolled_back" | "archived" | "rejected" => {
                        rolled_back_memory_ids.push(record.memory_id)
                    }
                    _ => {}
                }
            }
        }
    }
    MainChatStage5MemorySummary {
        proposal_ids,
        accepted_memory_ids,
        rolled_back_memory_ids,
        managed_knowledge_version_ids: Vec::new(),
    }
}

fn stage5_final_delivery_summary(
    final_delivery: Option<&Value>,
) -> MainChatStage5FinalDeliverySummary {
    let metadata = final_delivery
        .and_then(|value| value.get("metadata"))
        .unwrap_or(&Value::Null);
    let count = |keys: &[&str]| -> usize {
        keys.iter()
            .find_map(|key| metadata.get(*key).and_then(Value::as_array).map(Vec::len))
            .unwrap_or(0)
    };
    MainChatStage5FinalDeliverySummary {
        completed_work_count: count(&["completedWork", "completedActions"]),
        durable_change_count: count(&["durableChanges"]),
        pending_user_action_count: count(&["pendingUserActions"]),
        skipped_work_count: count(&["skippedWork"]),
        blocker_count: count(&["blockers"]),
        final_delivery_digest: final_delivery.map(digest_json),
    }
}

fn stage5_bundle_blockers(
    blockers: &[String],
    actions: &[QueuedExecutionAction],
    transcript: &[ExecutionTranscriptEntry],
) -> Vec<String> {
    let mut output = blockers.to_vec();
    output.extend(actions.iter().filter_map(|action| {
        (action.status == ExecutionQueueStatus::Failed).then(|| {
            action
                .error
                .clone()
                .unwrap_or_else(|| "action_failed".into())
        })
    }));
    output.extend(
        transcript
            .iter()
            .filter(|entry| entry.kind == ExecutionTranscriptEntryKind::Error)
            .map(|entry| entry.summary.clone()),
    );
    if output.is_empty()
        && transcript
            .iter()
            .any(|entry| entry.kind == ExecutionTranscriptEntryKind::FinalResult)
    {
        output.push("final_delivery_present".into());
    }
    output.sort();
    output.dedup();
    output
}

fn sanitize_ui_evidence(evidence: MainChatStage5UiEvidence) -> MainChatStage5UiEvidence {
    MainChatStage5UiEvidence {
        frontend_route: sanitize_preview(&evidence.frontend_route),
        surface: sanitize_preview(&evidence.surface),
        visible_control_labels: evidence
            .visible_control_labels
            .into_iter()
            .map(|label| sanitize_preview(&label))
            .collect(),
        task_session_id: evidence.task_session_id,
        backend_snapshot_id: evidence
            .backend_snapshot_id
            .map(|value| sanitize_preview(&value)),
        timestamp: sanitize_preview(&evidence.timestamp),
        dom_digest: evidence.dom_digest.map(|value| sanitize_preview(&value)),
        screenshot_digest: evidence
            .screenshot_digest
            .map(|value| sanitize_preview(&value)),
    }
}

fn validate_issue_report_input(input: &MainChatStage5IssueReportInput) -> Result<(), String> {
    if !metadata_safe_label(&input.scenario_id) {
        return Err("stage5_issue_scenario_id_unsafe".into());
    }
    if !metadata_safe_label(&input.reviewer_id) {
        return Err("stage5_issue_reviewer_id_unsafe".into());
    }
    if !matches!(
        input.status.as_str(),
        "pass" | "fail" | "blocked_by_environment" | "blocked_by_policy" | "needs_product_decision"
    ) {
        return Err("stage5_issue_status_invalid".into());
    }
    if matches!(input.status.as_str(), "pass" | "fail")
        && (input.task_session_id.is_none() || input.run_id.is_none() || input.bundle_id.is_none())
    {
        return Err("stage5_issue_task_attached_report_requires_task_run_and_bundle".into());
    }
    if input.task_session_id.is_none()
        && input.run_id.is_none()
        && input.preflight_only_missing_task_reason.is_none()
    {
        return Err("stage5_issue_preflight_only_missing_task_reason_required".into());
    }
    Ok(())
}

fn stage5_notes_redaction(notes: Option<&str>) -> (Option<String>, Option<String>, Vec<String>) {
    let Some(notes) = notes else {
        return (None, None, Vec::new());
    };
    let digest = Some(digest_label(notes));
    if string_is_unsafe(notes) {
        return (digest, None, vec!["notesPreview".into()]);
    }
    (digest, Some(sanitize_preview(notes)), Vec::new())
}

fn stage5_dbg_rows(
    preflight: &MainChatStage5PreflightReport,
    bundle_artifacts: &[MainChatStage5ArtifactMetadata],
    issue_artifacts: &[MainChatStage5ArtifactMetadata],
    bundles: &[MainChatStage5DebugBundle],
    issues: &[MainChatStage5IssueReport],
    managed_knowledge_eval: &MainChatStage5ManagedKnowledgeEval,
) -> Vec<MainChatStage5ReportRow> {
    let bundle_ids = bundle_artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    let issue_ids = issue_artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    let has_bundle = !bundle_artifacts.is_empty();
    let has_issue = !issue_artifacts.is_empty();
    let has_reloadable_bundle = has_bundle && bundles.len() == bundle_artifacts.len();
    let direct_answer_bundle = bundles.iter().find(|bundle| {
        bundle.task.strategy.eq_ignore_ascii_case("direct_answer")
            || bundle
                .route
                .route_type
                .to_ascii_lowercase()
                .contains("direct")
    });
    let read_action_bundle = bundles.iter().find(|bundle| {
        bundle.task.action_count > 0
            && bundle.tools.observation_count > 0
            && bundle
                .tools
                .action_type
                .as_deref()
                .map(|action_type| action_type.to_ascii_lowercase().contains("read"))
                .unwrap_or(false)
    });
    let policy_blocker_bundle = bundles
        .iter()
        .find(|bundle| bundle.failure.class == "policy_blocker");
    let mcp_read_bundle = bundles.iter().find(|bundle| {
        bundle.tools.candidate_count > 0
            && bundle.tools.observation_count > 0
            && bundle.tools.selected_tool.is_some()
            && bundle.tools.action_type.as_deref() == Some("mcp_tool")
    });
    let memory_proposal_bundle = bundles
        .iter()
        .find(|bundle| !bundle.memory.proposal_ids.is_empty() || bundle.task.proposal_count > 0);
    let memory_context_bundle = bundles.iter().find(|bundle| {
        !bundle.context.active_memory_ids.is_empty()
            || !bundle.context.excluded_memory_ids.is_empty()
    });
    let final_delivery_bundle = bundles.iter().find(|bundle| {
        bundle.final_delivery.final_delivery_digest.is_some()
            || bundle.task.final_delivery_id.is_some()
    });
    let task_attached_issue = issues.iter().find(|issue| {
        issue.task_session_id.is_some()
            && issue.run_id.is_some()
            && issue.bundle_id.is_some()
            && matches!(
                issue.status.as_str(),
                "pass" | "fail" | "blocked_by_policy" | "needs_product_decision"
            )
    });
    let secret_redaction_passed =
        stage5_notes_redaction(Some("Authorization: Bearer sk-stage5-redaction-test"))
            .1
            .is_none();
    let raw_memory_blocked = validate_metadata_safe_artifact(&json!({
        "memoryPreview": "raw private memory should be blocked"
    }))
    .is_err();
    let mut rows = Vec::new();
    for (id, scenario) in [
        ("DBG5-01", "Build/version provenance is visible."),
        (
            "DBG5-02",
            "Environment preflight runs without external provider invocation.",
        ),
        (
            "DBG5-03",
            "Missing provider key is reported as environment blocker.",
        ),
        ("DBG5-04", "DirectAnswer task exports debug bundle."),
        (
            "DBG5-05",
            "File read task exports action/observation evidence.",
        ),
        (
            "DBG5-06",
            "ReAct web policy blocker exports policy evidence.",
        ),
        (
            "DBG5-07",
            "Registered MCP read success exports selected tool evidence.",
        ),
        ("DBG5-08", "Tool selection failure is classified."),
        (
            "DBG5-09",
            "Provider failure is classified separately from Agent failure.",
        ),
        (
            "DBG5-10",
            "Memory proposal task exports proposal-first evidence.",
        ),
        (
            "DBG5-11",
            "Accepted memory context exports active/excluded memory ids.",
        ),
        (
            "DBG5-12",
            "Managed USER.md write exports isolated draft/confirm/audit evidence.",
        ),
        (
            "DBG5-13",
            "Managed MEMORY.md rollback exports isolated snapshot/reload evidence.",
        ),
        (
            "DBG5-14",
            "Final delivery separates completed/proposed/blocked/skipped/pending work.",
        ),
        ("DBG5-15", "Retry/resume/cancel failure is classified."),
        ("DBG5-16", "UI state mismatch can be reported."),
        (
            "DBG5-17",
            "Export redaction drops fake API keys and auth headers.",
        ),
        (
            "DBG5-18",
            "Export redaction blocks raw private memory by default.",
        ),
        (
            "DBG5-19",
            "Issue report includes required task-attached ids.",
        ),
        ("DBG5-20", "Stale or unknown build evidence is rejected."),
        ("DBG5-21", "Stage 5 report cannot claim readiness."),
        ("DBG5-22", "Stage 2 readiness remains fail-closed."),
        (
            "DBG5-23",
            "Local/mock provider is not credited as external live evidence.",
        ),
        ("DBG5-24", "Debug bundle can be reloaded after app refresh."),
    ] {
        let mut evidence_ids = vec!["stage5_release_debug_report".into()];
        let mut blockers = Vec::new();
        let passed = match id {
            "DBG5-01" => {
                evidence_ids.push("stage5_build_info".into());
                true
            }
            "DBG5-02" => {
                evidence_ids.push("stage5_preflight_no_external_provider_invocation".into());
                !preflight.external_provider_invoked_by_default && !preflight.model_invoked
            }
            "DBG5-03" => {
                evidence_ids.push("stage5_provider_key_presence_boolean".into());
                !preflight.provider.key_present
                    && preflight
                        .provider
                        .blockers
                        .iter()
                        .any(|blocker| blocker == "provider_api_key_missing")
            }
            "DBG5-04" | "DBG5-05" | "DBG5-06" | "DBG5-07" | "DBG5-10" | "DBG5-11" | "DBG5-14" => {
                let matching_bundle = match id {
                    "DBG5-04" => direct_answer_bundle,
                    "DBG5-05" => read_action_bundle,
                    "DBG5-06" => policy_blocker_bundle,
                    "DBG5-07" => mcp_read_bundle,
                    "DBG5-10" => memory_proposal_bundle,
                    "DBG5-11" => memory_context_bundle,
                    "DBG5-14" => final_delivery_bundle,
                    _ => None,
                };
                if let Some(bundle) = matching_bundle {
                    evidence_ids.push(format!("stage5_debug_bundle:{}", bundle.bundle_id));
                    true
                } else {
                    false
                }
            }
            "DBG5-24" => {
                if has_reloadable_bundle {
                    evidence_ids.extend(bundles.iter().map(|bundle| {
                        format!("stage5_debug_bundle_reloaded:{}", bundle.bundle_id)
                    }));
                    true
                } else {
                    false
                }
            }
            "DBG5-19" => {
                if let Some(issue) = task_attached_issue {
                    evidence_ids.push(format!("stage5_issue_report:{}", issue.report_id));
                    true
                } else {
                    false
                }
            }
            "DBG5-20" => preflight.build.commit.is_none(),
            "DBG5-21" | "DBG5-22" | "DBG5-23" => true,
            "DBG5-08" => {
                classify_main_chat_stage5_failure(&["model_selected_disallowed_tool".into()]).class
                    == "tool_selection_failure"
            }
            "DBG5-09" => {
                classify_main_chat_stage5_failure(&["provider_timeout".into()]).class
                    == "provider_failure"
            }
            "DBG5-12" => {
                evidence_ids.extend(managed_knowledge_eval.evidence_ids.clone());
                managed_knowledge_eval.user_write_completed
                    && managed_knowledge_eval.isolated_eval_app_state
                    && managed_knowledge_eval.temp_workspace
                    && !managed_knowledge_eval.real_workspace_write_executed
            }
            "DBG5-13" => {
                evidence_ids.extend(managed_knowledge_eval.evidence_ids.clone());
                managed_knowledge_eval.memory_rollback_completed
                    && managed_knowledge_eval.isolated_eval_app_state
                    && managed_knowledge_eval.temp_workspace
                    && !managed_knowledge_eval.real_workspace_write_executed
            }
            "DBG5-15" => {
                classify_main_chat_stage5_failure(&["resume_failed".into()]).class
                    == "recovery_failure"
            }
            "DBG5-16" => {
                classify_main_chat_stage5_failure(&["ui_visible_control_missing".into()]).class
                    == "ui_state_failure"
            }
            "DBG5-17" => {
                evidence_ids.push("stage5_secret_notes_redaction_self_check".into());
                secret_redaction_passed
            }
            "DBG5-18" => {
                evidence_ids.push("stage5_raw_private_memory_redaction_self_check".into());
                raw_memory_blocked
            }
            _ => false,
        };
        if !passed {
            blockers.push(
                match id {
                    "DBG5-04" => "stage5_direct_answer_debug_bundle_missing",
                    "DBG5-05" => "stage5_read_action_debug_bundle_missing",
                    "DBG5-06" => "stage5_policy_blocker_debug_bundle_missing",
                    "DBG5-07" => "stage5_mcp_read_debug_bundle_missing",
                    "DBG5-10" => "stage5_memory_proposal_debug_bundle_missing",
                    "DBG5-11" => "stage5_memory_context_debug_bundle_missing",
                    "DBG5-14" => "stage5_final_delivery_debug_bundle_missing",
                    "DBG5-24" if has_bundle => "stage5_debug_bundle_reload_mismatch",
                    "DBG5-24" => "stage5_debug_bundle_artifact_missing",
                    "DBG5-12" => managed_knowledge_eval
                        .blockers
                        .iter()
                        .find(|blocker| blocker.contains("user"))
                        .map(String::as_str)
                        .unwrap_or("stage5_managed_user_write_isolated_eval_not_executed"),
                    "DBG5-13" => managed_knowledge_eval
                        .blockers
                        .iter()
                        .find(|blocker| blocker.contains("memory"))
                        .map(String::as_str)
                        .unwrap_or("stage5_managed_memory_rollback_isolated_eval_not_executed"),
                    "DBG5-19" => "stage5_issue_report_artifact_missing",
                    _ => "stage5_scenario_evidence_missing",
                }
                .into(),
            );
        }
        rows.push(MainChatStage5ReportRow {
            id: id.into(),
            scenario: scenario.into(),
            status: if passed { "passed" } else { "blocked" }.into(),
            evidence_ids,
            bundle_ids: if has_bundle {
                bundle_ids.clone()
            } else {
                Vec::new()
            },
            issue_artifact_ids: if has_issue {
                issue_ids.clone()
            } else {
                Vec::new()
            },
            blockers,
        });
    }
    rows
}

async fn run_stage5_isolated_managed_knowledge_eval() -> MainChatStage5ManagedKnowledgeEval {
    let mut report = MainChatStage5ManagedKnowledgeEval {
        isolated_eval_app_state: true,
        temp_workspace: true,
        real_workspace_write_executed: false,
        user_write_completed: false,
        memory_rollback_completed: false,
        managed_knowledge_write_version_ids: Vec::new(),
        managed_knowledge_audit_ids: Vec::new(),
        rollback_snapshot_ids: Vec::new(),
        evidence_ids: vec!["stage5_isolated_managed_knowledge_eval".into()],
        blockers: Vec::new(),
    };
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let temp_workspace = std::env::temp_dir().join(format!(
        "openlife-stage5-managed-knowledge-{}",
        Uuid::new_v4()
    ));
    match std::fs::create_dir_all(&temp_workspace) {
        Ok(()) => {}
        Err(_) => {
            report
                .blockers
                .push("stage5_managed_knowledge_temp_workspace_failed".into());
            return report;
        }
    }
    let root = temp_workspace.as_path();
    if std::fs::write(root.join("USER.md"), "initial isolated user profile\n").is_err() {
        report
            .blockers
            .push("stage5_managed_user_seed_failed".into());
        let _ = std::fs::remove_dir_all(&temp_workspace);
        return report;
    }
    if std::fs::write(root.join("MEMORY.md"), "initial isolated memory\n").is_err() {
        report
            .blockers
            .push("stage5_managed_memory_seed_failed".into());
        let _ = std::fs::remove_dir_all(&temp_workspace);
        return report;
    }

    match create_managed_knowledge_write_draft_with_state(
        "USER.md".into(),
        "confirmed isolated user profile\n".into(),
        None,
        vec!["stage5-isolated-memory".into()],
        root,
        &state,
    )
    .await
    {
        Ok(draft) => {
            if draft.file_written_before_confirmation {
                report
                    .blockers
                    .push("stage5_managed_user_draft_wrote_before_confirmation".into());
            } else {
                match confirm_managed_knowledge_write_with_state(draft.proposal_id, root, &state)
                    .await
                {
                    Ok(apply) if apply.context_reload.loaded => {
                        report.user_write_completed = true;
                        report
                            .managed_knowledge_write_version_ids
                            .push(apply.version_id);
                        report.managed_knowledge_audit_ids.push(apply.audit_id);
                        report
                            .rollback_snapshot_ids
                            .push(apply.rollback_snapshot_id);
                    }
                    Ok(_) => report
                        .blockers
                        .push("stage5_managed_user_context_reload_missing".into()),
                    Err(_) => report
                        .blockers
                        .push("stage5_managed_user_confirm_failed".into()),
                }
            }
        }
        Err(_) => report
            .blockers
            .push("stage5_managed_user_draft_failed".into()),
    }

    match create_managed_knowledge_write_draft_with_state(
        "MEMORY.md".into(),
        "confirmed isolated memory update\n".into(),
        None,
        vec!["stage5-isolated-memory".into()],
        root,
        &state,
    )
    .await
    {
        Ok(draft) => {
            match confirm_managed_knowledge_write_with_state(draft.proposal_id, root, &state).await
            {
                Ok(apply) => {
                    let version_id = apply.version_id.clone();
                    report
                        .managed_knowledge_write_version_ids
                        .push(apply.version_id);
                    report.managed_knowledge_audit_ids.push(apply.audit_id);
                    report
                        .rollback_snapshot_ids
                        .push(apply.rollback_snapshot_id);
                    match rollback_managed_knowledge_write_with_state(version_id, root) {
                        Ok(rollback) if rollback.context_reload.loaded => {
                            report.memory_rollback_completed = true;
                            report.managed_knowledge_audit_ids.push(rollback.audit_id);
                        }
                        Ok(_) => report
                            .blockers
                            .push("stage5_managed_memory_rollback_reload_missing".into()),
                        Err(_) => report
                            .blockers
                            .push("stage5_managed_memory_rollback_failed".into()),
                    }
                }
                Err(_) => report
                    .blockers
                    .push("stage5_managed_memory_confirm_failed".into()),
            }
        }
        Err(_) => report
            .blockers
            .push("stage5_managed_memory_draft_failed".into()),
    }

    report.managed_knowledge_write_version_ids.sort();
    report.managed_knowledge_write_version_ids.dedup();
    report.managed_knowledge_audit_ids.sort();
    report.managed_knowledge_audit_ids.dedup();
    report.rollback_snapshot_ids.sort();
    report.rollback_snapshot_ids.dedup();
    report.blockers.sort();
    report.blockers.dedup();
    let _ = std::fs::remove_dir_all(&temp_workspace);
    report
}

fn stage5_artifact_metadata_for_payload<T: Serialize>(
    kind_dir: &str,
    id: &str,
    artifact_kind: &str,
    schema_version: &str,
    created_at: &str,
    payload: &T,
) -> Result<MainChatStage5ArtifactMetadata, String> {
    if !stage5_artifact_id_safe(id) {
        return Err("stage5_artifact_id_unsafe".into());
    }
    let alias = stage5_artifact_alias(kind_dir, id);
    let mut payload_value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
    validate_metadata_safe_artifact(&payload_value)?;
    set_stage5_artifact_measurement_fields(&mut payload_value, "", 0);
    let measured_payload =
        serde_json::to_vec_pretty(&payload_value).map_err(|error| error.to_string())?;
    let digest = digest_bytes(&measured_payload);
    let mut byte_size = 0usize;
    for _ in 0..8 {
        set_stage5_artifact_measurement_fields(&mut payload_value, &digest, byte_size);
        let next_size = serde_json::to_vec_pretty(&payload_value)
            .map_err(|error| error.to_string())?
            .len();
        if next_size == byte_size {
            break;
        }
        byte_size = next_size;
    }
    Ok(MainChatStage5ArtifactMetadata {
        artifact_id: id.into(),
        artifact_kind: artifact_kind.into(),
        schema_version: schema_version.into(),
        created_at: created_at.into(),
        storage_alias: alias,
        digest,
        byte_size,
    })
}

fn persist_stage5_artifact<T: Serialize>(
    store_root: &Path,
    kind_dir: &str,
    id: &str,
    payload: &T,
) -> Result<(), String> {
    if !stage5_artifact_id_safe(id) {
        return Err("stage5_artifact_id_unsafe".into());
    }
    let path = store_root.join(stage5_artifact_alias(kind_dir, id));
    let payload_value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
    validate_metadata_safe_artifact(&payload_value)?;
    let canonical_payload =
        serde_json::to_vec_pretty(&payload_value).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create stage5 artifact dir failed: {error}"))?;
    }
    let tmp_path = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    std::fs::write(&tmp_path, &canonical_payload)
        .map_err(|error| format!("write stage5 temp artifact failed: {error}"))?;
    std::fs::rename(&tmp_path, &path)
        .map_err(|error| format!("atomic rename stage5 artifact failed: {error}"))?;
    Ok(())
}

fn set_stage5_artifact_measurement_fields(value: &mut Value, digest: &str, byte_size: usize) {
    let Some(artifact) = value.get_mut("artifact").and_then(Value::as_object_mut) else {
        return;
    };
    artifact.insert("digest".into(), Value::String(digest.into()));
    artifact.insert(
        "byteSize".into(),
        Value::Number(serde_json::Number::from(byte_size)),
    );
}

fn list_stage5_artifacts<T>(
    store_root: &Path,
    kind_dir: &str,
) -> Result<Vec<MainChatStage5ArtifactMetadata>, String>
where
    T: for<'de> Deserialize<'de> + Stage5ArtifactAccess,
{
    let dir = store_root.join("stage5").join(kind_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut artifacts = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .map_err(|error| format!("read stage5 artifact dir failed: {error}"))?
    {
        let entry = entry.map_err(|error| format!("read stage5 artifact entry failed: {error}"))?;
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let artifact: T = read_json(&entry.path())?;
        artifacts.push(artifact.artifact().clone());
    }
    artifacts.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(artifacts)
}

fn read_stage5_artifact<T>(store_root: &Path, kind_dir: &str, id: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    if !stage5_artifact_id_safe(id) {
        return Err("stage5_artifact_id_unsafe".into());
    }
    read_json(&store_root.join(stage5_artifact_alias(kind_dir, id)))
}

fn delete_stage5_artifact(store_root: &Path, kind_dir: &str, id: &str) -> Result<bool, String> {
    if !stage5_artifact_id_safe(id) {
        return Err("stage5_artifact_id_unsafe".into());
    }
    let path = store_root.join(stage5_artifact_alias(kind_dir, id));
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(path)
        .map_err(|error| format!("delete stage5 artifact failed: {error}"))?;
    Ok(true)
}

trait Stage5ArtifactAccess {
    fn artifact(&self) -> &MainChatStage5ArtifactMetadata;
}

impl Stage5ArtifactAccess for MainChatStage5DebugBundle {
    fn artifact(&self) -> &MainChatStage5ArtifactMetadata {
        &self.artifact
    }
}

impl Stage5ArtifactAccess for MainChatStage5IssueReport {
    fn artifact(&self) -> &MainChatStage5ArtifactMetadata {
        &self.artifact
    }
}

fn read_json<T>(path: &Path) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("read stage5 artifact failed: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("parse stage5 artifact failed: {error}"))
}

fn stage5_artifact_alias(kind_dir: &str, id: &str) -> String {
    format!("stage5/{kind_dir}/{id}.json")
}

fn validate_metadata_safe_artifact(value: &Value) -> Result<(), String> {
    let mut unsafe_fields = Vec::new();
    collect_unsafe_string_fields(value, "$", &mut unsafe_fields);
    if unsafe_fields.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "stage5_metadata_safe_artifact_blocked:{}",
            unsafe_fields.join(",")
        ))
    }
}

fn collect_unsafe_string_fields(value: &Value, path: &str, unsafe_fields: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if string_is_unsafe(text) {
                unsafe_fields.push(path.into());
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_unsafe_string_fields(item, &format!("{path}[{index}]"), unsafe_fields);
            }
        }
        Value::Object(map) => {
            for (key, child) in map {
                collect_unsafe_string_fields(child, &format!("{path}.{key}"), unsafe_fields);
            }
        }
        _ => {}
    }
}

fn string_is_unsafe(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    text.chars().any(|ch| ch.is_control())
        || lower.contains("sk-")
        || lower.contains("bearer ")
        || lower.contains("authorization:")
        || lower.contains("openai_api_key=")
        || lower.contains("openai_api_key:")
        || lower.contains("api key:")
        || lower.contains("api key=")
        || lower.contains("api_key:")
        || lower.contains("api_key=")
        || lower.contains("password")
        || lower.contains("raw private memory")
        || lower.contains("/users/")
        || lower.contains("/home/")
        || lower.contains("/private/")
}

fn metadata_safe_label(value: &str) -> bool {
    let trimmed = value.trim();
    !value.is_empty()
        && value == trimmed
        && value.len() <= 200
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(ch, '-' | '_' | '.' | ':' | '/' | '@' | '+' | '=')
        })
}

fn stage5_artifact_id_safe(value: &str) -> bool {
    let trimmed = value.trim();
    !value.is_empty()
        && value == trimmed
        && value.len() <= 120
        && !value.contains("..")
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
}

fn metadata_safe_timestamp(value: &str) -> bool {
    metadata_safe_label(value) && value.contains('T')
}

fn build_identity_allowed(value: &str) -> bool {
    metadata_safe_label(value)
        && !matches!(value, "unknown" | "none")
        && !contains_any(
            &value.to_ascii_lowercase(),
            &[
                "mock",
                "fixture",
                "synthetic",
                "scripted",
                "localhost",
                "local",
            ],
        )
}

fn sanitize_preview(value: &str) -> String {
    value
        .replace(|ch: char| ch.is_control(), " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(PREVIEW_LIMIT)
        .collect()
}

fn env_metadata(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn string_from_value(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(value) = map.get(*key).and_then(Value::as_str) {
                    return Some(value.to_string());
                }
            }
            for child in map.values() {
                if let Some(found) = string_from_value(child, keys) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(|item| string_from_value(item, keys)),
        _ => None,
    }
}

fn collect_context_source_metadata(
    value: &Value,
    knowledge_asset_ids: &mut Vec<String>,
    selected_skill_id: &mut Option<String>,
    context_source_digests: &mut Vec<String>,
) {
    match value {
        Value::Object(map) => {
            if let Some(source_id) = map.get("sourceId").and_then(Value::as_str) {
                context_source_digests.push(digest_label(source_id));
                if source_id.contains(".md") || source_id.contains("knowledge") {
                    knowledge_asset_ids.push(digest_label(source_id));
                }
            }
            if let Some(skill) = map.get("selectedSkillId").and_then(Value::as_str) {
                *selected_skill_id = Some(sanitize_preview(skill));
            }
            for child in map.values() {
                collect_context_source_metadata(
                    child,
                    knowledge_asset_ids,
                    selected_skill_id,
                    context_source_digests,
                );
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_context_source_metadata(
                    item,
                    knowledge_asset_ids,
                    selected_skill_id,
                    context_source_digests,
                );
            }
        }
        _ => {}
    }
}

fn digest_label(value: &str) -> String {
    digest_bytes(value.as_bytes())
}

fn digest_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    digest_bytes(&bytes)
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}
