use futures::StreamExt;
use openlife_core::agent::ContextAssembler;
use openlife_core::agent::ReasoningTrace;
use openlife_core::builder::{BuilderSession, BuilderSessionStore};
use openlife_core::config::AppConfig;
use openlife_core::feedback::FeedbackStore;
use openlife_core::layer_router::{Layer, LayerRouter};
use openlife_core::life_model::{LifeModel, LifeModelManager};
use openlife_core::llm::ChatMessage;
use openlife_core::mcp::McpRegistry;
use openlife_core::mcp_audit::McpAuditStore;
use openlife_core::memory::{MemorySearchHit, MemoryStore};
use openlife_core::memory_cache::{HotMemoryCache, SharedHotCache};
use openlife_core::privacy::PrivacyEngine;
use openlife_core::router::{IntentRouter, RouterStatus};
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::vectors::{embed_text_with_config, MemoryChunk, VectorInsertItem, VectorStore};
use openlife_core::versioning::VersionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

pub mod a2a_server;
pub mod a2a_sidecar;
pub mod commands;
pub mod storage;

use commands::a2a::{
    a2a_bridge_local, a2a_discover_agent, a2a_handle_task, a2a_local_agent_card,
    a2a_restart_sidecar, a2a_send_task, a2a_stop_sidecar,
};
use commands::agent::{
    delete_agent_run, get_agent_run, list_agent_runs, list_agent_runs_for_session,
    replay_agent_action, restore_agent_run,
};
use commands::builder::{
    builder_apply_signals, builder_create_proposals, builder_delete_session,
    builder_get_pending_signals, builder_list_unfinished, builder_start, builder_step,
    get_model_4d_completion, goal_capability_gap_analysis, goal_capability_gap_report,
    identity_goal_alignment_check, identity_goal_alignment_report,
};
use commands::calibration::{
    apply_calibration, calibration_create_proposals, generate_calibration_report,
    generate_micro_evolution_changes, mark_calibration_shown, run_micro_evolution,
    should_show_calibration,
};
use commands::chat::{
    create_chat_session, delete_chat_session, get_chat_history, list_chat_sessions,
    rename_chat_session, save_chat_message,
};
use commands::diagnostics::{
    check_ollama_status, get_router_status, get_scheduler_config, get_system_diagnostics,
    set_scheduler_config,
};
use commands::execution::{
    check_tool_permission, disable_plugin, enable_plugin, get_skill_run_status,
    grant_tool_permission, list_plugins, list_skills, list_tool_permissions, reload_plugins,
    revoke_tool_permission, run_skill,
};
use commands::feedback::{
    apply_feedback_evolution, generate_evolution_report, get_feedback_summary, log_analytics_event,
    save_feedback,
};
// Hermes module removed: replaced by AgentRuntime
use commands::life_model::{get_life_model, save_life_model};
use commands::mcp::{
    clear_mcp_audit_logs, list_mcp_audit_logs, list_mcp_servers, list_mcp_templates,
    list_mcp_tools, recommend_mcp_manifests, register_mcp_server, unregister_mcp_server,
};
use commands::memory::{
    archive_low_access_memories, count_memory_chunks, get_hot_cache, get_memory_tier_stats,
    index_memory_chunk, list_archived_chunks, rebuild_memory_index, restore_archived_chunks,
    run_memory_tier_maintenance, search_memory,
};
use commands::metrics::{get_rollout_errors, get_rollout_metrics, get_rollout_summary};
use commands::proposal::{
    accept_proposal, batch_accept_low_risk_proposals, edit_proposal, get_pending_proposals,
    list_proposals, postpone_proposal, reject_proposal,
};
use commands::router::get_model_router_status;
use commands::settings::{
    cleanup_mcp_audit_logs, export_all_data, export_mcp_audit_logs, get_config,
    get_last_model_error, get_privacy_policy, has_completed_onboarding, import_all_data,
    mark_onboarding_completed, rotate_mcp_audit_key, save_config, set_privacy_policy, test_api_key,
    test_llm_connection,
};
use commands::state::{
    add_daily_goal, delete_daily_goal, get_daily_goals, get_state_alerts, get_state_history,
    record_state, toggle_daily_goal, update_daily_goal,
};
use commands::version::{create_snapshot, diff_snapshots, list_snapshots, restore_snapshot};
use storage::{
    app_data_dir, load_mcp_audit_keyring_from_path, load_privacy_policy_from_path,
    mcp_audit_keyring_path, privacy_policy_path,
};

#[derive(Clone, serde::Serialize)]
pub struct ToolCallResult {
    pub name: String,
    pub arguments: serde_json::Value,
    pub sanitized_arguments: Option<serde_json::Value>,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub permission_level: String,
    pub status: String,
    pub requires_confirmation: bool,
    pub pii_found: bool,
    pub privacy_warnings: Vec<String>,
    pub action_id: Option<String>,
    pub run_id: Option<String>,
    pub permission_decision: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SendMessageResult {
    pub reply: String,
    pub reasoning_trace: openlife_core::agent::ReasoningTrace,
    pub tool_calls: Vec<ToolCallResult>,
    pub run_id: Option<String>,
}

#[derive(serde::Serialize)]
pub struct BuilderCompletion {
    pub identity: f32,
    pub goals: f32,
    pub capabilities: f32,
    pub state: f32,
    pub overall: f32,
    pub lowest_dimension: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SystemDiagnostics {
    pub router: RouterStatus,
    pub mcp_server_count: usize,
    pub mcp_tool_count: usize,
    pub mcp_recent_audit_count: usize,
    pub mcp_recent_pii_count: usize,
    pub memory_chunk_count: usize,
    pub vector_corrupt_embedding_count: usize,
    pub unfinished_builder_sessions: usize,
    pub pending_builder_review_sessions: usize,
    pub ollama_online: bool,
    pub local_model: String,
    pub resolved_local_model: Option<String>,
    pub prefer_local_model: bool,
    pub cloud_api_configured: bool,
    pub cloud_provider: String,
    pub cloud_api_validated: bool,
    pub cloud_api_last_error: Option<String>,
    pub chat_ready: bool,
    pub readiness_issues: Vec<String>,
    pub data_dir: String,
    pub active_data_dir: String,
    pub legacy_data_dir: Option<String>,
    pub database_status: String,
    pub startup_warnings: Vec<String>,
    pub snapshot_count: usize,
    pub life_model_ready: bool,
    pub app_version: String,
    pub model_empty: bool,
    pub chat_session_count: usize,
    pub onboarding_completed: bool,
    pub beta_ready: bool,
    pub beta_readiness_issues: Vec<String>,
    pub builder_completion: BuilderCompletion,
    pub agent_run_count: usize,
    pub agent_run_store_status: String,
    pub pending_proposal_count: usize,
    pub high_risk_pending_proposal_count: usize,
    pub proposal_store_status: String,
}

fn recovery_db_path(file_name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir()
        .join("openlife-recovery")
        .join(std::process::id().to_string());
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "failed to create OpenLife recovery database directory {}: {}",
            dir.display(),
            e
        );
    }
    dir.join(file_name)
}

