use futures::StreamExt;
use openlife_core::builder::{BuilderSession, BuilderSessionStore};
use openlife_core::config::AppConfig;
use openlife_core::feedback::FeedbackStore;
use openlife_core::hermes::{HermesContext, HermesRequest, HermesTrace};
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
use commands::agent::{delete_agent_run, get_agent_run, list_agent_runs, list_agent_runs_for_session};
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
use commands::feedback::{
    apply_feedback_evolution, generate_evolution_report, get_feedback_summary, log_analytics_event,
    save_feedback,
};
use commands::hermes::hermes_dispatch;
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
use commands::proposal::{
    accept_proposal, batch_accept_low_risk_proposals, edit_proposal, get_pending_proposals,
    list_proposals, postpone_proposal, reject_proposal,
};
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
}

#[derive(serde::Serialize)]
pub struct SendMessageResult {
    pub reply: String,
    pub hermes_trace: HermesTrace,
    pub tool_calls: Vec<ToolCallResult>,
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
    pub hot_cache: SharedHotCache,
    pub startup_warnings: Vec<String>,
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
    if let Some(start) = text.find("```json") {
        let rest = &text[start + 7..];
        if let Some(end) = rest.find("```") {
            return Some(rest[..end].trim());
        }
    }
    if let Some(start) = text.find("```") {
        let rest = &text[start + 3..];
        if let Some(end) = rest.find("```") {
            let inner = rest[..end].trim();
            if inner.starts_with('{') || inner.starts_with('[') {
                return Some(inner);
            }
        }
    }
    if let Some(start) = text.find('{') {
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escape = false;
        let bytes = text.as_bytes();
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
                    return Some(&text[start..=idx]);
                }
            }
        }
    }
    None
}
fn execute_tool_call_internal(
    name: &str,
    args: serde_json::Value,
    permission_level: String,
    registry: &McpRegistry,
    audit: &McpAuditStore,
) -> ToolCallResult {
    let inspection = registry.inspect_call_arguments(name, &args);
    let pii_found = inspection.pii_found;
    let privacy_warnings = inspection
        .findings
        .iter()
        .map(|f| format!("{} 命中 {}: {}", f.path, f.privacy_type, f.matched))
        .collect::<Vec<_>>();
    match registry.call_tool(name, args.clone()) {
        Ok(r) => {
            if let Err(e) = audit.insert_log(name, &args, &r, true, pii_found) {
                eprintln!("[warn] 审计日志写入失败: {}", e);
            }
            ToolCallResult {
                name: name.to_string(),
                arguments: args,
                sanitized_arguments: Some(inspection.sanitized_arguments),
                success: true,
                output: Some(r),
                error: None,
                permission_level,
                status: "success".into(),
                requires_confirmation: false,
                pii_found,
                privacy_warnings,
            }
        }
        Err(e) => {
            if let Err(log_err) = audit.insert_log(name, &args, &e.to_string(), false, pii_found) {
                eprintln!("[warn] 审计日志写入失败: {}", log_err);
            }
            ToolCallResult {
                name: name.to_string(),
                arguments: args,
                sanitized_arguments: Some(inspection.sanitized_arguments),
                success: false,
                output: None,
                error: Some(e.to_string()),
                permission_level,
                status: "error".into(),
                requires_confirmation: false,
                pii_found,
                privacy_warnings,
            }
        }
    }
}
fn try_prepare_tool_calls(
    reply: &str,
    registry: &McpRegistry,
    audit: &McpAuditStore,
) -> Option<Vec<ToolCallResult>> {
    let json_str = try_extract_json(reply)?;
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let calls = v.get("tool_calls")?.as_array()?;
    let mut results = Vec::new();
    for call in calls {
        let name = call.get("name")?.as_str()?;
        let args = call
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let inspection = registry.inspect_call_arguments(name, &args);
        let privacy_warnings = inspection
            .findings
            .iter()
            .map(|f| format!("{} 命中 {}: {}", f.path, f.privacy_type, f.matched))
            .collect::<Vec<_>>();
        if inspection.requires_confirmation {
            results.push(ToolCallResult {
                name: name.to_string(),
                arguments: args,
                sanitized_arguments: Some(inspection.sanitized_arguments),
                success: false,
                output: None,
                error: None,
                permission_level: inspection.permission_level,
                status: "pending".into(),
                requires_confirmation: true,
                pii_found: inspection.pii_found,
                privacy_warnings,
            });
        } else {
            results.push(execute_tool_call_internal(
                name,
                args,
                inspection.permission_level,
                registry,
                audit,
            ));
        }
    }
    if results.is_empty() {
        None
    } else {
        Some(results)
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
        let mut cache = state.hot_cache.lock().unwrap();
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
    let memory_context = if let Some(user_msg) = messages.last() {
        if user_msg.role == "user" {
            let text_hits = {
                let store = state.memory_store.lock().await;
                store
                    .search_text_memories(Some(session_id), &user_msg.content, 3)
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
            memory_sources = results.iter().map(|(chunk, _)| chunk.source.clone()).collect();
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
        let cache = state.hot_cache.lock().unwrap();
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
        included_life_model_sections: vec!["identity".to_string(), "goals".to_string(), "capabilities".to_string(), "state".to_string()],
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
fn build_hermes_prompt(trace: &HermesTrace) -> String {
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
    if let Some(ref arbitration) = trace.arbitration_result {
        if let Some(warnings) = arbitration.get("warnings").and_then(|v| v.as_array()) {
            let text = warnings
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join("；");
            if !text.is_empty() {
                prompt.push_str(&format!("【仲裁提醒】{}\n", text));
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
            let _ = store.log_event(
                &format!("value_focus:{}", value.name),
                Some(session_id),
                Some("chat_match"),
            );
            let _ = store.save_conversation_inference(
                Some(session_id),
                "identity.values",
                &value.name,
                base_delta,
                confidence,
                "用户在对话中主动提及或强化了该价值观",
            );
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
            let _ = store.save_conversation_inference(
                Some(session_id),
                "goals",
                &goal.name,
                base_delta,
                (confidence - 0.05).max(0.2),
                "用户在对话中直接提到该目标，表明关注度发生变化",
            );
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
            let _ = store.save_conversation_inference(
                Some(session_id),
                "capabilities.skills",
                &skill.name,
                skill_delta,
                0.55,
                "用户在对话中主动提及技能投入或受阻情况",
            );
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
                let store = state.memory_store.lock().await;
                let _ = store.save_message(
                    &session_id,
                    &ChatMessage {
                        role: "assistant".into(),
                        content: reply.clone(),
                    },
                );
                return Ok(SendMessageResult {
                    reply,
                    hermes_trace: HermesTrace::default(),
                    tool_calls: vec![],
                });
            }
        }
    }

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        _context_summary,
    ) = preprocess_chat_input(&session_id, &messages, &state).await?;

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

    let life_model_yaml = serde_yaml::to_string(&life_model).unwrap_or_default();
    let hermes_req =
        HermesRequest::new("chat", Some(serde_json::json!({"session_id": &session_id})));
    let scheduler_clone = state.scheduler.lock().await.clone();
    let hermes_bus = openlife_core::hermes::build_bus(life_model.clone(), scheduler_clone.clone());

    let mut hermes_trace = HermesTrace::default();
    let mut messages_with_hermes = desensitized_messages.clone();

    // Layer 3: invoke Hermes for deep reasoning; on failure fallback to L2
    let _actual_layer = if layer == Layer::L3 {
        let mut hermes_ctx = HermesContext {
            life_model_yaml,
            life_model: Some(life_model.clone()),
            recent_messages: desensitized_messages.clone(),
            tools_prompt: Some(tools_prompt.clone()),
            memory_context: String::new(),
            extras: HashMap::new(),
            ..Default::default()
        };
        match hermes_bus
            .dispatch_with_arbitration(&hermes_req, &mut hermes_ctx)
            .await
        {
            Ok(trace) => {
                let prompt = build_hermes_prompt(&trace);
                if !prompt.is_empty() {
                    messages_with_hermes.insert(
                        0,
                        ChatMessage {
                            role: "system".into(),
                            content: prompt.trim().to_string(),
                        },
                    );
                }
                hermes_trace = trace;
                Layer::L3
            }
            Err(e) => {
                hermes_trace.errors.push(e);
                let lr = state.layer_router.lock().await;
                lr.fallback(Layer::L3).unwrap_or(Layer::L2)
            }
        }
    } else {
        layer
    };

    let first_reply = if let Some(ref ex) = hermes_trace.execution_result {
        ex.get("text")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };
    let first_reply = match first_reply {
        Some(text) => text,
        None => scheduler_clone
            .generate(
                messages_with_hermes.clone(),
                &life_model,
                Some(&tools_prompt),
            )
            .await
            .map_err(|e| e.to_string())?,
    };

    let tool_results = {
        let (reg, audit) = state.get_mcp_state().await;
        try_prepare_tool_calls(&first_reply, &reg, &audit)
    };

    let (reply, tool_calls) = if let Some(results) = tool_results {
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
            let mut follow_up = messages_with_hermes.clone();
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
    let inserted = persist_chat_message_if_needed(&session_id, &assistant_message, &state).await?;

    hermes_trace.execution_result = Some(serde_json::json!({ "text": &reply }));

    if let Some(err) = embed_err {
        hermes_trace.errors.push(err);
    }
    if inserted {
        persist_vector_memory_for_message(&session_id, &assistant_message, &state).await;
    }

    Ok(SendMessageResult {
        reply,
        hermes_trace,
        tool_calls,
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
        let _ = store.create_run(&agent_run);
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
                // 保存助手回复
                let assistant_msg = ChatMessage {
                    role: "assistant".into(),
                    content: reply.clone(),
                };
                let _ = persist_chat_message_if_needed(&session_id, &assistant_msg, &state).await?;
                let _ = app_handle.emit(
                    "stream-message-start",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "hermes_trace": HermesTrace::default(),
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
                if let Some(ref store_arc) = state.agent_run_store {
                    let store = store_arc.lock().await;
                    let _ = store.update_run(&agent_run);
                }
                let _ = app_handle.emit(
                    "stream-message-done",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "reply": reply,
                        "hermes_trace": HermesTrace::default(),
                        "tool_calls": Vec::<ToolCallResult>::new(),
                    }),
                );
                return Ok(());
            }
        }
    }

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        _embed_err,
        context_summary,
    ) = match preprocess_chat_input(&session_id, &messages, &state).await {
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
                    let _ = store.update_run(&agent_run);
                }
                return Err(message);
            }
        }
        msg
    } else {
        None
    };

    let life_model_yaml = serde_yaml::to_string(&life_model).unwrap_or_default();
    let hermes_req =
        HermesRequest::new("chat", Some(serde_json::json!({"session_id": &session_id})));
    let scheduler_clone = state.scheduler.lock().await.clone();
    let model_route = scheduler_clone
        .preview_chat_route(Some(&tools_prompt))
        .await;
    let hermes_bus = openlife_core::hermes::build_bus(life_model.clone(), scheduler_clone.clone());

    let mut hermes_trace = HermesTrace::default();
    let mut messages_with_hermes = desensitized_messages.clone();

    let _actual_layer = if layer == Layer::L3 {
        let mut hermes_ctx = HermesContext {
            life_model_yaml,
            life_model: Some(life_model.clone()),
            recent_messages: desensitized_messages.clone(),
            tools_prompt: Some(tools_prompt.clone()),
            memory_context: String::new(),
            extras: HashMap::new(),
            ..Default::default()
        };
        match hermes_bus
            .dispatch_with_arbitration(&hermes_req, &mut hermes_ctx)
            .await
        {
            Ok(trace) => {
                let prompt = build_hermes_prompt(&trace);
                if !prompt.is_empty() {
                    messages_with_hermes.insert(
                        0,
                        ChatMessage {
                            role: "system".into(),
                            content: prompt.trim().to_string(),
                        },
                    );
                }
                hermes_trace = trace;
                Layer::L3
            }
            Err(e) => {
                hermes_trace.errors.push(e);
                let lr = state.layer_router.lock().await;
                lr.fallback(Layer::L3).unwrap_or(Layer::L2)
            }
        }
    } else {
        layer
    };

    let _ = app_handle.emit(
        "stream-message-start",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "hermes_trace": hermes_trace.clone(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );

    let mut full_reply = String::new();
    if let Some(ref ex) = hermes_trace.execution_result {
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
                messages_with_hermes.clone(),
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
                            messages_with_hermes.clone(),
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
                                hermes_trace.errors.push(format!(
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
                                    let _ = store.update_run(&agent_run);
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
                            messages_with_hermes.clone(),
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
                                hermes_trace.errors.push(format!(
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
                                    let _ = store.update_run(&agent_run);
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
                    messages_with_hermes.clone(),
                    &life_model,
                    &tools_prompt,
                )
                .await
                {
                    Ok(reply) => {
                        hermes_trace.errors.push(format!(
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
                            let _ = store.update_run(&agent_run);
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
                messages_with_hermes.clone(),
                &life_model,
                &tools_prompt,
            )
            .await
            {
                Ok(reply) => {
                    hermes_trace.errors.push(format!(
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
                        let _ = store.update_run(&agent_run);
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
        try_prepare_tool_calls(&first_reply, &reg, &audit)
    };
    let (reply, tool_calls) = if let Some(results) = tool_results {
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
            let mut follow_up = messages_with_hermes.clone();
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
                        let _ = store.update_run(&agent_run);
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
    let inserted = persist_chat_message_if_needed(&session_id, &assistant_message, &state).await?;

    hermes_trace.execution_result = Some(serde_json::json!({ "text": &reply }));

    if inserted {
        persist_vector_memory_for_message(&session_id, &assistant_message, &state).await;
    }

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
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.update_run(&agent_run);
    }

    // Chat Proposal generation
    if let Some(ref user_msg) = user_msg {
        if user_msg.role == "user" {
            let config = state.config.lock().await;
            let enabled = config.chat_proposal.enabled;
            let generator = openlife_core::agent::ChatProposalGenerator::new(
                config.chat_proposal.min_message_length,
                config.chat_proposal.confidence_threshold,
                config.chat_proposal.cooldown_seconds,
            );
            drop(config);
            
            if enabled {
                match generator.generate_proposals(&session_id, &user_msg.content, &life_model) {
                    Ok(proposals) if !proposals.is_empty() => {
                        let proposal_store = openlife_core::agent::ProposalStore::new(
                            crate::storage::app_data_dir().join("proposals.db"),
                        );
                        if let Ok(store) = proposal_store {
                            for mut proposal in proposals {
                                proposal.run_id = Some(agent_run.id.clone());
                                if let Err(e) = store.create_proposal(&proposal) {
                                    eprintln!("[ChatProposal] Failed to save proposal: {}", e);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = app_handle.emit(
        "stream-message-done",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reply": reply,
            "hermes_trace": hermes_trace,
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
    let permission_level = reg.tool_permission_level(&name);
    Ok(execute_tool_call_internal(
        &name,
        arguments,
        permission_level,
        &reg,
        &audit,
    ))
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
        Arc::new(std::sync::Mutex::new(initial_cache))
    };

    let mcp_registry = McpRegistry::new();

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
        hot_cache,
        startup_warnings,
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
            hermes_dispatch,
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
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| eprintln!("Tauri runtime exited with error: {}", e));
}
