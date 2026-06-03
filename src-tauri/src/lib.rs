use futures::StreamExt;
use openlife_core::agent::ContextAssembler;
use openlife_core::agent::ReasoningTrace;
use openlife_core::agent::StreamingCallback;
use openlife_core::layer_router::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::memory::MemorySearchHit;
use openlife_core::router::RouterStatus;
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::vectors::{embed_text_with_config, MemoryChunk, VectorInsertItem};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use tokio::time::{timeout, Duration};

pub mod a2a_server;
pub mod a2a_sidecar;
pub mod bootstrap;
pub mod commands;
pub(crate) mod default_chat_adapter;
pub mod errors;
pub mod scheduler_runner;
pub mod state;
pub mod storage;

#[cfg(test)]
pub mod test_utils;

pub use state::AppState;

// Re-exports for test modules (imported as crate::...)
use commands::a2a::{
    a2a_bridge_local, a2a_discover_agent, a2a_handle_task, a2a_local_agent_card,
    a2a_restart_sidecar, a2a_send_task, a2a_stop_sidecar,
};
use commands::agent::{
    delete_agent_run, get_agent_run, list_agent_runs, list_agent_runs_for_session,
    replay_agent_action, restore_agent_run,
};
use commands::agent_runtime::{
    check_controlled_chat_cutover_candidate_promotion_readiness,
    check_controlled_chat_cutover_readiness, check_controlled_chat_migration_implementation_gate,
    check_controlled_chat_pilot_eligibility, check_controlled_pilot_promotion_readiness,
    check_default_chat_adapter_activation_implementation_gate,
    check_default_chat_adapter_contract_harness,
    check_default_chat_adapter_controlled_preview_approval_readiness,
    check_default_chat_adapter_cutover_plan_approval_readiness,
    check_default_chat_adapter_implementation_readiness,
    check_default_chat_adapter_narrow_implementation_discussion_gate,
    check_default_chat_adapter_narrow_implementation_plan_approval_readiness,
    check_runtime_migration_gate, draft_controlled_chat_migration_plan,
    draft_default_chat_adapter_activation_plan,
    draft_default_chat_adapter_cutover_implementation_plan,
    draft_default_chat_adapter_narrow_implementation_plan,
    get_controlled_chat_cutover_candidate_review_summary,
    get_controlled_chat_migration_review_decision_summary,
    get_controlled_chat_migration_shadow_review_summary,
    get_controlled_pilot_promotion_evidence_summary,
    get_default_chat_adapter_activation_review_summary,
    get_default_chat_adapter_controlled_preview_review_summary,
    get_default_chat_adapter_cutover_plan_review_summary,
    get_default_chat_adapter_dry_run_review_summary,
    get_default_chat_adapter_narrow_implementation_plan_review_summary,
    get_default_chat_adapter_ordinary_entry_preflight_status,
    get_default_chat_adapter_routing_status, get_default_chat_runtime_boundary_status,
    record_controlled_chat_cutover_candidate_review_decision,
    record_controlled_chat_migration_review_decision,
    record_controlled_chat_migration_shadow_review_decision,
    record_controlled_pilot_promotion_evidence,
    record_default_chat_adapter_activation_review_decision,
    record_default_chat_adapter_controlled_preview_review_decision,
    record_default_chat_adapter_cutover_plan_review_decision,
    record_default_chat_adapter_dry_run_review_decision,
    record_default_chat_adapter_narrow_implementation_plan_review_decision,
    run_controlled_chat_cutover_candidate, run_controlled_chat_migration_shadow_run,
    run_default_chat_adapter_controlled_preview, run_default_chat_adapter_dry_run,
    run_multi_strategy_agent_preview,
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
pub use openlife_core::memory_cache::HotMemoryCache;
pub use openlife_core::memory_cache::SharedHotCache;
pub use openlife_core::privacy::PrivacyEngine;
// Hermes module removed: replaced by AgentRuntime
use commands::life_model::{get_life_model, save_life_model};
use commands::mcp::{
    clear_mcp_audit_logs, list_mcp_audit_logs, list_mcp_servers, list_mcp_templates,
    list_mcp_tools, list_tool_manifests, recommend_mcp_manifests, register_mcp_server,
    unregister_mcp_server,
};
use commands::memory::{
    archive_low_access_memories, count_memory_chunks, get_hot_cache, get_memory_tier_stats,
    index_memory_chunk, list_archived_chunks, rebuild_memory_index, restore_archived_chunks,
    run_memory_tier_maintenance, search_memory,
};
use commands::metrics::{get_rollout_errors, get_rollout_metrics, get_rollout_summary};
use commands::proactive::get_proactive_suggestions;
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
use storage::app_data_dir;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Success,
    Error,
    Pending,
    Blocked,
    NeedsConfirmation,
}

#[derive(Clone, serde::Serialize)]
pub struct ToolCallResult {
    pub name: String,
    pub arguments: serde_json::Value,
    pub sanitized_arguments: Option<serde_json::Value>,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub permission_level: String,
    pub status: ToolCallStatus,
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
                log::warn!("[ChatProposal] Proposal generation failed: {}", e);
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
                log::warn!("[ChatProposal] Failed to save proposal: {}", e);
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
                log::warn!("[AgentRun] 关联 Chat Proposal 失败: {}", e);
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
    if agent_run.status == openlife_core::agent::AgentRunStatus::Running {
        agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    }
    agent_run.finished_at = Some(chrono::Utc::now());
    agent_run.reasoning_trace = Some(reasoning_trace.clone());

    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        match store.get_run(&agent_run.id) {
            Ok(Some(_)) => {
                if let Err(e) = store.update_run(agent_run) {
                    log::warn!("[AgentRun] 更新运行记录失败: {}", e);
                }
            }
            Ok(None) => {
                if let Err(e) = store.create_run(agent_run) {
                    log::warn!("[AgentRun] 保存运行记录失败: {}", e);
                }
            }
            Err(e) => {
                log::warn!("[AgentRun] 查询运行记录失败: {}", e);
                if let Err(e) = store.create_run(agent_run) {
                    log::warn!("[AgentRun] 保存运行记录失败: {}", e);
                }
            }
        }
    }

    if inserted
        && timeout(
            Duration::from_secs(CHAT_VECTOR_PERSIST_TIMEOUT_SECS),
            persist_vector_memory_for_message(session_id, assistant_message, state),
        )
        .await
        .is_err()
    {
        eprintln!(
            "[memory] vector persistence timed out after {}s for assistant message in session {}",
            CHAT_VECTOR_PERSIST_TIMEOUT_SECS, session_id
        );
    }

    if timeout(
        Duration::from_secs(CHAT_PROPOSAL_GENERATION_TIMEOUT_SECS),
        generate_and_persist_chat_proposals(state.inner(), agent_run, reply, life_model),
    )
    .await
    .is_err()
    {
        eprintln!(
            "[ChatProposal] Proposal generation timed out after {}s for run {}",
            CHAT_PROPOSAL_GENERATION_TIMEOUT_SECS, agent_run.id
        );
    }
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
                        log::warn!("[MemoryService] Failed: {}, falling back to no context", e);
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
        messages: std::sync::Arc::new(messages.to_vec()),
        life_model: std::sync::Arc::new(life_model),
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
    let mut desensitized_messages = output.desensitized_messages.to_vec();
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
        output.life_model.as_ref().clone(),
        output.tools_prompt,
        privacy_engine,
        output.privacy_map,
        desensitized_messages.to_vec(),
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
                log::warn!("[Memory] 记录价值观焦点事件失败: {}", e);
            }
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "identity.values",
                &value.name,
                base_delta,
                confidence,
                "用户在对话中主动提及或强化了该价值观",
            ) {
                log::warn!("[Memory] 保存价值观推断失败: {}", e);
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
                log::warn!("[Memory] 保存目标推断失败: {}", e);
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
                log::warn!("[Memory] 保存技能推断失败: {}", e);
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
    app_handle: tauri::AppHandle,
) -> Result<SendMessageResult, String> {
    let adapter_route = default_chat_adapter::resolve_default_chat_adapter_route();
    let _adapter_preflight =
        default_chat_adapter::ensure_default_chat_adapter_ordinary_entry_preflight(
            default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &adapter_route,
        )?;

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

    let layer = if let (Some(ref i), Some(ref m)) = (&intent, &user_msg) {
        let lr = state.layer_router.lock().await;
        lr.resolve(i, &m.content)
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
        app_handle,
    )
    .await;
}

/// AgentLoop-based chat execution (primary path).
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
    app_handle: tauri::AppHandle,
) -> Result<SendMessageResult, String> {
    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let calendar_ics_paths = cfg.system.calendar_ics_paths.clone();
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let loop_config = openlife_core::agent::AgentLoopConfig {
        max_steps: cfg.system.agent_loop_max_steps,
        max_tool_calls: cfg.system.agent_loop_max_tool_calls,
        timeout_seconds: cfg.system.agent_loop_timeout_seconds,
        allow_writes: true,
        allow_cloud: true,
        shutdown_notify: Some(state.inner().shutdown_notify.clone()),
        ..Default::default()
    };
    let agent_loop = openlife_core::agent::AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler.clone(),
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

    let hs_packet =
        build_chat_runtime_hs_packet(state.inner(), &task, &life_model, &tools_prompt, None)
            .await?;
    let network_policy = cfg.system.network_policy.clone();

    let loop_result = {
        let (reg, audit) = state.get_mcp_state().await;
        let permission_store = state.tool_permission_store.lock().await;
        let memory_store = state.memory_store.lock().await;
        let proposal_store_guard = if let Some(ref store) = state.proposal_store {
            Some(store.lock().await)
        } else {
            None
        };
        let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
            Some(store.lock().await)
        } else {
            None
        };
        let mut action_ctx = openlife_core::agent::ActionExecutionContext::new(
            &reg,
            &permission_store,
            &audit,
            &privacy_engine,
            &safe_paths,
        )
        .with_life_model(&life_model)
        .with_memory_store(&memory_store)
        .with_calendar_ics_paths(&calendar_ics_paths)
        .with_network_policy(&network_policy);
        if let Some(ref store) = proposal_store_guard {
            action_ctx = action_ctx.with_proposal_store(store);
        }
        if let Some(ref store) = agent_run_store_guard {
            action_ctx = action_ctx.with_agent_run_store(store);
        }
        if let Some(ref packet) = hs_packet {
            action_ctx = action_ctx.with_hs_runtime_packet(packet);
        }

        agent_loop
            .run(
                &task,
                &life_model,
                &tools_prompt,
                None,
                privacy_engine.clone(),
                &action_ctx,
            )
            .await
    };

    let (mut reply, mut agent_run, _status_updates) = match loop_result {
        Ok(result) => {
            // Emit AgentLoop status updates as Tauri events
            for update in &result.status_updates {
                emit_agent_status_update(
                    &app_handle,
                    &session_id,
                    &result.run.id,
                    &update.phase.to_string(),
                    &update.message,
                    update.step_index,
                    update.tool_call_index,
                );
            }
            (result.final_response, result.run, result.status_updates)
        }
        Err(e) => {
            eprintln!(
                "[warn] AgentLoop failed in send_message, falling back to legacy: {}",
                e
            );
            let user_input_text = user_msg
                .as_ref()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let (fallback_reply, agent_run) = handle_agent_loop_fallback(
                &scheduler,
                desensitized_messages.clone(),
                &life_model,
                &tools_prompt,
                &session_id,
                &user_input_text,
                state.agent_run_store.as_ref(),
                &e.to_string(),
                hs_packet.clone(),
            )
            .await?;

            return Ok(SendMessageResult {
                reply: fallback_reply,
                reasoning_trace: ReasoningTrace::default(),
                tool_calls: Vec::new(),
                run_id: Some(agent_run.id),
            });
        }
    };

    // Store status_updates in agent_run for persistence
    // (This requires adding a field to AgentRun, which we'll skip for now
    // and just use the Tauri events for real-time UI updates)

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

    let tool_calls = agent_actions_to_tool_call_results(&agent_run.actions, &agent_run.id);

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
const CHAT_VECTOR_PERSIST_TIMEOUT_SECS: u64 = 8;
const CHAT_PROPOSAL_GENERATION_TIMEOUT_SECS: u64 = 5;