fn init_memory_store(
    db_path: &std::path::Path,
    startup_warnings: &mut Vec<String>,
) -> Result<MemoryStore, String> {
    match MemoryStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("memory.db");
            startup_warnings.push(format!(
                "memory.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match MemoryStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.push(format!(
                        "临时 memory.db 初始化也失败，已降级为内存数据库；本次会话聊天记录不会持久化：{}",
                        fallback_err
                    ));
                    MemoryStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 memory store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_feedback_store(
    db_path: &std::path::Path,
    startup_warnings: &mut Vec<String>,
) -> Result<FeedbackStore, String> {
    match FeedbackStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("feedback.db");
            startup_warnings.push(format!(
                "feedback.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match FeedbackStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.push(format!(
                        "临时 feedback.db 初始化也失败，已降级为内存数据库；本次会话反馈不会持久化：{}",
                        fallback_err
                    ));
                    FeedbackStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 feedback store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_vector_store(
    db_path: &std::path::Path,
    startup_warnings: &mut Vec<String>,
) -> Result<VectorStore, String> {
    match VectorStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("vectors.db");
            startup_warnings.push(format!(
                "vectors.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match VectorStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.push(format!(
                        "临时 vectors.db 初始化也失败，已降级为内存数据库；本次会话向量记忆不会持久化：{}",
                        fallback_err
                    ));
                    VectorStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 vector store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_agent_run_store(
    db_path: &std::path::Path,
    startup_warnings: &mut Vec<String>,
) -> Result<openlife_core::agent::AgentRunStore, String> {
    match openlife_core::agent::AgentRunStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("agent_runs.db");
            startup_warnings.push(format!(
                "agent_runs.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match openlife_core::agent::AgentRunStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.push(format!(
                        "临时 agent_runs.db 初始化也失败，已降级为内存数据库；本次会话 AgentRun 记录不会持久化：{}",
                        fallback_err
                    ));
                    openlife_core::agent::AgentRunStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 agent run store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_proposal_store(
    db_path: &std::path::Path,
    startup_warnings: &mut Vec<String>,
) -> Result<openlife_core::agent::ProposalStore, String> {
    match openlife_core::agent::ProposalStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("proposals.db");
            startup_warnings.push(format!(
                "proposals.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match openlife_core::agent::ProposalStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.push(format!(
                        "临时 proposals.db 初始化也失败，已降级为内存数据库；本次会话 Proposal 记录不会持久化：{}",
                        fallback_err
                    ));
                    openlife_core::agent::ProposalStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 proposal store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

/// Cached provider health data to avoid probing on every Settings open.
#[derive(Clone)]
pub struct ProviderHealthCache {
    pub providers: Vec<crate::commands::router::ProviderStatus>,
    pub checked_at: String,
}

impl ProviderHealthCache {
    pub fn is_fresh(&self) -> bool {
        if let Ok(checked) = chrono::DateTime::parse_from_rfc3339(&self.checked_at) {
            let elapsed =
                chrono::Utc::now().signed_duration_since(checked.with_timezone(&chrono::Utc));
            elapsed.num_seconds() < 30 // Cache for 30 seconds
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub life_model_manager: Arc<Mutex<LifeModelManager>>,
    pub memory_store: Arc<Mutex<MemoryStore>>,
    pub mcp_registry: Arc<Mutex<McpRegistry>>,
    pub intent_router: Arc<Mutex<IntentRouter>>,
    pub layer_router: Arc<Mutex<LayerRouter>>,
    pub scheduler: Arc<Mutex<InferenceScheduler>>,
    pub privacy_engine: Arc<Mutex<PrivacyEngine>>,
    pub version_manager: Arc<Mutex<VersionManager>>,
    pub feedback_store: Arc<Mutex<FeedbackStore>>,
    pub vector_store: Arc<Mutex<VectorStore>>,
    pub builder_sessions: Arc<Mutex<HashMap<String, BuilderSession>>>,
    pub builder_session_store: Arc<Mutex<BuilderSessionStore>>,
    pub a2a_sidecar: Arc<Mutex<a2a_sidecar::A2ASidecar>>,
    pub last_snapshot_date: Arc<Mutex<Option<String>>>,
    pub mcp_audit_store: Arc<Mutex<McpAuditStore>>,
    pub agent_run_store: Option<Arc<Mutex<openlife_core::agent::AgentRunStore>>>,
    pub proposal_store: Option<Arc<Mutex<openlife_core::agent::ProposalStore>>>,
    pub patch_store: Option<Arc<Mutex<openlife_core::life_model::patch_store::PatchStore>>>,
    pub rollout_metrics_store: Option<Arc<Mutex<openlife_core::agent::RolloutMetricsStore>>>,
    pub tool_permission_store: Arc<Mutex<openlife_core::tool_permissions::ToolPermissionStore>>,
    pub skill_registry: Arc<Mutex<openlife_core::skills::SkillRegistry>>,
    pub plugin_registry: Arc<Mutex<openlife_core::plugins::PluginRegistry>>,
    pub hot_cache: SharedHotCache,
    pub proposal_engine: Arc<tokio::sync::Mutex<openlife_core::agent::ProposalEngine>>,
    pub startup_warnings: Vec<String>,
    pub provider_health_cache: Arc<tokio::sync::Mutex<Option<ProviderHealthCache>>>,
}

impl AppState {
    /// 按固定顺序获取 MCP 相关锁，避免死锁
    /// 顺序：mcp_registry → mcp_audit_store
    pub async fn get_mcp_state(
        &self,
    ) -> (
        tokio::sync::MutexGuard<'_, McpRegistry>,
        tokio::sync::MutexGuard<'_, McpAuditStore>,
    ) {
        let reg = self.mcp_registry.lock().await;
        let audit = self.mcp_audit_store.lock().await;
        (reg, audit)
    }
}

async fn generate_and_persist_chat_proposals(
    state: &Arc<AppState>,
    agent_run: &openlife_core::agent::AgentRun,
    reply: &str,
    life_model: &LifeModel,
) {
    let Some(ref proposal_store_arc) = state.proposal_store else {
        return;
    };

    let proposals = {
        let engine = state.proposal_engine.lock().await;
        match engine.generate_from_run(agent_run, reply, life_model) {
            Ok(proposals) => proposals,
            Err(e) => {
                eprintln!("[ChatProposal] Proposal generation failed: {}", e);
                return;
            }
        }
    };

    if proposals.is_empty() {
        return;
    }

    let mut created_proposal_ids = Vec::new();
    {
        let store = proposal_store_arc.lock().await;
        for proposal in proposals {
            let proposal_id = proposal.id.clone();
            if let Err(e) = store.create_proposal(&proposal) {
                eprintln!("[ChatProposal] Failed to save proposal: {}", e);
            } else {
                created_proposal_ids.push(proposal_id);
            }
        }
    }

    if created_proposal_ids.is_empty() {
        return;
    }

    if let Some(ref run_store_arc) = state.agent_run_store {
        let run_store = run_store_arc.lock().await;
        for proposal_id in created_proposal_ids {
            if let Err(e) = run_store.add_generated_proposal(&agent_run.id, &proposal_id) {
                eprintln!("[AgentRun] 关联 Chat Proposal 失败: {}", e);
            }
        }
    }
}

pub(crate) async fn persist_life_model(
    state: &Arc<AppState>,
    mut life_model: LifeModel,
    create_daily_snapshot: bool,
) -> Result<LifeModel, String> {
    let previous_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().ok()
    };
    openlife_core::versioning::prepare_model_for_save(previous_model.as_ref(), &mut life_model);
    {
        let manager = state.life_model_manager.lock().await;
        manager.save(&life_model).map_err(|e| e.to_string())?;
    }
    if create_daily_snapshot {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let should_snapshot = {
            let vm = state.version_manager.lock().await;
            !vm.has_snapshot_tag_on_date("auto:daily-save", &today)
                .map_err(|e| e.to_string())?
        };
        if should_snapshot {
            let vm = state.version_manager.lock().await;
            vm.snapshot(&life_model, "auto:daily-save", "当日首次保存自动快照")
                .map_err(|e| e.to_string())?;
            let mut last_snapshot_date = state.last_snapshot_date.lock().await;
            *last_snapshot_date = Some(today);
        }
    }
    Ok(life_model)
}
fn try_extract_json(text: &str) -> Option<&str> {
    // 1. Try code block with json label
    if let Some(start) = text.find("```json") {
        let rest = &text[start + 7..];
        if let Some(end) = rest.find("```") {
            let candidate = rest[..end].trim();
            if is_valid_agent_json(candidate) {
                return Some(candidate);
            }
        }
    }
    // 2. Try generic code block
    if let Some(start) = text.find("```") {
        let rest = &text[start + 3..];
        if let Some(end) = rest.find("```") {
            let inner = rest[..end].trim();
            if (inner.starts_with('{') || inner.starts_with('[')) && is_valid_agent_json(inner) {
                return Some(inner);
            }
        }
    }
    // 3. Try bare JSON - must be at start or after whitespace/newline
    let trimmed = text.trim_start();
    if let Some(start) = trimmed.find('{') {
        // Ensure the '{' is at the very beginning or preceded by whitespace
        let prefix = &trimmed[..start];
        if prefix.trim().is_empty() {
            let mut depth = 0usize;
            let mut in_string = false;
            let mut escape = false;
            let bytes = trimmed.as_bytes();
            for (idx, &b) in bytes.iter().enumerate().skip(start) {
                if in_string {
                    if escape {
                        escape = false;
                        continue;
                    }
                    if b == b'\\' {
                        escape = true;
                        continue;
                    }
                    if b == b'"' {
                        in_string = false;
                    }
                    continue;
                }
                if b == b'"' {
                    in_string = true;
                    continue;
                }
                if b == b'{' {
                    depth += 1;
                } else if b == b'}' {
                    if depth == 0 {
                        continue;
                    }
                    depth -= 1;
                    if depth == 0 {
                        let candidate = &trimmed[start..=idx];
                        if is_valid_agent_json(candidate) {
                            return Some(candidate);
                        }
                        return None;
                    }
                }
            }
        }
    }
    None
}

/// Validate that extracted JSON is a valid agent action envelope.
/// Must contain either "tool_calls" (array) or "final" (string) at top level.
fn is_valid_agent_json(text: &str) -> bool {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(obj) = value.as_object() {
            return obj.contains_key("tool_calls") || obj.contains_key("final");
        }
    }
    false
}

fn try_prepare_tool_calls(
    reply: &str,
    registry: &McpRegistry,
    audit: &McpAuditStore,
    permission_store: &openlife_core::tool_permissions::ToolPermissionStore,
    privacy_engine: &PrivacyEngine,
    step_index: u32,
    run_id: Option<String>,
) -> Option<(
    Vec<ToolCallResult>,
    Vec<openlife_core::agent::AgentAction>,
    Vec<openlife_core::agent::AgentObservation>,
)> {
    let json_str = try_extract_json(reply)?;
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let calls = v.get("tool_calls")?.as_array()?;
    if calls.is_empty() {
        return None;
    }

    let executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let ctx = openlife_core::agent::ActionExecutionContext {
        registry,
        permission_store,
        audit_store: audit,
        privacy_engine,
        safe_paths: &[],
        life_model: None,
        memory_store: None,
    };

    let mut results = Vec::new();
    let mut actions = Vec::new();
    let mut observations = Vec::new();

    for (idx, call) in calls.iter().enumerate() {
        let name = call.get("name")?.as_str()?;
        let args = call
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let request = openlife_core::agent::AgentActionRequest {
            action_type: "mcp_tool".to_string(),
            target: name.to_string(),
            input: serde_json::json!({ "arguments": args }),
            source_run_id: None,
            step_index: step_index + idx as u32,
        };

        match executor.execute(request, &ctx) {
            Ok(result) => {
                let tool_result =
                    action_execution_result_to_tool_call(&result, name, &args, run_id.clone());
                results.push(tool_result);
                actions.push(result.action);
                observations.push(result.observation);
            }
            Err(e) => {
                // Build a failed result for executor errors
                let now = chrono::Utc::now();
                let action_id = format!(
                    "action-{}-{}",
                    step_index + idx as u32,
                    now.timestamp_nanos_opt().unwrap_or_default()
                );
                let action = openlife_core::agent::AgentAction {
                    id: action_id.clone(),
                    action_type: "mcp_tool".into(),
                    target: Some(name.to_string()),
                    input: serde_json::json!({ "arguments": args }),
                    output: None,
                    status: "failed".into(),
                    permission_decision: None,
                    tool_scope: None,
                    started_at: Some(now),
                    finished_at: Some(now),
                    error: Some(e.to_string()),
                    timestamp: now,
                };
                let observation = openlife_core::agent::AgentObservation {
                    id: format!(
                        "observation-{}-{}",
                        step_index + idx as u32,
                        now.timestamp_nanos_opt().unwrap_or_default()
                    ),
                    action_id: Some(action_id),
                    content: e.to_string(),
                    source: "builtin".to_string(),
                    structured_result: Some(
                        serde_json::json!({ "success": false, "error": e.to_string() }),
                    ),
                    timestamp: now,
                };
                results.push(ToolCallResult {
                    name: name.to_string(),
                    arguments: args.clone(),
                    sanitized_arguments: Some(args),
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                    permission_level: "high".into(),
                    status: "error".into(),
                    requires_confirmation: false,
                    pii_found: false,
                    privacy_warnings: vec![],
                    action_id: Some(action.id.clone()),
                    run_id: run_id.clone(),
                    permission_decision: None,
                });
                actions.push(action);
                observations.push(observation);
            }
        }
    }

    if results.is_empty() {
        None
    } else {
        Some((results, actions, observations))
    }
}

fn action_execution_result_to_tool_call(
    result: &openlife_core::agent::ActionExecutionResult,
    name: &str,
    args: &serde_json::Value,
    run_id: Option<String>,
) -> ToolCallResult {
    let sanitized = args.clone(); // Simplified; in practice use privacy engine
    ToolCallResult {
        name: name.to_string(),
        arguments: args.clone(),
        sanitized_arguments: Some(sanitized),
        success: result.status == openlife_core::agent::ActionExecutionStatus::Succeeded,
        output: result
            .action
            .output
            .as_ref()
            .and_then(|o| o.get("text").and_then(|t| t.as_str()).map(String::from)),
        error: result.action.error.clone(),
        permission_level: result
            .action
            .tool_scope
            .as_ref()
            .map(|s| s.risk_level.clone())
            .unwrap_or_else(|| "medium".into()),
        status: match result.status {
            openlife_core::agent::ActionExecutionStatus::Succeeded => "success",
            openlife_core::agent::ActionExecutionStatus::Failed => "error",
            openlife_core::agent::ActionExecutionStatus::Blocked => "error",
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation => "pending",
        }
        .into(),
        requires_confirmation: result.status
            == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
        pii_found: false, // Simplified
        privacy_warnings: vec![],
        action_id: Some(result.action.id.clone()),
        run_id,
        permission_decision: result.action.permission_decision.clone(),
    }
}
fn try_auto_checkin_daily_goals(content: &str, life_model: &mut LifeModel) -> Option<String> {
    let lower = content.to_lowercase();
    let triggers = [
        "我今天完成了",
        "我完成了",
        "我已经完成了",
        "刚刚完成了",
        "我搞定了",
        "我做完了",
        "已经打卡了",
        "今天搞定了",
    ];
    let triggered = triggers.iter().any(|t| lower.contains(t));
    if !triggered {
        return None;
    }
    let mut checked = Vec::new();
    for goal in &mut life_model.goals.daily {
        if goal.done {
            continue;
        }
        if lower.contains(&goal.name.to_lowercase()) {
            goal.done = true;
            checked.push(goal.name.clone());
        }
    }
    if !checked.is_empty() {
        Some(format!("已自动打卡今日目标：{}", checked.join("、")))
    } else {
        None
    }
}

async fn persist_chat_message_if_needed(
    session_id: &str,
    msg: &ChatMessage,
    state: &State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    let store = state.memory_store.lock().await;
    let should_skip = store
        .load_recent_messages(session_id, 1)
        .map_err(|e| e.to_string())?
        .last()
        .map(|last| last.role == msg.role && last.content == msg.content)
        .unwrap_or(false);
    if should_skip {
        let _ = store.touch_chat_session(session_id);
        return Ok(false);
    }
    store
        .save_message(session_id, msg)
        .map_err(|e| e.to_string())?;
    let _ = store.touch_chat_session(session_id);
    Ok(true)
}

async fn persist_vector_memory_for_message(
    session_id: &str,
    msg: &ChatMessage,
    state: &State<'_, Arc<AppState>>,
) {
    let content = msg.content.trim();
    if content.is_empty() {
        return;
    }
    let (provider, openai_base, openai_key, embedding_model, embedding_enabled) = {
        let cfg = state.config.lock().await;
        (
            cfg.llm.provider.clone(),
            cfg.llm.openai_base.clone(),
            cfg.llm.openai_key.clone(),
            cfg.llm.embedding_model.clone(),
            cfg.llm.embedding_enabled,
        )
    };
    let embedding = match embed_text_with_config(
        content,
        &provider,
        &openai_base,
        &openai_key,
        &embedding_model,
        embedding_enabled,
    )
    .await
    {
        Ok(embedding) if !embedding.is_empty() => embedding,
        Ok(_) => return,
        Err(e) => {
            eprintln!(
                "[memory] embedding generation failed for {} message in session {}: {} - lib.rs:520",
                msg.role, session_id, e
            );
            return;
        }
    };
    let store = state.vector_store.lock().await;
    let item = VectorInsertItem {
        session_id,
        content,
        embedding: &embedding,
        source: if msg.role == "assistant" {
            "assistant_reply"
        } else {
            "user_message"
        },
    };
    if let Err(e) = store.insert_batch(&[item]) {
        eprintln!(
            "[memory] vector insert failed for {} message in session {}: {} - lib.rs:537",
            msg.role, session_id, e
        );
    }
}

async fn finalize_chat_agent_run(
    session_id: &str,
    assistant_message: &ChatMessage,
    reply: &str,
    reasoning_trace: &mut ReasoningTrace,
    agent_run: &mut openlife_core::agent::AgentRun,
    life_model: &LifeModel,
    state: &State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let inserted = persist_chat_message_if_needed(session_id, assistant_message, state).await?;

    reasoning_trace.generation_result = Some(serde_json::json!({ "text": reply }));
    agent_run.output_preview = Some(preview_text(reply, 200));
    agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    agent_run.finished_at = Some(chrono::Utc::now());
    agent_run.reasoning_trace = Some(reasoning_trace.clone());

    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        match store.get_run(&agent_run.id) {
            Ok(Some(_)) => {
                if let Err(e) = store.update_run(agent_run) {
                    eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                }
            }
            Ok(None) => {
                if let Err(e) = store.create_run(agent_run) {
                    eprintln!("[AgentRun] 保存运行记录失败: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[AgentRun] 查询运行记录失败: {}", e);
                if let Err(e) = store.create_run(agent_run) {
                    eprintln!("[AgentRun] 保存运行记录失败: {}", e);
                }
            }
        }
    }

    if inserted {
        persist_vector_memory_for_message(session_id, assistant_message, state).await;
    }

    generate_and_persist_chat_proposals(state.inner(), agent_run, reply, life_model).await;
    Ok(())
}

/// Shared preprocessing for chat commands:
/// saves user message, loads model/tools/config, applies privacy filter,
/// values filter, and vector memory retrieval.
async fn preprocess_chat_input(
    session_id: &str,
    messages: &[ChatMessage],
    state: &State<'_, Arc<AppState>>,
) -> Result<
    (
        LifeModel,
        String,
        PrivacyEngine,
        HashMap<String, String>,
        Vec<ChatMessage>,
        Option<String>,
        openlife_core::agent::types::ContextSummary,
    ),
    String,
> {
    if let Some(user_msg) = messages.last() {
        if user_msg.role == "user" {
            let inserted = persist_chat_message_if_needed(session_id, user_msg, state).await?;
            if inserted {
                persist_vector_memory_for_message(session_id, user_msg, state).await;
            }
        }
    }

    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };

    // Refresh hot cache if stale
    {
        let mut cache = state.hot_cache.write().await;
        if cache.is_stale(&life_model) {
            cache.refresh(&life_model);
        }
    }

    let tools_prompt = {
        let reg = state.mcp_registry.lock().await;
        reg.tools_prompt()
    };

    let privacy_engine = state.privacy_engine.lock().await.clone();
    let mut desensitized_messages = Vec::new();
    let mut privacy_map = HashMap::new();
    for msg in messages {
        if msg.role == "user" {
            let (masked, map) = privacy_engine.desensitize(&msg.content);
            privacy_map.extend(map);
            let mut final_text = masked;
            let router = state.intent_router.lock().await;
            if router.values_filter(&msg.content) {
                final_text = format!("[该消息涉及你的核心价值观] {}", final_text);
            }
            desensitized_messages.push(ChatMessage {
                role: msg.role.clone(),
                content: final_text,
            });
        } else {
            desensitized_messages.push(msg.clone());
        }
    }

    let (provider, openai_base, openai_key, embedding_model, embedding_enabled) = {
        let cfg = state.config.lock().await;
        (
            cfg.llm.provider.clone(),
            cfg.llm.openai_base.clone(),
            cfg.llm.openai_key.clone(),
            cfg.llm.embedding_model.clone(),
            cfg.llm.embedding_enabled,
        )
    };

    let mut embed_err = None;
    let mut memory_sources: Vec<String> = Vec::new();
    let mut memory_hit_count = 0usize;
    let memory_top_k = {
        let cfg = state.config.lock().await;
        cfg.system.memory_search_top_k
    };
    let memory_context = if let Some(user_msg) = messages.last() {
        if user_msg.role == "user" {
            let text_hits = {
                let store = state.memory_store.lock().await;
                store
                    .search_text_memories(Some(session_id), &user_msg.content, memory_top_k)
                    .unwrap_or_default()
            };

            let vector_hits = match embed_text_with_config(
                &user_msg.content,
                &provider,
                &openai_base,
                &openai_key,
                &embedding_model,
                embedding_enabled,
            )
            .await
            {
                Ok(emb) => {
                    let store = state.vector_store.lock().await;
                    store
                        .search_by_session(session_id, &emb, 3, 1000)
                        .unwrap_or_default()
                }
                Err(e) => {
                    embed_err = Some(format!("向量记忆检索失败，已降级到关键词检索: {}", e));
                    vec![]
                }
            };

            let results = merge_memory_hits(vector_hits, text_hits, 3);
            memory_hit_count = results.len();
            memory_sources = results
                .iter()
                .map(|(chunk, _)| chunk.source.clone())
                .collect();
            if results.is_empty() {
                String::new()
            } else {
                let snippets: Vec<String> = results
                    .iter()
                    .map(|(chunk, score)| {
                        format!(
                            "- [{}] {} (相关度: {:.2})",
                            chunk.source,
                            chunk.content.replace('\n', " "),
                            score
                        )
                    })
                    .collect();
                format!(
                    "\n以下是你过去记忆中的相关内容，请在回应中自然地参考它们：\n{}",
                    snippets.join("\n")
                )
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Prepend hot memory cache as a system message (always injected)
    let hot_context = {
        let cache = state.hot_cache.read().await;
        cache.to_context_string()
    };
    if !hot_context.is_empty() {
        desensitized_messages.insert(
            0,
            ChatMessage {
                role: "system".into(),
                content: hot_context,
            },
        );
    }

    if !memory_context.is_empty() {
        if let Some(last_user) = desensitized_messages.iter_mut().rfind(|m| m.role == "user") {
            last_user.content = format!("{}\n\n{}", last_user.content, memory_context);
        }
    }

    let context_summary = openlife_core::agent::types::ContextSummary {
        life_model_empty: life_model.identity.name.is_empty(),
        included_life_model_sections: vec![
            "identity".to_string(),
            "goals".to_string(),
            "capabilities".to_string(),
            "state".to_string(),
        ],
        memory_hit_count: memory_hit_count as i64,
        memory_sources,
        used_tools_prompt: !tools_prompt.is_empty(),
        redaction_applied: !privacy_map.is_empty(),
        redaction_level: if privacy_map.is_empty() {
            openlife_core::agent::types::RedactionLevel::None
        } else {
            openlife_core::agent::types::RedactionLevel::Light
        },
    };

    Ok((
        life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        context_summary,
    ))
}

/// V2 preprocessing using ContextAssembler.
/// This is functionally equivalent to preprocess_chat_input but uses
/// the modular ContextAssembler trait for better testability and extensibility.
async fn preprocess_chat_input_v2(
    session_id: &str,
    messages: &[ChatMessage],
    state: &State<'_, Arc<AppState>>,
) -> Result<
    (
        LifeModel,
        String,
        PrivacyEngine,
        HashMap<String, String>,
        Vec<ChatMessage>,
        Option<String>,
        openlife_core::agent::types::ContextSummary,
    ),
    String,
> {
    let start = std::time::Instant::now();

    // Step 1: Persist user message (same as v1)
    if let Some(user_msg) = messages.last() {
        if user_msg.role == "user" {
            let inserted = persist_chat_message_if_needed(session_id, user_msg, state).await?;
            if inserted {
                persist_vector_memory_for_message(session_id, user_msg, state).await;
            }
        }
    }

    // Step 2: Load LifeModel
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };

    // Step 3: Refresh hot cache
    {
        let mut cache = state.hot_cache.write().await;
        if cache.is_stale(&life_model) {
            cache.refresh(&life_model);
        }
    }

    // Step 4: Get tools prompt
    let tools_prompt = {
        let reg = state.mcp_registry.lock().await;
        reg.tools_prompt()
    };

    // Step 5: Prefetch memory using MemoryService
    let memory_top_k = {
        let cfg = state.config.lock().await;
        cfg.system.memory_search_top_k
    };
    let (memory_context_opt, memory_hits, memory_retrieval_time_ms) =
        if let Some(user_msg) = messages.last() {
            if user_msg.role == "user" {
                let cfg = state.config.lock().await;
                let embedding_config = openlife_core::agent::EmbeddingConfig {
                    enabled: cfg.llm.embedding_enabled,
                    provider: cfg.llm.provider.clone(),
                    openai_base: cfg.llm.openai_base.clone(),
                    openai_key: cfg.llm.openai_key.clone(),
                    embedding_model: cfg.llm.embedding_model.clone(),
                };
                drop(cfg);

                let service = openlife_core::agent::MemoryService::new();
                let memory_store = state.memory_store.lock().await;
                let vector_store = state.vector_store.lock().await;

                match service
                    .retrieve_context(
                        session_id,
                        &user_msg.content,
                        &memory_store,
                        &vector_store,
                        &embedding_config,
                        memory_top_k,
                    )
                    .await
                {
                    Ok(ctx) => {
                        eprintln!(
                            "[MemoryService] Retrieved {} hits in {}ms (embedding: {})",
                            ctx.hits.len(),
                            ctx.retrieval_time_ms,
                            ctx.used_embedding
                        );
                        (Some(ctx.context), ctx.hits, ctx.retrieval_time_ms)
                    }
                    Err(e) => {
                        eprintln!("[MemoryService] Failed: {}, falling back to no context", e);
                        (None, vec![], 0)
                    }
                }
            } else {
                (None, vec![], 0)
            }
        } else {
            (None, vec![], 0)
        };

    // Step 6: Build privacy engine
    let privacy_engine = state.privacy_engine.lock().await.clone();

    // Step 7: Assemble using ContextAssembler
    let input = openlife_core::agent::AssembleInput {
        session_id: session_id.to_string(),
        messages: messages.to_vec(),
        life_model,
        tools_prompt: tools_prompt.clone(),
        privacy_engine: privacy_engine.clone(),
        memory_context: memory_context_opt,
        memory_hits,
        memory_retrieval_time_ms,
    };

    let assembler = openlife_core::agent::CompositeAssembler::new()
        .with(Box::new(openlife_core::agent::LifeModelAssembler))
        .with(Box::new(openlife_core::agent::PrivacyAssembler))
        .with(Box::new(openlife_core::agent::MemoryAssembler))
        .with(Box::new(openlife_core::agent::ToolsAssembler));

    let output = assembler.assemble(&input).map_err(|e| e.to_string())?;

    // Step 8: Apply hot cache (same as v1)
    let mut desensitized_messages = output.desensitized_messages;
    let hot_context = {
        let cache = state.hot_cache.read().await;
        cache.to_context_string()
    };
    if !hot_context.is_empty() {
        desensitized_messages.insert(
            0,
            ChatMessage {
                role: "system".into(),
                content: hot_context,
            },
        );
    }

    // Step 9: Apply memory context to last user message (same as v1)
    if !output.memory_context.is_empty() {
        if let Some(last_user) = desensitized_messages.iter_mut().rfind(|m| m.role == "user") {
            last_user.content = format!("{}\n\n{}", last_user.content, output.memory_context);
        }
    }

    // Step 10: Build embed_err if memory retrieval had issues
    let embed_err = None; // Memory retrieval succeeded or wasn't attempted

    // Record rollout metric for context assembler v2
    if let Some(ref store_arc) = state.rollout_metrics_store {
        let elapsed_ms = start.elapsed().as_millis() as i64;
        let metric = openlife_core::agent::RolloutMetric {
            id: None,
            experiment: "context_assembler".into(),
            version: "v2".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms: elapsed_ms,
            success: true,
            error: None,
            metadata: Some(format!("memory_hits:{}", input.memory_hits.len())),
        };
        let store = store_arc.lock().await;
        let _ = store.record_metric(&metric);
    }

    Ok((
        output.life_model,
        output.tools_prompt,
        privacy_engine,
        output.privacy_map,
        desensitized_messages,
        embed_err,
        output.context_summary,
    ))
}

#[allow(dead_code)]
fn build_reasoning_trace_prompt(trace: &ReasoningTrace) -> String {
    let mut prompt = String::new();
    if let Some(ref m) = trace.meaning_result {
        if let Some(text) = m.get("text").and_then(|t| t.as_str()) {
            prompt.push_str(&format!("【意义层约束】{}\n", text));
        }
    }
    if let Some(ref s) = trace.strategy_result {
        if let Some(text) = s.get("text").and_then(|t| t.as_str()) {
            prompt.push_str(&format!("【策略层约束】{}\n", text));
        }
        if let Some(tools) = s.get("suggested_tools").and_then(|v| v.as_array()) {
            let tools_text = tools
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join("、");
            if !tools_text.is_empty() {
                prompt.push_str(&format!("【建议工具】{}\n", tools_text));
            }
        }
    }
    if let Some(ref safety) = trace.safety_check_result {
        if let Some(warnings) = safety.get("warnings").and_then(|v| v.as_array()) {
            let text = warnings
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join("；");
            if !text.is_empty() {
                prompt.push_str(&format!("【安全检查提醒】{}\n", text));
            }
        }
    }
    prompt
}
async fn capture_conversation_signals(
    session_id: &str,
    user_text: &str,
    life_model: &LifeModel,
    state: &Arc<AppState>,
) {
    let normalized = user_text.trim().to_lowercase();
    if normalized.is_empty() {
        return;
    }

    let positive = [
        "想继续",
        "想做",
        "准备",
        "投入",
        "推进",
        "喜欢",
        "热爱",
        "重要",
        "值得",
        "继续",
    ];
    let negative = [
        "不想",
        "放弃",
        "没意义",
        "拖延",
        "讨厌",
        "厌倦",
        "卡住",
        "做不到",
    ];
    let has_positive = positive.iter().any(|kw| normalized.contains(kw));
    let has_negative = negative.iter().any(|kw| normalized.contains(kw));
    let base_delta = if has_negative {
        -0.02
    } else if has_positive {
        0.02
    } else {
        0.01
    };
    let confidence = if has_positive || has_negative {
        0.68
    } else {
        0.45
    };

    let store = state.feedback_store.lock().await;

    for value in &life_model.identity.values {
        if normalized.contains(&value.name.to_lowercase()) {
            if let Err(e) = store.log_event(
                &format!("value_focus:{}", value.name),
                Some(session_id),
                Some("chat_match"),
            ) {
                eprintln!("[Memory] 记录价值观焦点事件失败: {}", e);
            }
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "identity.values",
                &value.name,
                base_delta,
                confidence,
                "用户在对话中主动提及或强化了该价值观",
            ) {
                eprintln!("[Memory] 保存价值观推断失败: {}", e);
            }
        }
    }

    for goal in life_model
        .goals
        .short_term
        .iter()
        .chain(life_model.goals.medium_term.iter())
        .chain(life_model.goals.long_term.iter())
        .chain(life_model.goals.life_goals.iter())
    {
        if normalized.contains(&goal.name.to_lowercase()) {
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "goals",
                &goal.name,
                base_delta,
                (confidence - 0.05).max(0.2),
                "用户在对话中直接提到该目标，表明关注度发生变化",
            ) {
                eprintln!("[Memory] 保存目标推断失败: {}", e);
            }
        }
    }

    for skill in &life_model.capabilities.skills {
        if normalized.contains(&skill.name.to_lowercase()) {
            let skill_delta = if normalized.contains("学习")
                || normalized.contains("练习")
                || normalized.contains("提升")
            {
                0.02
            } else {
                base_delta
            };
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "capabilities.skills",
                &skill.name,
                skill_delta,
                0.55,
                "用户在对话中主动提及技能投入或受阻情况",
            ) {
                eprintln!("[Memory] 保存技能推断失败: {}", e);
            }
        }
    }
}
pub(crate) fn merge_memory_hits(
    vector_hits: Vec<(MemoryChunk, f32)>,
    text_hits: Vec<MemorySearchHit>,
    top_k: usize,
) -> Vec<(MemoryChunk, f32)> {
    let mut merged: HashMap<(String, String), (MemoryChunk, f32)> = HashMap::new();

    for (chunk, score) in vector_hits {
        let key = (chunk.session_id.clone(), chunk.content.clone());
        merged
            .entry(key)
            .and_modify(|(_, existing_score)| *existing_score = existing_score.max(score))
            .or_insert((chunk, score));
    }

    for hit in text_hits {
        let key = (hit.chunk.session_id.clone(), hit.chunk.content.clone());
        merged
            .entry(key)
            .and_modify(|(_, existing_score)| {
                *existing_score = existing_score.max(hit.relevance_score)
            })
            .or_insert((hit.chunk, hit.relevance_score));
    }

    let mut results: Vec<_> = merged.into_values().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    results
}
#[tauri::command]
async fn send_message(
    session_id: String,
    messages: Vec<ChatMessage>,
    state: State<'_, Arc<AppState>>,
) -> Result<SendMessageResult, String> {
    let user_msg = messages.last().cloned();
    let intent = if let Some(ref m) = user_msg {
        if m.role == "user" {
            let router = state.intent_router.lock().await;
            Some(router.classify(&m.content))
        } else {
            None
        }
    } else {
        None
    };

    let layer = if let Some(ref i) = intent {
        let lr = state.layer_router.lock().await;
        lr.resolve(i, &user_msg.as_ref().unwrap().content)
    } else {
        Layer::L2
    };

    // Layer 1: direct reflex response
    if layer == Layer::L1 {
        if let Some(ref i) = intent {
            if let Some(reply) = i.direct_response() {
                // Persist user message
                if let Some(ref user) = user_msg {
                    if user.role == "user" {
                        let inserted =
                            persist_chat_message_if_needed(&session_id, user, &state).await?;
                        if inserted {
                            persist_vector_memory_for_message(&session_id, user, &state).await;
                        }
                    }
                }
                let assistant_msg = ChatMessage {
                    role: "assistant".into(),
                    content: reply.clone(),
                };

                // Create and finalize AgentRun for L1
                let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(
                    &session_id,
                    &user_msg
                        .as_ref()
                        .map(|m| m.content.clone())
                        .unwrap_or_default(),
                );
                let model_route = openlife_core::agent::ModelRouteTrace {
                    provider: "direct".to_string(),
                    model: "L1_reflex".to_string(),
                    route_type: "direct".to_string(),
                    prefer_local: false,
                    local_model: "".to_string(),
                    reason: "layer_1_direct_response".to_string(),
                    privacy_level: openlife_core::agent::types::RedactionLevel::None,
                    latency_ms: None,
                    retry_count: 0,
                    fallback_reason: None,
                    provider_health_is_estimated: Some(false),
                };
                let context_summary = openlife_core::agent::ContextSummary {
                    life_model_empty: false,
                    included_life_model_sections: vec![],
                    memory_hit_count: 0,
                    memory_sources: vec![],
                    used_tools_prompt: false,
                    redaction_applied: false,
                    redaction_level: openlife_core::agent::types::RedactionLevel::None,
                };
                agent_run.complete(&preview_text(&reply, 200), model_route, context_summary);
                let life_model = {
                    let manager = state.life_model_manager.lock().await;
                    manager
                        .load()
                        .map_err(|e| format!("人生模型加载失败: {}", e))?
                };
                let mut reasoning_trace = ReasoningTrace::default();
                finalize_chat_agent_run(
                    &session_id,
                    &assistant_msg,
                    &reply,
                    &mut reasoning_trace,
                    &mut agent_run,
                    &life_model,
                    &state,
                )
                .await?;

                return Ok(SendMessageResult {
                    reply,
                    reasoning_trace,
                    tool_calls: vec![],
                    run_id: Some(agent_run.id.clone()),
                });
            }
        }
    }

    // Gradual rollout: use v2 if experimental flag is enabled
    let use_v2 = {
        let cfg = state.config.lock().await;
        cfg.experimental_context_assembler
    };

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        _context_summary,
    ) = if use_v2 {
        preprocess_chat_input_v2(&session_id, &messages, &state).await?
    } else {
        preprocess_chat_input(&session_id, &messages, &state).await?
    };

    let auto_checkin_msg = if let Some(ref m) = user_msg {
        let msg = try_auto_checkin_daily_goals(&m.content, &mut life_model);
        capture_conversation_signals(&session_id, &m.content, &life_model, state.inner()).await;
        if msg.is_some() {
            let _ = persist_life_model(&state.inner().clone(), life_model.clone(), false).await?;
        }
        msg
    } else {
        None
    };

    // Dual-track: use AgentLoop if feature flag is enabled
    let use_agent_loop = {
        let cfg = state.config.lock().await;
        cfg.use_agent_loop
    };

    if use_agent_loop {
        return send_message_with_agent_loop(
            session_id,
            messages,
            user_msg,
            life_model,
            tools_prompt,
            privacy_engine,
            privacy_map,
            desensitized_messages,
            embed_err,
            auto_checkin_msg,
            layer,
            state,
        )
        .await;
    }

    // Legacy path (preserved completely)
    // Create AgentRun for tracking
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(
        &session_id,
        &user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
    );

    // AgentRuntime: unified execution entry
    let scheduler_clone = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler_clone.clone(), &cfg);
    drop(cfg);

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer,
    };

    let mut messages_with_reasoning = desensitized_messages.clone();
    let mut reasoning_trace = openlife_core::agent::ReasoningTrace::default();

    // Layer 3: use LayeredReasoner for deep reasoning; on failure fallback to L2
    let _actual_layer = if layer == Layer::L3 {
        let runtime_output = agent_runtime
            .execute_task(
                &task,
                &life_model,
                &tools_prompt,
                None,   // memory_context handled in preprocess
                vec![], // memory_hits handled in preprocess
                privacy_engine.clone(),
            )
            .await;

        match runtime_output {
            Ok(output) => {
                if !output
                    .reasoning_trace
                    .output
                    .as_ref()
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
                {
                    // Use reasoning trace output directly if available
                    messages_with_reasoning = output.final_messages;
                } else {
                    // Use system prompt from reasoning
                    messages_with_reasoning = output.final_messages;
                }
                reasoning_trace = output.reasoning_trace;
                agent_run.reasoning_strategy = Some("layered".to_string());
                agent_run.reasoning_trace = Some(reasoning_trace.clone());
                Layer::L3
            }
            Err(e) => {
                eprintln!("[AgentRuntime] Reasoning failed: {}, falling back to L2", e);
                agent_run.reasoning_strategy = Some("direct".to_string());
                let lr = state.layer_router.lock().await;
                lr.fallback(Layer::L3).unwrap_or(Layer::L2)
            }
        }
    } else {
        agent_run.reasoning_strategy = Some("direct".to_string());
        layer
    };

    let first_reply = scheduler_clone
        .generate(
            messages_with_reasoning.clone(),
            &life_model,
            Some(&tools_prompt),
        )
        .await
        .map_err(|e| e.to_string())?;

    let tool_results = {
        let (reg, audit) = state.get_mcp_state().await;
        let permission_store = state.tool_permission_store.lock().await;
        let privacy_engine = state.privacy_engine.lock().await;
        try_prepare_tool_calls(
            &first_reply,
            &reg,
            &audit,
            &permission_store,
            &privacy_engine,
            agent_run.actions.len() as u32,
            Some(agent_run.id.clone()),
        )
    };

    let (reply, tool_calls) = if let Some((results, actions, observations)) = tool_results {
        agent_run.actions.extend(actions);
        agent_run.observations.extend(observations);
        let executed_results: Vec<_> = results
            .iter()
            .filter(|r| !r.requires_confirmation)
            .cloned()
            .collect();
        let pending_count = results.iter().filter(|r| r.requires_confirmation).count();
        let results_text: String = executed_results
            .iter()
            .map(|r| {
                if r.success {
                    format!(
                        "{} 结果: {}",
                        r.name,
                        r.output.as_ref().unwrap_or(&String::new())
                    )
                } else {
                    format!(
                        "{} 错误: {}",
                        r.name,
                        r.error.as_ref().unwrap_or(&String::new())
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut reply = if executed_results.is_empty() {
            if pending_count > 0 {
                "我需要先执行一些高风险或含敏感参数的工具操作，确认后才能继续给你结果。".to_string()
            } else {
                first_reply
            }
        } else {
            let mut follow_up = messages_with_reasoning.clone();
            follow_up.push(ChatMessage {
                role: "assistant".into(),
                content: first_reply,
            });
            follow_up.push(ChatMessage {
                role: "user".into(),
                content: format!(
                    "你刚才请求调用了工具，以下是执行结果，请基于结果直接回答用户的问题：\n{}",
                    results_text
                ),
            });
            let scheduler_clone = state.scheduler.lock().await.clone();
            scheduler_clone
                .generate(follow_up, &life_model, None)
                .await
                .map_err(|e| e.to_string())?
        };
        if pending_count > 0 {
            reply.push_str(&format!(
                "\n\n[系统] 有 {} 个高风险或敏感参数工具调用待确认。请先在下方工具卡片中确认后再执行。",
                pending_count
            ));
        }
        (reply, results)
    } else {
        (first_reply, vec![])
    };

    let mut reply = privacy_engine.reconstruct(&reply, &privacy_map);
    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }
    finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await?;

    Ok(SendMessageResult {
        reply,
        reasoning_trace,
        tool_calls,
        run_id: Some(agent_run.id.clone()),
    })
}

/// AgentLoop-based chat execution (dual-track beta path).
#[allow(clippy::too_many_arguments)]
async fn send_message_with_agent_loop(
    session_id: String,
    _messages: Vec<ChatMessage>,
    user_msg: Option<ChatMessage>,
    life_model: LifeModel,
    tools_prompt: String,
    privacy_engine: PrivacyEngine,
    privacy_map: HashMap<String, String>,
    desensitized_messages: Vec<ChatMessage>,
    embed_err: Option<String>,
    auto_checkin_msg: Option<String>,
    layer: Layer,
    state: State<'_, Arc<AppState>>,
) -> Result<SendMessageResult, String> {
    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let loop_config = openlife_core::agent::AgentLoopConfig {
        max_steps: 5,
        max_tool_calls: 3,
        timeout_seconds: 120,
        allow_writes: true,
        allow_cloud: true,
    };
    let agent_loop = openlife_core::agent::AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler,
        loop_config,
    );

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer,
    };

    let (reg, audit) = state.get_mcp_state().await;
    let permission_store = state.tool_permission_store.lock().await;
    let memory_store = state.memory_store.lock().await;
    let action_ctx = openlife_core::agent::ActionExecutionContext {
        registry: &reg,
        permission_store: &permission_store,
        audit_store: &audit,
        privacy_engine: &privacy_engine,
        safe_paths: &safe_paths,
        life_model: Some(&life_model),
        memory_store: Some(&memory_store),
    };

    let loop_result = agent_loop
        .run(
            &task,
            &life_model,
            &tools_prompt,
            None,
            privacy_engine.clone(),
            &action_ctx,
        )
        .await
        .map_err(|e| format!("AgentLoop execution failed: {}", e))?;

    let mut reply = loop_result.final_response;
    let mut agent_run = loop_result.run;

    // Apply privacy reconstruction
    reply = privacy_engine.reconstruct(&reply, &privacy_map);

    // Apply auto checkin message
    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    let mut reasoning_trace = agent_run.reasoning_trace.clone().unwrap_or_default();
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }

    finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await?;

    // Convert AgentLoop actions to ToolCallResult for frontend compatibility
    let tool_calls: Vec<ToolCallResult> = agent_run
        .actions
        .iter()
        .map(|action| ToolCallResult {
            name: action.target.clone().unwrap_or_default(),
            arguments: action.input.clone(),
            sanitized_arguments: None,
            success: action.status == "succeeded" || action.status == "completed",
            output: action
                .output
                .as_ref()
                .and_then(|o| o.as_str().map(|s| s.to_string())),
            error: action.error.clone(),
            permission_level: action
                .tool_scope
                .as_ref()
                .map(|s| s.risk_level.clone())
                .unwrap_or_else(|| "low".to_string()),
            status: action.status.clone(),
            requires_confirmation: action.permission_decision.as_deref()
                == Some("needs_confirmation"),
            pii_found: false,
            privacy_warnings: Vec::new(),
            action_id: Some(action.id.clone()),
            run_id: Some(agent_run.id.clone()),
            permission_decision: action.permission_decision.clone(),
        })
        .collect();

    Ok(SendMessageResult {
        reply,
        reasoning_trace,
        tool_calls,
        run_id: Some(agent_run.id.clone()),
    })
}

#[derive(serde::Deserialize, Clone, Debug)]
struct StartStreamMessageArgs {
    session_id: String,
    messages: Vec<ChatMessage>,
}

const STREAM_INIT_TIMEOUT_SECS: u64 = 45;
const STREAM_CHUNK_TIMEOUT_SECS: u64 = 90;
const NON_STREAM_FALLBACK_TIMEOUT_SECS: u64 = 120;

fn emit_stream_error(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    run_id: &str,
    error: impl Into<String>,
) {
    let _ = app_handle.emit(
        "stream-message-error",
        serde_json::json!({
            "session_id": session_id,
            "run_id": run_id,
            "error": error.into(),
        }),
    );
}

async fn generate_non_stream_fallback(
    scheduler: &InferenceScheduler,
    messages: Vec<ChatMessage>,
    life_model: &LifeModel,
    tools_prompt: &str,
) -> Result<String, String> {
    timeout(
        Duration::from_secs(NON_STREAM_FALLBACK_TIMEOUT_SECS),
        scheduler.generate(messages, life_model, Some(tools_prompt)),
    )
    .await
    .map_err(|_| {
        format!(
            "非流式重试超时（{} 秒），请检查模型服务或切换后端。",
            NON_STREAM_FALLBACK_TIMEOUT_SECS
        )
    })?
    .map_err(|e| e.to_string())
}