/// Emit a unified agent-status-update event for both streaming and non-streaming paths.
/// Frontend AgentStateIndicator expects: phase, message, step_index, tool_call_index, timestamp.
fn emit_agent_status_update(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    run_id: &str,
    phase: &str,
    message: &str,
    step_index: u32,
    tool_call_index: Option<u32>,
) {
    let _ = app_handle.emit(
        "agent-status-update",
        serde_json::json!({
            "session_id": session_id,
            "run_id": run_id,
            "phase": phase,
            "message": message,
            "step_index": step_index,
            "tool_call_index": tool_call_index,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }),
    );
}

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
    hs_packet: Option<openlife_core::agent::RuntimeHSPacket>,
) -> Result<String, String> {
    let fallback = async {
        if let Some(packet) = hs_packet {
            scheduler
                .generate_with_hs_packet(messages, life_model, Some(tools_prompt), &packet)
                .await
        } else {
            scheduler
                .generate(messages, life_model, Some(tools_prompt))
                .await
        }
    };

    timeout(
        Duration::from_secs(NON_STREAM_FALLBACK_TIMEOUT_SECS),
        fallback,
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

/// Handle AgentLoop failure: try non-stream fallback, create AgentRun with
/// error context, persist the run. Returns (reply, agent_run) on success, or
/// an error message string if both AgentLoop and fallback fail.
#[allow(clippy::too_many_arguments)]
async fn handle_agent_loop_fallback(
    scheduler: &InferenceScheduler,
    messages: Vec<ChatMessage>,
    life_model: &LifeModel,
    tools_prompt: &str,
    session_id: &str,
    user_input_text: &str,
    agent_run_store: Option<
        &std::sync::Arc<tokio::sync::Mutex<openlife_core::agent::AgentRunStore>>,
    >,
    original_error: &str,
    hs_packet: Option<openlife_core::agent::RuntimeHSPacket>,
) -> Result<(String, openlife_core::agent::AgentRun), String> {
    let fallback_reply =
        generate_non_stream_fallback(scheduler, messages, life_model, tools_prompt, hs_packet)
            .await
            .map_err(|fallback_err| {
                format!(
                    "AgentLoop failed: {}. Fallback also failed: {}",
                    original_error, fallback_err
                )
            })?;

    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(session_id, user_input_text);
    agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    agent_run.output_preview = Some(preview_text(&fallback_reply, 200));
    agent_run
        .warnings
        .push(format!("fallback: agent_loop_error: {}", original_error));
    agent_run.finished_at = Some(chrono::Utc::now());

    if let Some(store_arc) = agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    Ok((fallback_reply, agent_run))
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

fn agent_actions_to_tool_call_results(
    actions: &[openlife_core::agent::AgentAction],
    run_id: &str,
) -> Vec<ToolCallResult> {
    actions
        .iter()
        .map(|action| {
            let output = action.output.as_ref().and_then(|value| {
                value
                    .get("text")
                    .and_then(|text| text.as_str())
                    .map(ToString::to_string)
                    .or_else(|| value.as_str().map(ToString::to_string))
            });
            ToolCallResult {
                name: action.target.clone().unwrap_or_default(),
                arguments: action.input.clone(),
                sanitized_arguments: None,
                success: matches!(
                    action.status.as_str(),
                    "succeeded" | "completed" | "success"
                ),
                output,
                error: action.error.clone(),
                permission_level: action
                    .tool_scope
                    .as_ref()
                    .map(|scope| scope.risk_level.clone())
                    .unwrap_or_else(|| "low".to_string()),
                status: match action.status.as_str() {
                    "success" | "succeeded" | "completed" => ToolCallStatus::Success,
                    "needs_confirmation" => ToolCallStatus::NeedsConfirmation,
                    "blocked" => ToolCallStatus::Blocked,
                    _ => ToolCallStatus::Error,
                },
                requires_confirmation: action.status == "needs_confirmation",
                pii_found: false,
                privacy_warnings: Vec::new(),
                action_id: Some(action.id.clone()),
                run_id: Some(run_id.to_string()),
                permission_decision: action.permission_decision.clone(),
            }
        })
        .collect()
}

pub(crate) async fn build_chat_runtime_hs_packet(
    state: &Arc<AppState>,
    task: &openlife_core::agent::AgentTask,
    life_model: &LifeModel,
    tools_prompt: &str,
    agent_run_id: Option<String>,
) -> Result<Option<openlife_core::agent::RuntimeHSPacket>, String> {
    let topic = classify_hs_policy_topic(&task.user_text, tools_prompt);
    let tool_requirements = hs_tool_requirements(&task.user_text, tools_prompt);
    let risk_level = hs_risk_level(topic, &tool_requirements);
    let state_hints = serde_json::json!({
        "energy": life_model.state.health_status.energy_level,
    });
    let sanitized_intent_summary =
        sanitized_hs_intent_summary(task.kind, topic, &tool_requirements, &task.user_text);

    let heuristic_store = state.heuristic_store.lock().await;
    openlife_core::agent::build_runtime_hs_packet(
        &state.policy_store,
        &heuristic_store,
        openlife_core::agent::RuntimeHSPacketBuildInput {
            task,
            sanitized_intent_summary,
            privacy_topic: topic,
            risk_level,
            tool_requirements,
            current_state_hints: state_hints,
            token_budget: 384,
            agent_run_id,
        },
    )
    .map_err(|e| format!("HS runtime packet build failed: {}", e))
}

fn classify_hs_policy_topic(
    user_text: &str,
    _tools_prompt: &str,
) -> openlife_core::agent::PolicyTopic {
    let text = user_text.to_lowercase();
    let privacy_engine = PrivacyEngine::new();
    let privacy_findings = privacy_engine.detect(&text);
    if privacy_findings
        .iter()
        .any(|(ptype, _)| matches!(ptype, openlife_core::privacy::PrivacyType::IdCard))
    {
        return openlife_core::agent::PolicyTopic::Identity;
    }
    if privacy_findings
        .iter()
        .any(|(ptype, _)| matches!(ptype, openlife_core::privacy::PrivacyType::BankCard))
    {
        return openlife_core::agent::PolicyTopic::Finance;
    }
    if privacy_findings.iter().any(|(ptype, _)| {
        matches!(
            ptype,
            openlife_core::privacy::PrivacyType::Email
                | openlife_core::privacy::PrivacyType::Phone
                | openlife_core::privacy::PrivacyType::Address
                | openlife_core::privacy::PrivacyType::Name
                | openlife_core::privacy::PrivacyType::Generic
        )
    }) {
        return openlife_core::agent::PolicyTopic::PrivateFile;
    }

    if contains_any(
        &text,
        &[
            "health",
            "medical",
            "medicine",
            "medication",
            "prescription",
            "doctor",
            "therapy",
            "mental",
            "mental health",
            "illness",
            "diagnosis",
            "diagnose",
            "anxiety",
            "depression",
            "drug",
            "药",
            "用药",
            "处方",
            "病",
            "医院",
            "健康",
            "心理",
            "焦虑",
            "抑郁",
            "诊断",
            "治疗",
        ],
    ) {
        openlife_core::agent::PolicyTopic::Health
    } else if contains_any(
        &text,
        &[
            "finance",
            "bank",
            "salary",
            "income",
            "insurance",
            "debt",
            "loan",
            "tax",
            "credit",
            "mortgage",
            "投资",
            "银行",
            "工资",
            "收入",
            "保险",
            "债务",
            "负债",
            "贷款",
            "税",
            "信用卡",
        ],
    ) {
        openlife_core::agent::PolicyTopic::Finance
    } else if contains_any(
        &text,
        &[
            "identity",
            "identity card",
            "id card",
            "passport",
            "ssn",
            "values",
            "mission",
            "身份",
            "身份证",
            "护照",
            "证件",
            "价值观",
            "使命",
        ],
    ) {
        openlife_core::agent::PolicyTopic::Identity
    } else if contains_any(
        &text,
        &[
            "relationship",
            "intimate relationship",
            "partner",
            "family",
            "breakup",
            "break up",
            "divorce",
            "family conflict",
            "关系",
            "亲密关系",
            "伴侣",
            "家人",
            "分手",
            "家庭矛盾",
            "家庭冲突",
            "婚姻",
            "离婚",
            "恋爱",
        ],
    ) {
        openlife_core::agent::PolicyTopic::Relationship
    } else if contains_any(
        &text,
        &[
            "private file",
            "privacy",
            "private",
            "secret",
            "confidential",
            "contract",
            "resume",
            "cv",
            "私人文件",
            "隐私",
            "机密",
            "合同",
            "简历",
        ],
    ) {
        openlife_core::agent::PolicyTopic::PrivateFile
    } else {
        openlife_core::agent::PolicyTopic::General
    }
}

fn hs_tool_requirements(user_text: &str, _tools_prompt: &str) -> Vec<String> {
    let text = user_text.to_lowercase();
    let mut requirements = Vec::new();
    if contains_any(
        &text,
        &[
            "write",
            "save",
            "send",
            "email",
            "calendar",
            "file.write",
            "propose_event",
            "保存",
            "写入",
            "发送",
            "邮件",
            "日历",
        ],
    ) {
        requirements.push("write".to_string());
    }
    if contains_any(
        &text,
        &[
            "send", "email", "calendar", "external", "发送", "邮件", "日历",
        ],
    ) {
        requirements.push("external_side_effect".to_string());
    }
    requirements.sort();
    requirements.dedup();
    requirements
}

fn hs_risk_level(
    topic: openlife_core::agent::PolicyTopic,
    tool_requirements: &[String],
) -> openlife_core::agent::RiskLevel {
    if topic != openlife_core::agent::PolicyTopic::General
        || tool_requirements
            .iter()
            .any(|requirement| requirement == "write" || requirement == "external_side_effect")
    {
        openlife_core::agent::RiskLevel::High
    } else {
        openlife_core::agent::RiskLevel::Low
    }
}

fn sanitized_hs_intent_summary(
    task_kind: openlife_core::agent::AgentTaskKind,
    topic: openlife_core::agent::PolicyTopic,
    tool_requirements: &[String],
    user_text: &str,
) -> String {
    let char_count = user_text.chars().count();
    let length_bucket = match char_count {
        0..=80 => "short",
        81..=240 => "medium",
        _ => "long",
    };
    format!(
        "task_kind={}; topic={:?}; length_bucket={}; tool_requirements={}",
        task_kind,
        topic,
        length_bucket,
        tool_requirements.join(",")
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

/// Streaming callback that forwards AgentLoop events to Tauri frontend via emit().
struct TauriStreamingCallback {
    app_handle: tauri::AppHandle,
    session_id: String,
    run_id: String,
}

#[async_trait::async_trait]
impl StreamingCallback for TauriStreamingCallback {
    async fn on_chunk(&self, chunk: &str, _step: u32, _phase: &str) {
        let _ = self.app_handle.emit(
            "stream-message-chunk",
            serde_json::json!({
                "session_id": self.session_id,
                "run_id": self.run_id,
                "chunk": chunk,
            }),
        );
    }

    async fn on_tool_start(&self, tool_name: &str, _step: u32) {
        let _ = self.app_handle.emit(
            "tool-start",
            serde_json::json!({
                "session_id": self.session_id,
                "run_id": self.run_id,
                "tool_name": tool_name,
                "phase": "executing_tool",
            }),
        );
    }

    async fn on_tool_result(&self, tool_name: &str, success: bool, _step: u32) {
        let _ = self.app_handle.emit(
            "tool-result",
            serde_json::json!({
                "session_id": self.session_id,
                "run_id": self.run_id,
                "tool_name": tool_name,
                "success": success,
                "phase": "observing",
            }),
        );
    }

    async fn on_proposal(&self, proposal_type: &str, proposal_id: &str) {
        let _ = self.app_handle.emit(
            "proposal-created",
            serde_json::json!({
                "session_id": self.session_id,
                "run_id": self.run_id,
                "proposal_type": proposal_type,
                "proposal_id": proposal_id,
            }),
        );
    }

    async fn on_status(&self, status: &str, message: &str, step: u32) {
        emit_agent_status_update(
            &self.app_handle,
            &self.session_id,
            &self.run_id,
            status,
            message,
            step,
            None,
        );
    }
}

/// Stream-mode AgentLoop execution: runs AgentLoop and emits real token-level stream events.
/// This provides consistency when use_agent_loop=true in stream mode.
async fn start_stream_message_with_agent_loop(
    session_id: String,
    messages: Vec<ChatMessage>,
    user_msg: Option<ChatMessage>,
    _layer: Layer,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Non-persisted placeholder id for pre-run errors. The stream start/done
    // events use the authoritative AgentLoop run id after execution.
    let user_input_text = user_msg
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let placeholder_run_id =
        openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text).id;

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
        Err(message) => return Err(message),
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

    // Run AgentLoop
    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let calendar_ics_paths = cfg.system.calendar_ics_paths.clone();
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let loop_config = openlife_core::agent::AgentLoopConfig {
        max_steps: cfg.system.agent_loop_max_steps,
        max_tool_calls: cfg.system.agent_loop_max_tool_calls,
        timeout_seconds: cfg.system.agent_loop_timeout_seconds,
        allow_writes: true,
        allow_cloud: true,
        shutdown_notify: Some(state.inner().shutdown_notify.clone()),
        ..Default::default()
    };
    let agent_loop = openlife_core::agent::AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler.clone(),
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

    let hs_packet =
        build_chat_runtime_hs_packet(state.inner(), &task, &life_model, &tools_prompt, None)
            .await?;
    let network_policy = cfg.system.network_policy.clone();

    // Create streaming callback with run_id placeholder (will be updated after AgentLoop starts)
    let callback = Arc::new(TauriStreamingCallback {
        app_handle: app_handle.clone(),
        session_id: session_id.clone(),
        run_id: placeholder_run_id.clone(),
    });

    // Emit stream-message-start before running agent loop
    let _ = app_handle.emit(
        "stream-message-start",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": placeholder_run_id,
            "reasoning_trace": ReasoningTrace::default(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );

    let loop_result = {
        let (reg, audit) = state.get_mcp_state().await;
        let permission_store = state.tool_permission_store.lock().await;
        let memory_store = state.memory_store.lock().await;
        let proposal_store_guard = if let Some(ref store) = state.proposal_store {
            Some(store.lock().await)
        } else {
            None
        };
        let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
            Some(store.lock().await)
        } else {
            None
        };
        let mut action_ctx = openlife_core::agent::ActionExecutionContext::new(
            &reg,
            &permission_store,
            &audit,
            &privacy_engine,
            &safe_paths,
        )
        .with_life_model(&life_model)
        .with_memory_store(&memory_store)
        .with_calendar_ics_paths(&calendar_ics_paths)
        .with_network_policy(&network_policy);
        if let Some(ref store) = proposal_store_guard {
            action_ctx = action_ctx.with_proposal_store(store);
        }
        if let Some(ref store) = agent_run_store_guard {
            action_ctx = action_ctx.with_agent_run_store(store);
        }
        if let Some(ref packet) = hs_packet {
            action_ctx = action_ctx.with_hs_runtime_packet(packet);
        }

        agent_loop
            .run_streaming(
                &task,
                &life_model,
                &tools_prompt,
                None,
                privacy_engine.clone(),
                &action_ctx,
                callback,
            )
            .await
    };

    let (mut reply, mut agent_run) = match loop_result {
        Ok(result) => (result.final_response, result.run),
        Err(e) => {
            eprintln!(
                "[warn] AgentLoop streaming failed, falling back to legacy: {}",
                e
            );
            let user_input_txt = user_msg
                .as_ref()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let (fallback_reply, agent_run) = match handle_agent_loop_fallback(
                &scheduler,
                desensitized_messages.clone(),
                &life_model,
                &tools_prompt,
                &session_id,
                &user_input_txt,
                state.agent_run_store.as_ref(),
                &e.to_string(),
                hs_packet.clone(),
            )
            .await
            {
                Ok(result) => result,
                Err(error_msg) => {
                    let _ = app_handle.emit(
                        "stream-message-error",
                        serde_json::json!({
                            "session_id": &session_id,
                            "run_id": placeholder_run_id,
                            "error": error_msg.clone(),
                        }),
                    );
                    return Err(error_msg);
                }
            };

            // Emit fallback reply as a single chunk
            let _ = app_handle.emit(
                "stream-message-chunk",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": placeholder_run_id,
                    "chunk": fallback_reply,
                }),
            );

            (fallback_reply, agent_run)
        }
    };

    // Apply privacy reconstruction
    reply = privacy_engine.reconstruct(&reply, &privacy_map);

    // Apply auto checkin message
    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let mut reasoning_trace = agent_run.reasoning_trace.clone().unwrap_or_default();
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

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
            let tool_calls = agent_actions_to_tool_call_results(&agent_run.actions, &agent_run.id);
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
        Err(e) => {
            let _ = app_handle.emit(
                "stream-message-error",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": agent_run.id,
                    "error": e,
                }),
            );
            Ok(())
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

    let adapter_route = default_chat_adapter::resolve_default_chat_adapter_route();
    let _adapter_preflight =
        default_chat_adapter::ensure_default_chat_adapter_ordinary_entry_preflight(
            default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &adapter_route,
        )?;

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

    let layer = if let (Some(ref i), Some(ref m)) = (&intent, &user_msg) {
        let lr = state.layer_router.lock().await;
        lr.resolve(i, &m.content)
    } else {
        Layer::L2
    };

    // Route L2/L3 to AgentLoop streaming path; L1 uses direct reflex below
    if layer != Layer::L1 {
        return start_stream_message_with_agent_loop(
            session_id, messages, user_msg, layer, app_handle, state,
        )
        .await;
    }

    // L1 direct reflex — no AgentLoop needed
    let user_input_text = messages
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text);
    let _agent_run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&agent_run) {
            log::warn!("[AgentRun] 保存运行记录失败: {}", e);
        }
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
                    log::warn!("[L1 Stream] finalize_chat_agent_run failed: {}", e);
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
                    log::warn!("[AgentRun] 更新运行记录失败: {}", e);
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
                        log::warn!("[AgentRun] 更新运行记录失败: {}", e);
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

    let hs_packet =
        build_chat_runtime_hs_packet(state.inner(), &task, &life_model, &tools_prompt, None)
            .await?;

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
                log::warn!("[AgentRuntime] Reasoning failed: {}, falling back to L2", e);
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
        let stream_init = if let Some(ref packet) = hs_packet {
            timeout(
                Duration::from_secs(STREAM_INIT_TIMEOUT_SECS),
                scheduler_clone.generate_stream_with_hs_packet(
                    messages_with_reasoning.clone(),
                    &life_model,
                    Some(&tools_prompt),
                    packet,
                ),
            )
            .await
        } else {
            timeout(
                Duration::from_secs(STREAM_INIT_TIMEOUT_SECS),
                scheduler_clone.generate_stream(
                    messages_with_reasoning.clone(),
                    &life_model,
                    Some(&tools_prompt),
                ),
            )
            .await
        };

        match stream_init
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
                            hs_packet.clone(),
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
                                        log::warn!("[AgentRun] 更新运行记录失败: {}", e);
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
                            hs_packet.clone(),
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
                                        log::warn!("[AgentRun] 更新运行记录失败: {}", e);
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
                    hs_packet.clone(),
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
                                log::warn!("[AgentRun] 更新运行记录失败: {}", e);
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
                hs_packet.clone(),
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
                            log::warn!("[AgentRun] 更新运行记录失败: {}", e);
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

    let reply = first_reply;
    let tool_calls: Vec<ToolCallResult> = vec![];

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
        log::warn!("[Stream] finalize_chat_agent_run failed: {}", e);
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

    // Create an AgentRun for direct tool execution audit trail
    let mut run = openlife_core::agent::AgentRun::new_tool_execution_run(&name);
    let run_id = run.id.clone();

    let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
        Some(store.lock().await)
    } else {
        None
    };

    let executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let ctx = openlife_core::agent::ActionExecutionContext::new(
        &reg,
        &permission_store,
        &audit,
        &privacy_engine,
        &safe_paths,
    );
    let ctx = if let Some(ref store) = agent_run_store_guard {
        ctx.with_agent_run_store(store)
    } else {
        ctx
    };

    let request = openlife_core::agent::AgentActionRequest {
        action_type: "mcp_tool".to_string(),
        target: name.clone(),
        input: serde_json::json!({ "arguments": arguments }),
        source_run_id: Some(run_id.clone()),
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).map_err(|e| e.to_string())?;

    // Persist the AgentRun
    run.actions.push(result.action.clone());
    run.observations.push(result.observation.clone());
    run.status = match result.status {
        openlife_core::agent::ActionExecutionStatus::Succeeded => {
            openlife_core::agent::AgentRunStatus::Completed
        }
        _ => openlife_core::agent::AgentRunStatus::Failed,
    };
    run.finished_at = Some(chrono::Utc::now());

    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&run);
    }

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
            openlife_core::agent::ActionExecutionStatus::Succeeded => ToolCallStatus::Success,
            openlife_core::agent::ActionExecutionStatus::Failed => ToolCallStatus::Error,
            openlife_core::agent::ActionExecutionStatus::Blocked => ToolCallStatus::Blocked,
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation => {
                ToolCallStatus::NeedsConfirmation
            }
        },
        requires_confirmation: result.status
            == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
        pii_found: false,
        privacy_warnings: vec![],
        action_id: Some(result.action.id),
        run_id: Some(run_id),
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