fn preview_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn included_life_model_sections(life_model: &LifeModel) -> Vec<String> {
    if life_model.is_effectively_empty() {
        Vec::new()
    } else {
        vec![
            "identity".to_string(),
            "goals".to_string(),
            "capabilities".to_string(),
            "state".to_string(),
        ]
    }
}

/// Stream-mode AgentLoop execution: runs AgentLoop and emits stream events.
/// This provides consistency when use_agent_loop=true in stream mode.
async fn start_stream_message_with_agent_loop(
    session_id: String,
    messages: Vec<ChatMessage>,
    user_msg: Option<ChatMessage>,
    _layer: Layer,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // AgentRun tracking
    let user_input_text = user_msg
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text);
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&agent_run) {
            eprintln!("[AgentRun] 保存运行记录失败: {}", e);
        }
    }

    // Preprocess
    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        _context_summary,
    ) = match preprocess_chat_input_v2(&session_id, &messages, &state).await {
        Ok(result) => result,
        Err(message) => {
            let error = openlife_core::agent::AgentRunError {
                message: message.clone(),
                phase: "preprocess".to_string(),
                recoverable: true,
            };
            agent_run.fail(error);
            if let Some(ref store_arc) = state.agent_run_store {
                let store = store_arc.lock().await;
                let _ = store.update_run(&agent_run);
            }
            return Err(message);
        }
    };

    let auto_checkin_msg = if let Some(ref m) = user_msg {
        let msg = try_auto_checkin_daily_goals(&m.content, &mut life_model);
        if msg.is_some() {
            let _ = persist_life_model(&state.inner().clone(), life_model.clone(), false).await;
        }
        msg
    } else {
        None
    };

    // Emit stream start
    let _ = app_handle.emit(
        "stream-message-start",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reasoning_trace": ReasoningTrace::default(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );

    // Run AgentLoop
    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let loop_config = openlife_core::agent::AgentLoopConfig {
        max_steps: 5,
        max_tool_calls: 3,
        timeout_seconds: 120,
        allow_writes: true,
        allow_cloud: true,
    };
    let agent_loop = openlife_core::agent::AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler,
        loop_config,
    );

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer: _layer,
    };

    let (reg, audit) = state.get_mcp_state().await;
    let permission_store = state.tool_permission_store.lock().await;
    let memory_store = state.memory_store.lock().await;
    let action_ctx = openlife_core::agent::ActionExecutionContext {
        registry: &reg,
        permission_store: &permission_store,
        audit_store: &audit,
        privacy_engine: &privacy_engine,
        safe_paths: &safe_paths,
        life_model: Some(&life_model),
        memory_store: Some(&memory_store),
    };

    let loop_result = match agent_loop
        .run(
            &task,
            &life_model,
            &tools_prompt,
            None,
            privacy_engine.clone(),
            &action_ctx,
        )
        .await
    {
        Ok(result) => result,
        Err(e) => {
            let error_msg = format!("AgentLoop execution failed: {}", e);
            let _ = app_handle.emit(
                "stream-message-error",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": agent_run.id,
                    "error": error_msg.clone(),
                }),
            );
            let error = openlife_core::agent::AgentRunError {
                message: error_msg.clone(),
                phase: "agent_loop".to_string(),
                recoverable: false,
            };
            agent_run.fail(error);
            if let Some(ref store_arc) = state.agent_run_store {
                let store = store_arc.lock().await;
                let _ = store.update_run(&agent_run);
            }
            return Err(error_msg);
        }
    };

    let mut reply = loop_result.final_response;
    let mut agent_run = loop_result.run;

    // Apply privacy reconstruction
    reply = privacy_engine.reconstruct(&reply, &privacy_map);

    // Apply auto checkin message
    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    // Emit the result as a single chunk (simulating streaming)
    let _ = app_handle.emit(
        "stream-message-chunk",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "chunk": reply.clone(),
        }),
    );

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    let mut reasoning_trace = agent_run.reasoning_trace.clone().unwrap_or_default();
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }

    // Save assistant message
    let _ = persist_chat_message_if_needed(&session_id, &assistant_message, &state).await;
    let _ = persist_vector_memory_for_message(&session_id, &assistant_message, &state).await;

    // Finalize AgentRun
    let result = finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await;

    match result {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream-message-done",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": agent_run.id,
                    "reply": reply,
                    "reasoning_trace": reasoning_trace,
                    "tool_calls": Vec::<ToolCallResult>::new(),
                }),
            );
            Ok(())
        }
        Err(e) => {
            let _ = app_handle.emit(
                "stream-message-error",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": agent_run.id,
                    "error": e.clone(),
                }),
            );
            Err(e)
        }
    }
}