fn ensure_main_window_visible<R: tauri::Runtime, M: Manager<R>>(manager: &M) -> tauri::Result<()> {
    let window = if let Some(window) = manager.get_webview_window("main") {
        window
    } else {
        tauri::WebviewWindowBuilder::new(
            manager,
            "main",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("OpenLife")
        .inner_size(1280.0, 800.0)
        .resizable(true)
        .center()
        .visible(true)
        .focused(true)
        .build()?
    };

    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = app_data_dir();
    let bootstrap = bootstrap::bootstrap(data_dir.clone());
    let app_state = bootstrap.state;
    let app_state_for_setup = app_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_http::init())
        .manage(app_state.clone())
        .setup(move |app| {
            if let Err(e) = ensure_main_window_visible(app) {
                log::warn!("[setup] failed to show main window: {}", e);
                return Err(Box::new(e));
            }
            log::info!("[setup] launching a2a sidecar");
            let a2a_sidecar = app_state_for_setup.a2a_sidecar.clone();
            let state = app_state_for_setup.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = a2a_sidecar.lock().await.start().await {
                    log::warn!("[setup] a2a sidecar start failed: {}", e);
                    log::warn!("[setup] falling back to embedded a2a server");
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
                            log::info!("[tier] initial maintenance done: upgraded={} downgraded={} - lib.rs:2255", upgraded, downgraded);
                        }
                        Err(e) => {
                            log::warn!("[tier] initial maintenance failed: {} - lib.rs:2258", e);
                        }
                    }
                }
                let interval = std::time::Duration::from_secs(600);
                loop {
                    tokio::time::sleep(interval).await;
                    let store = vs.lock().await;
                    match store.run_tier_maintenance() {
                        Ok((upgraded, downgraded)) => {
                            log::info!("[tier] periodic maintenance done: upgraded={} downgraded={} - lib.rs:2268", upgraded, downgraded);
                        }
                        Err(e) => {
                            log::warn!("[tier] periodic maintenance failed: {} - lib.rs:2271", e);
                        }
                    }
                }
            });
            // Start scheduled task runner
            scheduler_runner::start_scheduler_runner(app_state_for_setup.clone());
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
            run_multi_strategy_agent_preview,
            check_runtime_migration_gate,
            check_controlled_chat_pilot_eligibility,
            check_controlled_pilot_promotion_readiness,
            draft_controlled_chat_migration_plan,
            record_controlled_chat_migration_review_decision,
            get_controlled_chat_migration_review_decision_summary,
            check_controlled_chat_migration_implementation_gate,
            run_controlled_chat_migration_shadow_run,
            record_controlled_chat_migration_shadow_review_decision,
            get_controlled_chat_migration_shadow_review_summary,
            check_controlled_chat_cutover_readiness,
            run_controlled_chat_cutover_candidate,
            record_controlled_chat_cutover_candidate_review_decision,
            get_controlled_chat_cutover_candidate_review_summary,
            check_controlled_chat_cutover_candidate_promotion_readiness,
            record_controlled_pilot_promotion_evidence,
            get_controlled_pilot_promotion_evidence_summary,
            get_default_chat_runtime_boundary_status,
            draft_default_chat_adapter_activation_plan,
            record_default_chat_adapter_activation_review_decision,
            get_default_chat_adapter_activation_review_summary,
            check_default_chat_adapter_activation_implementation_gate,
            get_default_chat_adapter_routing_status,
            check_default_chat_adapter_contract_harness,
            get_default_chat_adapter_ordinary_entry_preflight_status,
            check_default_chat_adapter_narrow_implementation_discussion_gate,
            draft_default_chat_adapter_narrow_implementation_plan,
            record_default_chat_adapter_narrow_implementation_plan_review_decision,
            get_default_chat_adapter_narrow_implementation_plan_review_summary,
            check_default_chat_adapter_narrow_implementation_plan_approval_readiness,
            run_default_chat_adapter_dry_run,
            record_default_chat_adapter_dry_run_review_decision,
            get_default_chat_adapter_dry_run_review_summary,
            check_default_chat_adapter_implementation_readiness,
            run_default_chat_adapter_controlled_preview,
            record_default_chat_adapter_controlled_preview_review_decision,
            get_default_chat_adapter_controlled_preview_review_summary,
            check_default_chat_adapter_controlled_preview_approval_readiness,
            draft_default_chat_adapter_cutover_implementation_plan,
            record_default_chat_adapter_cutover_plan_review_decision,
            get_default_chat_adapter_cutover_plan_review_summary,
            check_default_chat_adapter_cutover_plan_approval_readiness,
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
            list_tool_manifests,
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
            get_proactive_suggestions,
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
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| panic!("Tauri build failed: {}", e))
        .run(|app_handle, event| match event {
            tauri::RunEvent::Ready => {
                if let Err(e) = ensure_main_window_visible(app_handle) {
                    log::warn!("[runtime] failed to show main window: {}", e);
                }
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                if let Err(e) = ensure_main_window_visible(app_handle) {
                    log::warn!("[runtime] failed to show main window: {}", e);
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod hs_runtime_tests {
    use super::*;

    #[test]
    fn default_chat_adapter_cutover_route_guard_defaults_to_disabled_legacy_stream() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        assert_eq!(route.current_mode, "legacy_stream");
        assert!(route.adapter_scaffold_present);
        assert!(!route.controlled_adapter_enabled);
        assert!(!route.automatic_migration_enabled);
        assert_eq!(route.default_send_path, "legacy_stream");
        assert_eq!(route.start_stream_path, "legacy_stream");
        assert!(route.requires_separate_cutover_implementation);
        crate::default_chat_adapter::ensure_default_chat_cutover_harness("send_message", &route)
            .expect("disabled scaffold must allow legacy send path");
    }

    #[test]
    fn default_chat_adapter_cutover_route_guard_fails_closed_for_enabled_route() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.controlled_adapter_enabled = true;
        route.default_send_path = "controlled_adapter".into();

        let error = crate::default_chat_adapter::ensure_default_chat_cutover_harness(
            "send_message",
            &route,
        )
        .expect_err("enabled adapter route must fail closed until cutover is implemented");

        assert!(error.contains("send_message"));
        assert!(error.contains("controlled_adapter_enabled"));
        assert!(error.contains("default_send_path_not_legacy_stream"));
    }

    #[test]
    fn default_chat_adapter_cutover_harness_is_legacy_guarded_and_side_effect_free() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let harness = crate::default_chat_adapter::evaluate_default_chat_adapter_cutover_harness(
            "send_message",
            &route,
        );

        assert!(harness.harness_ready);
        assert!(harness.route_guard_passed);
        assert_eq!(harness.invocation_mode, "legacy_guarded");
        assert_eq!(harness.default_send_path, "legacy_stream");
        assert_eq!(harness.start_stream_path, "legacy_stream");
        assert!(!harness.controlled_adapter_invocation_allowed);
        assert!(!harness.runtime_call_enabled);
        assert!(!harness.model_call_enabled);
        assert!(!harness.tool_call_enabled);
        assert!(!harness.allow_writes);
        assert_eq!(harness.max_tool_calls, 0);
        assert!(!harness.chat_message_saved);
        assert!(!harness.agent_run_recorded);
        assert!(!harness.evidence_recorded);
        assert!(harness.default_chat_path_unchanged);
        assert!(harness.requires_separate_cutover_implementation);
        assert!(harness.blocking_reasons.is_empty());
        crate::default_chat_adapter::ensure_default_chat_cutover_harness("send_message", &route)
            .expect("default route must satisfy the legacy guarded cutover harness");
    }

    #[test]
    fn default_chat_adapter_cutover_harness_fails_closed_for_route_drift() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.current_mode = "controlled_adapter".into();
        route.adapter_scaffold_present = false;
        route.controlled_adapter_enabled = true;
        route.automatic_migration_enabled = true;
        route.default_send_path = "controlled_adapter".into();
        route.start_stream_path = "controlled_adapter".into();
        route.requires_separate_cutover_implementation = false;

        let harness = crate::default_chat_adapter::evaluate_default_chat_adapter_cutover_harness(
            "start_stream_message",
            &route,
        );

        assert!(!harness.harness_ready);
        assert!(!harness.route_guard_passed);
        assert_eq!(harness.invocation_mode, "blocked");
        assert!(!harness.default_chat_path_unchanged);
        assert!(harness
            .blocking_reasons
            .contains(&"adapter_scaffold_missing".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"separate_cutover_implementation_not_required".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_cutover_harness(
            "start_stream_message",
            &route,
        )
        .expect_err("route drift must fail closed before default Chat can cut over");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("adapter_scaffold_missing"));
        assert!(error.contains("separate_cutover_implementation_not_required"));
    }

    #[test]
    fn default_chat_adapter_invocation_plan_selects_legacy_with_controlled_candidate_disabled() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let plan = crate::default_chat_adapter::plan_default_chat_adapter_invocation(
            "send_message",
            &route,
        );

        assert!(plan.plan_ready);
        assert!(plan.harness_ready);
        assert_eq!(plan.selected_adapter_path, "legacy_stream");
        assert_eq!(plan.fallback_adapter_path, "legacy_stream");
        assert_eq!(plan.controlled_adapter_candidate_path, "controlled_adapter");
        assert!(!plan.controlled_adapter_invocation_allowed);
        assert!(!plan.controlled_adapter_executor_attached);
        assert_eq!(plan.send_contract_shape, "send_message_compatible");
        assert_eq!(plan.stream_contract_shape, "stream_message_compatible");
        assert!(!plan.runtime_call_enabled);
        assert!(!plan.model_call_enabled);
        assert!(!plan.tool_call_enabled);
        assert!(!plan.allow_writes);
        assert_eq!(plan.max_tool_calls, 0);
        assert!(!plan.chat_message_saved);
        assert!(!plan.agent_run_recorded);
        assert!(!plan.evidence_recorded);
        assert!(plan.default_chat_path_unchanged);
        assert!(plan.blocking_reasons.is_empty());
        crate::default_chat_adapter::ensure_default_chat_adapter_invocation_plan(
            "send_message",
            &route,
        )
        .expect("default route must keep the invocation plan on legacy stream");
    }

    #[test]
    fn default_chat_adapter_invocation_plan_blocks_when_harness_blocks() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.controlled_adapter_enabled = true;
        route.default_send_path = "controlled_adapter".into();

        let plan = crate::default_chat_adapter::plan_default_chat_adapter_invocation(
            "send_message",
            &route,
        );

        assert!(!plan.plan_ready);
        assert!(!plan.harness_ready);
        assert_eq!(plan.selected_adapter_path, "blocked");
        assert_eq!(plan.fallback_adapter_path, "legacy_stream");
        assert!(!plan.default_chat_path_unchanged);
        assert!(plan
            .blocking_reasons
            .contains(&"cutover_harness_not_ready".to_string()));
        assert!(plan
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(plan
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_adapter_invocation_plan(
            "send_message",
            &route,
        )
        .expect_err("blocked harness must prevent default Chat invocation planning");
        assert!(error.contains("send_message"));
        assert!(error.contains("cutover_harness_not_ready"));
        assert!(error.contains("controlled_adapter_enabled"));
    }

    #[test]
    fn default_chat_adapter_invocation_boundary_requires_legacy_path_only() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let boundary =
            crate::default_chat_adapter::evaluate_default_chat_adapter_invocation_boundary(
                "send_message",
                &route,
            );

        assert!(boundary.boundary_ready);
        assert!(boundary.plan_ready);
        assert_eq!(boundary.selected_adapter_path, "legacy_stream");
        assert_eq!(boundary.required_callsite_path, "legacy_stream");
        assert_eq!(boundary.fallback_adapter_path, "legacy_stream");
        assert_eq!(
            boundary.controlled_adapter_candidate_path,
            "controlled_adapter"
        );
        assert!(boundary.legacy_adapter_invocation_required);
        assert!(!boundary.controlled_adapter_invocation_allowed);
        assert!(!boundary.controlled_adapter_executor_attached);
        assert!(boundary.side_effect_free_before_legacy_entry);
        assert!(!boundary.runtime_call_enabled);
        assert!(!boundary.model_call_enabled);
        assert!(!boundary.tool_call_enabled);
        assert!(!boundary.allow_writes);
        assert_eq!(boundary.max_tool_calls, 0);
        assert!(!boundary.chat_message_saved);
        assert!(!boundary.agent_run_recorded);
        assert!(!boundary.evidence_recorded);
        assert!(boundary.blocking_reasons.is_empty());

        let decision =
            crate::default_chat_adapter::ensure_default_chat_adapter_invocation_boundary(
                "send_message",
                &route,
            )
            .expect("default invocation boundary must select the legacy adapter path");
        assert_eq!(decision.selected_adapter_path, "legacy_stream");
    }

    #[test]
    fn default_chat_adapter_invocation_boundary_blocks_when_plan_blocks() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.start_stream_path = "controlled_adapter".into();

        let boundary =
            crate::default_chat_adapter::evaluate_default_chat_adapter_invocation_boundary(
                "start_stream_message",
                &route,
            );

        assert!(!boundary.boundary_ready);
        assert!(!boundary.plan_ready);
        assert_eq!(boundary.selected_adapter_path, "blocked");
        assert_eq!(boundary.required_callsite_path, "legacy_stream");
        assert!(!boundary.legacy_adapter_invocation_required);
        assert!(boundary
            .blocking_reasons
            .contains(&"invocation_plan_not_ready".to_string()));
        assert!(boundary
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_adapter_invocation_boundary(
            "start_stream_message",
            &route,
        )
        .expect_err("blocked invocation plan must prevent default adapter boundary entry");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("invocation_plan_not_ready"));
        assert!(error.contains("start_stream_path_not_legacy_stream"));
    }

    #[test]
    fn default_chat_adapter_callsite_contract_selects_typed_legacy_paths() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let send_contract =
            crate::default_chat_adapter::evaluate_default_chat_adapter_callsite_contract(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
            );
        let stream_contract =
            crate::default_chat_adapter::evaluate_default_chat_adapter_callsite_contract(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
            );

        assert!(send_contract.contract_ready);
        assert!(send_contract.boundary_ready);
        assert_eq!(send_contract.callsite, "send_message");
        assert_eq!(send_contract.contract_shape, "send_message_compatible");
        assert_eq!(send_contract.actual_callsite_path, "legacy_stream");
        assert_eq!(send_contract.required_callsite_path, "legacy_stream");
        assert_eq!(send_contract.selected_adapter_path, "legacy_stream");
        assert!(!send_contract.controlled_adapter_executor_attached);
        assert!(send_contract.side_effect_free_before_legacy_entry);
        assert!(send_contract.blocking_reasons.is_empty());

        assert!(stream_contract.contract_ready);
        assert!(stream_contract.boundary_ready);
        assert_eq!(stream_contract.callsite, "start_stream_message");
        assert_eq!(stream_contract.contract_shape, "stream_message_compatible");
        assert_eq!(stream_contract.actual_callsite_path, "legacy_stream");
        assert_eq!(stream_contract.required_callsite_path, "legacy_stream");
        assert_eq!(stream_contract.selected_adapter_path, "legacy_stream");
        assert!(!stream_contract.controlled_adapter_executor_attached);
        assert!(stream_contract.side_effect_free_before_legacy_entry);
        assert!(stream_contract.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_callsite_contract_blocks_when_callsite_route_drifts() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.default_send_path = "controlled_adapter".into();

        let contract = crate::default_chat_adapter::evaluate_default_chat_adapter_callsite_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
        );

        assert!(!contract.contract_ready);
        assert!(!contract.boundary_ready);
        assert_eq!(contract.callsite, "send_message");
        assert_eq!(contract.actual_callsite_path, "controlled_adapter");
        assert_eq!(contract.required_callsite_path, "legacy_stream");
        assert_eq!(contract.selected_adapter_path, "blocked");
        assert!(contract
            .blocking_reasons
            .contains(&"invocation_boundary_not_ready".to_string()));
        assert!(contract
            .blocking_reasons
            .contains(&"callsite_path_not_legacy_stream".to_string()));
        assert!(contract
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_adapter_callsite_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
        )
        .expect_err("route drift must block the typed default Chat adapter callsite");
        assert!(error.contains("send_message"));
        assert!(error.contains("callsite_path_not_legacy_stream"));
    }

    #[test]
    fn default_chat_adapter_ordinary_entry_preflight_locks_zero_side_effect_budget() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let send_preflight =
            crate::default_chat_adapter::evaluate_default_chat_adapter_ordinary_entry_preflight(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
            );
        let stream_preflight =
            crate::default_chat_adapter::evaluate_default_chat_adapter_ordinary_entry_preflight(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
            );

        assert!(send_preflight.preflight_ready);
        assert!(send_preflight.contract_ready);
        assert!(send_preflight.legacy_entry_allowed);
        assert_eq!(send_preflight.callsite, "send_message");
        assert_eq!(send_preflight.contract_shape, "send_message_compatible");
        assert_eq!(send_preflight.ordinary_entry_path, "legacy_stream");
        assert_eq!(send_preflight.required_entry_path, "legacy_stream");
        assert!(send_preflight.side_effect_lock_engaged);
        assert!(!send_preflight.default_chat_migration_allowed);
        assert!(!send_preflight.controlled_adapter_executor_attached);
        assert!(!send_preflight.runtime_call_enabled);
        assert!(!send_preflight.model_call_enabled);
        assert!(!send_preflight.tool_call_enabled);
        assert!(!send_preflight.allow_writes);
        assert_eq!(send_preflight.max_tool_calls, 0);
        assert!(!send_preflight.chat_message_saved);
        assert!(!send_preflight.agent_run_recorded);
        assert!(!send_preflight.evidence_recorded);
        assert!(send_preflight.blocking_reasons.is_empty());

        assert!(stream_preflight.preflight_ready);
        assert!(stream_preflight.contract_ready);
        assert!(stream_preflight.legacy_entry_allowed);
        assert_eq!(stream_preflight.callsite, "start_stream_message");
        assert_eq!(stream_preflight.contract_shape, "stream_message_compatible");
        assert_eq!(stream_preflight.ordinary_entry_path, "legacy_stream");
        assert_eq!(stream_preflight.required_entry_path, "legacy_stream");
        assert!(stream_preflight.side_effect_lock_engaged);
        assert!(!stream_preflight.default_chat_migration_allowed);
        assert!(!stream_preflight.controlled_adapter_executor_attached);
        assert!(!stream_preflight.runtime_call_enabled);
        assert!(!stream_preflight.model_call_enabled);
        assert!(!stream_preflight.tool_call_enabled);
        assert!(!stream_preflight.allow_writes);
        assert_eq!(stream_preflight.max_tool_calls, 0);
        assert!(!stream_preflight.chat_message_saved);
        assert!(!stream_preflight.agent_run_recorded);
        assert!(!stream_preflight.evidence_recorded);
        assert!(stream_preflight.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_ordinary_entry_preflight_blocks_route_drift() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.start_stream_path = "controlled_adapter".into();

        let preflight =
            crate::default_chat_adapter::evaluate_default_chat_adapter_ordinary_entry_preflight(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
            );

        assert!(!preflight.preflight_ready);
        assert!(!preflight.contract_ready);
        assert!(!preflight.legacy_entry_allowed);
        assert_eq!(preflight.callsite, "start_stream_message");
        assert_eq!(preflight.ordinary_entry_path, "blocked");
        assert_eq!(preflight.required_entry_path, "legacy_stream");
        assert!(preflight.side_effect_lock_engaged);
        assert!(preflight
            .blocking_reasons
            .contains(&"callsite_contract_not_ready".to_string()));
        assert!(preflight
            .blocking_reasons
            .contains(&"callsite_path_not_legacy_stream".to_string()));
        assert!(preflight
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_adapter_ordinary_entry_preflight(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
            )
            .expect_err("route drift must block the ordinary default Chat adapter entry preflight");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("callsite_contract_not_ready"));
    }

    #[test]
    fn default_chat_adapter_descriptor_is_metadata_safe_and_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input =
            "raw-user-secret prompt-token assistant-output tool-payload lifemodel-memory";

        let descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(descriptor.metadata_safe);
        assert!(!descriptor.contains_raw_content);
        assert_eq!(descriptor.callsite_kind, "send_message");
        assert_eq!(descriptor.contract_shape, "send_message_compatible");
        assert_eq!(descriptor.input_length_bytes, raw_input.len());
        assert_eq!(descriptor.input_length_chars, raw_input.chars().count());
        assert!(descriptor.input_sha256.starts_with("sha256:"));
        assert!(!descriptor.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{descriptor:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "lifemodel-memory",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "descriptor leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_descriptor_keeps_controlled_executor_disabled_unattached() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "metadata only input",
            );

        assert!(descriptor.descriptor_ready);
        assert!(!descriptor.fail_closed);
        assert_eq!(descriptor.route_mode, "legacy_stream");
        assert_eq!(
            descriptor.controlled_adapter_candidate_path,
            "controlled_adapter"
        );
        assert!(!descriptor.controlled_adapter_enabled);
        assert!(!descriptor.controlled_adapter_invocation_allowed);
        assert!(!descriptor.controlled_adapter_executor_enabled);
        assert!(!descriptor.controlled_adapter_executor_attached);
        assert_eq!(
            descriptor.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!descriptor.allow_writes);
        assert_eq!(descriptor.max_tool_calls, 0);
        assert_eq!(descriptor.side_effect_budget.runtime_calls, 0);
        assert_eq!(descriptor.side_effect_budget.model_calls, 0);
        assert_eq!(descriptor.side_effect_budget.tool_calls, 0);
        assert_eq!(descriptor.side_effect_budget.store_writes, 0);
        assert_eq!(descriptor.side_effect_budget.chat_message_writes, 0);
        assert_eq!(descriptor.side_effect_budget.agent_run_writes, 0);
        assert_eq!(descriptor.side_effect_budget.evidence_writes, 0);
        assert_eq!(descriptor.side_effect_budget.proposal_writes, 0);
        assert_eq!(descriptor.side_effect_budget.memory_writes, 0);
        assert_eq!(descriptor.side_effect_budget.life_model_writes, 0);
        assert_eq!(descriptor.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(descriptor.side_effect_budget.external_writes, 0);
    }

    #[test]
    fn default_chat_adapter_descriptor_default_send_stream_routes_remain_legacy_stream() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let send_descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "send input",
            );
        let stream_descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "stream input",
            );

        assert!(send_descriptor.descriptor_ready);
        assert!(stream_descriptor.descriptor_ready);
        assert_eq!(send_descriptor.selected_adapter_path, "legacy_stream");
        assert_eq!(stream_descriptor.selected_adapter_path, "legacy_stream");
        assert_eq!(send_descriptor.default_send_path, "legacy_stream");
        assert_eq!(send_descriptor.start_stream_path, "legacy_stream");
        assert_eq!(stream_descriptor.default_send_path, "legacy_stream");
        assert_eq!(stream_descriptor.start_stream_path, "legacy_stream");
        assert!(!send_descriptor.migration_permission);
        assert!(!stream_descriptor.migration_permission);
        assert!(send_descriptor.blocking_reasons.is_empty());
        assert!(stream_descriptor.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_descriptor_fails_closed_for_route_drift_enabled_and_auto_migration() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.current_mode = "controlled_adapter".into();
        route.controlled_adapter_enabled = true;
        route.automatic_migration_enabled = true;
        route.default_send_path = "controlled_adapter".into();
        route.requires_separate_cutover_implementation = false;

        let descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "blocked input",
            );

        assert!(!descriptor.descriptor_ready);
        assert!(descriptor.fail_closed);
        assert!(!descriptor.route_guard_passed);
        assert_eq!(descriptor.selected_adapter_path, "blocked");
        assert!(!descriptor.migration_permission);
        assert!(!descriptor.controlled_adapter_invocation_allowed);
        assert!(!descriptor.controlled_adapter_executor_enabled);
        assert!(!descriptor.controlled_adapter_executor_attached);
        assert!(descriptor
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(descriptor
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(descriptor
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
        assert!(descriptor
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));
        assert!(descriptor
            .blocking_reasons
            .contains(&"separate_cutover_implementation_not_required".to_string()));
    }

    #[test]
    fn default_chat_adapter_descriptor_mapper_is_side_effect_free_and_stable() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let first = crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            "stable side-effect-free input",
        );
        let second =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "stable side-effect-free input",
            );

        assert_eq!(first, second);
        assert!(first.mapper_side_effect_free);
        assert!(first.side_effect_budget.is_zero());
        assert!(!first.allow_writes);
        assert_eq!(first.max_tool_calls, 0);
        assert!(!first.migration_permission);
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_contract_send_ready_without_migration_permission() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report = crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            "send raw prompt should be hashed only",
        );

        assert!(report.contract_ready);
        assert!(report.descriptor_ready);
        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert!(report.mapper_side_effect_free);
        assert_eq!(report.callsite_kind, "send_message");
        assert_eq!(report.contract_shape, "send_message_compatible");
        assert_eq!(report.selected_adapter_path, "legacy_stream");
        assert_eq!(report.required_callsite_path, "legacy_stream");
        assert_eq!(report.actual_callsite_path, "legacy_stream");
        assert_eq!(report.default_send_path, "legacy_stream");
        assert_eq!(report.start_stream_path, "legacy_stream");
        assert_eq!(
            report.controlled_adapter_candidate_path,
            "controlled_adapter"
        );
        assert!(!report.controlled_adapter_enabled);
        assert!(!report.automatic_migration_enabled);
        assert!(!report.controlled_adapter_invocation_allowed);
        assert!(!report.controlled_adapter_executor_enabled);
        assert!(!report.controlled_adapter_executor_attached);
        assert_eq!(
            report.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!report.allow_writes);
        assert_eq!(report.max_tool_calls, 0);
        assert!(report.side_effect_budget.is_zero());
        assert!(!report.migration_permission);
        assert!(report.default_chat_unchanged);
        assert!(report.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            "send raw prompt should be hashed only",
        )
        .expect("clean send route should produce a ready metadata-only contract report");
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_contract_stream_ready_without_migration_permission()
    {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report = crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &route,
            "stream raw prompt should be hashed only",
        );

        assert!(report.contract_ready);
        assert!(report.descriptor_ready);
        assert_eq!(report.callsite_kind, "start_stream_message");
        assert_eq!(report.contract_shape, "stream_message_compatible");
        assert_eq!(report.selected_adapter_path, "legacy_stream");
        assert_eq!(report.required_callsite_path, "legacy_stream");
        assert_eq!(report.actual_callsite_path, "legacy_stream");
        assert!(!report.controlled_adapter_invocation_allowed);
        assert!(!report.controlled_adapter_executor_enabled);
        assert!(!report.controlled_adapter_executor_attached);
        assert!(!report.migration_permission);
        assert!(report.default_chat_unchanged);
        assert!(report.side_effect_budget.is_zero());
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_contract_fails_closed_for_route_drift() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.current_mode = "controlled_adapter".into();
        route.controlled_adapter_enabled = true;
        route.automatic_migration_enabled = true;
        route.start_stream_path = "controlled_adapter".into();
        route.requires_separate_cutover_implementation = false;

        let report = crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &route,
            "blocked input",
        );

        assert!(!report.contract_ready);
        assert!(!report.descriptor_ready);
        assert!(!report.default_chat_unchanged);
        assert_eq!(report.selected_adapter_path, "blocked");
        assert!(!report.controlled_adapter_invocation_allowed);
        assert!(!report.controlled_adapter_executor_enabled);
        assert!(!report.controlled_adapter_executor_attached);
        assert!(!report.migration_permission);
        assert!(report
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"callsite_path_not_legacy_stream".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"separate_cutover_implementation_not_required".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &route,
            "blocked input",
        )
        .expect_err("route drift must fail closed");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("controlled_adapter_contract_not_ready"));
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_contract_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input =
            "raw-user-secret prompt-token assistant-output tool-payload lifemodel-memory";

        let report = crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            raw_input,
        );

        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert!(report.input_sha256.starts_with("sha256:"));
        assert!(!report.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{report:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "lifemodel-memory",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "contract report leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_send_ready_keeps_legacy_without_migration_permission(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "send raw prompt should be metadata only";
        let contract =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(harness.harness_ready);
        assert_eq!(
            harness.harness_kind,
            "default_chat_controlled_adapter_non_default_invocation_harness"
        );
        assert_eq!(harness.callsite_kind, "send_message");
        assert_eq!(harness.contract_shape, contract.contract_shape);
        assert!(harness.contract_ready);
        assert!(harness.metadata_safe);
        assert!(!harness.contains_raw_content);
        assert!(harness.non_default);
        assert!(harness.ordinary_default_chat_path_unchanged);
        assert_eq!(harness.selected_adapter_path, "legacy_stream");
        assert_eq!(harness.candidate_adapter_path, "controlled_adapter");
        assert!(!harness.controlled_adapter_invocation_allowed);
        assert!(!harness.controlled_adapter_executor_enabled);
        assert!(!harness.controlled_adapter_executor_attached);
        assert_eq!(
            harness.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!harness.allow_writes);
        assert_eq!(harness.max_tool_calls, 0);
        assert!(!harness.migration_permission);
        assert!(harness.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_invocation_harness(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            raw_input,
        )
        .expect("send harness should prove only the non-default invocation shape");
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_stream_ready_keeps_legacy_without_migration_permission(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "stream raw prompt should be metadata only",
            );

        assert!(harness.harness_ready);
        assert_eq!(harness.callsite_kind, "start_stream_message");
        assert_eq!(harness.contract_shape, "stream_message_compatible");
        assert!(harness.contract_ready);
        assert!(harness.metadata_safe);
        assert!(!harness.contains_raw_content);
        assert!(harness.non_default);
        assert!(harness.ordinary_default_chat_path_unchanged);
        assert_eq!(harness.selected_adapter_path, "legacy_stream");
        assert_eq!(harness.candidate_adapter_path, "controlled_adapter");
        assert!(!harness.controlled_adapter_invocation_allowed);
        assert!(!harness.controlled_adapter_executor_enabled);
        assert!(!harness.controlled_adapter_executor_attached);
        assert_eq!(
            harness.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!harness.migration_permission);
        assert!(harness.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_fails_closed_for_route_drift_controlled_adapter_and_auto_migration(
    ) {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.current_mode = "controlled_adapter".into();
        route.controlled_adapter_enabled = true;
        route.automatic_migration_enabled = true;
        route.default_send_path = "controlled_adapter".into();
        route.start_stream_path = "controlled_adapter".into();
        route.requires_separate_cutover_implementation = false;

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "blocked raw input",
            );

        assert!(!harness.harness_ready);
        assert!(!harness.contract_ready);
        assert!(!harness.ordinary_default_chat_path_unchanged);
        assert_eq!(harness.selected_adapter_path, "blocked");
        assert_eq!(harness.candidate_adapter_path, "controlled_adapter");
        assert!(!harness.controlled_adapter_invocation_allowed);
        assert!(!harness.controlled_adapter_executor_enabled);
        assert!(!harness.controlled_adapter_executor_attached);
        assert_eq!(
            harness.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!harness.migration_permission);
        assert!(harness
            .blocking_reasons
            .contains(&"contract_not_ready".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"callsite_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"separate_cutover_implementation_not_required".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "blocked raw input",
            )
            .expect_err("route drift must fail the non-default invocation harness closed");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("controlled_adapter_invocation_harness_not_ready"));
        assert!(error.contains("contract_not_ready"));
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_keeps_executor_unattached_and_side_effect_budget_zero(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "metadata only",
            );

        assert!(harness.harness_ready);
        assert!(!harness.controlled_adapter_executor_enabled);
        assert!(!harness.controlled_adapter_executor_attached);
        assert_eq!(
            harness.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!harness.runtime_call_enabled);
        assert!(!harness.model_call_enabled);
        assert!(!harness.tool_call_enabled);
        assert!(harness.business_write_disabled);
        assert!(!harness.allow_writes);
        assert_eq!(harness.max_tool_calls, 0);
        assert!(harness.side_effect_budget_zero);
        assert_eq!(harness.side_effect_budget.runtime_calls, 0);
        assert_eq!(harness.side_effect_budget.model_calls, 0);
        assert_eq!(harness.side_effect_budget.tool_calls, 0);
        assert_eq!(harness.side_effect_budget.store_writes, 0);
        assert_eq!(harness.side_effect_budget.chat_message_writes, 0);
        assert_eq!(harness.side_effect_budget.agent_run_writes, 0);
        assert_eq!(harness.side_effect_budget.evidence_writes, 0);
        assert_eq!(harness.side_effect_budget.proposal_writes, 0);
        assert_eq!(harness.side_effect_budget.memory_writes, 0);
        assert_eq!(harness.side_effect_budget.life_model_writes, 0);
        assert_eq!(harness.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(harness.side_effect_budget.external_writes, 0);
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input =
            "raw-user-secret prompt-token assistant-output tool-payload lifemodel-memory";

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(harness.metadata_safe);
        assert!(!harness.contains_raw_content);
        assert!(harness.input_sha256.starts_with("sha256:"));
        assert!(!harness.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{harness:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "lifemodel-memory",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "invocation harness leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_send_ready_keeps_selected_legacy_and_no_writes() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "send raw prompt should be metadata only";

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(proof.proof_ready);
        assert!(proof.send_message_result_compatible);
        assert!(proof.descriptor_ready);
        assert!(proof.contract_ready);
        assert!(proof.harness_ready);
        assert!(proof.metadata_safe);
        assert!(!proof.contains_raw_content);
        assert_eq!(
            proof.proof_kind,
            "default_chat_controlled_adapter_send_compatible_proof"
        );
        assert_eq!(proof.callsite_kind, "send_message");
        assert_eq!(proof.contract_shape, "send_message_compatible");
        assert_eq!(proof.selected_adapter_path, "legacy_stream");
        assert_eq!(proof.candidate_adapter_path, "controlled_adapter");
        assert_eq!(proof.required_callsite_path, "legacy_stream");
        assert_eq!(proof.actual_callsite_path, "legacy_stream");
        assert_eq!(proof.default_send_path, "legacy_stream");
        assert_eq!(proof.start_stream_path, "legacy_stream");
        assert!(!proof.controlled_adapter_enabled);
        assert!(!proof.automatic_migration_enabled);
        assert!(!proof.controlled_adapter_invocation_allowed);
        assert!(!proof.controlled_adapter_executor_enabled);
        assert!(!proof.controlled_adapter_executor_attached);
        assert_eq!(
            proof.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!proof.allow_writes);
        assert_eq!(proof.max_tool_calls, 0);
        assert!(proof.side_effect_budget_zero);
        assert!(!proof.runtime_call_enabled);
        assert!(!proof.model_call_enabled);
        assert!(!proof.tool_call_enabled);
        assert!(proof.business_write_disabled);
        assert!(!proof.migration_permission);
        assert!(!proof.chat_message_saved);
        assert!(!proof.agent_run_recorded);
        assert!(!proof.evidence_recorded);
        assert!(!proof.proposal_created);
        assert!(!proof.memory_written);
        assert!(!proof.life_model_written);
        assert!(!proof.external_write_recorded);
        assert!(proof.default_chat_unchanged);
        assert!(proof.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_send_compatible_proof(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            raw_input,
        )
        .expect("clean send route should produce only a send-compatible proof");
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_stream_callsite_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "stream raw prompt should be metadata only",
            );

        assert!(!proof.proof_ready);
        assert!(!proof.send_message_result_compatible);
        assert!(proof.harness_ready);
        assert_eq!(proof.callsite_kind, "start_stream_message");
        assert_eq!(proof.contract_shape, "stream_message_compatible");
        assert_eq!(proof.selected_adapter_path, "legacy_stream");
        assert_eq!(proof.candidate_adapter_path, "controlled_adapter");
        assert!(!proof.controlled_adapter_invocation_allowed);
        assert!(!proof.controlled_adapter_executor_enabled);
        assert!(!proof.migration_permission);
        assert!(proof.default_chat_unchanged);
        assert!(proof
            .blocking_reasons
            .contains(&"callsite_not_send_message".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "stream raw prompt should be metadata only",
            )
            .expect_err("stream callsite must fail closed for send-compatible proof");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("send_compatible_proof_not_ready"));
        assert!(error.contains("callsite_not_send_message"));
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_fails_closed_for_route_drift_controlled_adapter_and_auto_migration(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.default_send_path = "controlled_adapter".into();
        let drift_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &drift_route,
                "blocked raw prompt",
            );

        assert!(!drift_proof.proof_ready);
        assert!(!drift_proof.send_message_result_compatible);
        assert!(!drift_proof.default_chat_unchanged);
        assert_eq!(drift_proof.selected_adapter_path, "blocked");
        assert!(!drift_proof.migration_permission);
        assert!(drift_proof
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(drift_proof
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &enabled_route,
                "blocked raw prompt",
            );

        assert!(!enabled_proof.proof_ready);
        assert!(!enabled_proof.send_message_result_compatible);
        assert!(!enabled_proof.default_chat_unchanged);
        assert!(!enabled_proof.controlled_adapter_invocation_allowed);
        assert!(enabled_proof
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &auto_route,
                "blocked raw prompt",
            );

        assert!(!auto_proof.proof_ready);
        assert!(!auto_proof.send_message_result_compatible);
        assert!(!auto_proof.default_chat_unchanged);
        assert!(!auto_proof.migration_permission);
        assert!(auto_proof
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(proof.metadata_safe);
        assert!(!proof.contains_raw_content);
        assert!(proof.input_sha256.starts_with("sha256:"));
        assert!(!proof.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{proof:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "send-compatible proof leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_side_effect_budget_is_all_zero() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "metadata only",
            );

        assert!(proof.proof_ready);
        assert!(proof.side_effect_budget_zero);
        assert_eq!(proof.side_effect_budget.runtime_calls, 0);
        assert_eq!(proof.side_effect_budget.model_calls, 0);
        assert_eq!(proof.side_effect_budget.tool_calls, 0);
        assert_eq!(proof.side_effect_budget.store_writes, 0);
        assert_eq!(proof.side_effect_budget.chat_message_writes, 0);
        assert_eq!(proof.side_effect_budget.agent_run_writes, 0);
        assert_eq!(proof.side_effect_budget.evidence_writes, 0);
        assert_eq!(proof.side_effect_budget.proposal_writes, 0);
        assert_eq!(proof.side_effect_budget.memory_writes, 0);
        assert_eq!(proof.side_effect_budget.life_model_writes, 0);
        assert_eq!(proof.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(proof.side_effect_budget.external_writes, 0);
        assert!(!proof.runtime_call_enabled);
        assert!(!proof.model_call_enabled);
        assert!(!proof.tool_call_enabled);
        assert!(proof.business_write_disabled);
        assert!(!proof.chat_message_saved);
        assert!(!proof.agent_run_recorded);
        assert!(!proof.evidence_recorded);
        assert!(!proof.proposal_created);
        assert!(!proof.memory_written);
        assert!(!proof.life_model_written);
        assert!(!proof.external_write_recorded);
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_ready_keeps_selected_legacy_without_stream_or_writes(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "stream raw prompt should be metadata only";

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
            );

        assert!(proof.proof_ready);
        assert!(proof.stream_message_compatible);
        assert!(proof.descriptor_ready);
        assert!(proof.contract_ready);
        assert!(proof.harness_ready);
        assert!(proof.metadata_safe);
        assert!(!proof.contains_raw_content);
        assert_eq!(
            proof.proof_kind,
            "default_chat_controlled_adapter_stream_boundary_proof"
        );
        assert_eq!(proof.callsite_kind, "start_stream_message");
        assert_eq!(proof.contract_shape, "stream_message_compatible");
        assert_eq!(proof.selected_adapter_path, "legacy_stream");
        assert_eq!(proof.candidate_adapter_path, "controlled_adapter");
        assert_eq!(proof.required_callsite_path, "legacy_stream");
        assert_eq!(proof.actual_callsite_path, "legacy_stream");
        assert_eq!(proof.default_send_path, "legacy_stream");
        assert_eq!(proof.start_stream_path, "legacy_stream");
        assert!(!proof.controlled_adapter_enabled);
        assert!(!proof.automatic_migration_enabled);
        assert!(!proof.controlled_adapter_invocation_allowed);
        assert!(!proof.stream_started);
        assert!(!proof.stream_events_emitted);
        assert!(!proof.event_channel_opened);
        assert!(!proof.executor_enabled);
        assert!(!proof.executor_attached);
        assert_eq!(proof.executor_state, "disabled_unattached");
        assert!(!proof.allow_writes);
        assert_eq!(proof.max_tool_calls, 0);
        assert!(proof.side_effect_budget_zero);
        assert!(!proof.runtime_call_enabled);
        assert!(!proof.model_call_enabled);
        assert!(!proof.tool_call_enabled);
        assert!(proof.business_write_disabled);
        assert!(!proof.migration_permission);
        assert!(!proof.chat_message_saved);
        assert!(!proof.agent_run_recorded);
        assert!(!proof.evidence_recorded);
        assert!(!proof.proposal_created);
        assert!(!proof.memory_written);
        assert!(!proof.life_model_written);
        assert!(!proof.mcp_audit_written);
        assert!(!proof.external_write_recorded);
        assert!(proof.default_chat_unchanged);
        assert!(proof.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_stream_boundary_proof(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &route,
            raw_input,
        )
        .expect("clean stream route should produce only a stream-compatible boundary proof");
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_send_callsite_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "send raw prompt should be metadata only",
            );

        assert!(!proof.proof_ready);
        assert!(!proof.stream_message_compatible);
        assert!(proof.harness_ready);
        assert_eq!(proof.callsite_kind, "send_message");
        assert_eq!(proof.contract_shape, "send_message_compatible");
        assert_eq!(proof.selected_adapter_path, "legacy_stream");
        assert_eq!(proof.candidate_adapter_path, "controlled_adapter");
        assert!(!proof.controlled_adapter_invocation_allowed);
        assert!(!proof.stream_started);
        assert!(!proof.event_channel_opened);
        assert!(!proof.stream_events_emitted);
        assert!(!proof.migration_permission);
        assert!(proof.default_chat_unchanged);
        assert!(proof
            .blocking_reasons
            .contains(&"callsite_not_start_stream_message".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "send raw prompt should be metadata only",
            )
            .expect_err("send callsite must fail closed for stream boundary proof");
        assert!(error.contains("send_message"));
        assert!(error.contains("stream_boundary_proof_not_ready"));
        assert!(error.contains("callsite_not_start_stream_message"));
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_fails_closed_for_route_drift_controlled_adapter_and_auto_migration(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.start_stream_path = "controlled_adapter".into();
        let drift_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &drift_route,
                "blocked raw prompt",
            );

        assert!(!drift_proof.proof_ready);
        assert!(!drift_proof.stream_message_compatible);
        assert!(!drift_proof.default_chat_unchanged);
        assert_eq!(drift_proof.selected_adapter_path, "blocked");
        assert!(!drift_proof.migration_permission);
        assert!(drift_proof
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(drift_proof
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &enabled_route,
                "blocked raw prompt",
            );

        assert!(!enabled_proof.proof_ready);
        assert!(!enabled_proof.stream_message_compatible);
        assert!(!enabled_proof.default_chat_unchanged);
        assert!(!enabled_proof.controlled_adapter_invocation_allowed);
        assert!(enabled_proof
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &auto_route,
                "blocked raw prompt",
            );

        assert!(!auto_proof.proof_ready);
        assert!(!auto_proof.stream_message_compatible);
        assert!(!auto_proof.default_chat_unchanged);
        assert!(!auto_proof.migration_permission);
        assert!(auto_proof
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
            );

        assert!(proof.metadata_safe);
        assert!(!proof.contains_raw_content);
        assert!(proof.input_sha256.starts_with("sha256:"));
        assert!(!proof.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{proof:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "stream boundary proof leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_side_effect_budget_is_all_zero_without_stream_emit(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "metadata only",
            );

        assert!(proof.proof_ready);
        assert!(proof.side_effect_budget_zero);
        assert_eq!(proof.side_effect_budget.runtime_calls, 0);
        assert_eq!(proof.side_effect_budget.model_calls, 0);
        assert_eq!(proof.side_effect_budget.tool_calls, 0);
        assert_eq!(proof.side_effect_budget.store_writes, 0);
        assert_eq!(proof.side_effect_budget.chat_message_writes, 0);
        assert_eq!(proof.side_effect_budget.agent_run_writes, 0);
        assert_eq!(proof.side_effect_budget.evidence_writes, 0);
        assert_eq!(proof.side_effect_budget.proposal_writes, 0);
        assert_eq!(proof.side_effect_budget.memory_writes, 0);
        assert_eq!(proof.side_effect_budget.life_model_writes, 0);
        assert_eq!(proof.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(proof.side_effect_budget.external_writes, 0);
        assert!(!proof.runtime_call_enabled);
        assert!(!proof.model_call_enabled);
        assert!(!proof.tool_call_enabled);
        assert!(proof.business_write_disabled);
        assert!(!proof.chat_message_saved);
        assert!(!proof.agent_run_recorded);
        assert!(!proof.evidence_recorded);
        assert!(!proof.proposal_created);
        assert!(!proof.memory_written);
        assert!(!proof.life_model_written);
        assert!(!proof.mcp_audit_written);
        assert!(!proof.external_write_recorded);
        assert!(!proof.stream_started);
        assert!(!proof.event_channel_opened);
        assert!(!proof.stream_events_emitted);
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_report_generates_under_clean_legacy_route_without_permissions(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "raw prompt should remain metadata only",
            );

        assert_eq!(
            report.report_kind,
            "default_chat_controlled_adapter_executor_attachment_gate_report"
        );
        assert!(report.gate_report_metadata_ready);
        assert!(report.executor_skeleton_discussion_ready);
        assert!(!report.executor_attachment_allowed);
        assert!(!report.executor_attached);
        assert!(!report.executor_enabled);
        assert!(!report.route_cutover_permission);
        assert!(!report.migration_permission);
        assert!(report.ordinary_default_chat_unchanged);
        assert_eq!(report.selected_adapter_path, "legacy_stream");
        assert!(!report.controlled_adapter_invocation_allowed);
        assert!(report.send_proof_ready);
        assert!(report.stream_boundary_proof_ready);
        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert!(!report.runtime_call_enabled);
        assert!(!report.model_call_enabled);
        assert!(!report.tool_call_enabled);
        assert!(!report.stream_started);
        assert!(!report.event_channel_opened);
        assert!(!report.stream_events_emitted);
        assert!(report.side_effect_budget_zero);
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_reuses_send_stream_and_metadata_safe_layers() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            );

        assert!(report.send_proof_ready);
        assert!(report.send_message_result_compatible);
        assert!(report.stream_boundary_proof_ready);
        assert!(report.stream_message_compatible);
        assert!(report.send_descriptor_ready);
        assert!(report.send_contract_ready);
        assert!(report.send_harness_ready);
        assert!(report.stream_descriptor_ready);
        assert!(report.stream_contract_ready);
        assert!(report.stream_harness_ready);
        assert!(report.w65_w67_metadata_safe);
        assert!(report.w68_send_compatible_proof_ready);
        assert!(report.w69_stream_boundary_proof_ready);
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_fails_closed_for_route_drift_controlled_adapter_and_auto_migration(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.default_send_path = "controlled_adapter".into();
        drift_route.start_stream_path = "controlled_adapter".into();
        let drift_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &drift_route,
                "blocked raw prompt",
            );

        assert!(!drift_report.gate_report_metadata_ready);
        assert!(!drift_report.executor_skeleton_discussion_ready);
        assert!(!drift_report.executor_attachment_allowed);
        assert!(!drift_report.migration_permission);
        assert!(!drift_report.ordinary_default_chat_unchanged);
        assert!(drift_report
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(drift_report
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));
        assert!(drift_report
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));
        assert!(drift_report
            .blocking_reasons
            .contains(&"send_proof_not_ready".to_string()));
        assert!(drift_report
            .blocking_reasons
            .contains(&"stream_boundary_proof_not_ready".to_string()));

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &enabled_route,
                "blocked raw prompt",
            );

        assert!(!enabled_report.gate_report_metadata_ready);
        assert!(!enabled_report.executor_attachment_allowed);
        assert!(!enabled_report.controlled_adapter_invocation_allowed);
        assert!(enabled_report
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &auto_route,
                "blocked raw prompt",
            );

        assert!(!auto_report.gate_report_metadata_ready);
        assert!(!auto_report.executor_attachment_allowed);
        assert!(!auto_report.migration_permission);
        assert!(auto_report
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_blocks_missing_executor_review_and_cutover_authorization(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            );

        for blocker in [
            "executor_implementation_missing",
            "human_review_missing",
            "route_cutover_not_authorized",
        ] {
            assert!(
                report.blocking_reasons.contains(&blocker.to_string()),
                "expected W70 attachment blocker: {blocker}"
            );
        }
        assert!(!report.executor_attachment_allowed);
        assert!(!report.route_cutover_permission);

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            )
            .expect_err("W70 must fail closed because it is not executor attachment");
        assert!(error.contains("executor_attachment_gate_not_ready"));
        assert!(error.contains("executor_implementation_missing"));
        assert!(error.contains("human_review_missing"));
        assert!(error.contains("route_cutover_not_authorized"));
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );

        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert!(report.input_sha256.starts_with("sha256:"));
        assert!(!report.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{report:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "executor attachment gate report leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_side_effect_budget_is_all_zero() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            );

        assert!(report.side_effect_budget_zero);
        assert_eq!(report.side_effect_budget.runtime_calls, 0);
        assert_eq!(report.side_effect_budget.model_calls, 0);
        assert_eq!(report.side_effect_budget.tool_calls, 0);
        assert_eq!(report.side_effect_budget.store_writes, 0);
        assert_eq!(report.side_effect_budget.chat_message_writes, 0);
        assert_eq!(report.side_effect_budget.agent_run_writes, 0);
        assert_eq!(report.side_effect_budget.evidence_writes, 0);
        assert_eq!(report.side_effect_budget.proposal_writes, 0);
        assert_eq!(report.side_effect_budget.memory_writes, 0);
        assert_eq!(report.side_effect_budget.life_model_writes, 0);
        assert_eq!(report.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(report.side_effect_budget.external_writes, 0);
        assert!(!report.runtime_call_enabled);
        assert!(!report.model_call_enabled);
        assert!(!report.tool_call_enabled);
        assert!(!report.chat_message_saved);
        assert!(!report.agent_run_recorded);
        assert!(!report.evidence_recorded);
        assert!(!report.proposal_created);
        assert!(!report.memory_written);
        assert!(!report.life_model_written);
        assert!(!report.mcp_audit_written);
        assert!(!report.external_write_recorded);
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_clean_legacy_route_is_metadata_safe_and_disabled(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw prompt should only become skeleton metadata";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.skeleton_contract_ready);
        assert!(skeleton.metadata_safe);
        assert!(!skeleton.contains_raw_content);
        assert!(skeleton.executor_skeleton_present);
        assert!(!skeleton.executor_enabled);
        assert!(!skeleton.executor_attached);
        assert!(!skeleton.executor_runnable);
        assert!(!skeleton.invocation_allowed);
        assert!(!skeleton.route_cutover_permission);
        assert!(!skeleton.migration_permission);
        assert_eq!(skeleton.selected_adapter_path, "legacy_stream");
        assert!(skeleton.ordinary_default_chat_unchanged);
        assert!(skeleton.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_disabled_executor_skeleton(
            &input,
            &gate,
        )
        .expect("clean legacy metadata should satisfy the disabled skeleton contract");
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_send_shape_returns_send_result_placeholder()
    {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "send raw prompt should not appear in the skeleton";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.skeleton_contract_ready);
        assert_eq!(
            skeleton.output.output_kind,
            "default_chat_controlled_adapter_disabled_executor_skeleton_output"
        );
        assert_eq!(skeleton.output.compatible_shape, "send_message_result");
        assert_eq!(skeleton.output.executor_state, "disabled_unattached");
        assert!(skeleton.output.no_user_visible_output);
        assert!(!skeleton.output.raw_output_present);
        assert!(skeleton.output.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_stream_shape_returns_stream_boundary_placeholder_without_stream_emit(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "stream raw prompt should not open event channels";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
                "stream_boundary",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.skeleton_contract_ready);
        assert_eq!(skeleton.output.compatible_shape, "stream_boundary");
        assert_eq!(skeleton.output.executor_state, "disabled_unattached");
        assert!(skeleton.output.no_user_visible_output);
        assert!(!skeleton.output.raw_output_present);
        assert!(!skeleton.stream_started);
        assert!(!skeleton.event_channel_opened);
        assert!(!skeleton.stream_events_emitted);
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_unknown_shape_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "unknown shape raw prompt should not leak";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "unexpected_shape",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(!skeleton.skeleton_contract_ready);
        assert_eq!(skeleton.output.compatible_shape, "blocked");
        assert!(skeleton
            .blocking_reasons
            .contains(&"unknown_requested_shape".to_string()));
        assert!(skeleton
            .output
            .blocking_reasons
            .contains(&"unknown_requested_shape".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            )
            .expect_err("unknown shape must fail closed");
        assert!(error.contains("disabled_executor_skeleton_not_ready"));
        assert!(error.contains("unknown_requested_shape"));
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_route_drift_controlled_adapter_and_auto_migration_fail_closed(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.default_send_path = "controlled_adapter".into();
        drift_route.start_stream_path = "controlled_adapter".into();
        let drift_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &drift_route,
                "drift raw prompt",
            );
        let drift_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &drift_route,
                "drift raw prompt",
                "send_message_result",
            );
        let drift_skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &drift_input,
                &drift_gate,
            );

        assert!(!drift_skeleton.skeleton_contract_ready);
        assert!(!drift_skeleton.ordinary_default_chat_unchanged);
        assert!(drift_skeleton
            .blocking_reasons
            .contains(&"w70_gate_report_not_metadata_ready".to_string()));
        assert!(drift_skeleton
            .blocking_reasons
            .contains(&"route_drift_from_legacy_stream".to_string()));
        assert!(!drift_skeleton.executor_enabled);
        assert!(!drift_skeleton.executor_attached);
        assert!(!drift_skeleton.invocation_allowed);

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &enabled_route,
                "enabled raw prompt",
            );
        let enabled_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &enabled_route,
                "enabled raw prompt",
                "send_message_result",
            );
        let enabled_skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &enabled_input,
                &enabled_gate,
            );
        assert!(!enabled_skeleton.skeleton_contract_ready);
        assert!(enabled_skeleton
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &auto_route,
                "auto migration raw prompt",
            );
        let auto_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &auto_route,
                "auto migration raw prompt",
                "send_message_result",
            );
        let auto_skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &auto_input,
                &auto_gate,
            );
        assert!(!auto_skeleton.skeleton_contract_ready);
        assert!(!auto_skeleton.migration_permission);
        assert!(auto_skeleton
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.metadata_safe);
        assert!(!skeleton.contains_raw_content);
        assert_eq!(skeleton.input_length_bytes, raw_input.len());
        assert_eq!(skeleton.input_length_chars, raw_input.chars().count());
        assert!(skeleton.input_sha256.starts_with("sha256:"));
        assert!(!skeleton.input_sha256.contains("raw-user-secret"));
        assert!(input.input_sha256.starts_with("sha256:"));
        assert!(!input.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{input:?} {skeleton:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "disabled executor skeleton leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_side_effect_budget_is_all_zero() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "metadata only",
                "send_message_result",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.side_effect_budget_zero);
        assert_eq!(skeleton.side_effect_budget.runtime_calls, 0);
        assert_eq!(skeleton.side_effect_budget.model_calls, 0);
        assert_eq!(skeleton.side_effect_budget.tool_calls, 0);
        assert_eq!(skeleton.side_effect_budget.store_writes, 0);
        assert_eq!(skeleton.side_effect_budget.chat_message_writes, 0);
        assert_eq!(skeleton.side_effect_budget.agent_run_writes, 0);
        assert_eq!(skeleton.side_effect_budget.evidence_writes, 0);
        assert_eq!(skeleton.side_effect_budget.proposal_writes, 0);
        assert_eq!(skeleton.side_effect_budget.memory_writes, 0);
        assert_eq!(skeleton.side_effect_budget.life_model_writes, 0);
        assert_eq!(skeleton.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(skeleton.side_effect_budget.external_writes, 0);
        assert!(!skeleton.runtime_call_enabled);
        assert!(!skeleton.model_call_enabled);
        assert!(!skeleton.tool_call_enabled);
        assert!(skeleton.business_write_disabled);
        assert!(!skeleton.chat_message_saved);
        assert!(!skeleton.agent_run_recorded);
        assert!(!skeleton.evidence_recorded);
        assert!(!skeleton.proposal_created);
        assert!(!skeleton.memory_written);
        assert!(!skeleton.life_model_written);
        assert!(!skeleton.mcp_audit_written);
        assert!(!skeleton.external_write_recorded);
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_clean_send_ready_but_no_run_no_permission_no_write(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "send binding raw prompt must remain metadata only";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert_eq!(
            report.report_kind,
            "default_chat_controlled_adapter_skeleton_binding_integrity_report"
        );
        assert!(report.binding_metadata_ready);
        assert!(report.binding_integrity_ready);
        assert!(report.skeleton_contract_ready);
        assert_eq!(report.callsite_kind, "send_message");
        assert_eq!(report.requested_shape, "send_message_result");
        assert_eq!(
            report.skeleton_output_compatible_shape,
            "send_message_result"
        );
        assert_eq!(report.selected_adapter_path, "legacy_stream");
        assert!(!report.executor_enabled);
        assert!(!report.executor_attached);
        assert!(!report.executor_runnable);
        assert!(!report.invocation_allowed);
        assert!(!report.route_cutover_permission);
        assert!(!report.migration_permission);
        assert!(!report.runtime_call_enabled);
        assert!(!report.model_call_enabled);
        assert!(!report.tool_call_enabled);
        assert!(report.business_write_disabled);
        assert!(report.side_effect_budget_zero);
        assert!(report.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_skeleton_binding_integrity(
            &input,
            &gate,
        )
        .expect("clean send binding metadata should be integrity-ready");
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_clean_stream_ready_but_no_stream_or_event_channel(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "stream binding raw prompt must not emit";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
                "stream_boundary",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(report.binding_integrity_ready);
        assert!(report.binding_metadata_ready);
        assert_eq!(report.callsite_kind, "start_stream_message");
        assert_eq!(report.requested_shape, "stream_boundary");
        assert_eq!(report.skeleton_output_compatible_shape, "stream_boundary");
        assert!(!report.stream_started);
        assert!(!report.event_channel_opened);
        assert!(!report.stream_events_emitted);
        assert!(!report.executor_runnable);
        assert!(!report.invocation_allowed);
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_input_gate_hash_mismatch_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "gate prompt",
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "different input prompt",
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(!report.binding_integrity_ready);
        assert!(!report.binding_metadata_ready);
        assert!(report
            .blocking_reasons
            .contains(&"input_gate_hash_mismatch".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"input_gate_length_mismatch".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            )
            .expect_err("mismatched input/gate metadata must fail closed");
        assert!(error.contains("skeleton_binding_integrity_not_ready"));
        assert!(error.contains("input_gate_hash_mismatch"));
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_route_metadata_mismatch_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "route metadata prompt";
        let mut gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        gate.default_send_path = "controlled_adapter".into();
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(!report.binding_integrity_ready);
        assert!(report
            .blocking_reasons
            .contains(&"route_metadata_mismatch".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"route_drift_from_legacy_stream".to_string()));
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_requested_shape_callsite_mismatch_fails_closed(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "callsite shape prompt";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let send_bound_to_stream =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "stream_boundary",
            );
        let stream_bound_to_send =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        for input in [send_bound_to_stream, stream_bound_to_send] {
            let report =
                crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                    &input,
                    &gate,
                );

            assert!(!report.binding_integrity_ready);
            assert!(!report.binding_metadata_ready);
            assert!(report
                .blocking_reasons
                .contains(&"requested_shape_callsite_mismatch".to_string()));
        }
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_unknown_shape_fails_closed_through_skeleton_and_binding(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "unknown shape binding prompt";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "unknown_future_shape",
            );
        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(!skeleton.skeleton_contract_ready);
        assert!(skeleton
            .blocking_reasons
            .contains(&"unknown_requested_shape".to_string()));

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(!report.binding_integrity_ready);
        assert!(!report.binding_metadata_ready);
        assert_eq!(report.skeleton_output_compatible_shape, "blocked");
        assert!(report
            .blocking_reasons
            .contains(&"skeleton_contract_not_ready".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"unknown_requested_shape".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"output_shape_mismatch".to_string()));
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_route_drift_controlled_adapter_and_auto_migration_fail_closed(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.default_send_path = "controlled_adapter".into();
        drift_route.start_stream_path = "controlled_adapter".into();
        let drift_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &drift_route,
                "drift prompt",
            );
        let drift_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &drift_route,
                "drift prompt",
                "send_message_result",
            );
        let drift_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &drift_input,
                &drift_gate,
            );
        assert!(!drift_report.binding_integrity_ready);
        assert!(drift_report
            .blocking_reasons
            .contains(&"route_drift_from_legacy_stream".to_string()));

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &enabled_route,
                "enabled prompt",
            );
        let enabled_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &enabled_route,
                "enabled prompt",
                "send_message_result",
            );
        let enabled_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &enabled_input,
                &enabled_gate,
            );
        assert!(!enabled_report.binding_integrity_ready);
        assert!(enabled_report
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &auto_route,
                "auto prompt",
            );
        let auto_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &auto_route,
                "auto prompt",
                "send_message_result",
            );
        let auto_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &auto_input,
                &auto_gate,
            );
        assert!(!auto_report.binding_integrity_ready);
        assert!(!auto_report.migration_permission);
        assert!(auto_report
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert_eq!(report.input_length_bytes, raw_input.len());
        assert_eq!(report.input_length_chars, raw_input.chars().count());
        assert!(report.input_sha256.starts_with("sha256:"));
        assert!(!report.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{input:?} {report:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "skeleton binding integrity report leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_side_effect_budget_is_all_zero() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "budget binding prompt";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(report.side_effect_budget_zero);
        assert_eq!(report.side_effect_budget.runtime_calls, 0);
        assert_eq!(report.side_effect_budget.model_calls, 0);
        assert_eq!(report.side_effect_budget.tool_calls, 0);
        assert_eq!(report.side_effect_budget.store_writes, 0);
        assert_eq!(report.side_effect_budget.chat_message_writes, 0);
        assert_eq!(report.side_effect_budget.agent_run_writes, 0);
        assert_eq!(report.side_effect_budget.evidence_writes, 0);
        assert_eq!(report.side_effect_budget.proposal_writes, 0);
        assert_eq!(report.side_effect_budget.memory_writes, 0);
        assert_eq!(report.side_effect_budget.life_model_writes, 0);
        assert_eq!(report.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(report.side_effect_budget.external_writes, 0);
        assert!(!report.runtime_call_enabled);
        assert!(!report.model_call_enabled);
        assert!(!report.tool_call_enabled);
        assert!(report.business_write_disabled);
        assert!(!report.chat_message_saved);
        assert!(!report.agent_run_recorded);
        assert!(!report.evidence_recorded);
        assert!(!report.proposal_created);
        assert!(!report.memory_written);
        assert!(!report.life_model_written);
        assert!(!report.mcp_audit_written);
        assert!(!report.external_write_recorded);
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_is_not_called_by_ordinary_entrypoints() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
        let forbidden_binding_calls = [
            "DefaultChatControlledAdapterSkeletonBindingIntegrityReport",
            "evaluate_default_chat_controlled_adapter_skeleton_binding_integrity",
            "ensure_default_chat_controlled_adapter_skeleton_binding_integrity",
        ];

        for forbidden in forbidden_binding_calls {
            assert!(
                !send_body.contains(forbidden),
                "send_message must not call {forbidden}"
            );
            assert!(
                !stream_body.contains(forbidden),
                "start_stream_message must not call {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_entrypoints_do_not_call_w19_w60_command_surfaces_or_w73_readiness_report_or_w74_invocation(
    ) {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
        let forbidden_maturation_calls = [
            "LifeModelMaturationReadinessInput",
            "LifeModelMaturationReadinessReport",
            "evaluate_lifemodel_maturation_readiness",
            "ensure_lifemodel_maturation_readiness",
            "LifeModelMaturationNonDefaultInvocationInput",
            "LifeModelMaturationNonDefaultInvocationReport",
            "run_lifemodel_maturation_non_default_invocation",
            "ensure_lifemodel_maturation_non_default_invocation",
        ];

        for forbidden in forbidden_maturation_calls {
            assert!(
                !send_body.contains(forbidden),
                "send_message must not call W73/W74 maturation API {forbidden}"
            );
            assert!(
                !stream_body.contains(forbidden),
                "start_stream_message must not call W73/W74 maturation API {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_is_not_called_by_ordinary_entrypoints() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
        let forbidden_skeleton_calls = [
            "DefaultChatControlledAdapterExecutorSkeletonInput",
            "evaluate_default_chat_controlled_adapter_disabled_executor_skeleton",
            "ensure_default_chat_controlled_adapter_disabled_executor_skeleton",
        ];

        for forbidden in forbidden_skeleton_calls {
            assert!(
                !send_body.contains(forbidden),
                "send_message must not call {forbidden}"
            );
            assert!(
                !stream_body.contains(forbidden),
                "start_stream_message must not call {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_is_not_called_by_ordinary_entrypoints() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
        let forbidden_gate_calls = [
            "evaluate_default_chat_controlled_adapter_executor_attachment_gate",
            "ensure_default_chat_controlled_adapter_executor_attachment_gate",
        ];

        for forbidden in forbidden_gate_calls {
            assert!(
                !send_body.contains(forbidden),
                "send_message must not call {forbidden}"
            );
            assert!(
                !stream_body.contains(forbidden),
                "start_stream_message must not call {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_is_not_called_by_ordinary_entrypoints() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
        let forbidden_proof_calls = [
            "evaluate_default_chat_controlled_adapter_stream_boundary_proof",
            "ensure_default_chat_controlled_adapter_stream_boundary_proof",
        ];

        for forbidden in forbidden_proof_calls {
            assert!(
                !send_body.contains(forbidden),
                "send_message must not call {forbidden}"
            );
            assert!(
                !stream_body.contains(forbidden),
                "start_stream_message must not call {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_is_not_called_by_ordinary_entrypoints(
    ) {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
        let forbidden_harness_calls = [
            "evaluate_default_chat_controlled_adapter_invocation_harness",
            "ensure_default_chat_controlled_adapter_invocation_harness",
        ];

        for forbidden in forbidden_harness_calls {
            assert!(
                !send_body.contains(forbidden),
                "send_message must not call {forbidden}"
            );
            assert!(
                !stream_body.contains(forbidden),
                "start_stream_message must not call {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_is_not_called_by_ordinary_entrypoints() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
        let forbidden_proof_calls = [
            "evaluate_default_chat_controlled_adapter_send_compatible_proof",
            "ensure_default_chat_controlled_adapter_send_compatible_proof",
        ];

        for forbidden in forbidden_proof_calls {
            assert!(
                !send_body.contains(forbidden),
                "send_message must not call {forbidden}"
            );
            assert!(
                !stream_body.contains(forbidden),
                "start_stream_message must not call {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_entrypoints_do_not_call_w19_w60_command_surfaces() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
        let forbidden_command_surfaces = [
            "run_multi_strategy_agent_preview",
            "check_runtime_migration_gate",
            "check_controlled_chat_pilot_eligibility",
            "draft_controlled_chat_migration_plan",
            "record_controlled_chat_migration_review_decision",
            "get_controlled_chat_migration_review_decision_summary",
            "check_controlled_chat_migration_implementation_gate",
            "run_controlled_chat_migration_shadow_run",
            "record_controlled_chat_migration_shadow_review_decision",
            "get_controlled_chat_migration_shadow_review_summary",
            "check_controlled_chat_cutover_readiness",
            "run_controlled_chat_cutover_candidate",
            "record_controlled_chat_cutover_candidate_review_decision",
            "get_controlled_chat_cutover_candidate_review_summary",
            "check_controlled_chat_cutover_candidate_promotion_readiness",
            "draft_default_chat_adapter_activation_plan",
            "record_default_chat_adapter_activation_review_decision",
            "get_default_chat_adapter_activation_review_summary",
            "check_default_chat_adapter_activation_implementation_gate",
            "get_default_chat_adapter_routing_status",
            "check_default_chat_adapter_contract_harness",
            "get_default_chat_adapter_ordinary_entry_preflight_status",
            "check_default_chat_adapter_narrow_implementation_discussion_gate",
            "draft_default_chat_adapter_narrow_implementation_plan",
            "record_default_chat_adapter_narrow_implementation_plan_review_decision",
            "get_default_chat_adapter_narrow_implementation_plan_review_summary",
            "check_default_chat_adapter_narrow_implementation_plan_approval_readiness",
            "run_default_chat_adapter_dry_run",
            "record_default_chat_adapter_dry_run_review_decision",
            "get_default_chat_adapter_dry_run_review_summary",
            "check_default_chat_adapter_implementation_readiness",
            "run_default_chat_adapter_controlled_preview",
            "record_default_chat_adapter_controlled_preview_review_decision",
            "get_default_chat_adapter_controlled_preview_review_summary",
            "check_default_chat_adapter_controlled_preview_approval_readiness",
            "draft_default_chat_adapter_cutover_implementation_plan",
            "record_default_chat_adapter_cutover_plan_review_decision",
            "get_default_chat_adapter_cutover_plan_review_summary",
            "check_default_chat_adapter_cutover_plan_approval_readiness",
            "get_default_chat_runtime_boundary_status",
            "record_maturation_proposal_outcome_evidence",
            "evaluate_maturation_proposal_outcome_evidence",
            "MaturationProposalOutcome",
            "MaturationProposalOutcomeEvidenceReport",
            "LowEnergyCollaborationRuleCandidateInput",
            "LowEnergyCollaborationRuleCandidateReport",
            "evaluate_low_energy_collaboration_rule_candidate",
            "propose_low_energy_collaboration_rule_candidate",
            "AcceptedLowEnergyRuleSelectionInput",
            "AcceptedLowEnergyRuleSelectionReport",
            "AcceptedLowEnergyRuleSelectionHSPacketAuditProof",
            "evaluate_accepted_low_energy_rule_selection",
            "ensure_accepted_low_energy_rule_selection",
        ];

        for forbidden in forbidden_command_surfaces {
            assert!(
                !send_body.contains(forbidden),
                "send_message must not call {forbidden}"
            );
            assert!(
                !stream_body.contains(forbidden),
                "start_stream_message must not call {forbidden}"
            );
        }
    }

    #[test]
    fn chat_page_does_not_call_default_adapter_migration_preview_or_review_commands() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root");
        let chat_paths = [
            repo_root.join("frontend/src/pages/ChatPage.tsx"),
            repo_root.join("frontend/src/pages/chat/ChatInputArea.tsx"),
            repo_root.join("frontend/src/pages/chat/useChatStreaming.ts"),
            repo_root.join("frontend/src/pages/chat/useChatContext.ts"),
            repo_root.join("frontend/src/pages/chat/useChatSessions.ts"),
        ];
        let forbidden = [
            "default_chat_adapter",
            "DefaultChatAdapter",
            "checkRuntimeMigrationGate",
            "draftControlledChatMigrationPlan",
            "recordControlledChatMigrationReviewDecision",
            "checkControlledChatMigrationImplementationGate",
            "runControlledChatMigrationShadowRun",
            "runDefaultChatAdapterControlledPreview",
            "draftDefaultChatAdapterActivationPlan",
            "draftDefaultChatAdapterCutoverImplementationPlan",
            "draftDefaultChatAdapterNarrowImplementationPlan",
            "recordDefaultChatAdapter",
            "checkDefaultChatAdapter",
            "getDefaultChatAdapter",
        ];

        for path in chat_paths {
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            for forbidden in forbidden {
                assert!(
                    !source.contains(forbidden),
                    "{} must not call {}",
                    path.display(),
                    forbidden
                );
            }
        }
    }

    fn extract_rust_function_body(source: &str, signature: &str) -> String {
        let signature_start = source.find(signature).expect("function signature exists");
        let brace_start = source[signature_start..]
            .find('{')
            .map(|index| signature_start + index)
            .expect("function body starts");
        let mut depth = 0usize;

        for (offset, ch) in source[brace_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let end = brace_start + offset + ch.len_utf8();
                        return source[brace_start..end].to_string();
                    }
                }
                _ => {}
            }
        }

        panic!("function body closes");
    }

    fn local_only_test_packet() -> openlife_core::agent::RuntimeHSPacket {
        openlife_core::agent::RuntimeHSPacket {
            selected_policies: vec![openlife_core::agent::SelectedPolicyRef {
                policy_id: openlife_core::agent::BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
                reason: "test_sensitive_topic".into(),
                route: Some(openlife_core::agent::ModelRoutePolicy::LocalOnly),
                digest: "digest".into(),
            }],
            selected_heuristics: vec![],
            estimated_tokens: 0,
            audit: openlife_core::agent::HSSelectionAudit {
                agent_task_id: None,
                agent_run_id: Some("run-fallback-hs".into()),
                input_digest: "input-digest".into(),
                selected_policy_ids: vec![
                    openlife_core::agent::BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
                ],
                selected_heuristic_ids: vec![],
                excluded_assets: vec![],
                estimated_tokens: 0,
                token_budget: 128,
            },
        }
    }

    #[tokio::test]
    async fn chat_runtime_hs_packet_uses_sanitized_inputs_and_seeded_stores() {
        let state = crate::test_utils::test_app_state();
        let mut life_model = LifeModel::default();
        life_model.state.health_status.energy_level = 2;

        let task = openlife_core::agent::AgentTask {
            kind: openlife_core::agent::AgentTaskKind::Planning,
            session_id: "session-chat-hs".into(),
            user_text: "raw-health-secret-999 please write a plan".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "raw-health-secret-999 please write a plan".into(),
            }],
            layer: Layer::L1,
        };

        let packet = build_chat_runtime_hs_packet(
            &state,
            &task,
            &life_model,
            "file.write(path, content)",
            Some("run-chat-hs".into()),
        )
        .await
        .unwrap()
        .expect("planning health write task should select HS assets");

        assert!(packet
            .selected_policies
            .iter()
            .any(|policy| policy.route == Some(openlife_core::agent::ModelRoutePolicy::LocalOnly)));
        assert!(packet
            .audit
            .selected_heuristic_ids
            .contains(&openlife_core::agent::BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING.to_string()));
        assert_eq!(packet.audit.agent_run_id.as_deref(), Some("run-chat-hs"));

        let audit_json = serde_json::to_string(&packet.audit).unwrap();
        assert!(!audit_json.contains("raw-health-secret-999"));
        assert!(!audit_json.contains("Reduce planning intensity"));
    }

    #[tokio::test]
    async fn tools_prompt_catalog_alone_does_not_trigger_external_write_proposal_policy() {
        let state = crate::test_utils::test_app_state();
        let life_model = LifeModel::default();
        let task = openlife_core::agent::AgentTask {
            kind: openlife_core::agent::AgentTaskKind::Conversation,
            session_id: "session-read-only-tools-catalog".into(),
            user_text: "Summarize what you know about my current goals without changing anything."
                .into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content:
                    "Summarize what you know about my current goals without changing anything."
                        .into(),
            }],
            layer: Layer::L1,
        };
        let tools_prompt = r#"
            Tools:
            file.write_proposal(path, content)
            email.propose_draft(to, subject, body)
            calendar.propose_event(title, scheduled_at)
            write external_side_effect
        "#;

        let requirements = hs_tool_requirements(&task.user_text, tools_prompt);
        assert!(!requirements
            .iter()
            .any(|requirement| requirement == "write"));
        assert!(!requirements
            .iter()
            .any(|requirement| requirement == "external_side_effect"));

        let packet = build_chat_runtime_hs_packet(&state, &task, &life_model, tools_prompt, None)
            .await
            .unwrap();
        if let Some(packet) = packet {
            assert!(!packet.selected_policies.iter().any(|policy| {
                policy.policy_id
                    == openlife_core::agent::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST
            }));
        }
    }

    #[tokio::test]
    async fn hs_runtime_fallback_local_only_does_not_fall_back_to_cloud_without_ollama() {
        let mut router = openlife_core::agent::ModelRouter::new();
        router.providers.insert(
            "ollama".into(),
            openlife_core::agent::ProviderAvailability {
                provider: "ollama".into(),
                available: false,
                latency_ms: None,
                models: vec![],
                last_checked: chrono::Utc::now(),
                last_error: Some("not running".into()),
                health_is_estimated: false,
            },
        );
        router.providers.insert(
            "openai".into(),
            openlife_core::agent::ProviderAvailability {
                provider: "openai".into(),
                available: true,
                latency_ms: Some(120),
                models: vec!["gpt-4o-mini".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: false,
            },
        );
        let scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "openlife-test-local-model-that-should-not-exist".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test-cloud-key-present".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            true,
        )
        .with_model_router(router);

        let err = generate_non_stream_fallback(
            &scheduler,
            vec![ChatMessage {
                role: "user".into(),
                content: "请处理我的用药记录".into(),
            }],
            &LifeModel::default(),
            "",
            Some(local_only_test_packet()),
        )
        .await
        .unwrap_err();

        assert!(
            err.contains("LocalOnly") || err.contains("local") || err.contains("本地"),
            "unexpected fallback error: {}",
            err
        );
    }

    #[tokio::test]
    async fn hs_runtime_topic_keywords_select_sensitive_local_only_policy() {
        let state = crate::test_utils::test_app_state();
        let life_model = LifeModel::default();
        let text = "最近用药、债务、身份证和分手这些事情让我压力很大";
        let topic = classify_hs_policy_topic(text, "");
        assert_ne!(topic, openlife_core::agent::PolicyTopic::General);

        let task = openlife_core::agent::AgentTask {
            kind: openlife_core::agent::AgentTaskKind::Conversation,
            session_id: "session-sensitive-zh".into(),
            user_text: text.into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: text.into(),
            }],
            layer: Layer::L2,
        };

        let packet = build_chat_runtime_hs_packet(&state, &task, &life_model, "", None)
            .await
            .unwrap()
            .expect("Chinese sensitive keywords should select HS assets");

        assert!(packet
            .selected_policies
            .iter()
            .any(|policy| policy.route == Some(openlife_core::agent::ModelRoutePolicy::LocalOnly)));
    }
}