#[tauri::command]
async fn start_stream_message(
    args: Option<StartStreamMessageArgs>,
    session_id: Option<String>,
    messages: Option<Vec<ChatMessage>>,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let (session_id, messages) = if let Some(args) = args {
        (args.session_id, args.messages)
    } else {
        (
            session_id.ok_or_else(|| "start_stream_message 缺少 session_id".to_string())?,
            messages.ok_or_else(|| "start_stream_message 缺少 messages".to_string())?,
        )
    };

    // AgentRun tracking
    let user_input_text = messages
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text);
    let _agent_run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&agent_run) {
            eprintln!("[AgentRun] 保存运行记录失败: {}", e);
        }
    }

    let user_msg = messages.last().cloned();
    let intent = if let Some(ref m) = user_msg {
        if m.role == "user" {
            let router = state.intent_router.lock().await;
            Some(router.classify(&m.content))
        } else {
            None
        }
    } else {
        None
    };

    let layer = if let Some(ref i) = intent {
        let lr = state.layer_router.lock().await;
        lr.resolve(i, &user_msg.as_ref().unwrap().content)
    } else {
        Layer::L2
    };

    // AgentLoop stream integration: when use_agent_loop is enabled,
    // delegate to AgentLoop and simulate streaming by emitting the result
    let cfg = state.config.lock().await;
    let use_agent_loop = cfg.system.use_agent_loop.unwrap_or(false);
    drop(cfg);

    if use_agent_loop && layer != Layer::L1 {
        return start_stream_message_with_agent_loop(
            session_id, messages, user_msg, layer, app_handle, state,
        )
        .await;
    }

    // Layer 1: direct reflex response (non-streaming, emit as single chunk)
    if layer == Layer::L1 {
        if let Some(ref i) = intent {
            if let Some(reply) = i.direct_response() {
                // 先保存用户消息
                if let Some(ref user) = user_msg {
                    if user.role == "user" {
                        let user_inserted =
                            persist_chat_message_if_needed(&session_id, user, &state).await?;
                        if user_inserted {
                            persist_vector_memory_for_message(&session_id, user, &state).await;
                        }
                    }
                }

                // 加载 LifeModel 用于 finalizer
                let life_model = {
                    let manager = state.life_model_manager.lock().await;
                    manager.load().map_err(|e| e.to_string())?
                };

                let assistant_msg = ChatMessage {
                    role: "assistant".into(),
                    content: reply.clone(),
                };

                let _ = app_handle.emit(
                    "stream-message-start",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "reasoning_trace": ReasoningTrace::default(),
                        "tool_calls": Vec::<ToolCallResult>::new(),
                    }),
                );
                let _ = app_handle.emit(
                    "stream-message-chunk",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "chunk": reply.clone(),
                    }),
                );

                // 使用统一的 finalizer，避免重复保存和手动更新 AgentRun
                let mut reasoning_trace = ReasoningTrace::default();
                let model_route = openlife_core::agent::ModelRouteTrace {
                    provider: "direct".to_string(),
                    model: "L1_reflex".to_string(),
                    route_type: "direct".to_string(),
                    prefer_local: false,
                    local_model: "".to_string(),
                    reason: "layer_1_direct_response".to_string(),
                    privacy_level: openlife_core::agent::types::RedactionLevel::None,
                    latency_ms: None,
                    retry_count: 0,
                    fallback_reason: None,
                    provider_health_is_estimated: Some(false),
                };
                let context_summary = openlife_core::agent::ContextSummary {
                    life_model_empty: true,
                    included_life_model_sections: vec![],
                    memory_hit_count: 0,
                    memory_sources: vec![],
                    used_tools_prompt: false,
                    redaction_applied: false,
                    redaction_level: openlife_core::agent::types::RedactionLevel::None,
                };
                let preview = preview_text(&reply, 200);
                agent_run.complete(&preview, model_route, context_summary);

                if let Err(e) = finalize_chat_agent_run(
                    &session_id,
                    &assistant_msg,
                    &reply,
                    &mut reasoning_trace,
                    &mut agent_run,
                    &life_model,
                    &state,
                )
                .await
                {
                    eprintln!("[L1 Stream] finalize_chat_agent_run failed: {}", e);
                    let _ = app_handle.emit(
                        "stream-message-error",
                        serde_json::json!({
                            "session_id": &session_id,
                            "run_id": agent_run.id,
                            "error": format!("AgentRun 持久化失败: {}", e),
                        }),
                    );
                    return Err(e);
                }

                let _ = app_handle.emit(
                    "stream-message-done",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "reply": reply,
                        "reasoning_trace": ReasoningTrace::default(),
                        "tool_calls": Vec::<ToolCallResult>::new(),
                    }),
                );
                return Ok(());
            }
        }
    }

    // Gradual rollout: use v2 if experimental flag is enabled
    let use_v2 = {
        let cfg = state.config.lock().await;
        cfg.experimental_context_assembler
    };

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        _embed_err,
        context_summary,
    ) = match if use_v2 {
        preprocess_chat_input_v2(&session_id, &messages, &state).await
    } else {
        preprocess_chat_input(&session_id, &messages, &state).await
    } {
        Ok(result) => result,
        Err(message) => {
            let error = openlife_core::agent::AgentRunError {
                message: message.clone(),
                phase: "preprocess".to_string(),
                recoverable: true,
            };
            agent_run.fail(error);
            if let Some(ref store_arc) = state.agent_run_store {
                let store = store_arc.lock().await;
                if let Err(e) = store.update_run(&agent_run) {
                    eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                }
            }
            return Err(message);
        }
    };

    let auto_checkin_msg_stream = if let Some(ref m) = user_msg {
        let msg = try_auto_checkin_daily_goals(&m.content, &mut life_model);
        capture_conversation_signals(&session_id, &m.content, &life_model, state.inner()).await;
        if msg.is_some() {
            if let Err(message) =
                persist_life_model(&state.inner().clone(), life_model.clone(), false).await
            {
                let error = openlife_core::agent::AgentRunError {
                    message: message.clone(),
                    phase: "preprocess".to_string(),
                    recoverable: true,
                };
                agent_run.fail(error);
                if let Some(ref store_arc) = state.agent_run_store {
                    let store = store_arc.lock().await;
                    if let Err(e) = store.update_run(&agent_run) {
                        eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                    }
                }
                return Err(message);
            }
        }
        msg
    } else {
        None
    };

    let scheduler_clone = state.scheduler.lock().await.clone();
    let model_route = scheduler_clone
        .preview_chat_route(Some(&tools_prompt))
        .await;
    let cfg = state.config.lock().await;
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler_clone.clone(), &cfg);
    drop(cfg);

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer,
    };

    let mut reasoning_trace = ReasoningTrace::default();
    let mut messages_with_reasoning = desensitized_messages.clone();

    let _actual_layer = if layer == Layer::L3 {
        let runtime_output = agent_runtime
            .execute_task(
                &task,
                &life_model,
                &tools_prompt,
                None,
                vec![],
                privacy_engine.clone(),
            )
            .await;

        match runtime_output {
            Ok(output) => {
                messages_with_reasoning = output.final_messages;
                reasoning_trace = output.reasoning_trace;
                agent_run.reasoning_strategy = Some("layered".to_string());
                agent_run.reasoning_trace = Some(reasoning_trace.clone());
                Layer::L3
            }
            Err(e) => {
                eprintln!("[AgentRuntime] Reasoning failed: {}, falling back to L2", e);
                agent_run.reasoning_strategy = Some("direct".to_string());
                let lr = state.layer_router.lock().await;
                lr.fallback(Layer::L3).unwrap_or(Layer::L2)
            }
        }
    } else {
        agent_run.reasoning_strategy = Some("direct".to_string());
        layer
    };

    let _ = app_handle.emit(
        "stream-message-start",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reasoning_trace": reasoning_trace.clone(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );

    let mut full_reply = String::new();
    if let Some(ref ex) = reasoning_trace.generation_result {
        if let Some(text) = ex.get("text").and_then(|t| t.as_str()) {
            full_reply = text.to_string();
            let _ = app_handle.emit(
                "stream-message-chunk",
                serde_json::json!({
                    "session_id": &session_id,
                    "chunk": text,
                }),
            );
        }
    }

    if full_reply.is_empty() {
        match timeout(
            Duration::from_secs(STREAM_INIT_TIMEOUT_SECS),
            scheduler_clone.generate_stream(
                messages_with_reasoning.clone(),
                &life_model,
                Some(&tools_prompt),
            ),
        )
        .await
        .map_err(|_| format!("流式响应初始化超时（{} 秒）", STREAM_INIT_TIMEOUT_SECS))
        .and_then(|result| result.map_err(|e| e.to_string()))
        {
            Ok(mut stream) => loop {
                let next_chunk = match timeout(
                    Duration::from_secs(STREAM_CHUNK_TIMEOUT_SECS),
                    stream.next(),
                )
                .await
                {
                    Ok(next) => next,
                    Err(_) => {
                        let stream_error =
                            format!("超过 {} 秒没有收到模型输出", STREAM_CHUNK_TIMEOUT_SECS);
                        match generate_non_stream_fallback(
                            &scheduler_clone,
                            messages_with_reasoning.clone(),
                            &life_model,
                            &tools_prompt,
                        )
                        .await
                        {
                            Ok(reply) => {
                                let fallback_text = if full_reply.is_empty() {
                                    reply
                                } else {
                                    format!(
                                            "\n\n[系统] 流式连接长时间无输出，已自动用非流式请求重试并补全回复：\n\n{}",
                                            reply
                                        )
                                };
                                reasoning_trace.errors.push(format!(
                                    "流式响应超时，已降级为非流式响应：{}",
                                    stream_error
                                ));
                                full_reply.push_str(&fallback_text);
                                let _ = app_handle.emit(
                                    "stream-message-chunk",
                                    serde_json::json!({
                                        "session_id": &session_id,
                                        "chunk": fallback_text,
                                    }),
                                );
                                break;
                            }
                            Err(fallback_error) => {
                                let message = format!(
                                    "流式响应超时，非流式重试也失败：{}；重试错误：{}",
                                    stream_error, fallback_error
                                );
                                emit_stream_error(
                                    &app_handle,
                                    &session_id,
                                    &agent_run.id,
                                    message.clone(),
                                );
                                let error = openlife_core::agent::AgentRunError {
                                    message: message.clone(),
                                    phase: "stream".to_string(),
                                    recoverable: true,
                                };
                                agent_run.fail(error);
                                if let Some(ref store_arc) = state.agent_run_store {
                                    let store = store_arc.lock().await;
                                    if let Err(e) = store.update_run(&agent_run) {
                                        eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                                    }
                                }
                                return Err(message);
                            }
                        }
                    }
                };
                let Some(chunk_result) = next_chunk else {
                    break;
                };
                match chunk_result {
                    Ok(chunk) => {
                        if !chunk.is_empty() {
                            full_reply.push_str(&chunk);
                            let _ = app_handle.emit(
                                "stream-message-chunk",
                                serde_json::json!({
                                    "session_id": &session_id,
                                    "chunk": chunk,
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        let stream_error = e.to_string();
                        match generate_non_stream_fallback(
                            &scheduler_clone,
                            messages_with_reasoning.clone(),
                            &life_model,
                            &tools_prompt,
                        )
                        .await
                        {
                            Ok(reply) => {
                                let fallback_text = if full_reply.is_empty() {
                                    reply
                                } else {
                                    format!(
                                            "\n\n[系统] 流式连接中断，已自动用非流式请求重试并补全回复：\n\n{}",
                                            reply
                                        )
                                };
                                reasoning_trace.errors.push(format!(
                                    "流式响应中断，已降级为非流式响应：{}",
                                    stream_error
                                ));
                                full_reply.push_str(&fallback_text);
                                let _ = app_handle.emit(
                                    "stream-message-chunk",
                                    serde_json::json!({
                                        "session_id": &session_id,
                                        "chunk": fallback_text,
                                    }),
                                );
                                break;
                            }
                            Err(fallback_error) => {
                                let message = format!(
                                    "流式响应失败，非流式重试也失败：{}；重试错误：{}",
                                    stream_error, fallback_error
                                );
                                emit_stream_error(
                                    &app_handle,
                                    &session_id,
                                    &agent_run.id,
                                    message.clone(),
                                );
                                let error = openlife_core::agent::AgentRunError {
                                    message: message.clone(),
                                    phase: "stream".to_string(),
                                    recoverable: true,
                                };
                                agent_run.fail(error);
                                if let Some(ref store_arc) = state.agent_run_store {
                                    let store = store_arc.lock().await;
                                    if let Err(e) = store.update_run(&agent_run) {
                                        eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                                    }
                                }
                                return Err(message);
                            }
                        }
                    }
                }
            },
            Err(stream_error) => {
                let stream_error = stream_error.to_string();
                match generate_non_stream_fallback(
                    &scheduler_clone,
                    messages_with_reasoning.clone(),
                    &life_model,
                    &tools_prompt,
                )
                .await
                {
                    Ok(reply) => {
                        reasoning_trace.errors.push(format!(
                            "流式响应初始化失败，已降级为非流式响应：{}",
                            stream_error
                        ));
                        full_reply = reply.clone();
                        let _ = app_handle.emit(
                            "stream-message-chunk",
                            serde_json::json!({
                                "session_id": &session_id,
                                "chunk": reply,
                            }),
                        );
                    }
                    Err(fallback_error) => {
                        let message = format!(
                            "流式响应初始化失败，非流式重试也失败：{}；重试错误：{}",
                            stream_error, fallback_error
                        );
                        emit_stream_error(&app_handle, &session_id, &agent_run.id, message.clone());
                        let error = openlife_core::agent::AgentRunError {
                            message: message.clone(),
                            phase: "stream".to_string(),
                            recoverable: true,
                        };
                        agent_run.fail(error);
                        if let Some(ref store_arc) = state.agent_run_store {
                            let store = store_arc.lock().await;
                            if let Err(e) = store.update_run(&agent_run) {
                                eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                            }
                        }
                        return Err(message);
                    }
                }
            }
        }
        if full_reply.trim().is_empty() {
            let stream_error = "流式响应已结束，但没有收到可显示内容".to_string();
            match generate_non_stream_fallback(
                &scheduler_clone,
                messages_with_reasoning.clone(),
                &life_model,
                &tools_prompt,
            )
            .await
            {
                Ok(reply) => {
                    reasoning_trace.errors.push(format!(
                        "流式响应为空，已降级为非流式响应：{}",
                        stream_error
                    ));
                    full_reply = reply.clone();
                    let _ = app_handle.emit(
                        "stream-message-chunk",
                        serde_json::json!({
                            "session_id": &session_id,
                            "chunk": reply,
                        }),
                    );
                }
                Err(fallback_error) => {
                    let message = format!(
                        "流式响应为空，非流式重试也失败：{}；重试错误：{}",
                        stream_error, fallback_error
                    );
                    emit_stream_error(&app_handle, &session_id, &agent_run.id, message.clone());
                    let error = openlife_core::agent::AgentRunError {
                        message: message.clone(),
                        phase: "stream".to_string(),
                        recoverable: true,
                    };
                    agent_run.fail(error);
                    if let Some(ref store_arc) = state.agent_run_store {
                        let store = store_arc.lock().await;
                        if let Err(e) = store.update_run(&agent_run) {
                            eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                        }
                    }
                    return Err(message);
                }
            }
        }
    }

    let mut first_reply = privacy_engine.reconstruct(&full_reply, &privacy_map);
    if let Some(msg) = auto_checkin_msg_stream {
        if !first_reply.contains(&msg) {
            first_reply = format!("{}\n\n[系统] {}", first_reply, msg);
        }
    }

    let tool_results = {
        let (reg, audit) = state.get_mcp_state().await;
        let permission_store = state.tool_permission_store.lock().await;
        let privacy_engine = state.privacy_engine.lock().await;
        try_prepare_tool_calls(
            &first_reply,
            &reg,
            &audit,
            &permission_store,
            &privacy_engine,
            agent_run.actions.len() as u32,
            Some(agent_run.id.clone()),
        )
    };
    let (reply, tool_calls) = if let Some((results, actions, observations)) = tool_results {
        agent_run.actions.extend(actions);
        agent_run.observations.extend(observations);
        let executed_results: Vec<_> = results
            .iter()
            .filter(|r| !r.requires_confirmation)
            .cloned()
            .collect();
        let pending_count = results.iter().filter(|r| r.requires_confirmation).count();
        let results_text: String = executed_results
            .iter()
            .map(|r| {
                if r.success {
                    format!(
                        "{} 结果: {}",
                        r.name,
                        r.output.as_ref().unwrap_or(&String::new())
                    )
                } else {
                    format!(
                        "{} 错误: {}",
                        r.name,
                        r.error.as_ref().unwrap_or(&String::new())
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut reply = if executed_results.is_empty() {
            if pending_count > 0 {
                "我需要先执行一些高风险或含敏感参数的工具操作，确认后才能继续给你结果。".to_string()
            } else {
                first_reply
            }
        } else {
            let mut follow_up = messages_with_reasoning.clone();
            follow_up.push(ChatMessage {
                role: "assistant".into(),
                content: first_reply,
            });
            follow_up.push(ChatMessage {
                role: "user".into(),
                content: format!(
                    "你刚才请求调用了工具，以下是执行结果，请基于结果直接回答用户的问题：\n{}",
                    results_text
                ),
            });
            let scheduler_clone = state.scheduler.lock().await.clone();
            match scheduler_clone.generate(follow_up, &life_model, None).await {
                Ok(r) => privacy_engine.reconstruct(&r, &privacy_map),
                Err(e) => {
                    let _ = app_handle.emit(
                        "stream-message-error",
                        serde_json::json!({
                            "session_id": &session_id,
                            "run_id": agent_run.id,
                            "error": e.to_string(),
                        }),
                    );
                    let error = openlife_core::agent::AgentRunError {
                        message: e.to_string(),
                        phase: "model".to_string(),
                        recoverable: true,
                    };
                    agent_run.fail(error);
                    if let Some(ref store_arc) = state.agent_run_store {
                        let store = store_arc.lock().await;
                        if let Err(e) = store.update_run(&agent_run) {
                            eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                        }
                    }
                    return Err(e.to_string());
                }
            }
        };
        if pending_count > 0 {
            reply.push_str(&format!(
                "\n\n[系统] 有 {} 个高风险或敏感参数工具调用待确认。请先在下方工具卡片中确认后再执行。",
                pending_count
            ));
        }
        (reply, results)
    } else {
        (first_reply, vec![])
    };

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    let context_summary = openlife_core::agent::ContextSummary {
        life_model_empty: life_model.is_effectively_empty(),
        included_life_model_sections: included_life_model_sections(&life_model),
        memory_hit_count: context_summary.memory_hit_count,
        memory_sources: context_summary.memory_sources,
        used_tools_prompt: !tools_prompt.is_empty(),
        redaction_applied: !privacy_map.is_empty(),
        redaction_level: if privacy_map.is_empty() {
            openlife_core::agent::types::RedactionLevel::None
        } else {
            openlife_core::agent::types::RedactionLevel::Light
        },
    };
    let preview = preview_text(&reply, 200);
    agent_run.complete(&preview, model_route, context_summary);
    if let Err(e) = finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await
    {
        eprintln!("[Stream] finalize_chat_agent_run failed: {}", e);
        let _ = app_handle.emit(
            "stream-message-error",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": agent_run.id,
                "error": format!("AgentRun 持久化失败: {}", e),
            }),
        );
        return Err(e);
    }

    let _ = app_handle.emit(
        "stream-message-done",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reply": reply,
            "reasoning_trace": reasoning_trace,
            "tool_calls": tool_calls,
        }),
    );

    Ok(())
}
#[tauri::command]
async fn execute_tool_call(
    name: String,
    arguments: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolCallResult, String> {
    let (reg, audit) = state.get_mcp_state().await;
    let permission_store = state.tool_permission_store.lock().await;
    let privacy_engine = state.privacy_engine.lock().await;
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();

    let executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let ctx = openlife_core::agent::ActionExecutionContext {
        registry: &reg,
        permission_store: &permission_store,
        audit_store: &audit,
        privacy_engine: &privacy_engine,
        safe_paths: &safe_paths,
        life_model: None,
        memory_store: None,
    };

    let request = openlife_core::agent::AgentActionRequest {
        action_type: "mcp_tool".to_string(),
        target: name.clone(),
        input: serde_json::json!({ "arguments": arguments }),
        source_run_id: None,
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).map_err(|e| e.to_string())?;

    let tool_result = ToolCallResult {
        name: name.clone(),
        arguments: arguments.clone(),
        sanitized_arguments: Some(arguments),
        success: result.status == openlife_core::agent::ActionExecutionStatus::Succeeded,
        output: result
            .action
            .output
            .as_ref()
            .and_then(|o| o.get("text").and_then(|t| t.as_str()).map(String::from)),
        error: result.action.error.clone(),
        permission_level: result
            .action
            .tool_scope
            .as_ref()
            .map(|s| s.risk_level.clone())
            .unwrap_or_else(|| "medium".into()),
        status: match result.status {
            openlife_core::agent::ActionExecutionStatus::Succeeded => "success",
            openlife_core::agent::ActionExecutionStatus::Failed => "error",
            openlife_core::agent::ActionExecutionStatus::Blocked => "error",
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation => "pending",
        }
        .into(),
        requires_confirmation: result.status
            == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
        pii_found: false,
        privacy_warnings: vec![],
        action_id: Some(result.action.id),
        run_id: None,
        permission_decision: result.action.permission_decision,
    };

    Ok(tool_result)
}
#[tauri::command]
async fn inspect_mcp_call(
    name: String,
    arguments: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::mcp::McpArgumentInspection, String> {
    let reg = state.mcp_registry.lock().await;
    Ok(reg.inspect_call_arguments(&name, &arguments))
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = app_data_dir();
    let mut startup_warnings = Vec::new();
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        startup_warnings.push(format!(
            "应用数据目录创建失败：{} ({})",
            data_dir.display(),
            e
        ));
    }
    let config_path = data_dir.join("config.yaml");
    let (config, config_warning) = AppConfig::load_or_default_with_warning(&config_path);
    if let Some(warning) = config_warning {
        startup_warnings.push(warning);
    }
    // Apply system configuration
    openlife_core::ollama::set_ollama_cache_ttl_seconds(config.system.ollama_cache_ttl_seconds);
    let life_model_manager = LifeModelManager::new(data_dir.join("life-model").join("current"));

    let db_path = data_dir.join("memory.db");
    let memory_store = init_memory_store(&db_path, &mut startup_warnings).unwrap_or_else(|e| {
        startup_warnings.push(e);
        MemoryStore::new_in_memory().expect("致命错误：无法初始化 memory store，系统资源耗尽")
    });
    let feedback_db_path = data_dir.join("feedback.db");
    let feedback_store = init_feedback_store(&feedback_db_path, &mut startup_warnings)
        .unwrap_or_else(|e| {
            startup_warnings.push(e);
            FeedbackStore::new_in_memory()
                .expect("致命错误：无法初始化 feedback store，系统资源耗尽")
        });
    let vector_db_path = data_dir.join("vectors.db");
    let vector_store =
        init_vector_store(&vector_db_path, &mut startup_warnings).unwrap_or_else(|e| {
            startup_warnings.push(e);
            VectorStore::new_in_memory().expect("致命错误：无法初始化 vector store，系统资源耗尽")
        });
    let agent_runs_db_path = data_dir.join("agent_runs.db");
    let agent_run_store = init_agent_run_store(&agent_runs_db_path, &mut startup_warnings)
        .unwrap_or_else(|e| {
            startup_warnings.push(e);
            openlife_core::agent::AgentRunStore::new_in_memory()
                .expect("致命错误：无法初始化 agent run store，系统资源耗尽")
        });
    let proposals_db_path = data_dir.join("proposals.db");
    let proposal_store = init_proposal_store(&proposals_db_path, &mut startup_warnings)
        .unwrap_or_else(|e| {
            startup_warnings.push(e);
            openlife_core::agent::ProposalStore::new_in_memory()
                .expect("致命错误：无法初始化 proposal store，系统资源耗尽")
        });
    let patches_db_path = data_dir.join("patches.db");
    let patch_store = openlife_core::life_model::patch_store::PatchStore::new(&patches_db_path)
        .unwrap_or_else(|e| {
            startup_warnings.push(format!("PatchStore 初始化失败: {}", e));
            openlife_core::life_model::patch_store::PatchStore::new_in_memory()
                .expect("致命错误：无法初始化 patch store，系统资源耗尽")
        });

    let model_dir = data_dir.join("models");
    let intent_router = IntentRouter::with_optional_onnx(Some(&model_dir));
    let layer_router = LayerRouter::new();
    let scheduler = InferenceScheduler::new(
        config.local_model.clone(),
        config.prefer_local_model,
        config.llm.provider.clone(),
        config.llm.openai_base.clone(),
        config.llm.openai_key.clone(),
        config.llm.chat_model.clone(),
        config.llm.embedding_model.clone(),
        config.llm.embedding_enabled,
    );
    let privacy_engine =
        PrivacyEngine::with_policy(load_privacy_policy_from_path(&privacy_policy_path()));
    let version_manager = VersionManager::new(data_dir.join("life-model").join("versions"));
    let mcp_audit_store = McpAuditStore::with_keyring(
        data_dir.join("mcp_audit.db"),
        load_mcp_audit_keyring_from_path(&mcp_audit_keyring_path()),
    );

    // Initialize hot cache from current life model
    let hot_cache: SharedHotCache = {
        let manager = &life_model_manager;
        let initial_cache = match manager.load() {
            Ok(model) => HotMemoryCache::from_life_model(&model),
            Err(_) => HotMemoryCache::default(),
        };
        Arc::new(tokio::sync::RwLock::new(initial_cache))
    };

    let mcp_registry = McpRegistry::new();
    let tool_permission_store = openlife_core::tool_permissions::ToolPermissionStore::new(
        data_dir.join("tool_permissions.db"),
    )
    .unwrap_or_else(|e| {
        startup_warnings.push(format!("tool_permissions.db 初始化失败: {}", e));
        openlife_core::tool_permissions::ToolPermissionStore::new_in_memory()
            .expect("致命错误：无法初始化 tool permission store，系统资源耗尽")
    });
    let mut plugin_registry = openlife_core::plugins::PluginRegistry::new(data_dir.join("plugins"));
    if let Err(e) = plugin_registry.reload() {
        startup_warnings.push(format!("plugins manifest reload failed: {}", e));
    }

    let app_state = Arc::new(AppState {
        config: Arc::new(Mutex::new(config)),
        life_model_manager: Arc::new(Mutex::new(life_model_manager)),
        memory_store: Arc::new(Mutex::new(memory_store)),
        mcp_registry: Arc::new(Mutex::new(mcp_registry)),
        intent_router: Arc::new(Mutex::new(intent_router)),
        layer_router: Arc::new(Mutex::new(layer_router)),
        scheduler: Arc::new(Mutex::new(scheduler)),
        privacy_engine: Arc::new(Mutex::new(privacy_engine)),
        version_manager: Arc::new(Mutex::new(version_manager)),
        feedback_store: Arc::new(Mutex::new(feedback_store)),
        vector_store: Arc::new(Mutex::new(vector_store)),
        builder_sessions: Arc::new(Mutex::new(HashMap::new())),
        builder_session_store: Arc::new(Mutex::new(BuilderSessionStore::new(
            data_dir.join("builder_sessions.json"),
        ))),
        a2a_sidecar: Arc::new(Mutex::new(a2a_sidecar::A2ASidecar::new(8765))),
        last_snapshot_date: Arc::new(Mutex::new(None)),
        mcp_audit_store: Arc::new(Mutex::new(mcp_audit_store)),
        agent_run_store: Some(Arc::new(Mutex::new(agent_run_store))),
        proposal_store: Some(Arc::new(Mutex::new(proposal_store))),
        patch_store: Some(Arc::new(Mutex::new(patch_store))),
        rollout_metrics_store: {
            let store_path = data_dir.join("rollout_metrics.db");
            match openlife_core::agent::RolloutMetricsStore::new(&store_path) {
                Ok(store) => Some(Arc::new(Mutex::new(store))),
                Err(e) => {
                    startup_warnings.push(format!("rollout_metrics.db 初始化失败: {}", e));
                    None
                }
            }
        },
        tool_permission_store: Arc::new(Mutex::new(tool_permission_store)),
        skill_registry: Arc::new(Mutex::new(openlife_core::skills::SkillRegistry::built_in())),
        plugin_registry: Arc::new(Mutex::new(plugin_registry)),
        hot_cache,
        proposal_engine: Arc::new(tokio::sync::Mutex::new({
            let mut engine = openlife_core::agent::ProposalEngine::new();
            engine.register(Box::new(
                openlife_core::agent::ChatProposalGeneratorAdapter::new(),
            ));
            engine.register(Box::new(openlife_core::agent::FeedbackProposalGenerator));
            engine.register(Box::new(openlife_core::agent::MemoryProposalGenerator));
            engine.register(Box::new(openlife_core::agent::ToolProposalGenerator));
            engine
        })),
        startup_warnings,
        provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
    });

    let app_state_for_setup = app_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_http::init())
        .manage(app_state.clone())
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            } else {
                let _ = tauri::WebviewWindowBuilder::new(
                    app,
                    "main",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("OpenLife")
                .inner_size(1280.0, 800.0)
                .resizable(true)
                .center()
                .visible(true)
                .focused(true)
                .build()
                .map_err(|e| {
                    eprintln!("[setup] failed to create main window: {} - lib.rs:1768", e);
                    e
                })?;
            }
            println!("[setup] launching a2a sidecar");
            let a2a_sidecar = app_state_for_setup.a2a_sidecar.clone();
            let state = app_state_for_setup.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = a2a_sidecar.lock().await.start().await {
                    eprintln!("[setup] a2a sidecar start failed: {}", e);
                    eprintln!("[setup] falling back to embedded a2a server");
                    a2a_server::start(state).await;
                }
            });
            if std::env::var("OPENLIFE_AUTOSTART_FILESYSTEM_MCP").as_deref() == Ok("1") {
                let mcp_registry = app_state_for_setup.mcp_registry.clone();
                tauri::async_runtime::spawn(async move {
                    let mut registry = mcp_registry.lock().await;
                    if let Err(e) = registry.register(
                        "filesystem",
                        "npx",
                        &["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
                    ) {
                        eprintln!(
                            "[setup] autoregister filesystem mcp failed: {} - lib.rs:2246",
                            e
                        );
                    }
                });
            }
            let vs = app_state_for_setup.vector_store.clone();
            tauri::async_runtime::spawn(async move {
                {
                    let store = vs.lock().await;
                    match store.run_tier_maintenance() {
                        Ok((upgraded, downgraded)) => {
                            println!("[tier] initial maintenance done: upgraded={} downgraded={} - lib.rs:2255", upgraded, downgraded);
                        }
                        Err(e) => {
                            eprintln!("[tier] initial maintenance failed: {} - lib.rs:2258", e);
                        }
                    }
                }
                let interval = std::time::Duration::from_secs(600);
                loop {
                    tokio::time::sleep(interval).await;
                    let store = vs.lock().await;
                    match store.run_tier_maintenance() {
                        Ok((upgraded, downgraded)) => {
                            println!("[tier] periodic maintenance done: upgraded={} downgraded={} - lib.rs:2268", upgraded, downgraded);
                        }
                        Err(e) => {
                            eprintln!("[tier] periodic maintenance failed: {} - lib.rs:2271", e);
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_life_model,
            save_life_model,
            get_config,
            save_config,
            get_agent_run,
            list_agent_runs,
            list_agent_runs_for_session,
            delete_agent_run,
            restore_agent_run,
            replay_agent_action,
            get_pending_proposals,
            list_proposals,
            batch_accept_low_risk_proposals,
            accept_proposal,
            reject_proposal,
            edit_proposal,
            postpone_proposal,
            send_message,
            start_stream_message,
            get_chat_history,
            save_chat_message,
            execute_tool_call,
            inspect_mcp_call,
            register_mcp_server,
            unregister_mcp_server,
            list_mcp_servers,
            list_mcp_tools,
            list_mcp_templates,
            recommend_mcp_manifests,
            list_mcp_audit_logs,
            clear_mcp_audit_logs,
            get_system_diagnostics,
            check_ollama_status,
            get_router_status,
            get_model_router_status,
            get_scheduler_config,
            set_scheduler_config,
            create_snapshot,
            list_snapshots,
            restore_snapshot,
            diff_snapshots,
            save_feedback,
            get_feedback_summary,
            apply_feedback_evolution,
            generate_evolution_report,
            run_memory_tier_maintenance,
            count_memory_chunks,
            log_analytics_event,
            index_memory_chunk,
            search_memory,
            a2a_discover_agent,
            a2a_send_task,
            a2a_local_agent_card,
            a2a_handle_task,
            a2a_bridge_local,
            a2a_restart_sidecar,
            a2a_stop_sidecar,
            builder_start,
            builder_step,
            builder_list_unfinished,
            builder_delete_session,
            builder_get_pending_signals,
            builder_create_proposals,
            builder_apply_signals,
            get_model_4d_completion,
            goal_capability_gap_analysis,
            goal_capability_gap_report,
            identity_goal_alignment_check,
            identity_goal_alignment_report,
            export_all_data,
            import_all_data,
            test_api_key,
            test_llm_connection,
            get_last_model_error,
            list_chat_sessions,
            create_chat_session,
            rename_chat_session,
            delete_chat_session,
            record_state,
            get_state_history,
            get_state_alerts,
            get_daily_goals,
            add_daily_goal,
            update_daily_goal,
            delete_daily_goal,
            toggle_daily_goal,
            run_micro_evolution,
            generate_calibration_report,
            generate_micro_evolution_changes,
            apply_calibration,
            calibration_create_proposals,
            should_show_calibration,
            mark_calibration_shown,
            get_hot_cache,
            archive_low_access_memories,
            restore_archived_chunks,
            list_archived_chunks,
            get_memory_tier_stats,
            rebuild_memory_index,
            export_mcp_audit_logs,
            cleanup_mcp_audit_logs,
            rotate_mcp_audit_key,
            get_privacy_policy,
            set_privacy_policy,
            has_completed_onboarding,
            mark_onboarding_completed,
            get_rollout_metrics,
            get_rollout_summary,
            get_rollout_errors,
            list_tool_permissions,
            grant_tool_permission,
            revoke_tool_permission,
            check_tool_permission,
            list_skills,
            run_skill,
            get_skill_run_status,
            list_plugins,
            reload_plugins,
            enable_plugin,
            disable_plugin,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| eprintln!("Tauri runtime exited with error: {}", e));
}
