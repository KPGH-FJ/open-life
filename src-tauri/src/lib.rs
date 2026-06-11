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
use openlife_core::vectors::{embed_text_with_privacy, MemoryChunk};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use tokio::time::{timeout, Duration};

use crate::legacy_write_convergence::{
    ensure_lifemodel_materializer_caller_restriction, LifeModelMaterializerCallerContext,
    LifeModelMaterializerCallerKind, LifeModelMaterializerCallerPurpose,
};

pub mod a2a_server;
pub mod a2a_sidecar;
pub mod bootstrap;
pub mod commands;
pub(crate) mod default_chat_adapter;
pub mod errors;
pub(crate) mod legacy_write_convergence;
#[allow(dead_code)]
pub(crate) mod main_chat_command_surface_eval;
pub(crate) mod main_chat_context_loader;
#[allow(dead_code)]
pub(crate) mod main_chat_eval_state;
#[allow(dead_code)]
pub(crate) mod main_chat_final_gate;
pub(crate) mod main_chat_generation_support;
pub(crate) mod main_chat_hs_runtime;
pub(crate) mod main_chat_live_provider_harness;
pub(crate) mod main_chat_proposal_support;
pub(crate) mod main_chat_react_execution;
pub(crate) mod main_chat_react_runtime;
pub(crate) mod main_chat_react_tool_selection;
pub(crate) mod main_chat_runtime_support;
pub(crate) mod main_chat_task_controls;
pub mod scheduler_runner;
pub mod state;
pub mod storage;
pub(crate) mod workspace_file_resolver;

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
    cancel_plan_execute_session, check_controlled_chat_cutover_candidate_promotion_readiness,
    check_controlled_chat_cutover_readiness, check_controlled_chat_migration_implementation_gate,
    check_controlled_chat_pilot_eligibility, check_controlled_pilot_promotion_readiness,
    check_default_chat_adapter_activation_implementation_gate,
    check_default_chat_adapter_contract_harness,
    check_default_chat_adapter_controlled_preview_approval_readiness,
    check_default_chat_adapter_cutover_plan_approval_readiness,
    check_default_chat_adapter_implementation_readiness,
    check_default_chat_adapter_narrow_implementation_discussion_gate,
    check_default_chat_adapter_narrow_implementation_plan_approval_readiness,
    check_runtime_migration_gate, create_plan_execute_session,
    draft_controlled_chat_migration_plan, draft_default_chat_adapter_activation_plan,
    draft_default_chat_adapter_cutover_implementation_plan,
    draft_default_chat_adapter_narrow_implementation_plan, execute_plan_execute_step,
    finalize_plan_execute_session, get_controlled_chat_cutover_candidate_review_summary,
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
    get_plan_execute_session, get_react_beta_execution_status,
    get_runtime_strategy_registry_status, list_plan_execute_sessions,
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
    run_main_chat_agent_execution_v1_eval_gate,
    run_main_chat_agent_execution_v1_final_acceptance_gate, run_multi_strategy_agent_preview,
    update_plan_execute_session_draft,
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
    get_skill_runtime_status, grant_tool_permission, list_plugins, list_skills,
    list_tool_permissions, reload_plugins, revoke_tool_permission, run_skill,
};
use commands::feedback::{
    apply_feedback_evolution, generate_evolution_report, get_feedback_summary, log_analytics_event,
    save_feedback,
};
use main_chat_context_loader::compile_main_chat_context;
#[cfg(test)]
use main_chat_context_loader::sanitize_main_chat_selected_skill_id;
use main_chat_generation_support::{
    finalize_chat_agent_run, generate_non_stream_fallback, main_chat_provider_endpoint_kind,
    persist_chat_message_if_needed, persist_vector_memory_for_message, preview_text,
};
#[cfg(test)]
use main_chat_hs_runtime::hs_tool_requirements;
pub(crate) use main_chat_hs_runtime::{
    build_chat_runtime_hs_packet, classify_hs_policy_topic, included_life_model_sections,
};
use main_chat_proposal_support::{
    attach_main_chat_tool_permission_proposal_metadata, create_main_chat_agent_proposal,
};
use main_chat_react_execution::execute_main_chat_react_action_with_executor;
use main_chat_react_runtime::{
    agent_actions_to_tool_call_results, main_chat_permission_blocker_reason,
    synthesize_main_chat_react_follow_up, tool_call_from_action,
    try_run_main_chat_react_agent_loop,
};
use main_chat_react_tool_selection::build_main_chat_react_action_plan;
#[cfg(test)]
use main_chat_react_tool_selection::main_chat_workspace_file_target;
use main_chat_runtime_support::{
    append_main_chat_agent_transcript, append_main_chat_direct_answer_contract_transcript,
    complete_main_chat_agent_turn_session, enqueue_main_chat_agent_action, fail_main_chat_action,
    start_main_chat_agent_turn, transition_main_chat_action, MainChatAgentTurn,
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
use main_chat_task_controls::{
    cancel_main_chat_agent_task, get_main_chat_agent_task_state, resume_main_chat_agent_task,
    retry_main_chat_agent_action,
};
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
    pub react_trace: Option<openlife_core::agent::ReactActionTraceEnvelope>,
}

#[derive(serde::Serialize)]
pub struct SendMessageResult {
    pub reply: String,
    pub reasoning_trace: openlife_core::agent::ReasoningTrace,
    pub tool_calls: Vec<ToolCallResult>,
    pub run_id: Option<String>,
    pub agent_ingress: Option<openlife_core::agent::main_chat_agent_v1::AgentIngressDecision>,
    pub execution_transcript:
        Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
    pub legacy_fallback_used: bool,
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

pub(crate) async fn persist_life_model(
    state: &Arc<AppState>,
    mut life_model: LifeModel,
    create_daily_snapshot: bool,
    caller_context: LifeModelMaterializerCallerContext,
) -> Result<LifeModel, String> {
    ensure_lifemodel_materializer_caller_restriction(&caller_context, "persist_life_model")?;
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

#[allow(clippy::too_many_arguments)]
async fn try_run_main_chat_agent_strategy(
    session_id: &str,
    user_msg: Option<&ChatMessage>,
    messages_for_generation: &[ChatMessage],
    life_model: &LifeModel,
    context_summary: openlife_core::agent::ContextSummary,
    embed_err: Option<String>,
    auto_checkin_msg: Option<String>,
    main_chat_agent_turn: &MainChatAgentTurn,
    state: &Arc<AppState>,
    privacy_engine: &PrivacyEngine,
    privacy_map: &HashMap<String, String>,
    existing_agent_run: Option<openlife_core::agent::AgentRun>,
    selected_skill_id: Option<&str>,
) -> Result<Option<SendMessageResult>, String> {
    use openlife_core::agent::main_chat_agent_v1::{
        ExecutionQueueStatus, ExecutionTranscriptEntryKind, MainChatAgentStrategy,
    };

    let strategy = main_chat_agent_turn.decision.selected_strategy;
    let user_text = user_msg
        .map(|message| message.content.as_str())
        .unwrap_or("");
    let task_session_id = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
        .ok_or_else(|| "Main Chat Agent task session missing".to_string())?;
    let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::Plan,
            "Main Chat Agent strategy execution started.",
            serde_json::json!({
                "selectedStrategy": strategy.as_str(),
                "policyReasonCode": main_chat_agent_turn.decision.privacy_risk.policy_reason_code,
                "silentWritesAllowed": false,
            }),
        )
        .await,
    );

    let compiled_context = compile_main_chat_context(
        state,
        &main_chat_agent_turn.decision,
        task_session_id,
        user_text,
        selected_skill_id,
    )
    .await;
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::Observation,
            "Bounded context was selected for this strategy.",
            serde_json::json!({
                "contextSnapshotRef": compiled_context.context_snapshot_ref,
                "selectedSourceCount": compiled_context.selected_sources.len(),
                "totalTokenEstimate": compiled_context.total_token_estimate,
                "rawLifeModelYamlIncluded": compiled_context.raw_life_model_yaml_included,
                "rawTopKMemoryTrusted": compiled_context.raw_topk_memory_trusted,
                "workspacePolicyOverrideBlocked": compiled_context.workspace_policy_override_blocked,
                "selectedSkillInstructionLoaded": compiled_context.selected_skill_instruction_loaded,
                "sources": compiled_context.selected_sources,
            }),
        )
        .await,
    );

    let mut tool_calls = Vec::new();
    let mut reply: String;
    let mut pending_blockers = Vec::new();
    let mut completed = false;
    let mut hard_blocked = false;
    let mut model_route_override = None;
    let mut direct_answer_generation_metadata: Option<serde_json::Value> = None;

    match strategy {
        MainChatAgentStrategy::DirectAnswer => {
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Plan,
                    "DirectAnswer prompt contract was prepared.",
                    serde_json::json!({
                        "selectedStrategy": strategy.as_str(),
                        "contextSnapshotRef": compiled_context.context_snapshot_ref,
                        "toolExecutionAllowed": false,
                        "writeExecutionAllowed": false,
                        "silentWritesAllowed": false,
                        "localOnlyRequired": main_chat_agent_turn.decision.privacy_risk.local_only_required,
                    }),
                )
                .await,
            );
            let scheduler = state.scheduler.lock().await.clone();
            let task = openlife_core::agent::AgentTask {
                kind: openlife_core::agent::AgentTaskKind::Conversation,
                session_id: session_id.to_string(),
                user_text: user_text.to_string(),
                messages: messages_for_generation.to_vec(),
                layer: Layer::L2,
            };
            let hs_packet =
                build_chat_runtime_hs_packet(state, &task, life_model, "", None).await?;
            let direct_answer_model_route = scheduler.preview_chat_route(None).await;
            let scripted_provider_response = scheduler.scripted_generation_response.is_some();
            let provider_endpoint_kind =
                main_chat_provider_endpoint_kind(&scheduler, scripted_provider_response);
            let live_provider_invoked = !scripted_provider_response
                && direct_answer_model_route.provider != "none"
                && direct_answer_model_route.route_type == "cloud";
            let generated = generate_non_stream_fallback(
                &scheduler,
                messages_for_generation.to_vec(),
                life_model,
                "",
                hs_packet.clone(),
            )
            .await?;
            reply = privacy_engine.reconstruct(&generated, privacy_map);
            model_route_override = Some(direct_answer_model_route.clone());
            let generation_metadata = serde_json::json!({
                "hsPacketSelected": hs_packet.is_some(),
                "toolCallCount": 0,
                "directWritesExecuted": false,
                "legacyFallbackUsed": false,
                "modelGenerated": true,
                "schedulerGenerationCalled": true,
                "providerGenerationPath": "main_chat_direct_answer_scheduler",
                "provider": direct_answer_model_route.provider,
                "model": direct_answer_model_route.model,
                "routeType": direct_answer_model_route.route_type,
                "routeReason": direct_answer_model_route.reason,
                "providerHealthEstimated": direct_answer_model_route.provider_health_is_estimated,
                "scriptedProviderResponse": scripted_provider_response,
                "liveProviderInvoked": live_provider_invoked,
                "providerEndpointKind": provider_endpoint_kind,
                "localProviderHttpHarness": live_provider_invoked
                    && provider_endpoint_kind == "local_test_http",
                "externalLiveProviderEvalPreflighted": false,
            });
            direct_answer_generation_metadata = Some(generation_metadata.clone());
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Observation,
                    "DirectAnswer generated a model response without tools or writes.",
                    generation_metadata,
                )
                .await,
            );
            completed = true;
        }
        MainChatAgentStrategy::ReActToolExecution => {
            let action_plan = build_main_chat_react_action_plan(session_id, user_text)?;
            let queued = enqueue_main_chat_agent_action(
                state,
                task_session_id,
                &action_plan.queue_action_type,
                &action_plan.description,
                &mut execution_transcript,
            )
            .await?;
            if queued.policy.execution_allowed {
                transition_main_chat_action(
                    state,
                    &queued.id,
                    ExecutionQueueStatus::Executing,
                    None,
                )
                .await?;
                let agent_loop_attempt = try_run_main_chat_react_agent_loop(
                    state,
                    task_session_id,
                    session_id,
                    user_text,
                    messages_for_generation,
                    life_model,
                    privacy_engine,
                    privacy_map,
                    &action_plan,
                    main_chat_agent_turn
                        .decision
                        .privacy_risk
                        .local_only_required,
                )
                .await?;
                execution_transcript.extend(agent_loop_attempt.transcript_entries);
                if let Some(queue_status) = agent_loop_attempt.queue_status.clone() {
                    match queue_status {
                        ExecutionQueueStatus::Completed => {
                            transition_main_chat_action(
                                state,
                                &queued.id,
                                ExecutionQueueStatus::Observed,
                                Some(agent_loop_attempt.metadata.clone()),
                            )
                            .await?;
                            transition_main_chat_action(
                                state,
                                &queued.id,
                                ExecutionQueueStatus::Completed,
                                None,
                            )
                            .await?;
                            completed = true;
                        }
                        ExecutionQueueStatus::PendingPermission => {
                            let blocker_reason = agent_loop_attempt
                                .blocker_reason
                                .clone()
                                .unwrap_or_else(|| "tool_permission_required".into());
                            let permission_blocker =
                                main_chat_permission_blocker_reason(&action_plan, &blocker_reason);
                            let (metadata, permission_transcript) =
                                attach_main_chat_tool_permission_proposal_metadata(
                                    state,
                                    task_session_id,
                                    &action_plan,
                                    Some(&permission_blocker),
                                    agent_loop_attempt.metadata.clone(),
                                )
                                .await?;
                            execution_transcript.extend(permission_transcript);
                            transition_main_chat_action(
                                state,
                                &queued.id,
                                ExecutionQueueStatus::PendingPermission,
                                Some(metadata),
                            )
                            .await?;
                            pending_blockers.push(permission_blocker);
                        }
                        ExecutionQueueStatus::Failed => {
                            if agent_loop_attempt
                                .metadata
                                .get("agentLoopActionStatus")
                                .and_then(serde_json::Value::as_str)
                                == Some("blocked")
                            {
                                hard_blocked = true;
                            }
                            let blocker_reason = agent_loop_attempt
                                .blocker_reason
                                .clone()
                                .unwrap_or_else(|| "agent_loop_action_failed".into());
                            pending_blockers.push(blocker_reason.clone());
                            fail_main_chat_action(
                                state,
                                &queued.id,
                                &blocker_reason,
                                agent_loop_attempt.metadata.clone(),
                            )
                            .await?;
                        }
                        ExecutionQueueStatus::Planned
                        | ExecutionQueueStatus::Executing
                        | ExecutionQueueStatus::Observed
                        | ExecutionQueueStatus::Retrying
                        | ExecutionQueueStatus::Cancelled => {
                            fail_main_chat_action(
                                state,
                                &queued.id,
                                "agent_loop_action_incomplete",
                                agent_loop_attempt.metadata.clone(),
                            )
                            .await?;
                        }
                    }
                    if let Some(model_route) = agent_loop_attempt.model_route {
                        model_route_override = Some(model_route);
                    }
                    tool_calls.extend(agent_loop_attempt.tool_calls);
                    reply = agent_loop_attempt.reply.unwrap_or_else(|| {
                        "The governed ReAct AgentLoop completed without a final response.".into()
                    });
                } else {
                    match execute_main_chat_react_action_with_executor(
                        state,
                        &action_plan,
                        main_chat_agent_turn
                            .decision
                            .privacy_risk
                            .local_only_required,
                    )
                    .await
                    {
                        Ok(observation) => {
                            let mut observation_metadata = observation.metadata.clone();
                            if observation.executor_status
                                == openlife_core::agent::ActionExecutionStatus::Succeeded
                            {
                                transition_main_chat_action(
                                    state,
                                    &queued.id,
                                    ExecutionQueueStatus::Observed,
                                    Some(observation_metadata.clone()),
                                )
                                .await?;
                                transition_main_chat_action(
                                    state,
                                    &queued.id,
                                    ExecutionQueueStatus::Completed,
                                    None,
                                )
                                .await?;
                                completed = true;
                            } else if observation.executor_status
                                == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation
                            {
                                let blocker_reason = observation
                                    .blocker_reason
                                    .clone()
                                    .unwrap_or_else(|| "tool_permission_required".into());
                                let permission_blocker = main_chat_permission_blocker_reason(
                                    &action_plan,
                                    &blocker_reason,
                                );
                                let (permission_metadata, permission_transcript) =
                                    attach_main_chat_tool_permission_proposal_metadata(
                                        state,
                                        task_session_id,
                                        &action_plan,
                                        Some(&permission_blocker),
                                        observation_metadata.clone(),
                                    )
                                    .await?;
                                observation_metadata = permission_metadata;
                                execution_transcript.extend(permission_transcript);
                                transition_main_chat_action(
                                    state,
                                    &queued.id,
                                    ExecutionQueueStatus::PendingPermission,
                                    Some(observation_metadata.clone()),
                                )
                                .await?;
                                pending_blockers.push(permission_blocker);
                            } else {
                                if observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::Blocked
                                {
                                    hard_blocked = true;
                                    pending_blockers.push(
                                        observation
                                            .blocker_reason
                                            .clone()
                                            .unwrap_or_else(|| "read_action_blocked".into()),
                                    );
                                }
                                fail_main_chat_action(
                                    state,
                                    &queued.id,
                                    observation
                                        .blocker_reason
                                        .as_deref()
                                        .unwrap_or("ActionExecutor read action failed."),
                                    observation_metadata.clone(),
                                )
                                .await?;
                            }
                            execution_transcript.extend(
                                append_main_chat_agent_transcript(
                                    state,
                                    Some(task_session_id),
                                    ExecutionTranscriptEntryKind::Observation,
                                    observation.summary.clone(),
                                    observation_metadata.clone(),
                                )
                                .await,
                            );
                            tool_calls.push(tool_call_from_action(
                                &action_plan.target,
                                &queued.id,
                                observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::Succeeded,
                                if observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::Succeeded
                                {
                                    Some(observation.output_preview.clone())
                                } else {
                                    None
                                },
                                if observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::Succeeded
                                {
                                    None
                                } else {
                                    observation.blocker_reason.clone()
                                },
                                match observation.executor_status {
                                    openlife_core::agent::ActionExecutionStatus::Succeeded => {
                                        ToolCallStatus::Success
                                    }
                                    openlife_core::agent::ActionExecutionStatus::NeedsConfirmation
                                    | openlife_core::agent::ActionExecutionStatus::Blocked => {
                                        ToolCallStatus::Blocked
                                    }
                                    openlife_core::agent::ActionExecutionStatus::Failed => {
                                        ToolCallStatus::Error
                                    }
                                },
                                observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
                            ));
                            if observation.executor_status
                                == openlife_core::agent::ActionExecutionStatus::Succeeded
                            {
                                let follow_up = synthesize_main_chat_react_follow_up(
                                    state,
                                    task_session_id,
                                    session_id,
                                    user_text,
                                    messages_for_generation,
                                    life_model,
                                    privacy_engine,
                                    privacy_map,
                                    &observation,
                                )
                                .await?;
                                if let Some(model_route) = follow_up.model_route {
                                    model_route_override = Some(model_route);
                                }
                                execution_transcript.extend(follow_up.transcript_entries);
                                reply = follow_up.reply;
                            } else {
                                reply = observation.final_answer.clone();
                            }
                        }
                        Err(error) => {
                            fail_main_chat_action(
                                state,
                                &queued.id,
                                &error,
                                serde_json::json!({
                                    "error": error,
                                    "actionExecutorBacked": true,
                                    "failSoft": true,
                                }),
                            )
                            .await?;
                            execution_transcript.extend(
                                append_main_chat_agent_transcript(
                                    state,
                                    Some(task_session_id),
                                    ExecutionTranscriptEntryKind::Error,
                                    "Read-only tool action could not complete.",
                                    serde_json::json!({
                                        "actionId": queued.id,
                                        "error": error,
                                        "retryAvailable": true,
                                    }),
                                )
                                .await,
                            );
                            tool_calls.push(tool_call_from_action(
                                &action_plan.target,
                                &queued.id,
                                false,
                                None,
                                Some(error.clone()),
                                ToolCallStatus::Error,
                                false,
                            ));
                            reply = format!(
                                "I could not complete that read-only action yet. Blocker: {error}\n\nYou can retry or narrow the request."
                            );
                        }
                    }
                }
            } else {
                pending_blockers.push(queued.policy.reason_code.clone());
                reply = "This action needs review before it can run.".into();
            }
        }
        MainChatAgentStrategy::PlanExecute => {
            let queued = enqueue_main_chat_agent_action(
                state,
                task_session_id,
                "plan_execute.create_session",
                "Create a governed PlanExecute draft session from Main Chat.",
                &mut execution_transcript,
            )
            .await?;
            transition_main_chat_action(
                state,
                &queued.id,
                ExecutionQueueStatus::Executing,
                Some(serde_json::json!({
                    "executor": "plan_execute.create_session",
                    "directWritesExecuted": false,
                })),
            )
            .await?;
            let plan_session = commands::agent_runtime::create_plan_execute_session_with_state(
                commands::agent_runtime::CreatePlanExecuteSessionInput {
                    scenario_id: Some("weekly_planning".into()),
                    source_chat_session_id: Some(session_id.to_string()),
                    max_steps: Some(5),
                },
                state,
            )
            .await?;
            transition_main_chat_action(
                state,
                &queued.id,
                ExecutionQueueStatus::Observed,
                Some(serde_json::json!({
                    "planExecuteSessionId": plan_session.session_id,
                    "stepCount": plan_session.steps.len(),
                })),
            )
            .await?;
            transition_main_chat_action(state, &queued.id, ExecutionQueueStatus::Completed, None)
                .await?;
            if let Some(ref store_arc) = state.main_chat_agent_session_store {
                let store = store_arc.lock().await;
                if let Err(err) = store.update_plan_summary(
                    task_session_id,
                    Some(format!(
                        "PlanExecute draft {} has {} steps.",
                        plan_session.session_id,
                        plan_session.steps.len()
                    )),
                ) {
                    log::warn!("[MainChatAgent] update plan summary failed: {}", err);
                }
            }
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Plan,
                    "Governed PlanExecute draft session was created.",
                    serde_json::json!({
                        "planExecuteSessionId": plan_session.session_id,
                        "status": plan_session.status,
                        "stepCount": plan_session.steps.len(),
                        "directWritesExecuted": false,
                    }),
                )
                .await,
            );
            tool_calls.push(tool_call_from_action(
                "plan_execute.create_session",
                &queued.id,
                true,
                Some(format!(
                    "PlanExecute draft with {} steps",
                    plan_session.steps.len()
                )),
                None,
                ToolCallStatus::Success,
                false,
            ));
            reply = format!(
                "I created a governed draft plan with {} steps. It is not saved as accepted truth yet; review or adjust it before executing any write-like step.",
                plan_session.steps.len()
            );
            completed = true;
        }
        MainChatAgentStrategy::MemoryProposal | MainChatAgentStrategy::LifeModelProposal => {
            let proposal =
                create_main_chat_agent_proposal(state, task_session_id, strategy, user_text)
                    .await?;
            pending_blockers.push(format!("proposal:{}", proposal.id));
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::ProposalRequest,
                    "A governed proposal was created for review.",
                    serde_json::json!({
                        "proposalId": proposal.id,
                        "proposalType": proposal.proposal_type,
                        "riskLevel": proposal.risk_level,
                        "status": proposal.status,
                        "directWritesExecuted": false,
                    }),
                )
                .await,
            );
            reply = match strategy {
                MainChatAgentStrategy::MemoryProposal => {
                    "I created a Memory proposal for review. I did not write it into long-term memory.".into()
                }
                _ => {
                    "I created a LifeModel proposal for review. I did not update accepted LifeModel truth.".into()
                }
            };
        }
        MainChatAgentStrategy::ReviewMaturation => {
            let queued = enqueue_main_chat_agent_action(
                state,
                task_session_id,
                "review.maturation_read",
                "Review metadata-safe maturation/evidence surfaces.",
                &mut execution_transcript,
            )
            .await?;
            transition_main_chat_action(
                state,
                &queued.id,
                ExecutionQueueStatus::Observed,
                Some(serde_json::json!({
                    "reviewSurface": "metadata_safe",
                    "directWritesExecuted": false,
                })),
            )
            .await?;
            transition_main_chat_action(state, &queued.id, ExecutionQueueStatus::Completed, None)
                .await?;
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Observation,
                    "Review request completed as a metadata-safe summary.",
                    serde_json::json!({
                        "reviewedAcceptedTruth": false,
                        "proposalCreated": false,
                        "directWritesExecuted": false,
                    }),
                )
                .await,
            );
            reply = "I can review the relevant metadata-safe trace, evidence, and proposal surfaces here. I did not promote any observation into accepted guidance or LifeModel truth.".into();
            completed = true;
        }
        MainChatAgentStrategy::BlockedConfirmation => {
            let queued = enqueue_main_chat_agent_action(
                state,
                task_session_id,
                "external.write",
                "External or sensitive write requested from Main Chat.",
                &mut execution_transcript,
            )
            .await?;
            pending_blockers.push(queued.policy.reason_code.clone());
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::PermissionRequest,
                    "External or sensitive write is blocked pending explicit confirmation and provider support.",
                    serde_json::json!({
                        "actionId": queued.id,
                        "policyLevel": queued.policy.level.as_str(),
                        "requiresConfirmation": queued.policy.requires_confirmation,
                        "externalWritesExecuted": false,
                    }),
                )
                .await,
            );
            tool_calls.push(tool_call_from_action(
                "external.write",
                &queued.id,
                false,
                None,
                Some("External or sensitive write requires explicit confirmation and is not executed in Main Chat v1.".into()),
                ToolCallStatus::Blocked,
                true,
            ));
            reply = "I cannot send or write that directly. It requires explicit confirmation and a governed provider path; no external write was executed.".into();
        }
    }

    if !pending_blockers.is_empty() {
        if let Some(ref store_arc) = state.main_chat_agent_session_store {
            let store = store_arc.lock().await;
            if let Err(err) = store.set_pending_blockers(task_session_id, pending_blockers.clone())
            {
                log::warn!("[MainChatAgent] set blockers failed: {}", err);
            }
            let transition_result = if hard_blocked {
                store.block_session(task_session_id, "Main Chat Agent v1 blocked by governance.")
            } else {
                store.mark_waiting_permission(task_session_id)
            };
            if let Err(err) = transition_result {
                log::warn!("[MainChatAgent] mark blocked/waiting failed: {}", err);
            }
        }
    } else if completed {
        if let Some(ref store_arc) = state.main_chat_agent_session_store {
            let store = store_arc.lock().await;
            if let Err(err) =
                store.complete_session(task_session_id, "Main Chat Agent v1 completed.")
            {
                log::warn!("[MainChatAgent] complete session failed: {}", err);
            }
        }
    }

    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let user_input_text = user_msg
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let mut agent_run = existing_agent_run.unwrap_or_else(|| {
        openlife_core::agent::AgentRun::new_chat_run(session_id, &user_input_text)
    });
    agent_run.reasoning_strategy = Some(format!("main_chat_agent_v1_{}", strategy.as_str()));
    let model_route =
        model_route_override.unwrap_or_else(|| openlife_core::agent::ModelRouteTrace {
            provider: "openlife".to_string(),
            model: "main_chat_agent_v1".to_string(),
            route_type: "agent_runtime".to_string(),
            prefer_local: main_chat_agent_turn
                .decision
                .privacy_risk
                .local_only_required,
            local_model: "".to_string(),
            reason: format!("strategy={}", strategy.as_str()),
            privacy_level: if main_chat_agent_turn
                .decision
                .privacy_risk
                .local_only_required
            {
                openlife_core::agent::types::RedactionLevel::LocalOnly
            } else {
                openlife_core::agent::types::RedactionLevel::None
            },
            latency_ms: None,
            retry_count: 0,
            fallback_reason: None,
            provider_health_is_estimated: Some(false),
        });
    let generation_result = if strategy == MainChatAgentStrategy::DirectAnswer {
        let mut generation_result = serde_json::json!({
            "text": reply,
            "mainChatAgentV1": true,
            "selectedStrategy": strategy.as_str(),
            "legacyFallbackUsed": false,
            "modelGenerated": true,
            "schedulerGenerationCalled": true,
            "providerGenerationPath": "main_chat_direct_answer_scheduler",
            "provider": model_route.provider,
            "model": model_route.model,
            "routeType": model_route.route_type,
            "routeReason": model_route.reason,
            "providerHealthEstimated": model_route.provider_health_is_estimated,
        });
        if let (Some(serde_json::Value::Object(extra)), Some(target)) = (
            direct_answer_generation_metadata,
            generation_result.as_object_mut(),
        ) {
            for (key, value) in extra {
                target.insert(key, value);
            }
        }
        generation_result
    } else {
        serde_json::json!({
            "text": reply,
            "mainChatAgentV1": true,
            "selectedStrategy": strategy.as_str(),
            "legacyFallbackUsed": false,
        })
    };
    agent_run.complete(&preview_text(&reply, 200), model_route, context_summary);
    let mut reasoning_trace = ReasoningTrace {
        generation_result: Some(generation_result),
        ..Default::default()
    };
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }
    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };
    finalize_chat_agent_run(
        session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        life_model,
        state,
    )
    .await?;
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::FinalResult,
            "Main Chat Agent v1 response was delivered.",
            serde_json::json!({
                "runId": agent_run.id,
                "legacyFallbackUsed": false,
                "pendingBlockerCount": pending_blockers.len(),
            }),
        )
        .await,
    );

    Ok(Some(SendMessageResult {
        reply,
        reasoning_trace,
        tool_calls,
        run_id: Some(agent_run.id),
        agent_ingress: Some(main_chat_agent_turn.decision.clone()),
        execution_transcript,
        legacy_fallback_used: false,
    }))
}

/// Shared preprocessing for chat commands:
/// saves user message, loads model/tools/config, applies privacy filter,
/// values filter, and vector memory retrieval.
async fn preprocess_chat_input(
    session_id: &str,
    messages: &[ChatMessage],
    state: &Arc<AppState>,
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
            let (memory_query, _) = privacy_engine.desensitize(&user_msg.content);
            let text_hits = {
                let store = state.memory_store.lock().await;
                store
                    .search_text_memories(Some(session_id), &memory_query, memory_top_k)
                    .unwrap_or_default()
            };

            let hs_local_only = classify_hs_policy_topic(&user_msg.content, &tools_prompt)
                != openlife_core::agent::PolicyTopic::General;
            let vector_hits = match embed_text_with_privacy(
                &memory_query,
                &provider,
                &openai_base,
                &openai_key,
                &embedding_model,
                embedding_enabled,
                &privacy_engine,
                hs_local_only,
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
    state: &Arc<AppState>,
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

    // Step 5: Build privacy engine before memory retrieval so queries are redacted before embedding.
    let privacy_engine = state.privacy_engine.lock().await.clone();

    // Step 6: Prefetch memory using MemoryService
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
                    hs_local_only: classify_hs_policy_topic(&user_msg.content, &tools_prompt)
                        != openlife_core::agent::PolicyTopic::General,
                };
                drop(cfg);

                let service = openlife_core::agent::MemoryService::new();
                let memory_store = state.memory_store.lock().await;
                let vector_store = state.vector_store.lock().await;
                let (memory_query, _) = privacy_engine.desensitize(&user_msg.content);

                match service
                    .retrieve_context(
                        session_id,
                        &memory_query,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrdinaryChatRouteKind {
    LegacyNonStream,
    LegacyStream,
    DirectReflex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrdinaryChatExecutionPlan {
    route_kind: OrdinaryChatRouteKind,
    constructs_agent_loop: bool,
    constructs_action_executor: bool,
    tool_execution_allowed: bool,
    agent_actions_allowed: bool,
    agent_observations_allowed: bool,
    mcp_audit_write_allowed: bool,
    external_write_allowed: bool,
    plan_execute_allowed: bool,
    golden_path_allowed: bool,
    final_gate_allowed: bool,
    guidance_consumption_enabled: bool,
}

impl OrdinaryChatExecutionPlan {
    fn legacy(route_kind: OrdinaryChatRouteKind) -> Self {
        Self {
            route_kind,
            constructs_agent_loop: false,
            constructs_action_executor: false,
            tool_execution_allowed: false,
            agent_actions_allowed: false,
            agent_observations_allowed: false,
            mcp_audit_write_allowed: false,
            external_write_allowed: false,
            plan_execute_allowed: false,
            golden_path_allowed: false,
            final_gate_allowed: false,
            guidance_consumption_enabled: false,
        }
    }
}

fn ordinary_send_chat_execution_plan(_layer: Layer) -> OrdinaryChatExecutionPlan {
    OrdinaryChatExecutionPlan::legacy(OrdinaryChatRouteKind::LegacyNonStream)
}

fn ordinary_stream_chat_execution_plan(layer: Layer) -> OrdinaryChatExecutionPlan {
    if layer == Layer::L1 {
        OrdinaryChatExecutionPlan::legacy(OrdinaryChatRouteKind::DirectReflex)
    } else {
        OrdinaryChatExecutionPlan::legacy(OrdinaryChatRouteKind::LegacyStream)
    }
}

#[tauri::command]
async fn send_message(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<SendMessageResult, String> {
    let selected_skill_id = selected_skill_id.as_deref().map(str::to_owned);
    send_message_with_state(session_id, messages, selected_skill_id, state.inner()).await
}

pub(crate) async fn send_message_with_state(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
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

    let layer = if let (Some(ref i), Some(ref m)) = (&intent, &user_msg) {
        let lr = state.layer_router.lock().await;
        lr.resolve(i, &m.content)
    } else {
        Layer::L2
    };
    let main_chat_agent_turn = start_main_chat_agent_turn(
        &session_id,
        user_msg.as_ref(),
        openlife_core::agent::AgentTaskKind::Conversation,
        &state,
    )
    .await?;

    // Layer 1: direct reflex response
    if layer == Layer::L1 {
        if let Some(ref i) = intent {
            if let Some(reply) = i.direct_response() {
                let user_text = user_msg
                    .as_ref()
                    .map(|message| message.content.as_str())
                    .unwrap_or_default();
                let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
                execution_transcript.extend(
                    append_main_chat_direct_answer_contract_transcript(
                        &state,
                        &main_chat_agent_turn,
                        user_text,
                        selected_skill_id.as_deref(),
                    )
                    .await,
                );
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
                agent_run.reasoning_strategy = Some("main_chat_agent_v1_direct_answer".into());
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
                complete_main_chat_agent_turn_session(
                    &state,
                    &main_chat_agent_turn,
                    "DirectAnswer completed without tool execution.",
                )
                .await;
                execution_transcript.extend(
                    append_main_chat_agent_transcript(
                        &state,
                        main_chat_agent_turn
                            .decision
                            .agent_task_session_id
                            .as_deref(),
                        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult,
                        "DirectAnswer completed without tool execution.",
                        serde_json::json!({
                            "runId": agent_run.id,
                            "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                            "legacyFallbackUsed": false,
                        }),
                    )
                    .await,
                );

                return Ok(SendMessageResult {
                    reply,
                    reasoning_trace,
                    tool_calls: vec![],
                    run_id: Some(agent_run.id.clone()),
                    agent_ingress: Some(main_chat_agent_turn.decision),
                    execution_transcript,
                    legacy_fallback_used: false,
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
        capture_conversation_signals(&session_id, &m.content, &life_model, state).await;
        if msg.is_some() {
            let _ = persist_life_model(
                state,
                life_model.clone(),
                false,
                LifeModelMaterializerCallerContext::new(
                    "ordinary_chat_auto_checkin_source_data",
                    LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
                    LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
                ),
            )
            .await?;
        }
        msg
    } else {
        None
    };

    if let Some(result) = try_run_main_chat_agent_strategy(
        &session_id,
        user_msg.as_ref(),
        &desensitized_messages,
        &life_model,
        _context_summary.clone(),
        embed_err.clone(),
        auto_checkin_msg.clone(),
        &main_chat_agent_turn,
        &state,
        &privacy_engine,
        &privacy_map,
        None,
        selected_skill_id.as_deref(),
    )
    .await?
    {
        return Ok(result);
    }

    let ordinary_plan = ordinary_send_chat_execution_plan(layer);
    return send_message_with_legacy_generation(
        session_id,
        user_msg,
        life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        auto_checkin_msg,
        layer,
        _context_summary,
        ordinary_plan,
        main_chat_agent_turn,
        state,
    )
    .await;
}

/// Legacy non-stream fallback used only when Main Chat v1 returns no governed
/// result. It intentionally does not construct AgentLoop, ActionExecutor, or
/// tool actions.
#[allow(clippy::too_many_arguments)]
async fn send_message_with_legacy_generation(
    session_id: String,
    user_msg: Option<ChatMessage>,
    life_model: LifeModel,
    tools_prompt: String,
    privacy_engine: PrivacyEngine,
    privacy_map: HashMap<String, String>,
    desensitized_messages: Vec<ChatMessage>,
    embed_err: Option<String>,
    auto_checkin_msg: Option<String>,
    layer: Layer,
    context_summary: openlife_core::agent::types::ContextSummary,
    ordinary_plan: OrdinaryChatExecutionPlan,
    main_chat_agent_turn: MainChatAgentTurn,
    state: &Arc<AppState>,
) -> Result<SendMessageResult, String> {
    debug_assert_eq!(
        ordinary_plan.route_kind,
        OrdinaryChatRouteKind::LegacyNonStream
    );
    debug_assert!(!ordinary_plan.constructs_agent_loop);
    debug_assert!(!ordinary_plan.constructs_action_executor);
    debug_assert!(!ordinary_plan.tool_execution_allowed);

    let scheduler = state.scheduler.lock().await.clone();
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
        build_chat_runtime_hs_packet(state, &task, &life_model, &tools_prompt, None).await?;

    let mut reply = generate_non_stream_fallback(
        &scheduler,
        desensitized_messages,
        &life_model,
        &tools_prompt,
        hs_packet.clone(),
    )
    .await?;
    reply = privacy_engine.reconstruct(&reply, &privacy_map);

    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };
    let user_input_text = user_msg
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text);
    let legacy_fallback_used = main_chat_agent_turn.decision.selected_strategy
        != openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer;
    agent_run.reasoning_strategy = Some(if legacy_fallback_used {
        format!(
            "main_chat_agent_v1_{}_legacy_fallback",
            main_chat_agent_turn.decision.selected_strategy.as_str()
        )
    } else {
        "main_chat_agent_v1_direct_answer".to_string()
    });
    agent_run.hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
    let model_route = scheduler.preview_chat_route(Some(&tools_prompt)).await;
    agent_run.complete(&preview_text(&reply, 200), model_route, context_summary);

    let mut reasoning_trace = ReasoningTrace {
        generation_result: Some(serde_json::json!({ "text": reply })),
        ..Default::default()
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
    let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
    if legacy_fallback_used {
        execution_transcript.extend(
            append_main_chat_agent_transcript(
                &state,
                main_chat_agent_turn
                    .decision
                    .agent_task_session_id
                    .as_deref(),
                openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Fallback,
                "Legacy generation fallback was used for this Main Chat turn.",
                serde_json::json!({
                    "runId": agent_run.id,
                    "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                    "fallbackReason": "strategy_executor_not_yet_available_for_this_path",
                    "fallbackVisible": true,
                }),
            )
            .await,
        );
    }
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            &state,
            main_chat_agent_turn
                .decision
                .agent_task_session_id
                .as_deref(),
            openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult,
            "Assistant response was delivered.",
            serde_json::json!({
                "runId": agent_run.id,
                "legacyFallbackUsed": legacy_fallback_used,
            }),
        )
        .await,
    );

    Ok(SendMessageResult {
        reply,
        reasoning_trace,
        tool_calls: Vec::new(),
        run_id: Some(agent_run.id.clone()),
        agent_ingress: Some(main_chat_agent_turn.decision),
        execution_transcript,
        legacy_fallback_used,
    })
}

/// AgentLoop-based chat execution for explicit non-default / controlled paths only.
#[allow(dead_code, clippy::too_many_arguments)]
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
                agent_ingress: None,
                execution_transcript: Vec::new(),
                legacy_fallback_used: true,
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
        agent_ingress: None,
        execution_transcript: Vec::new(),
        legacy_fallback_used: false,
    })
}

#[derive(serde::Deserialize, Clone, Debug)]
struct StartStreamMessageArgs {
    session_id: String,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    selected_skill_id: Option<String>,
}

const STREAM_INIT_TIMEOUT_SECS: u64 = 45;
const STREAM_CHUNK_TIMEOUT_SECS: u64 = 90;

/// Emit a unified agent-status-update event for both streaming and non-streaming paths.
/// Frontend AgentStateIndicator expects: phase, message, step_index, tool_call_index, timestamp.
#[allow(dead_code)]
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

/// Handle AgentLoop failure: try non-stream fallback, create AgentRun with
/// error context, persist the run. Returns (reply, agent_run) on success, or
/// an error message string if both AgentLoop and fallback fail.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
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

/// Streaming callback that forwards AgentLoop events to Tauri frontend via emit().
#[allow(dead_code)]
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
#[allow(dead_code)]
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
            let _ = persist_life_model(
                &state.inner().clone(),
                life_model.clone(),
                false,
                LifeModelMaterializerCallerContext::new(
                    "ordinary_stream_agent_loop_auto_checkin_source_data",
                    LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
                    LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
                ),
            )
            .await;
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
async fn start_stream_message<R: tauri::Runtime>(
    args: Option<StartStreamMessageArgs>,
    session_id: Option<String>,
    messages: Option<Vec<ChatMessage>>,
    selected_skill_id: Option<String>,
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let (session_id, messages, selected_skill_id) = if let Some(args) = args {
        (args.session_id, args.messages, args.selected_skill_id)
    } else {
        (
            session_id.ok_or_else(|| "start_stream_message 缺少 session_id".to_string())?,
            messages.ok_or_else(|| "start_stream_message 缺少 messages".to_string())?,
            selected_skill_id,
        )
    };

    let selected_skill_id = selected_skill_id.as_deref().map(str::to_owned);
    let app_handle = app_handle.clone();
    start_stream_message_with_state(
        session_id,
        messages,
        selected_skill_id,
        state.inner(),
        move |event, payload| {
            let _ = app_handle.emit(event, payload);
        },
    )
    .await
}

pub(crate) async fn start_stream_message_with_state(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
    mut emit_stream_event: impl FnMut(&str, serde_json::Value) + Send,
) -> Result<(), String> {
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
    let main_chat_agent_turn = start_main_chat_agent_turn(
        &session_id,
        user_msg.as_ref(),
        openlife_core::agent::AgentTaskKind::Conversation,
        state,
    )
    .await?;

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
                let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
                execution_transcript.extend(
                    append_main_chat_direct_answer_contract_transcript(
                        state,
                        &main_chat_agent_turn,
                        &user_input_text,
                        selected_skill_id.as_deref(),
                    )
                    .await,
                );
                // 先保存用户消息
                if let Some(ref user) = user_msg {
                    if user.role == "user" {
                        let user_inserted =
                            persist_chat_message_if_needed(&session_id, user, state).await?;
                        if user_inserted {
                            persist_vector_memory_for_message(&session_id, user, state).await;
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

                emit_stream_event(
                    "stream-message-start",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "reasoning_trace": ReasoningTrace::default(),
                        "tool_calls": Vec::<ToolCallResult>::new(),
                        "agent_ingress": main_chat_agent_turn.decision.clone(),
                        "execution_transcript": execution_transcript.clone(),
                        "legacy_fallback_used": false,
                    }),
                );
                emit_stream_event(
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
                agent_run.reasoning_strategy = Some("main_chat_agent_v1_direct_answer".into());
                agent_run.complete(&preview, model_route, context_summary);

                if let Err(e) = finalize_chat_agent_run(
                    &session_id,
                    &assistant_msg,
                    &reply,
                    &mut reasoning_trace,
                    &mut agent_run,
                    &life_model,
                    state,
                )
                .await
                {
                    log::warn!("[L1 Stream] finalize_chat_agent_run failed: {}", e);
                    emit_stream_event(
                        "stream-message-error",
                        serde_json::json!({
                            "session_id": &session_id,
                            "run_id": agent_run.id,
                            "error": format!("AgentRun 持久化失败: {}", e),
                        }),
                    );
                    return Err(e);
                }
                complete_main_chat_agent_turn_session(
                    state,
                    &main_chat_agent_turn,
                    "DirectAnswer completed without tool execution.",
                )
                .await;
                execution_transcript.extend(
                    append_main_chat_agent_transcript(
                        state,
                        main_chat_agent_turn
                            .decision
                            .agent_task_session_id
                            .as_deref(),
                        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult,
                        "DirectAnswer completed without tool execution.",
                        serde_json::json!({
                            "runId": agent_run.id,
                            "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                            "legacyFallbackUsed": false,
                        }),
                    )
                    .await,
                );

                emit_stream_event(
                    "stream-message-done",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "reply": reply,
                        "reasoning_trace": ReasoningTrace::default(),
                        "tool_calls": Vec::<ToolCallResult>::new(),
                        "agent_ingress": main_chat_agent_turn.decision.clone(),
                        "execution_transcript": execution_transcript,
                        "legacy_fallback_used": false,
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
        embed_err,
        context_summary,
    ) = match if use_v2 {
        preprocess_chat_input_v2(&session_id, &messages, state).await
    } else {
        preprocess_chat_input(&session_id, &messages, state).await
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
        capture_conversation_signals(&session_id, &m.content, &life_model, state).await;
        if msg.is_some() {
            if let Err(message) = persist_life_model(
                &state.clone(),
                life_model.clone(),
                false,
                LifeModelMaterializerCallerContext::new(
                    "ordinary_stream_legacy_auto_checkin_source_data",
                    LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
                    LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
                ),
            )
            .await
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

    if let Some(result) = try_run_main_chat_agent_strategy(
        &session_id,
        user_msg.as_ref(),
        &desensitized_messages,
        &life_model,
        context_summary.clone(),
        embed_err.clone(),
        auto_checkin_msg_stream.clone(),
        &main_chat_agent_turn,
        state,
        &privacy_engine,
        &privacy_map,
        Some(agent_run.clone()),
        selected_skill_id.as_deref(),
    )
    .await?
    {
        let run_id = result.run_id.clone().unwrap_or_default();
        emit_stream_event(
            "stream-message-start",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": run_id,
                "reasoning_trace": result.reasoning_trace.clone(),
                "tool_calls": result.tool_calls.clone(),
                "agent_ingress": result.agent_ingress.clone(),
                "execution_transcript": result.execution_transcript.clone(),
                "legacy_fallback_used": result.legacy_fallback_used,
            }),
        );
        emit_stream_event(
            "stream-message-chunk",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": run_id,
                "chunk": result.reply.clone(),
            }),
        );
        emit_stream_event(
            "stream-message-done",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": run_id,
                "reply": result.reply,
                "reasoning_trace": result.reasoning_trace,
                "tool_calls": result.tool_calls,
                "agent_ingress": result.agent_ingress,
                "execution_transcript": result.execution_transcript,
                "legacy_fallback_used": result.legacy_fallback_used,
            }),
        );
        return Ok(());
    }

    let ordinary_plan = ordinary_stream_chat_execution_plan(layer);
    debug_assert!(!ordinary_plan.constructs_agent_loop);
    debug_assert!(!ordinary_plan.constructs_action_executor);
    debug_assert!(!ordinary_plan.tool_execution_allowed);

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
        build_chat_runtime_hs_packet(state, &task, &life_model, &tools_prompt, None).await?;

    let mut reasoning_trace = ReasoningTrace::default();
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }
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
    let legacy_fallback_used = main_chat_agent_turn.decision.selected_strategy
        != openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer;
    agent_run.reasoning_strategy = Some(if legacy_fallback_used {
        format!(
            "main_chat_agent_v1_{}_legacy_stream_fallback",
            main_chat_agent_turn.decision.selected_strategy.as_str()
        )
    } else {
        "main_chat_agent_v1_direct_answer_stream".to_string()
    });

    emit_stream_event(
        "stream-message-start",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reasoning_trace": reasoning_trace.clone(),
            "tool_calls": Vec::<ToolCallResult>::new(),
            "agent_ingress": main_chat_agent_turn.decision.clone(),
            "execution_transcript": main_chat_agent_turn.transcript_entries.clone(),
            "legacy_fallback_used": legacy_fallback_used,
        }),
    );

    let mut full_reply = String::new();
    if let Some(ref ex) = reasoning_trace.generation_result {
        if let Some(text) = ex.get("text").and_then(|t| t.as_str()) {
            full_reply = text.to_string();
            emit_stream_event(
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
                                emit_stream_event(
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
                                emit_stream_event(
                                    "stream-message-error",
                                    serde_json::json!({
                                        "session_id": &session_id,
                                        "run_id": agent_run.id,
                                        "error": message.clone(),
                                    }),
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
                            emit_stream_event(
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
                                emit_stream_event(
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
                                emit_stream_event(
                                    "stream-message-error",
                                    serde_json::json!({
                                        "session_id": &session_id,
                                        "run_id": agent_run.id,
                                        "error": message.clone(),
                                    }),
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
                        emit_stream_event(
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
                        emit_stream_event(
                            "stream-message-error",
                            serde_json::json!({
                                "session_id": &session_id,
                                "run_id": agent_run.id,
                                "error": message.clone(),
                            }),
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
                    emit_stream_event(
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
                    emit_stream_event(
                        "stream-message-error",
                        serde_json::json!({
                            "session_id": &session_id,
                            "run_id": agent_run.id,
                            "error": message.clone(),
                        }),
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
        state,
    )
    .await
    {
        log::warn!("[Stream] finalize_chat_agent_run failed: {}", e);
        emit_stream_event(
            "stream-message-error",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": agent_run.id,
                "error": format!("AgentRun 持久化失败: {}", e),
            }),
        );
        return Err(e);
    }
    let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
    if legacy_fallback_used {
        execution_transcript.extend(
            append_main_chat_agent_transcript(
                state,
                main_chat_agent_turn
                    .decision
                    .agent_task_session_id
                    .as_deref(),
                openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Fallback,
                "Legacy streaming fallback was used for this Main Chat turn.",
                serde_json::json!({
                    "runId": agent_run.id,
                    "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                    "fallbackReason": "strategy_stream_executor_not_yet_available_for_this_path",
                    "fallbackVisible": true,
                }),
            )
            .await,
        );
    }
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            main_chat_agent_turn
                .decision
                .agent_task_session_id
                .as_deref(),
            openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult,
            "Assistant response was delivered.",
            serde_json::json!({
                "runId": agent_run.id,
                "legacyFallbackUsed": legacy_fallback_used,
            }),
        )
        .await,
    );

    emit_stream_event(
        "stream-message-done",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reply": reply,
            "reasoning_trace": reasoning_trace,
            "tool_calls": tool_calls,
            "agent_ingress": main_chat_agent_turn.decision.clone(),
            "execution_transcript": execution_transcript,
            "legacy_fallback_used": legacy_fallback_used,
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
        react_trace: result.action.react_trace,
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
            run_main_chat_agent_execution_v1_eval_gate,
            run_main_chat_agent_execution_v1_final_acceptance_gate,
            get_runtime_strategy_registry_status,
            get_react_beta_execution_status,
            create_plan_execute_session,
            get_plan_execute_session,
            list_plan_execute_sessions,
            update_plan_execute_session_draft,
            finalize_plan_execute_session,
            cancel_plan_execute_session,
            execute_plan_execute_step,
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
            get_main_chat_agent_task_state,
            resume_main_chat_agent_task,
            cancel_main_chat_agent_task,
            retry_main_chat_agent_action,
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
            get_skill_runtime_status,
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
mod legacy_write_convergence_tests;

#[cfg(test)]
mod hs_runtime_tests {
    use super::*;
    use crate::main_chat_command_surface_eval::{
        MainChatCommandSurfaceEvalEntryPoint, MainChatCommandSurfaceEvalEvidence,
        MainChatCommandSurfaceEvalReport, MainChatCommandSurfaceEvalScenario,
        MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES,
    };
    use crate::main_chat_final_gate::{
        main_chat_live_provider_acceptance_evidence, main_chat_live_provider_report_blockers,
        MainChatAgentExecutionV1FinalGateReport, MainChatLiveProviderEvalHarnessReport,
        MainChatLiveProviderEvalHarnessScenario,
    };
    use crate::main_chat_live_provider_harness::{
        run_main_chat_live_provider_eval_harness, MainChatLiveProviderEvalHarnessInput,
    };
    use crate::main_chat_react_tool_selection::{
        build_main_chat_react_agent_loop_messages, main_chat_react_agent_loop_execution_plan,
    };

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
    fn legacy_send_fallback_plan_has_no_agent_loop_or_tool_side_effects() {
        let plan = ordinary_send_chat_execution_plan(Layer::L2);

        assert_eq!(plan.route_kind, OrdinaryChatRouteKind::LegacyNonStream);
        assert!(!plan.constructs_agent_loop);
        assert!(!plan.constructs_action_executor);
        assert!(!plan.tool_execution_allowed);
        assert!(!plan.agent_actions_allowed);
        assert!(!plan.agent_observations_allowed);
        assert!(!plan.mcp_audit_write_allowed);
        assert!(!plan.external_write_allowed);
        assert!(!plan.plan_execute_allowed);
        assert!(!plan.golden_path_allowed);
        assert!(!plan.final_gate_allowed);
        assert!(!plan.guidance_consumption_enabled);
    }

    #[test]
    fn legacy_stream_fallback_plan_stays_legacy_stream_for_l2_l3() {
        for layer in [Layer::L2, Layer::L3] {
            let plan = ordinary_stream_chat_execution_plan(layer);

            assert_eq!(plan.route_kind, OrdinaryChatRouteKind::LegacyStream);
            assert!(!plan.constructs_agent_loop);
            assert!(!plan.constructs_action_executor);
            assert!(!plan.tool_execution_allowed);
            assert!(!plan.agent_actions_allowed);
            assert!(!plan.agent_observations_allowed);
            assert!(!plan.mcp_audit_write_allowed);
            assert!(!plan.external_write_allowed);
            assert!(!plan.plan_execute_allowed);
            assert!(!plan.golden_path_allowed);
            assert!(!plan.final_gate_allowed);
            assert!(!plan.guidance_consumption_enabled);
        }
    }

    #[test]
    fn ordinary_stream_legacy_plan_is_built_after_governed_strategy_attempt() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let stream_start = source
            .find("async fn start_stream_message<R: tauri::Runtime>(")
            .expect("stream command exists");
        let next_command = source
            .find("async fn execute_tool_call(")
            .expect("stream command should be followed by tool execution command");
        let stream_body = &source[stream_start..next_command];
        let strategy_attempt = stream_body
            .find("try_run_main_chat_agent_strategy(")
            .expect("stream command should attempt Main Chat v1 strategy");
        let legacy_plan = stream_body
            .find("ordinary_stream_chat_execution_plan(layer)")
            .expect("stream command should keep a legacy stream fallback plan");

        assert!(
            strategy_attempt < legacy_plan,
            "start_stream_message should attempt Main Chat v1 before building the legacy fallback plan"
        );
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
    fn main_chat_direct_answer_strategy_does_not_return_to_hidden_legacy_generation() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let strategy_body =
            extract_rust_function_body(&source, "async fn try_run_main_chat_agent_strategy(");

        assert!(
            !strategy_body.contains("if strategy == MainChatAgentStrategy::DirectAnswer")
                || !strategy_body.contains("return Ok(None);"),
            "DirectAnswer must execute as a Main Chat strategy instead of returning to legacy generation"
        );
    }

    #[test]
    fn main_chat_react_strategy_uses_action_executor_instead_of_keyword_mapper_core() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let strategy_body =
            extract_rust_function_body(&source, "async fn try_run_main_chat_agent_strategy(");

        assert!(
            !strategy_body.contains("main_chat_react_action_type("),
            "ReActToolExecution must not use the keyword action mapper as its core execution path"
        );
        assert!(
            strategy_body.contains("execute_main_chat_react_action_with_executor("),
            "ReActToolExecution should delegate read actions to the governed ActionExecutor path"
        );
    }

    #[test]
    fn main_chat_react_strategy_synthesizes_follow_up_after_observation() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let strategy_body =
            extract_rust_function_body(&source, "async fn try_run_main_chat_agent_strategy(");

        assert!(
            strategy_body.contains("synthesize_main_chat_react_follow_up("),
            "ReActToolExecution must synthesize a governed follow-up/final answer after observation instead of echoing the observation"
        );
        assert!(
            !strategy_body.contains("reply = observation.final_answer;"),
            "ReActToolExecution should not use the raw observation answer as its final response"
        );
    }

    #[test]
    fn retry_main_chat_action_enters_manual_blocker_when_not_replayable() {
        let module_path = format!(
            "{}/src/main_chat_task_controls.rs",
            env!("CARGO_MANIFEST_DIR")
        );
        let source = std::fs::read_to_string(module_path).expect("read task-control module");
        let retry_body = extract_rust_function_body(
            &source,
            "pub(crate) async fn retry_main_chat_agent_action(",
        );

        assert!(
            retry_body.contains("manual_blocker_required"),
            "retry command must inspect whether the failed action can be replayed"
        );
        assert!(
            retry_body.contains("ExecutionQueueStatus::PendingPermission"),
            "non-replayable retries must become an explicit manual blocker"
        );
        assert!(
            retry_body.contains("manualReplayRequired"),
            "manual retry blocker metadata must be visible in task state/transcript"
        );
    }

    #[test]
    fn retry_main_chat_action_replays_replayable_action_instead_of_state_only() {
        let module_path = format!(
            "{}/src/main_chat_task_controls.rs",
            env!("CARGO_MANIFEST_DIR")
        );
        let source = std::fs::read_to_string(module_path).expect("read task-control module");
        let retry_body = extract_rust_function_body(
            &source,
            "pub(crate) async fn retry_main_chat_agent_action(",
        );

        assert!(
            retry_body.contains("replay_main_chat_agent_action("),
            "replayable Main Chat retries must execute the failed action again instead of only changing queue state"
        );
        assert!(
            source.contains("automaticReplayCompleted"),
            "automatic retry replay completion must be visible in task state/transcript metadata"
        );
    }

    #[test]
    fn resume_main_chat_task_preserves_pending_permission_blocker_instead_of_state_only() {
        let module_path = format!(
            "{}/src/main_chat_task_controls.rs",
            env!("CARGO_MANIFEST_DIR")
        );
        let source = std::fs::read_to_string(module_path).expect("read task-control module");
        let resume_body =
            extract_rust_function_body(&source, "pub(crate) async fn resume_main_chat_agent_task(");

        assert!(
            resume_body.contains("evaluate_main_chat_task_resume("),
            "resume command must evaluate blockers/actions before changing task state"
        );
        assert!(
            resume_body.contains("remain_waiting_permission"),
            "resume command must preserve pending permission state when blockers remain"
        );
        assert!(
            resume_body.contains("resumeBlockedByPendingPermission"),
            "resume command must expose permission-preserving resume metadata"
        );
    }

    #[tokio::test]
    async fn resume_main_chat_task_replays_pending_action_after_tool_permission_acceptance() {
        use openlife_core::agent::main_chat_agent_v1::{
            AgentTaskSessionDraft, AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy,
            ExecutionQueueStatus, MainChatAgentStrategy,
        };
        use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![resume_main_chat_agent_task])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");

        let proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.builtin_echo",
            serde_json::json!({
                "tool_name": "builtin_echo",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "permission": "allow_until_revoked",
            }),
            "Allow the pending Main Chat MCP read action to continue.",
            0.7,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        );
        let proposal_id = proposal.id.clone();
        {
            let proposal_store = state.proposal_store.as_ref().expect("proposal store");
            proposal_store
                .lock()
                .await
                .create_proposal(&proposal)
                .expect("create tool permission proposal");
        }

        let session = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store")
                .lock()
                .await;
            store
                .create_session(AgentTaskSessionDraft {
                    chat_session_id: "resume-permission-command-surface".into(),
                    user_goal: "Use mcp builtin_echo read-only now.".into(),
                    selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                    current_plan_summary: Some(
                        "Waiting for ToolPermission acceptance before replaying MCP read.".into(),
                    ),
                    context_snapshot_refs: vec!["resume-permission-context".into()],
                })
                .expect("create main chat task session")
        };
        let action = ExecutionAction::new(
            "mcp.read_only",
            "Pending registered MCP read action blocked on ToolPermission.",
        );
        let queued = {
            let queue = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue")
                .lock()
                .await;
            let queued = queue
                .enqueue(
                    &session.id,
                    action.clone(),
                    ExecutionPolicy::default().classify(&action),
                )
                .expect("enqueue pending mcp action");
            queue
                .transition(&queued.id, ExecutionQueueStatus::Executing, None)
                .expect("move action to executing");
            queue
                .transition(
                    &queued.id,
                    ExecutionQueueStatus::PendingPermission,
                    Some(serde_json::json!({
                        "proposalId": proposal_id,
                        "toolName": "builtin_echo",
                        "resumeReplayable": true,
                        "directWritesExecuted": false,
                    })),
                )
                .expect("move action to pending permission");
            queued
        };
        {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store")
                .lock()
                .await;
            store
                .record_action_queue_id(&session.id, &queued.id)
                .expect("record action id");
            store
                .set_pending_blockers(&session.id, vec!["tool_permission_required".into()])
                .expect("set pending blocker");
            store
                .mark_waiting_permission(&session.id)
                .expect("mark waiting permission");
        }

        crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .expect("accept tool permission proposal");

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "resume_main_chat_agent_task",
                serde_json::json!({
                    "taskSessionId": session.id,
                    "task_session_id": session.id,
                }),
            ),
        );
        assert!(response.is_ok(), "resume command failed: {response:?}");

        let resumed = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store")
                .lock()
                .await;
            store
                .load_session(&session.id)
                .expect("load resumed session")
                .expect("resumed session exists")
        };
        assert_eq!(resumed.status, AgentTaskSessionStatus::Completed);
        assert!(resumed.pending_blockers.is_empty());

        let replayed = {
            let queue = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue")
                .lock()
                .await;
            queue
                .load(&queued.id)
                .expect("load replayed action")
                .expect("replayed action exists")
        };
        assert_eq!(replayed.status, ExecutionQueueStatus::Completed);
        let metadata = replayed
            .observation_metadata
            .as_ref()
            .expect("replay observation metadata");
        assert_eq!(
            metadata["automaticResumeReplayCompleted"],
            serde_json::json!(true)
        );
        assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
    }

    #[tokio::test]
    async fn cancel_main_chat_task_cancels_nonterminal_queued_actions() {
        use openlife_core::agent::main_chat_agent_v1::{
            AgentTaskSessionDraft, AgentTaskSessionStatus, ExecutionAction, ExecutionPolicy,
            ExecutionQueueStatus, MainChatAgentStrategy,
        };

        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![cancel_main_chat_agent_task])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");

        let session = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store")
                .lock()
                .await;
            store
                .create_session(AgentTaskSessionDraft {
                    chat_session_id: "cancel-command-surface".into(),
                    user_goal: "Search memory then request external write confirmation.".into(),
                    selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                    current_plan_summary: Some("Cancel should stop queued work.".into()),
                    context_snapshot_refs: vec!["cancel-context".into()],
                })
                .expect("create main chat task session")
        };
        let planned_action = ExecutionAction::new("memory.search", "Queued read action.");
        let permission_action = ExecutionAction::new("external.write", "Queued external write.");
        let (planned_id, permission_id) = {
            let queue = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue")
                .lock()
                .await;
            let planned = queue
                .enqueue(
                    &session.id,
                    planned_action.clone(),
                    ExecutionPolicy::default().classify(&planned_action),
                )
                .expect("enqueue planned action");
            let pending = queue
                .enqueue(
                    &session.id,
                    permission_action.clone(),
                    ExecutionPolicy::default().classify(&permission_action),
                )
                .expect("enqueue pending action");
            (planned.id, pending.id)
        };
        {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store")
                .lock()
                .await;
            store
                .record_action_queue_id(&session.id, &planned_id)
                .expect("record planned action");
            store
                .record_action_queue_id(&session.id, &permission_id)
                .expect("record pending action");
            store
                .set_pending_blockers(
                    &session.id,
                    vec!["external_write_requires_confirmation".into()],
                )
                .expect("set pending blocker");
            store
                .mark_waiting_permission(&session.id)
                .expect("mark waiting permission");
        }

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "cancel_main_chat_agent_task",
                serde_json::json!({
                    "taskSessionId": session.id,
                    "task_session_id": session.id,
                }),
            ),
        );
        assert!(response.is_ok(), "cancel command failed: {response:?}");

        let cancelled_session = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store")
                .lock()
                .await;
            store
                .load_session(&session.id)
                .expect("load cancelled session")
                .expect("cancelled session exists")
        };
        assert_eq!(cancelled_session.status, AgentTaskSessionStatus::Cancelled);

        let actions = {
            let queue = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue")
                .lock()
                .await;
            queue
                .list_for_session(&session.id)
                .expect("list cancelled actions")
        };
        for action in actions {
            assert_eq!(
                action.status,
                ExecutionQueueStatus::Cancelled,
                "cancel must stop queued action {}",
                action.id
            );
            assert_eq!(
                action
                    .observation_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get("cancelRequested"))
                    .and_then(serde_json::Value::as_bool),
                Some(true)
            );
        }
    }

    #[test]
    fn main_chat_mcp_read_resolves_registered_tool_instead_of_wrapper_only() {
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_react_execution.rs");
        let source =
            std::fs::read_to_string(module_path).expect("read src/main_chat_react_execution.rs");
        let executor_body = extract_rust_function_body(
            &source,
            "pub(crate) async fn execute_main_chat_react_action_with_executor(",
        );

        assert!(
            executor_body.contains("resolve_main_chat_mcp_read_target("),
            "Main Chat MCP reads must resolve a named registered read tool before falling back to blockers"
        );
        assert!(
            executor_body.contains("mcpReadTargetResolved"),
            "MCP read target resolution must be visible in metadata"
        );
    }

    #[test]
    fn main_chat_react_attempts_agent_loop_before_single_step_fallback() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let strategy_body =
            extract_rust_function_body(&source, "async fn try_run_main_chat_agent_strategy(");
        let runtime_module_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_react_runtime.rs");
        let runtime_source =
            std::fs::read_to_string(runtime_module_path).expect("read main_chat_react_runtime.rs");

        assert!(
            strategy_body.contains("try_run_main_chat_react_agent_loop("),
            "ReActToolExecution must attempt the governed AgentLoop before single-step fallback"
        );
        assert!(
            runtime_source.contains("agentLoopAttempted"),
            "Main Chat ReAct AgentLoop attempt/fallback must be visible in transcript metadata"
        );
    }

    #[test]
    fn main_chat_react_agent_loop_receives_plan_guidance_without_raw_arguments() {
        let plan = build_main_chat_react_action_plan(
            "session-plan-guidance",
            "what did i ask yesterday about API keys?",
        )
        .expect("build session search plan");
        let original_messages = vec![ChatMessage {
            role: "user".into(),
            content: "what did i ask yesterday about API keys?".into(),
        }];

        let guided_messages = build_main_chat_react_agent_loop_messages(&original_messages, &plan);

        assert_eq!(guided_messages.len(), original_messages.len() + 1);
        let guidance = guided_messages
            .first()
            .expect("plan guidance should be prepended");
        assert_eq!(guidance.role, "system");
        assert!(guidance
            .content
            .contains("plannedActionType=session.search"));
        assert!(guidance.content.contains("plannedTarget=session.search"));
        assert!(guidance.content.contains("argumentsDigest="));
        assert!(
            !guidance.content.contains("what did i ask yesterday"),
            "plan guidance must not duplicate raw user text"
        );
        assert!(
            !guidance.content.contains("session-plan-guidance"),
            "plan guidance must not include raw executor argument values"
        );
        assert!(
            !guidance.content.contains("\"query\""),
            "plan guidance must not include structured executor arguments"
        );
    }

    #[test]
    fn main_chat_react_agent_loop_guidance_declares_governed_tool_candidate_set() {
        let plan = build_main_chat_react_action_plan(
            "session-candidate-guidance",
            "what did i ask yesterday about API keys?",
        )
        .expect("build session search plan");
        let original_messages = vec![ChatMessage {
            role: "user".into(),
            content: "what did i ask yesterday about API keys?".into(),
        }];

        let guided_messages = build_main_chat_react_agent_loop_messages(&original_messages, &plan);
        let guidance = guided_messages
            .first()
            .expect("tool candidate guidance should be prepended");

        assert!(
            guidance.content.contains("allowedToolCandidates="),
            "AgentLoop guidance must expose the governed candidate set, not only a prose planned action"
        );
        assert!(
            guidance.content.contains("candidateCount=1"),
            "single-candidate turns must still declare candidate count for auditability"
        );
        assert!(guidance.content.contains("candidateId=session.search"));
        assert!(guidance.content.contains("candidateTarget=session.search"));
        assert!(guidance.content.contains("toolsetAllowlistRequired=true"));
        assert!(
            !guidance.content.contains("what did i ask yesterday"),
            "candidate guidance must not duplicate raw user text"
        );
        assert!(
            !guidance.content.contains("\"query\""),
            "candidate guidance must not include structured executor arguments"
        );
    }

    #[test]
    fn main_chat_react_agent_loop_execution_plan_can_declare_multiple_governed_mcp_candidates() {
        let registry = openlife_core::mcp::McpRegistry::new();
        let plan = build_main_chat_react_action_plan(
            "session-multi-candidate-guidance",
            "Use an mcp read-only utility tool now.",
        )
        .expect("build generic MCP read plan");

        let agent_loop_plan = main_chat_react_agent_loop_execution_plan(&registry, &plan);
        let candidate_ids = agent_loop_plan.tool_candidate_ids();
        let contract = agent_loop_plan.tool_candidate_contract();

        assert!(
            agent_loop_plan.tool_candidate_count() >= 2,
            "generic MCP reads must expose a governed manifest candidate set, not only the wrapper"
        );
        assert!(
            candidate_ids
                .iter()
                .any(|candidate| candidate == "builtin_echo"),
            "candidate set should include the registered read-only builtin echo manifest"
        );
        assert!(
            candidate_ids
                .iter()
                .any(|candidate| candidate == "tool.list_available"),
            "candidate set should include another registered read-only manifest for model selection"
        );
        assert!(contract.contains("allowedToolCandidates="));
        assert!(contract.contains("candidateId=builtin_echo"));
        assert!(contract.contains("candidateTarget=builtin_echo"));
        assert!(contract.contains("candidateId=tool.list_available"));
        assert!(contract.contains("candidateTarget=tool.list_available"));
        assert!(
            contract.contains(&format!(
                "candidateCount={}",
                agent_loop_plan.tool_candidate_count()
            )),
            "candidate contract must publish the real governed candidate count"
        );
        assert!(
            !contract.contains("candidateCount=1;"),
            "multi-candidate MCP plans must not retain singleton audit metadata"
        );
    }

    #[test]
    fn main_chat_react_agent_loop_ranks_mcp_candidates_by_manifest_capability_match() {
        let mut registry = openlife_core::mcp::McpRegistry::new();
        registry.register_builtin(
            openlife_core::tool_manifest::ToolManifest {
                id: "zzz_calendar.read".into(),
                name: "zzz_calendar.read".into(),
                description: "Read calendar availability without writes.".into(),
                parameters: serde_json::json!({ "type": "object" }),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["read".into(), "calendar".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                tags: vec!["calendar".into()],
            },
            Box::new(|_| Ok("metadata-safe calendar read placeholder".into())),
        );
        let plan = build_main_chat_react_action_plan(
            "session-capability-ranked-candidate",
            "Use an MCP read-only calendar tool now.",
        )
        .expect("build generic MCP read plan");

        let agent_loop_plan = main_chat_react_agent_loop_execution_plan(&registry, &plan);
        let candidate_ids = agent_loop_plan.tool_candidate_ids();
        let contract = agent_loop_plan.tool_candidate_contract();

        assert!(
            candidate_ids
                .first()
                .is_some_and(|candidate| candidate.contains("calendar")),
            "generic MCP candidate selection should rank manifest capability/name matches ahead of generic utility reads"
        );
        assert!(
            contract.contains("candidateRank=1"),
            "candidate contract must expose deterministic rank evidence"
        );
        assert!(
            contract.contains("candidateSource="),
            "candidate contract must expose manifest source evidence"
        );
        assert!(
            contract.contains("capabilitiesDigest="),
            "candidate contract must expose metadata-safe capability evidence"
        );
        assert!(
            contract.contains("matchReason=capability_or_name_match"),
            "candidate contract must explain capability/name match ranking without raw prompt text"
        );
    }

    #[test]
    fn main_chat_react_agent_loop_mcp_candidate_set_excludes_high_risk_confirmation_manifests() {
        let mut registry = openlife_core::mcp::McpRegistry::new();
        registry.register_builtin(
            openlife_core::tool_manifest::ToolManifest {
                id: "dangerous_secret.read".into(),
                name: "dangerous_secret.read".into(),
                description: "High-risk secret read should require confirmation.".into(),
                parameters: serde_json::json!({ "type": "object" }),
                permission_level: "high".into(),
                risk_level: "high".into(),
                version: "1.0.0".into(),
                source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["read".into()],
                requires_confirmation: true,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                tags: vec!["secrets".into()],
            },
            Box::new(|_| Ok("metadata-safe secret read placeholder".into())),
        );
        let plan = build_main_chat_react_action_plan(
            "session-high-risk-candidate-filter",
            "Use an mcp read-only utility tool now.",
        )
        .expect("build generic MCP read plan");

        let agent_loop_plan = main_chat_react_agent_loop_execution_plan(&registry, &plan);
        let candidate_ids = agent_loop_plan.tool_candidate_ids();
        let contract = agent_loop_plan.tool_candidate_contract();

        assert!(
            candidate_ids
                .iter()
                .any(|candidate| candidate == "builtin_echo"),
            "safe read candidates must remain available"
        );
        assert!(
            !candidate_ids
                .iter()
                .any(|candidate| candidate == "dangerous_secret.read"),
            "high-risk or confirmation-required read-like manifests must not become model-selectable candidates"
        );
        assert!(
            !contract.contains("candidateId=dangerous_secret.read"),
            "metadata-safe candidate contract must exclude high-risk read-like manifests"
        );
    }

    #[test]
    fn main_chat_react_tool_selection_helpers_are_extracted_from_lib_rs() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_react_tool_selection.rs");

        assert!(
            source.contains("pub(crate) mod main_chat_react_tool_selection;"),
            "Main Chat ReAct tool-selection helpers must live in a focused module"
        );
        assert!(
            module_path.is_file(),
            "Main Chat ReAct tool-selection module file must exist outside lib.rs"
        );
        assert!(
            !source.contains("\npub(crate) struct MainChatReactToolCandidate"),
            "tool candidate struct should not stay concentrated in lib.rs"
        );
        assert!(
            !source.contains("\npub(crate) struct MainChatReactActionPlan"),
            "action plan struct should not stay concentrated in lib.rs"
        );
        assert!(
            !source.contains("\nfn main_chat_governed_mcp_read_tool_candidates("),
            "governed MCP candidate selection should not stay concentrated in lib.rs"
        );
    }

    #[test]
    fn main_chat_hs_runtime_helpers_are_extracted_from_lib_rs() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_hs_runtime.rs");

        assert!(
            source.contains("pub(crate) mod main_chat_hs_runtime;"),
            "Main Chat HS runtime helpers must live in a focused module"
        );
        assert!(
            module_path.is_file(),
            "Main Chat HS runtime module file must exist outside lib.rs"
        );
        let module_source =
            std::fs::read_to_string(&module_path).expect("read src/main_chat_hs_runtime.rs");
        assert!(
            module_source.contains("pub(crate) async fn build_chat_runtime_hs_packet("),
            "HS packet builder must be reusable outside src/lib.rs"
        );
        assert!(
            module_source.contains("pub(crate) fn classify_hs_policy_topic("),
            "HS topic classifier must be reusable outside src/lib.rs"
        );
        assert!(
            module_source.contains("pub(crate) fn hs_tool_requirements("),
            "HS tool requirement classifier must be reusable outside src/lib.rs"
        );
        assert!(
            module_source.contains("pub(crate) fn included_life_model_sections("),
            "LifeModel section metadata helper must be reusable outside src/lib.rs"
        );
        assert!(
            !source.contains("\npub(crate) async fn build_chat_runtime_hs_packet("),
            "HS packet builder should not stay concentrated in lib.rs"
        );
        assert!(
            !source.contains("\nfn classify_hs_policy_topic("),
            "HS topic classifier should not stay concentrated in lib.rs"
        );
        assert!(
            !source.contains("\nfn hs_tool_requirements("),
            "HS tool requirement classifier should not stay concentrated in lib.rs"
        );
        assert!(
            !source.contains("\nfn included_life_model_sections("),
            "LifeModel section metadata helper should not stay concentrated in lib.rs"
        );
    }

    #[test]
    fn main_chat_react_agent_loop_configures_tool_allowlist_from_candidate_set() {
        let module_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_react_runtime.rs");
        let source = std::fs::read_to_string(module_path).expect("read main_chat_react_runtime.rs");
        let attempt_body = extract_rust_function_body(
            &source,
            "pub(crate) async fn try_run_main_chat_react_agent_loop(",
        );

        assert!(
            attempt_body.contains("toolset_allowlist: agent_loop_plan.allowed_tool_targets()"),
            "AgentLoop config must enforce the governed candidate target set through toolset_allowlist"
        );
    }

    #[test]
    fn main_chat_context_loader_declares_controlled_knowledge_format_surfaces() {
        let module_path = format!(
            "{}/src/main_chat_context_loader.rs",
            env!("CARGO_MANIFEST_DIR")
        );
        let source = std::fs::read_to_string(module_path)
            .expect("Main Chat knowledge context loader module should exist");

        for required_surface in [
            "AGENTS.md",
            "SOUL.md",
            "USER.md",
            "MEMORY.md",
            "memories/USER.md",
            "memories/MEMORY.md",
            "skills/<selected>/SKILL.md",
        ] {
            assert!(
                source.contains(required_surface),
                "knowledge context loader must declare bounded surface {required_surface}"
            );
        }
        assert!(source.contains("ContextSourceKind::SkillInstruction"));
        assert!(source.contains("selected_skill_id"));
        assert!(source.contains("validate_selected_skill_id"));
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
    fn obsolete_ordinary_chat_legacy_only_guard_wording_is_retired() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let obsolete_test_name = concat!(
            "ordinary_chat_entrypoints_",
            "do_not_dispatch_to_agent_loop_helpers"
        );
        let obsolete_assertion = concat!(
            "ordinary send_message must not construct ",
            "AgentLoop or ActionExecutor"
        );

        assert!(
            !source.contains(obsolete_test_name),
            "ordinary Chat tests should no longer describe Main Chat v1 as a legacy-only route"
        );
        assert!(
            !source.contains(obsolete_assertion),
            "ordinary Chat tests should describe deprecated helper isolation, not forbid the governed Main Chat v1 strategy path"
        );
    }

    #[test]
    fn ordinary_chat_entrypoints_avoid_deprecated_agent_loop_helpers_and_direct_executor_construction(
    ) {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");

        assert!(
            !send_body.contains("send_message_with_agent_loop("),
            "ordinary send_message must not dispatch to the deprecated legacy AgentLoop helper"
        );
        assert!(
            !stream_body.contains("start_stream_message_with_agent_loop("),
            "ordinary start_stream_message must not dispatch to the deprecated legacy AgentLoop helper"
        );
        assert!(
            !send_body.contains("ActionExecutor::new(") && !send_body.contains("AgentLoop::new("),
            "ordinary send_message should delegate governed execution through Main Chat v1 instead of constructing executors inline"
        );
        assert!(
            !stream_body.contains("ActionExecutor::new(")
                && !stream_body.contains("AgentLoop::new("),
            "ordinary start_stream_message should delegate governed execution through Main Chat v1 instead of constructing executors inline"
        );
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
        let ordinary_chat_bodies = [
            ("send_message", &send_body),
            ("start_stream_message", &stream_body),
        ];
        let forbidden_command_surfaces = [
            "run_multi_strategy_agent_preview",
            "get_runtime_strategy_registry_status",
            "get_runtime_strategy_registry_status_with_state",
            "get_react_beta_execution_status",
            "get_react_beta_execution_status_with_state",
            "ReactBetaExecutionStatusReport",
            "MultiStrategyRuntimeMaturityReport",
            "RuntimeStrategyRegistry::maturity_report",
            "freeze_pre_ui_backend_read_model_contracts",
            "evaluate_final_backend_completion_gate",
            "PreUiBackendContractFreezeReport",
            "FinalBackendCompletionGateReport",
            "create_plan_execute_session",
            "get_plan_execute_session",
            "list_plan_execute_sessions",
            "update_plan_execute_session_draft",
            "finalize_plan_execute_session",
            "cancel_plan_execute_session",
            "execute_plan_execute_step",
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
            "LowEnergyRuleTraceVisibilityInput",
            "LowEnergyRuleTraceVisibilityReport",
            "evaluate_low_energy_rule_trace_visibility",
            "ensure_low_energy_rule_trace_visibility",
            "LegacyWriteRiskClass",
            "LegacyWriteConvergenceStatus",
            "LegacyWritePathKind",
            "LegacyWriteInventoryEntry",
            "LegacyWriteConvergenceReport",
            "legacy_write_convergence_inventory",
            "evaluate_legacy_write_convergence_inventory",
            "ensure_legacy_write_convergence_inventory_guard",
            "builder_apply_signals",
            "builder_apply_signals_with_state",
            "GovernedManualLifeModelOverrideRequest",
            "ManualLifeModelOverrideAuditReport",
            "GovernedSnapshotRestoreRequest",
            "restore_snapshot_governed_operation",
            "GovernedDataImportRequest",
            "import_all_data_governed_operation",
            "StateSourceDataBoundaryReport",
            "evaluate_state_source_data_boundary",
            "ensure_state_source_data_boundary",
            "evaluate_lifemodel_backend_completion_readiness",
            "LifeModelBackendCompletionReadinessReport",
            "LifeEventStore",
            "LifeEventSourceRef",
            "LifeSignalExtractorInput",
            "extract_life_signals",
            "LifeSignalBridgeInput",
            "bridge_life_signal_to_evidence",
            "LifeSignalEvidenceBridgeReport",
            "EvidenceGraphInput",
            "EvidenceGraphReport",
            "EvidenceTimelineReadModel",
            "evaluate_evidence_graph",
            "build_evidence_timeline",
            "MaturationEngineV1Input",
            "MaturationEngineV1Report",
            "MaturationEngineCandidate",
            "MaturationCandidateDomain",
            "MaturationCandidateSuppressionReport",
            "evaluate_maturation_engine_v1",
            "AcceptedGuidanceLifecycleInput",
            "AcceptedGuidanceLifecycleReport",
            "AcceptedGuidanceRollbackPath",
            "create_accepted_guidance_from_maturation_candidate",
            "deactivate_accepted_guidance",
            "LifeModelVersionReadModel",
            "LifeModelVersionAssetDiffRef",
            "LifeModelRollbackReadModelRef",
            "build_lifemodel_version_read_model",
            "materialize_yaml_compatibility_view_with_provenance",
            "extract_hs_compatibility_view_from_yaml",
            "RuntimeGuidanceConsumptionMode::ExplicitRuntime",
            "with_guidance_consumption_mode",
            "apply_react_guidance_to_config",
            "build_guidance_impact_read_model",
            "GuidanceAffectedSurface::ReactPrompt",
            "GuidanceAffectedSurface::ReactConfig",
            "GuidanceAffectedSurface::ActionBoundary",
            "GuidanceAffectedSurface::PlanExecuteDraft",
            "GuidanceAffectedSurface::PlanExecuteTrace",
            "run_weekly_planning_golden_path",
            "run_low_energy_support_golden_path",
            "run_preference_correction_golden_path",
            "WeeklyPlanningGoldenPathInput",
            "WeeklyPlanningGoldenPathReport",
            "LowEnergySupportGoldenPathInput",
            "LowEnergySupportGoldenPathReport",
            "PreferenceCorrectionGoldenPathInput",
            "PreferenceCorrectionGoldenPathReport",
            "get_skill_runtime_status",
            "get_skill_runtime_status_with_state",
            "run_skill",
            "SkillRuntimeStatusReport",
            "SkillRuntimeReadinessReport",
            "run_main_chat_agent_execution_v1_eval_gate",
            "run_main_chat_agent_execution_v1_eval_gate_with_state",
            "MainChatAgentExecutionV1EvalGateReport",
            "run_main_chat_agent_execution_v1_final_acceptance_gate",
            "run_main_chat_agent_execution_v1_final_acceptance_gate_with_state",
            "MainChatAgentExecutionV1FinalAcceptanceGateCommandReport",
        ];

        for forbidden in forbidden_command_surfaces {
            for (body_name, body) in ordinary_chat_bodies {
                assert!(
                    !body.contains(forbidden),
                    "{body_name} must not call or enable {forbidden}"
                );
            }
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
            "getRuntimeStrategyRegistryStatus",
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
            guidance_refs: vec![],
            estimated_tokens: 0,
            audit: openlife_core::agent::HSSelectionAudit {
                agent_task_id: None,
                agent_run_id: Some("run-fallback-hs".into()),
                input_digest: "input-digest".into(),
                selected_policy_ids: vec![
                    openlife_core::agent::BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
                ],
                selected_heuristic_ids: vec![],
                selected_guidance_ids: vec![],
                selected_guidance_refs: vec![],
                excluded_assets: vec![],
                estimated_tokens: 0,
                token_budget: 128,
            },
        }
    }

    fn main_chat_invoke_request(
        cmd: &str,
        body: serde_json::Value,
    ) -> tauri::webview::InvokeRequest {
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        }
    }

    #[test]
    fn workspace_file_resolver_accepts_explicit_nested_workspace_relative_path() {
        let (label, path) = main_chat_workspace_file_target("Read plans/README.md").unwrap();

        assert_eq!(label, "plans/README.md");
        assert!(path.ends_with("plans/README.md"));
    }

    #[test]
    fn workspace_file_resolver_blocks_explicit_path_traversal() {
        let error = main_chat_workspace_file_target("Read ../Cargo.toml").unwrap_err();

        assert!(error.contains("outside workspace") || error.contains("path traversal"));
    }

    fn create_main_chat_knowledge_workspace() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("create knowledge workspace");
        std::fs::create_dir_all(root.path().join("plans")).expect("create plans dir");
        std::fs::create_dir_all(root.path().join("memories")).expect("create memories dir");
        std::fs::create_dir_all(root.path().join("skills/summarize"))
            .expect("create selected skill dir");
        std::fs::create_dir_all(root.path().join("skills/other")).expect("create other skill dir");
        std::fs::write(
            root.path().join("AGENTS.md"),
            format!("workspace instruction\n{}", "A".repeat(1600)),
        )
        .expect("write AGENTS.md");
        std::fs::write(root.path().join("SOUL.md"), "bounded soul context").expect("write SOUL.md");
        std::fs::write(root.path().join("memories/USER.md"), "bounded user context")
            .expect("write USER.md");
        std::fs::write(
            root.path().join("memories/MEMORY.md"),
            "bounded memory context",
        )
        .expect("write MEMORY.md");
        std::fs::write(
            root.path().join("skills/summarize/SKILL.md"),
            "selected summarize skill instructions",
        )
        .expect("write selected SKILL.md");
        std::fs::write(
            root.path().join("skills/other/SKILL.md"),
            "unselected skill instructions must not load",
        )
        .expect("write other SKILL.md");
        root
    }

    #[test]
    fn main_chat_knowledge_context_loader_loads_bounded_workspace_formats() {
        let root = create_main_chat_knowledge_workspace();
        let candidates =
            crate::main_chat_context_loader::load_workspace_knowledge_context_candidates(
                root.path(),
                Some("summarize"),
            );
        let source_ids = candidates
            .iter()
            .map(|candidate| candidate.source_id.as_str())
            .collect::<Vec<_>>();

        assert!(source_ids.contains(&"AGENTS.md"));
        assert!(source_ids.contains(&"SOUL.md"));
        assert!(source_ids.contains(&"memories/USER.md"));
        assert!(source_ids.contains(&"memories/MEMORY.md"));
        assert!(source_ids.contains(&"skills/summarize/SKILL.md"));
        assert!(!source_ids.contains(&"skills/other/SKILL.md"));
        assert!(candidates
            .iter()
            .all(|candidate| candidate.content.chars().count() <= 1200));
        let selected_skill = candidates
            .iter()
            .find(|candidate| candidate.source_id == "skills/summarize/SKILL.md")
            .expect("selected skill candidate");
        assert_eq!(
            selected_skill.source_kind,
            openlife_core::agent::main_chat_agent_v1::ContextSourceKind::SkillInstruction
        );
        assert_eq!(
            selected_skill.selected_skill_id.as_deref(),
            Some("summarize")
        );
    }

    #[test]
    fn main_chat_knowledge_context_loader_does_not_load_unselected_skill_instruction() {
        let root = create_main_chat_knowledge_workspace();
        let candidates =
            crate::main_chat_context_loader::load_workspace_knowledge_context_candidates(
                root.path(),
                None,
            );

        assert!(!candidates.iter().any(|candidate| {
            candidate.source_kind
                == openlife_core::agent::main_chat_agent_v1::ContextSourceKind::SkillInstruction
        }));
    }

    #[test]
    fn main_chat_context_compiler_is_extracted_to_context_loader() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_context_loader.rs");
        let module_source =
            std::fs::read_to_string(&module_path).expect("read src/main_chat_context_loader.rs");
        let compile_body = extract_rust_function_body(
            &module_source,
            "pub(crate) async fn compile_main_chat_context(",
        );

        assert!(
            compile_body.contains("load_current_workspace_knowledge_context_candidates"),
            "Main Chat context assembly must use the controlled knowledge-format loader"
        );
        assert!(
            !compile_body.contains("std::fs::read_to_string(&path)"),
            "Main Chat context assembly must not regress to ad hoc AGENTS.md-only reads"
        );
        assert!(
            module_source.contains("selected_skill_id: Option<&str>"),
            "Main Chat context compiler must accept a selected skill id from ordinary chat surfaces"
        );
        assert!(
            !compile_body.contains("let selected_skill_id: Option<String> = None;"),
            "Main Chat context compiler must not discard selected skill ids"
        );
        assert!(
            module_source.contains("pub(crate) fn sanitize_main_chat_selected_skill_id("),
            "selected skill id sanitization should live with Main Chat context loading"
        );
        assert!(
            !source.contains("\nasync fn compile_main_chat_context("),
            "Main Chat context compiler should not remain in lib.rs"
        );
        assert!(
            !source.contains("\nfn sanitize_main_chat_selected_skill_id("),
            "selected skill id sanitizer should not remain in lib.rs"
        );
    }

    #[test]
    fn main_chat_runtime_support_helpers_are_extracted_from_lib_rs() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_runtime_support.rs");
        assert!(
            module_path.exists(),
            "Main Chat runtime support helper module file must exist outside lib.rs"
        );
        let module_source =
            std::fs::read_to_string(&module_path).expect("read src/main_chat_runtime_support.rs");

        for expected in [
            "pub(crate) struct MainChatAgentTurn",
            "pub(crate) async fn start_main_chat_agent_turn(",
            "pub(crate) async fn append_main_chat_agent_transcript(",
            "pub(crate) async fn append_main_chat_direct_answer_contract_transcript(",
            "pub(crate) async fn complete_main_chat_agent_turn_session(",
            "pub(crate) async fn enqueue_main_chat_agent_action(",
            "pub(crate) async fn transition_main_chat_action(",
            "pub(crate) async fn fail_main_chat_action(",
        ] {
            assert!(
                module_source.contains(expected),
                "runtime support module must expose {expected}"
            );
        }
        for forbidden in [
            "\nstruct MainChatAgentTurn",
            "\nasync fn start_main_chat_agent_turn(",
            "\npub(crate) async fn append_main_chat_agent_transcript(",
            "\nasync fn append_main_chat_direct_answer_contract_transcript(",
            "\nasync fn complete_main_chat_agent_turn_session(",
            "\nasync fn enqueue_main_chat_agent_action(",
            "\npub(crate) async fn transition_main_chat_action(",
            "\npub(crate) async fn fail_main_chat_action(",
        ] {
            assert!(
                !source.contains(forbidden),
                "runtime support helper {forbidden} should not remain in lib.rs"
            );
        }
    }

    #[test]
    fn main_chat_generation_support_helpers_are_extracted_from_lib_rs() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_generation_support.rs");
        assert!(
            module_path.exists(),
            "Main Chat generation/finalization support module file must exist outside lib.rs"
        );
        let module_source = std::fs::read_to_string(&module_path)
            .expect("read src/main_chat_generation_support.rs");

        for expected in [
            "pub(crate) async fn persist_chat_message_if_needed(",
            "pub(crate) async fn persist_vector_memory_for_message(",
            "pub(crate) async fn finalize_chat_agent_run(",
            "pub(crate) async fn generate_non_stream_fallback(",
            "pub(crate) fn main_chat_provider_endpoint_kind(",
            "pub(crate) fn preview_text(",
        ] {
            assert!(
                module_source.contains(expected),
                "generation support module must expose {expected}"
            );
        }
        for forbidden in [
            "\nasync fn persist_chat_message_if_needed(",
            "\nasync fn persist_vector_memory_for_message(",
            "\nasync fn generate_and_persist_chat_proposals(",
            "\nasync fn finalize_chat_agent_run(",
            "\npub(crate) async fn generate_non_stream_fallback(",
            "\npub(crate) fn main_chat_provider_endpoint_kind(",
            "\npub(crate) fn preview_text(",
        ] {
            assert!(
                !source.contains(forbidden),
                "generation support helper {forbidden} should not remain in lib.rs"
            );
        }
    }

    #[test]
    fn main_chat_react_runtime_helpers_are_extracted_from_lib_rs() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let module_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_react_runtime.rs");
        assert!(
            module_path.exists(),
            "Main Chat ReAct runtime helper module file must exist outside lib.rs"
        );
        let module_source =
            std::fs::read_to_string(&module_path).expect("read src/main_chat_react_runtime.rs");

        for expected in [
            "pub(crate) struct MainChatObservation",
            "pub(crate) struct MainChatReactFollowUp",
            "pub(crate) struct MainChatReactAgentLoopAttempt",
            "pub(crate) async fn synthesize_main_chat_react_follow_up(",
            "pub(crate) fn main_chat_permission_blocker_reason(",
            "pub(crate) fn blocked_main_chat_observation(",
            "pub(crate) fn tool_call_from_action(",
            "pub(crate) fn agent_actions_to_tool_call_results(",
            "pub(crate) async fn try_run_main_chat_react_agent_loop(",
        ] {
            assert!(
                module_source.contains(expected),
                "ReAct runtime helper module must expose {expected}"
            );
        }
        for forbidden in [
            "\npub(crate) struct MainChatObservation",
            "\nstruct MainChatReactFollowUp",
            "\nstruct MainChatReactAgentLoopAttempt",
            "\nasync fn synthesize_main_chat_react_follow_up(",
            "\nfn main_chat_permission_blocker_reason(",
            "\nfn blocked_main_chat_observation(",
            "\nfn tool_call_from_action(",
            "\nfn agent_actions_to_tool_call_results(",
            "\nasync fn try_run_main_chat_react_agent_loop(",
        ] {
            assert!(
                !source.contains(forbidden),
                "ReAct runtime helper {forbidden} should not remain in lib.rs"
            );
        }
    }

    #[test]
    fn main_chat_proposal_support_helpers_are_extracted_from_lib_rs() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        assert!(
            source.contains("pub(crate) mod main_chat_proposal_support;"),
            "Main Chat proposal support module must be declared from lib.rs"
        );
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_proposal_support.rs");
        assert!(
            module_path.exists(),
            "Main Chat proposal support module file must exist outside lib.rs"
        );
        let module_source =
            std::fs::read_to_string(&module_path).expect("read src/main_chat_proposal_support.rs");

        for expected in [
            "pub(crate) async fn create_main_chat_agent_proposal(",
            "pub(crate) async fn attach_main_chat_tool_permission_proposal_metadata(",
        ] {
            assert!(
                module_source.contains(expected),
                "proposal support module must expose {expected}"
            );
        }
        for forbidden in [
            "\nasync fn create_main_chat_agent_proposal(",
            "\nasync fn attach_main_chat_tool_permission_proposal_metadata(",
        ] {
            assert!(
                !source.contains(forbidden),
                "proposal support helper {forbidden} should not remain in lib.rs"
            );
        }
    }

    #[test]
    fn main_chat_react_execution_helper_is_extracted_from_lib_rs() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        assert!(
            source.contains("pub(crate) mod main_chat_react_execution;"),
            "Main Chat ReAct execution module must be declared from lib.rs"
        );
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_react_execution.rs");
        assert!(
            module_path.exists(),
            "Main Chat ReAct execution module file must exist outside lib.rs"
        );
        let module_source =
            std::fs::read_to_string(&module_path).expect("read src/main_chat_react_execution.rs");

        assert!(
            module_source
                .contains("pub(crate) async fn execute_main_chat_react_action_with_executor("),
            "ReAct execution module must expose the ActionExecutor fallback helper"
        );
        assert!(
            module_source.contains("ActionExecutor::new("),
            "ReAct execution module must own ActionExecutor fallback construction"
        );
        assert!(
            module_source.contains("resolve_main_chat_mcp_read_target("),
            "ReAct execution module must preserve registered MCP read resolution"
        );
        assert!(
            !source.contains("\npub(crate) async fn execute_main_chat_react_action_with_executor("),
            "ActionExecutor fallback helper should not remain in lib.rs"
        );
    }

    #[test]
    fn main_chat_selected_skill_id_is_sanitized_before_context_compilation() {
        assert_eq!(
            sanitize_main_chat_selected_skill_id(Some(" summarize ")).as_deref(),
            Some("summarize")
        );
        assert_eq!(
            sanitize_main_chat_selected_skill_id(Some("planner.v1_beta-2")).as_deref(),
            Some("planner.v1_beta-2")
        );
        assert!(sanitize_main_chat_selected_skill_id(Some("../summarize")).is_none());
        assert!(sanitize_main_chat_selected_skill_id(Some("skills/summarize")).is_none());
        assert!(sanitize_main_chat_selected_skill_id(Some("bad skill")).is_none());
        assert!(sanitize_main_chat_selected_skill_id(None).is_none());
    }

    #[test]
    fn ordinary_chat_commands_plumb_selected_skill_id_to_context_loader() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");
        let stream_body = extract_rust_function_body(
            &source,
            "async fn start_stream_message<R: tauri::Runtime>(",
        );

        assert!(
            source.contains("selected_skill_id: Option<String>"),
            "ordinary chat command inputs must expose optional selected skill id"
        );
        assert!(
            send_body.contains("selected_skill_id.as_deref()"),
            "send_message must pass selected skill id into Main Chat context assembly"
        );
        assert!(
            stream_body.contains("selected_skill_id.as_deref()"),
            "start_stream_message must pass selected skill id into Main Chat context assembly"
        );
        assert!(
            source.contains("selected_skill_id: Option<String>,"),
            "stream args must carry optional selected skill id for args payloads"
        );
    }

    #[test]
    fn main_chat_final_gate_aggregation_is_not_hidden_in_test_module() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");

        assert!(
            source.contains("pub(crate) mod main_chat_final_gate;"),
            "pure Main Chat final-gate aggregation must live in a non-test module"
        );
        assert!(
            source.contains(
                "main_chat_final_gate::build_main_chat_agent_execution_v1_final_gate_report("
            ),
            "the final acceptance runner must use the reusable final-gate aggregation module"
        );
        assert!(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/main_chat_final_gate.rs")
                .is_file(),
            "final-gate aggregation module file must exist outside #[cfg(test)]"
        );
    }

    #[test]
    fn main_chat_command_surface_eval_report_normalization_is_not_hidden_in_test_module() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_command_surface_eval.rs");

        assert!(
            source.contains("pub(crate) mod main_chat_command_surface_eval;"),
            "command-surface eval case matrix and evidence normalization must live in a non-test module"
        );
        assert!(
            module_path.is_file(),
            "command-surface eval report module file must exist outside #[cfg(test)]"
        );
        let module_source =
            std::fs::read_to_string(&module_path).expect("read command-surface eval module");
        assert!(
            module_source.contains("MainChatCommandSurfaceEvalReport"),
            "command-surface eval report type must be reusable by production/test code"
        );
        assert!(
            module_source.contains("MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES"),
            "the 24-case send/stream command-surface matrix must not be hidden in tests"
        );
        assert!(
            module_source.contains("acceptance_evidence("),
            "command-surface acceptance evidence normalization must be reusable outside tests"
        );
        assert!(
            module_source.contains("from_case_evidence("),
            "command-surface report aggregation must be reusable outside tests"
        );
        assert!(
            source.contains("MainChatCommandSurfaceEvalReport::from_case_evidence("),
            "the 24-case command-surface runner must call the reusable report aggregation"
        );
    }

    #[test]
    fn main_chat_live_provider_blocked_report_builder_is_not_hidden_in_test_module() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_final_gate.rs");
        let module_source = std::fs::read_to_string(&module_path).expect("read final gate module");

        assert!(
            module_source
                .contains("pub(crate) fn blocked_main_chat_live_provider_eval_harness_report("),
            "preflight-blocked live-provider harness reports must be built by reusable production code"
        );
        assert!(
            source.contains(
                "main_chat_final_gate::blocked_main_chat_live_provider_eval_harness_report("
            ),
            "the live-provider harness must use the reusable blocked-report builder"
        );
    }

    #[test]
    fn main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_final_gate.rs");
        let module_source = std::fs::read_to_string(&module_path).expect("read final gate module");

        assert!(
            module_source
                .contains("pub(crate) fn completed_main_chat_live_provider_eval_harness_report("),
            "completed live-provider harness report shape must be reusable production code"
        );
        assert!(
            module_source.contains("pub(crate) fn main_chat_live_provider_required_evidence("),
            "live-provider required-evidence list must not be duplicated in test helpers"
        );
        assert!(
            source.contains(
                "main_chat_final_gate::completed_main_chat_live_provider_eval_harness_report("
            ),
            "final-gate tests must build completed live-provider reports through the reusable helper"
        );
    }

    #[test]
    fn main_chat_live_provider_harness_execution_is_not_concentrated_in_lib_rs() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_live_provider_harness.rs");

        assert!(
            source.contains("pub(crate) mod main_chat_live_provider_harness;"),
            "live-provider harness execution must live in a focused non-test module"
        );
        assert!(
            module_path.is_file(),
            "live-provider harness execution module file must exist outside #[cfg(test)]"
        );
        let module_source =
            std::fs::read_to_string(&module_path).expect("read live-provider harness module");
        assert!(
            module_source.contains("run_main_chat_live_provider_eval_harness_suite_from_state"),
            "live-provider harness suite must be reusable by the real final acceptance runner"
        );
        assert!(
            module_source.contains("send_message_with_state("),
            "live-provider harness execution must use the ordinary Main Chat send path"
        );
        assert!(
            !source.contains(
                "\npub(crate) async fn run_main_chat_live_provider_eval_harness_suite_from_state("
            ),
            "live-provider harness suite must not remain concentrated in src/lib.rs"
        );
        assert!(
            !source.contains("\npub(crate) async fn run_main_chat_live_provider_eval_harness("),
            "live-provider harness execution must not remain concentrated in src/lib.rs"
        );
    }

    #[test]
    fn main_chat_task_controls_are_not_concentrated_in_lib_rs() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_task_controls.rs");

        assert!(
            source.contains("pub(crate) mod main_chat_task_controls;"),
            "Main Chat task-control commands must live in a focused non-test module"
        );
        assert!(
            module_path.is_file(),
            "Main Chat task-control module file must exist outside #[cfg(test)]"
        );
        let module_source =
            std::fs::read_to_string(&module_path).expect("read task-control module");
        assert!(
            module_source.contains("pub struct MainChatAgentTaskState"),
            "task-state response shape must move with the task-control commands"
        );
        assert!(
            module_source.contains("pub(crate) async fn resume_main_chat_agent_task("),
            "resume command implementation must be reusable outside src/lib.rs"
        );
        assert!(
            module_source.contains("pub(crate) async fn cancel_main_chat_agent_task("),
            "cancel command implementation must be reusable outside src/lib.rs"
        );
        assert!(
            module_source.contains("pub(crate) async fn retry_main_chat_agent_action("),
            "retry command implementation must be reusable outside src/lib.rs"
        );
        assert!(
            !source.contains("\nasync fn resume_main_chat_agent_task("),
            "resume command body must not remain concentrated in src/lib.rs"
        );
        assert!(
            !source.contains("\nasync fn cancel_main_chat_agent_task("),
            "cancel command body must not remain concentrated in src/lib.rs"
        );
        assert!(
            !source.contains("\nasync fn retry_main_chat_agent_action("),
            "retry command body must not remain concentrated in src/lib.rs"
        );
    }

    #[test]
    fn isolated_main_chat_eval_state_factory_is_not_hidden_in_test_module() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_eval_state.rs");

        assert!(
            source.contains("pub(crate) mod main_chat_eval_state;"),
            "isolated Main Chat eval state factory must live in a non-test module"
        );
        assert!(
            module_path.is_file(),
            "isolated Main Chat eval state module file must exist outside #[cfg(test)]"
        );
        let module_source = std::fs::read_to_string(&module_path).expect("read eval state module");
        assert!(
            module_source.contains("build_isolated_main_chat_eval_state"),
            "production/test code must share an isolated state factory for command-surface evidence"
        );
        assert!(
            !module_source.contains("#[cfg(test)]"),
            "isolated eval state factory must be callable by the real non-default final gate"
        );
    }

    #[test]
    fn main_chat_command_surface_eval_scenario_setup_is_not_hidden_in_test_module() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_command_surface_eval.rs");
        let module_source =
            std::fs::read_to_string(&module_path).expect("read command-surface eval module");

        assert!(
            module_source
                .contains("pub(crate) async fn configure_main_chat_command_surface_eval_state("),
            "scenario-specific command-surface eval state setup must be reusable outside #[cfg(test)]"
        );
        assert!(
            module_source.contains("pub(crate) fn main_chat_command_surface_eval_user_text("),
            "scenario prompt mapping must be reusable outside #[cfg(test)]"
        );
        assert!(
            module_source.contains("pub(crate) fn main_chat_command_surface_eval_session_id("),
            "deterministic session-id mapping must be reusable outside #[cfg(test)]"
        );
        assert!(
            !source.contains("\n    async fn configure_main_chat_command_surface_eval_state("),
            "scenario setup must not remain as a test-only helper in src/lib.rs"
        );
        assert!(
            !source.contains("\n    fn main_chat_command_surface_eval_user_text("),
            "scenario prompt mapping must not remain as a test-only helper in src/lib.rs"
        );
    }

    #[test]
    fn main_chat_command_surface_eval_assertions_are_not_hidden_in_test_module() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_command_surface_eval.rs");
        let module_source =
            std::fs::read_to_string(&module_path).expect("read command-surface eval module");

        assert!(
            module_source
                .contains("pub(crate) async fn assert_main_chat_command_surface_eval_case("),
            "command-surface case interpretation must be reusable outside #[cfg(test)]"
        );
        assert!(
            !module_source.contains(
                "#[cfg(test)]\npub(crate) async fn assert_main_chat_command_surface_eval_case("
            ),
            "command-surface case interpretation must not be cfg(test)-gated"
        );
        assert!(
            module_source
                .contains("pub(crate) fn main_chat_command_surface_eval_has_silent_write("),
            "no-silent-write detection must be reusable outside #[cfg(test)]"
        );
        assert!(
            !module_source.contains(
                "#[cfg(test)]\npub(crate) fn main_chat_command_surface_eval_has_silent_write("
            ),
            "no-silent-write detection must not be cfg(test)-gated"
        );
        assert!(
            !source.contains("\n    async fn assert_main_chat_command_surface_eval_case("),
            "command-surface assertions must not remain as test-only helpers in src/lib.rs"
        );
        assert!(
            !source.contains("\n    fn main_chat_command_surface_eval_has_silent_write("),
            "no-silent-write detection must not remain as a test-only helper in src/lib.rs"
        );
    }

    #[test]
    fn main_chat_command_surface_send_eval_runner_uses_case_assertions() {
        let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_command_surface_eval.rs");
        let module_source =
            std::fs::read_to_string(&module_path).expect("read command-surface eval module");
        let send_case_body = extract_rust_function_body(
            &module_source,
            "async fn run_main_chat_command_surface_state_eval_case(",
        );

        assert!(
            send_case_body.contains("assert_main_chat_command_surface_eval_case("),
            "production command-surface eval must verify real scenario effects before awarding coverage"
        );
        assert!(
            send_case_body.contains("list_transcript_entries("),
            "production command-surface eval must inspect transcript evidence"
        );
        assert!(
            send_case_body.contains("list_pending_proposals(20)"),
            "production command-surface eval must inspect proposal evidence"
        );
        assert!(
            send_case_body.contains("start_stream_message_with_state("),
            "production command-surface eval must execute stream cases through the reusable stream state executor"
        );
    }

    #[test]
    fn main_chat_send_command_has_non_tauri_state_executor() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let send_body = extract_rust_function_body(&source, "async fn send_message(");

        assert!(
            source.contains("\npub(crate) async fn send_message_with_state("),
            "send_message must delegate to an Arc<AppState> executor that non-default gates can call without tauri::State"
        );
        assert!(
            send_body.contains(
                "send_message_with_state(session_id, messages, selected_skill_id, state.inner()).await"
            ),
            "the Tauri command wrapper must call the reusable send_message_with_state executor"
        );
    }

    #[test]
    fn main_chat_stream_command_has_non_tauri_state_executor() {
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
        let stream_body = extract_rust_function_body(
            &source,
            "async fn start_stream_message<R: tauri::Runtime>(",
        );

        assert!(
            source.contains("\npub(crate) async fn start_stream_message_with_state("),
            "start_stream_message must delegate to an Arc<AppState> executor that final gates can call without tauri::State or mock IPC"
        );
        assert!(
            stream_body.contains("start_stream_message_with_state("),
            "the Tauri stream command wrapper must call the reusable start_stream_message_with_state executor"
        );
    }

    fn successful_live_provider_harness_report(
        scenario: MainChatLiveProviderEvalHarnessScenario,
    ) -> MainChatLiveProviderEvalHarnessReport {
        main_chat_final_gate::completed_main_chat_live_provider_eval_harness_report(
            scenario,
            "openai",
            "external_provider",
            format!("live-run-{}", scenario.as_str()),
            format!("live-task-{}", scenario.as_str()),
            "Live provider response.",
        )
    }

    fn blocked_live_provider_harness_report(
        blocker: &str,
    ) -> MainChatLiveProviderEvalHarnessReport {
        main_chat_final_gate::blocked_main_chat_live_provider_eval_harness_report(
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            "openai",
            "external_provider",
            vec![blocker.into()],
            main_chat_final_gate::main_chat_live_provider_required_evidence(),
        )
    }

    #[derive(Debug, Clone, Copy)]
    enum MainChatLiveProviderEvalConfigMode {
        FromEnvironment,
        NoCredentials,
    }

    async fn run_main_chat_agent_execution_v1_final_acceptance_gate(
        include_live_provider: bool,
    ) -> MainChatAgentExecutionV1FinalGateReport {
        run_main_chat_agent_execution_v1_final_acceptance_gate_with_config_mode(
            include_live_provider,
            MainChatLiveProviderEvalConfigMode::FromEnvironment,
        )
        .await
    }

    async fn run_main_chat_agent_execution_v1_final_acceptance_gate_with_config_mode(
        include_live_provider: bool,
        live_config_mode: MainChatLiveProviderEvalConfigMode,
    ) -> MainChatAgentExecutionV1FinalGateReport {
        let runtime_report =
            openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
                openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
            );
        let command_surface_report = run_main_chat_command_surface_eval_gate().await;
        let live_reports = if include_live_provider {
            let mut reports = Vec::new();
            for scenario in [
                MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
                MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
                MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
                MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
            ] {
                let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
                match live_config_mode {
                    MainChatLiveProviderEvalConfigMode::FromEnvironment => {
                        configure_live_provider_eval_state(&state).await;
                    }
                    MainChatLiveProviderEvalConfigMode::NoCredentials => {
                        configure_live_provider_eval_state_without_credentials(&state).await;
                    }
                }
                match run_main_chat_live_provider_eval_harness(
                    state,
                    MainChatLiveProviderEvalHarnessInput {
                        scenario,
                        session_id: format!("final-acceptance-live-{}", scenario.as_str()),
                        prompt: scenario.prompt().into(),
                        explicit_live_eval_requested: true,
                        local_only_required: false,
                    },
                )
                .await
                {
                    Ok(report) => reports.push(report),
                    Err(error) => reports.push(MainChatLiveProviderEvalHarnessReport {
                        scenario,
                        ready: false,
                        status: "failed".into(),
                        provider: String::new(),
                        provider_endpoint_kind: "error".into(),
                        blockers: vec![error],
                        required_evidence: Vec::new(),
                        live_provider_invocation_allowed: false,
                        main_chat_invoked: false,
                        model_invoked: false,
                        direct_writes_executed: false,
                        legacy_fallback_used: false,
                        agent_loop_succeeded: false,
                        single_step_fallback_used: false,
                        agent_loop_action_status: None,
                        mcp_read_target_resolved: false,
                        tool_permission_proposal_created: false,
                        run_id: None,
                        task_session_id: None,
                        response_preview: None,
                    }),
                }
            }
            reports
        } else {
            Vec::new()
        };
        main_chat_final_gate::build_main_chat_agent_execution_v1_final_gate_report(
            runtime_report,
            command_surface_report.total_cases,
            command_surface_report.acceptance_evidence(),
            include_live_provider,
            live_reports,
        )
    }

    fn main_chat_agent_execution_v1_final_gate_report_from_parts(
        runtime_report: openlife_core::agent::main_chat_agent_v1::MainChatRuntimeEvalReport,
        command_surface_report: MainChatCommandSurfaceEvalReport,
        live_provider_attempted: bool,
        live_reports: Vec<MainChatLiveProviderEvalHarnessReport>,
    ) -> MainChatAgentExecutionV1FinalGateReport {
        main_chat_final_gate::build_main_chat_agent_execution_v1_final_gate_report(
            runtime_report,
            command_surface_report.total_cases,
            command_surface_report.acceptance_evidence(),
            live_provider_attempted,
            live_reports,
        )
    }

    async fn configure_live_provider_eval_state(state: &Arc<AppState>) {
        {
            let mut config = state.config.lock().await;
            config.llm.provider =
                std::env::var("OPENLIFE_LIVE_EVAL_PROVIDER").unwrap_or_else(|_| "openai".into());
            config.llm.openai_base = std::env::var("OPENLIFE_LIVE_EVAL_BASE")
                .unwrap_or_else(|_| "https://api.openai.com/v1".into());
            config.llm.chat_model =
                std::env::var("OPENLIFE_LIVE_EVAL_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
            config.llm.openai_key = std::env::var("OPENLIFE_LIVE_EVAL_API_KEY")
                .unwrap_or_else(|_| std::env::var("OPENAI_API_KEY").unwrap_or_default());
            config.system.network_policy.enabled = true;
        }
        {
            let config = state.config.lock().await.clone();
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                config.local_model.clone(),
                false,
                config.llm.provider.clone(),
                config.llm.openai_base.clone(),
                config.llm.openai_key.clone(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                false,
            );
        }
    }

    async fn configure_live_provider_eval_state_without_credentials(state: &Arc<AppState>) {
        {
            let mut config = state.config.lock().await;
            config.llm.provider = "openai".into();
            config.llm.openai_base = "https://api.openai.com/v1".into();
            config.llm.chat_model = "gpt-4o-mini".into();
            config.llm.openai_key.clear();
            config.system.network_policy.enabled = true;
        }
        {
            let config = state.config.lock().await.clone();
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                config.local_model.clone(),
                false,
                config.llm.provider.clone(),
                config.llm.openai_base.clone(),
                String::new(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                false,
            );
        }
    }

    async fn configure_live_provider_eval_state_with_local_http_provider(
        state: &Arc<AppState>,
        reply: &'static str,
    ) {
        let provider_base = fake_local_chat_provider_endpoint(reply).await;
        {
            let mut config = state.config.lock().await;
            config.llm.provider = "openai".into();
            config.llm.openai_base = provider_base.clone();
            config.llm.chat_model = "gpt-local-provider-harness".into();
            config.llm.openai_key = "test-key".into();
            config.system.network_policy.enabled = true;
        }
        {
            let config = state.config.lock().await.clone();
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                config.local_model.clone(),
                false,
                config.llm.provider.clone(),
                provider_base,
                config.llm.openai_key.clone(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                false,
            );
        }
    }

    async fn fake_local_chat_provider_endpoint(reply: &'static str) -> String {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("bind local fake chat provider");
        let addr = listener.local_addr().expect("local fake provider addr");
        std::thread::spawn(move || {
            let _ = listener.set_nonblocking(true);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            let mut handled = 0usize;
            while handled < 8 && std::time::Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        handled += 1;
                        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
                        let mut buffer = [0u8; 8192];
                        let _ = std::io::Read::read(&mut stream, &mut buffer);
                        let body = serde_json::json!({
                            "id": "chatcmpl-main-chat-live-provider-local",
                            "object": "chat.completion",
                            "choices": [{
                                "index": 0,
                                "message": {
                                    "role": "assistant",
                                    "content": reply
                                },
                                "finish_reason": "stop"
                            }]
                        })
                        .to_string();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        format!("http://{addr}/v1")
    }

    async fn run_main_chat_command_surface_eval_gate() -> MainChatCommandSurfaceEvalReport {
        let mut evidence = Vec::new();
        let mut failures = Vec::new();
        for (entry_point, scenario) in MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES {
            match run_main_chat_command_surface_eval_case(entry_point, scenario).await {
                Ok(case_evidence) => evidence.push(case_evidence),
                Err(error) => failures.push(format!("{entry_point:?}/{scenario:?}: {error}")),
            }
        }

        MainChatCommandSurfaceEvalReport::from_case_evidence(
            MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES.len(),
            evidence,
            failures,
        )
    }

    async fn run_main_chat_command_surface_eval_case(
        entry_point: MainChatCommandSurfaceEvalEntryPoint,
        scenario: MainChatCommandSurfaceEvalScenario,
    ) -> std::result::Result<MainChatCommandSurfaceEvalEvidence, String> {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        main_chat_command_surface_eval::configure_main_chat_command_surface_eval_state(
            &state, scenario,
        )
        .await?;

        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message, start_stream_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .map_err(|error| format!("build mock tauri app failed: {error}"))?;
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .map_err(|error| format!("build mock webview failed: {error}"))?;

        let session_id = main_chat_command_surface_eval::main_chat_command_surface_eval_session_id(
            entry_point,
            scenario,
        );
        let user_text =
            main_chat_command_surface_eval::main_chat_command_surface_eval_user_text(scenario);
        let messages = serde_json::json!([{ "role": "user", "content": user_text }]);
        let response = invoke_main_chat_command_surface_eval_case(
            &webview,
            entry_point,
            &session_id,
            messages,
        )?;
        let legacy_fallback_used = response
            .as_ref()
            .and_then(|value| value.get("legacy_fallback_used"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let decision = openlife_core::agent::main_chat_agent_v1::AgentIngress::default().decide(
            &session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let task_session_id = decision
            .agent_task_session_id
            .as_deref()
            .ok_or_else(|| "missing deterministic task session id".to_string())?;
        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .ok_or_else(|| "missing main chat session store".to_string())?;
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .map_err(|error| format!("load task session failed: {error}"))?
                .ok_or_else(|| "task session missing after command".to_string())?
        };
        let transcript = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .ok_or_else(|| "missing main chat session store".to_string())?;
            let store = store_arc.lock().await;
            store
                .list_transcript_entries(task_session_id)
                .map_err(|error| format!("list transcript failed: {error}"))?
        };
        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .ok_or_else(|| "missing main chat action queue store".to_string())?;
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .map_err(|error| format!("list actions failed: {error}"))?
        };
        let proposals = if let Some(ref proposal_arc) = state.proposal_store {
            let proposal_store = proposal_arc.lock().await;
            proposal_store
                .list_pending_proposals(20)
                .map_err(|error| format!("list proposals failed: {error}"))?
        } else {
            Vec::new()
        };
        let runs = if let Some(ref run_store_arc) = state.agent_run_store {
            let run_store = run_store_arc.lock().await;
            run_store
                .list_runs_for_session(&session_id, 20)
                .map_err(|error| format!("list runs failed: {error}"))?
        } else {
            Vec::new()
        };

        main_chat_command_surface_eval::assert_main_chat_command_surface_eval_case(
            scenario,
            &state,
            task_session_id,
            &session,
            &transcript,
            &actions,
            &proposals,
            &runs,
            response.as_ref(),
        )
        .await?;

        Ok(MainChatCommandSurfaceEvalEvidence {
            entry_point,
            provider_generation: scenario
                == MainChatCommandSurfaceEvalScenario::DirectProviderTrace,
            file_read: scenario == MainChatCommandSurfaceEvalScenario::FileReadSuccess,
            plan_execute: scenario == MainChatCommandSurfaceEvalScenario::PlanExecuteDraft,
            proposal: scenario == MainChatCommandSurfaceEvalScenario::ProposalPath,
            web_policy_blocker: scenario == MainChatCommandSurfaceEvalScenario::WebPolicyBlocker,
            web_agent_loop_blocker: scenario
                == MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker,
            web_agent_loop_success: scenario
                == MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess,
            mcp_missing_read_target_blocker: scenario
                == MainChatCommandSurfaceEvalScenario::MissingMcpBlocker,
            mcp_registered_read_success: matches!(
                scenario,
                MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess
                    | MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess
            ),
            mcp_agent_loop_success: scenario
                == MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess,
            mcp_tool_permission_proposal: matches!(
                scenario,
                MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal
                    | MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal
            ),
            mcp_agent_loop_tool_permission_proposal: scenario
                == MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal,
            legacy_fallback_used,
            silent_write_detected:
                main_chat_command_surface_eval::main_chat_command_surface_eval_has_silent_write(
                    response.as_ref(),
                    &transcript,
                    &actions,
                    &runs,
                ),
        })
    }

    fn invoke_main_chat_command_surface_eval_case<W>(
        webview: &W,
        entry_point: MainChatCommandSurfaceEvalEntryPoint,
        session_id: &str,
        messages: serde_json::Value,
    ) -> std::result::Result<Option<serde_json::Value>, String>
    where
        W: AsRef<tauri::Webview<tauri::test::MockRuntime>>,
    {
        match entry_point {
            MainChatCommandSurfaceEvalEntryPoint::Send => tauri::test::get_ipc_response(
                webview,
                main_chat_invoke_request(
                    "send_message",
                    serde_json::json!({
                        "sessionId": session_id,
                        "session_id": session_id,
                        "messages": messages
                    }),
                ),
            )
            .map_err(|error| format!("send_message invoke failed: {error:?}"))?
            .deserialize::<serde_json::Value>()
            .map(Some)
            .map_err(|error| format!("deserialize send response failed: {error}")),
            MainChatCommandSurfaceEvalEntryPoint::Stream => {
                let response = tauri::test::get_ipc_response(
                    webview,
                    main_chat_invoke_request(
                        "start_stream_message",
                        serde_json::json!({
                            "sessionId": session_id,
                            "session_id": session_id,
                            "messages": messages,
                            "args": {
                                "sessionId": session_id,
                                "session_id": session_id,
                                "messages": messages
                            }
                        }),
                    ),
                );
                response
                    .map(|_| None)
                    .map_err(|error| format!("start_stream_message invoke failed: {error:?}"))
            }
        }
    }

    #[tokio::test]
    async fn main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix() {
        let report = run_main_chat_command_surface_eval_gate().await;

        assert_eq!(report.failed_cases, 0, "{:?}", report.failures);
        assert!(report.total_cases >= 24);
        let two_case_coverage = 2.0 / report.total_cases as f32;
        assert!(report.send_coverage >= 0.45);
        assert!(report.stream_coverage >= 0.45);
        assert!(report.provider_generation_coverage >= two_case_coverage);
        assert!(report.file_read_coverage >= two_case_coverage);
        assert!(report.plan_execute_coverage >= two_case_coverage);
        assert!(report.proposal_coverage >= two_case_coverage);
        assert!(report.web_policy_blocker_coverage >= two_case_coverage);
        assert!(report.web_agent_loop_blocker_coverage >= two_case_coverage);
        assert!(report.web_agent_loop_success_coverage >= two_case_coverage);
        assert!(report.mcp_missing_read_target_blocker_coverage >= two_case_coverage);
        assert!(report.mcp_registered_read_success_coverage >= two_case_coverage);
        assert!(report.mcp_agent_loop_success_coverage >= two_case_coverage);
        assert!(report.mcp_tool_permission_proposal_coverage >= two_case_coverage);
        assert!(report.mcp_agent_loop_tool_permission_proposal_coverage >= two_case_coverage);
        assert_eq!(report.live_provider_generation_coverage, 0.0);
        assert_eq!(report.live_provider_web_mcp_agent_loop_coverage, 0.0);
        assert_eq!(report.live_provider_web_agent_loop_coverage, 0.0);
        assert_eq!(report.live_provider_mcp_agent_loop_coverage, 0.0);
        assert_eq!(report.live_provider_proposal_permission_coverage, 0.0);
        assert!(!report.final_completion_ready);
        assert!(report
            .final_completion_blockers
            .contains(&"live_provider_generation_not_executed".to_string()));
        assert!(report
            .final_completion_blockers
            .contains(&"provider_backed_web_mcp_agent_loop_not_executed".to_string()));
        assert!(report
            .final_completion_blockers
            .contains(&"provider_backed_web_agent_loop_not_executed".to_string()));
        assert!(report
            .final_completion_blockers
            .contains(&"provider_backed_mcp_agent_loop_not_executed".to_string()));
        assert!(report
            .final_completion_blockers
            .contains(&"provider_live_proposal_permission_not_executed".to_string()));
        assert_eq!(report.legacy_fallback_count, 0);
        assert_eq!(report.silent_write_count, 0);
    }

    #[tokio::test]
    async fn main_chat_final_acceptance_gate_uses_real_command_surface_eval_evidence() {
        let command_surface_report = run_main_chat_command_surface_eval_gate().await;
        assert_eq!(
            command_surface_report.failed_cases, 0,
            "{:?}",
            command_surface_report.failures
        );

        let runtime_report =
            openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
                openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
            );
        let report =
            openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_agent_execution_v1_acceptance_gate(
                openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceInput {
                    runtime_report,
                    command_surface: command_surface_report.acceptance_evidence(),
                    live_provider:
                        openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceLiveEvidence {
                            generation_eval_executed: false,
                            web_mcp_agent_loop_eval_executed: false,
                            web_agent_loop_eval_executed: false,
                            mcp_agent_loop_eval_executed: false,
                            proposal_permission_eval_executed: false,
                            no_silent_writes: true,
                        },
                },
            );

        assert!(!report.ready);
        assert_eq!(report.status, "blocked");
        assert!(!report.command_surface_gate_ready);
        assert!(!report.live_provider_gate_ready);
        assert!(!report.direct_writes_executed);
        assert!(report
            .blockers
            .contains(&"command_surface_final_completion_not_ready".to_string()));
        assert!(report
            .blockers
            .contains(&"live_provider_generation_not_executed".to_string()));
    }

    #[tokio::test]
    async fn main_chat_final_acceptance_gate_runner_fails_closed_without_live_provider_opt_in() {
        let report = run_main_chat_agent_execution_v1_final_acceptance_gate(false).await;

        assert_eq!(report.runtime_total_cases, 100);
        assert_eq!(report.command_surface_total_cases, 24);
        assert!(!report.live_provider_attempted);
        assert_eq!(report.live_provider_report_count, 0);
        assert_eq!(report.live_provider_ready_count, 0);
        assert!(!report.live_provider_direct_writes_executed);
        assert!(!report.acceptance.ready);
        assert_eq!(report.acceptance.status, "blocked");
        assert!(!report.acceptance.runtime_gate_ready);
        assert!(!report.acceptance.command_surface_gate_ready);
        assert!(!report.acceptance.live_provider_gate_ready);
        assert!(!report.acceptance.direct_writes_executed);
        assert!(report
            .acceptance
            .blockers
            .contains(&"runtime_eval_final_completion_not_ready".to_string()));
        assert!(report
            .acceptance
            .blockers
            .contains(&"command_surface_final_completion_not_ready".to_string()));
        assert!(report
            .acceptance
            .blockers
            .contains(&"live_provider_generation_not_executed".to_string()));
        assert!(report
            .acceptance
            .blockers
            .contains(&"provider_backed_web_agent_loop_not_executed".to_string()));
        assert!(report
            .acceptance
            .blockers
            .contains(&"provider_backed_mcp_agent_loop_not_executed".to_string()));
        assert!(report
            .acceptance
            .blockers
            .contains(&"provider_live_proposal_permission_not_executed".to_string()));
    }

    #[test]
    fn main_chat_final_acceptance_gate_accepts_complete_live_evidence_overlaying_local_gates() {
        let runtime_report =
            openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
                openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
            );
        let live_provider = main_chat_live_provider_acceptance_evidence(&[
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
            ),
        ]);
        let runtime_report =
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_report_with_live_provider_evidence(
                runtime_report,
                &live_provider,
            );
        let command_surface_report = MainChatCommandSurfaceEvalReport {
            total_cases: 24,
            failed_cases: 0,
            send_coverage: 0.5,
            stream_coverage: 0.5,
            provider_generation_coverage: 1.0 / 24.0,
            file_read_coverage: 1.0 / 24.0,
            plan_execute_coverage: 1.0 / 24.0,
            proposal_coverage: 1.0 / 24.0,
            web_policy_blocker_coverage: 1.0 / 24.0,
            web_agent_loop_blocker_coverage: 1.0 / 24.0,
            web_agent_loop_success_coverage: 1.0 / 24.0,
            mcp_missing_read_target_blocker_coverage: 1.0 / 24.0,
            mcp_registered_read_success_coverage: 1.0 / 24.0,
            mcp_agent_loop_success_coverage: 1.0 / 24.0,
            mcp_tool_permission_proposal_coverage: 1.0 / 24.0,
            mcp_agent_loop_tool_permission_proposal_coverage: 1.0 / 24.0,
            final_completion_ready: false,
            final_completion_blockers: vec![
                "live_provider_generation_not_executed".into(),
                "provider_backed_web_mcp_agent_loop_not_executed".into(),
                "provider_backed_web_agent_loop_not_executed".into(),
                "provider_backed_mcp_agent_loop_not_executed".into(),
                "provider_live_proposal_permission_not_executed".into(),
            ],
            ..Default::default()
        };

        let report = openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_agent_execution_v1_acceptance_gate(
            openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceInput {
                runtime_report,
                command_surface: command_surface_report
                    .acceptance_evidence_with_live_provider(&live_provider),
                live_provider,
            },
        );

        assert!(report.ready, "{:?}", report.blockers);
        assert!(report.runtime_gate_ready);
        assert!(report.command_surface_gate_ready);
        assert!(report.live_provider_gate_ready);
        assert!(!report.direct_writes_executed);
    }

    #[test]
    fn main_chat_final_acceptance_gate_report_preserves_live_provider_failure_audit() {
        let runtime_report =
            openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
                openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
            );
        let command_surface_report = MainChatCommandSurfaceEvalReport {
            total_cases: 24,
            failed_cases: 0,
            send_coverage: 0.5,
            stream_coverage: 0.5,
            provider_generation_coverage: 1.0 / 24.0,
            file_read_coverage: 1.0 / 24.0,
            plan_execute_coverage: 1.0 / 24.0,
            proposal_coverage: 1.0 / 24.0,
            web_policy_blocker_coverage: 1.0 / 24.0,
            web_agent_loop_blocker_coverage: 1.0 / 24.0,
            web_agent_loop_success_coverage: 1.0 / 24.0,
            mcp_missing_read_target_blocker_coverage: 1.0 / 24.0,
            mcp_registered_read_success_coverage: 1.0 / 24.0,
            mcp_agent_loop_success_coverage: 1.0 / 24.0,
            mcp_tool_permission_proposal_coverage: 1.0 / 24.0,
            mcp_agent_loop_tool_permission_proposal_coverage: 1.0 / 24.0,
            ..Default::default()
        };

        let report = main_chat_agent_execution_v1_final_gate_report_from_parts(
            runtime_report,
            command_surface_report,
            true,
            vec![
                blocked_live_provider_harness_report("provider_api_key_missing"),
                blocked_live_provider_harness_report("network_disabled"),
            ],
        );

        assert!(report.live_provider_attempted);
        assert_eq!(report.live_provider_report_count, 2);
        assert_eq!(report.live_provider_ready_count, 0);
        assert_eq!(report.live_provider_main_chat_invoked_count, 0);
        assert_eq!(report.live_provider_model_invoked_count, 0);
        assert!(!report.live_provider_direct_writes_executed);
        assert!(report
            .live_provider_blockers
            .contains(&"provider_api_key_missing".to_string()));
        assert!(report
            .live_provider_blockers
            .contains(&"network_disabled".to_string()));
        assert!(!report.acceptance.ready);
    }

    #[test]
    fn main_chat_final_acceptance_gate_report_derives_post_invocation_live_provider_blockers() {
        let runtime_report =
            openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
                openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
            );
        let command_surface_report = MainChatCommandSurfaceEvalReport {
            total_cases: 24,
            failed_cases: 0,
            send_coverage: 0.5,
            stream_coverage: 0.5,
            provider_generation_coverage: 1.0 / 24.0,
            file_read_coverage: 1.0 / 24.0,
            plan_execute_coverage: 1.0 / 24.0,
            proposal_coverage: 1.0 / 24.0,
            web_policy_blocker_coverage: 1.0 / 24.0,
            web_agent_loop_blocker_coverage: 1.0 / 24.0,
            web_agent_loop_success_coverage: 1.0 / 24.0,
            mcp_missing_read_target_blocker_coverage: 1.0 / 24.0,
            mcp_registered_read_success_coverage: 1.0 / 24.0,
            mcp_agent_loop_success_coverage: 1.0 / 24.0,
            mcp_tool_permission_proposal_coverage: 1.0 / 24.0,
            mcp_agent_loop_tool_permission_proposal_coverage: 1.0 / 24.0,
            ..Default::default()
        };
        let mut failed_web = successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        );
        failed_web.ready = false;
        failed_web.status = "failed".into();
        failed_web.blockers.clear();
        failed_web.agent_loop_succeeded = false;
        failed_web.agent_loop_action_status = None;

        let report = main_chat_agent_execution_v1_final_gate_report_from_parts(
            runtime_report,
            command_surface_report,
            true,
            vec![failed_web],
        );

        assert!(report.live_provider_attempted);
        assert_eq!(report.live_provider_report_count, 1);
        assert_eq!(report.live_provider_ready_count, 0);
        assert!(report.live_provider_main_chat_invoked_count > 0);
        assert!(report
            .live_provider_blockers
            .contains(&"live_provider_web_agent_loop_not_completed".to_string()));
        assert!(!report.acceptance.ready);
    }

    #[test]
    fn main_chat_live_provider_report_blockers_rejects_inconsistent_ready_report() {
        let mut inconsistent = successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
        );
        inconsistent.direct_writes_executed = true;
        inconsistent.legacy_fallback_used = true;
        inconsistent.response_preview = None;

        let blockers = main_chat_live_provider_report_blockers(&inconsistent);

        assert!(blockers.contains(&"live_provider_direct_writes_detected".to_string()));
        assert!(blockers.contains(&"live_provider_legacy_fallback_detected".to_string()));
        assert!(blockers.contains(&"live_provider_trace_missing".to_string()));
    }

    #[tokio::test]
    async fn main_chat_final_acceptance_gate_runner_reports_live_preflight_blockers_without_invocation(
    ) {
        let report = run_main_chat_agent_execution_v1_final_acceptance_gate_with_config_mode(
            true,
            MainChatLiveProviderEvalConfigMode::NoCredentials,
        )
        .await;

        assert!(report.live_provider_attempted);
        assert_eq!(report.live_provider_report_count, 4);
        assert_eq!(report.live_provider_ready_count, 0);
        assert_eq!(report.live_provider_main_chat_invoked_count, 0);
        assert_eq!(report.live_provider_model_invoked_count, 0);
        assert!(!report.live_provider_direct_writes_executed);
        assert!(report
            .live_provider_blockers
            .contains(&"provider_api_key_missing".to_string()));
        assert!(!report.acceptance.ready);
        assert!(!report.acceptance.live_provider_gate_ready);
        assert!(report
            .acceptance
            .blockers
            .contains(&"live_provider_generation_not_executed".to_string()));
    }

    #[test]
    fn main_chat_live_provider_harness_reports_build_structured_acceptance_evidence() {
        let complete = main_chat_live_provider_acceptance_evidence(&[
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
            ),
        ]);

        assert!(complete.generation_eval_executed);
        assert!(complete.web_mcp_agent_loop_eval_executed);
        assert!(complete.web_agent_loop_eval_executed);
        assert!(complete.mcp_agent_loop_eval_executed);
        assert!(complete.proposal_permission_eval_executed);
        assert!(complete.no_silent_writes);

        let missing_mcp = main_chat_live_provider_acceptance_evidence(&[
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
            ),
        ]);

        assert!(missing_mcp.web_agent_loop_eval_executed);
        assert!(!missing_mcp.mcp_agent_loop_eval_executed);
        assert!(!missing_mcp.web_mcp_agent_loop_eval_executed);
    }

    #[test]
    fn main_chat_live_provider_harness_evidence_requires_matching_scenario_identity() {
        let mut mislabeled_web = successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        );
        mislabeled_web.scenario = MainChatLiveProviderEvalHarnessScenario::DirectAnswer;
        let evidence = main_chat_live_provider_acceptance_evidence(&[mislabeled_web]);

        assert!(!evidence.web_agent_loop_eval_executed);
        assert!(!evidence.web_mcp_agent_loop_eval_executed);
    }

    #[test]
    fn main_chat_live_provider_harness_evidence_requires_traceable_run_task_for_all_live_scenarios()
    {
        let mut missing_web_run = successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        );
        missing_web_run.run_id = None;

        let mut missing_mcp_task = successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
        );
        missing_mcp_task.task_session_id = None;

        let mut empty_proposal_preview = successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
        );
        empty_proposal_preview.response_preview = Some("   ".into());

        let evidence = main_chat_live_provider_acceptance_evidence(&[
            missing_web_run,
            missing_mcp_task,
            empty_proposal_preview,
        ]);

        assert!(!evidence.web_agent_loop_eval_executed);
        assert!(!evidence.mcp_agent_loop_eval_executed);
        assert!(!evidence.web_mcp_agent_loop_eval_executed);
        assert!(!evidence.proposal_permission_eval_executed);
    }

    #[test]
    fn main_chat_live_provider_harness_evidence_rejects_ready_report_with_failed_status_or_blockers(
    ) {
        let mut failed_status = successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
        );
        failed_status.status = "failed".into();

        let mut blocked_web = successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        );
        blocked_web
            .blockers
            .push("provider_returned_no_tool_action".into());

        let evidence = main_chat_live_provider_acceptance_evidence(&[failed_status, blocked_web]);

        assert!(!evidence.generation_eval_executed);
        assert!(!evidence.web_agent_loop_eval_executed);
        assert!(!evidence.web_mcp_agent_loop_eval_executed);
    }

    #[tokio::test]
    async fn main_chat_live_provider_eval_preflight_from_command_state_fails_closed() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            config.llm.provider = "custom".into();
            config.llm.openai_base = "https://example.invalid/v1".into();
            config.llm.openai_key.clear();
            config.system.network_policy.enabled = false;
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "custom".into(),
                "https://example.invalid/v1".into(),
                String::new(),
                "gpt-command-surface-live-preflight".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response("scripted response must block live eval");
        }

        let config = state.config.lock().await.clone();
        let scripted_provider_response_present = state
            .scheduler
            .lock()
            .await
            .scripted_generation_response
            .is_some();
        let report =
            openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_live_provider_eval_preflight_from_config(
                &config,
                false,
                scripted_provider_response_present,
                false,
            );

        assert!(!report.ready);
        assert_eq!(report.provider, "custom");
        assert!(!report.live_provider_invocation_allowed);
        assert!(!report.model_invoked);
        assert!(!report.direct_writes_executed);
        assert!(report
            .blockers
            .contains(&"explicit_live_eval_required".to_string()));
        assert!(report
            .blockers
            .contains(&"provider_api_key_missing".to_string()));
        assert!(report.blockers.contains(&"network_disabled".to_string()));
        assert!(report
            .blockers
            .contains(&"scripted_provider_response_not_allowed".to_string()));
        assert!(report
            .required_evidence
            .contains(&"live_provider_generation".to_string()));
        let serialized = serde_json::to_string(&report).expect("serialize preflight report");
        assert!(!serialized.contains("apiKey"));
        assert!(!serialized.contains("scripted response must block live eval"));
    }

    #[tokio::test]
    async fn main_chat_live_provider_eval_harness_blocks_before_command_invocation_when_preflight_fails(
    ) {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            config.llm.provider = "custom".into();
            config.llm.openai_base = "https://example.invalid/v1".into();
            config.llm.openai_key.clear();
            config.system.network_policy.enabled = false;
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "custom".into(),
                "https://example.invalid/v1".into(),
                String::new(),
                "gpt-command-surface-live-preflight".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response("scripted response must block live eval");
        }

        let report = run_main_chat_live_provider_eval_harness(
            state.clone(),
            MainChatLiveProviderEvalHarnessInput {
                scenario: MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
                session_id: "live-provider-eval-blocked".into(),
                prompt: "Give a short live-provider DirectAnswer proof.".into(),
                explicit_live_eval_requested: false,
                local_only_required: false,
            },
        )
        .await
        .expect("live provider harness report");

        assert!(!report.ready);
        assert_eq!(report.status, "blocked");
        assert_eq!(report.provider, "custom");
        assert_eq!(report.provider_endpoint_kind, "scripted_scheduler_response");
        assert!(!report.main_chat_invoked);
        assert!(!report.model_invoked);
        assert!(!report.direct_writes_executed);
        assert!(!report.legacy_fallback_used);
        assert!(report.run_id.is_none());
        assert!(report.task_session_id.is_none());
        assert!(report
            .blockers
            .contains(&"explicit_live_eval_required".to_string()));
        assert!(report
            .blockers
            .contains(&"provider_api_key_missing".to_string()));
        assert!(report
            .blockers
            .contains(&"scripted_provider_response_not_allowed".to_string()));
        assert!(report
            .required_evidence
            .contains(&"live_provider_generation".to_string()));
        assert!(report
            .required_evidence
            .contains(&"provider_backed_web_mcp_agent_loop".to_string()));
        let run_count = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await
            .run_count()
            .expect("run count");
        assert_eq!(run_count, 0);
    }

    #[tokio::test]
    async fn main_chat_live_provider_eval_harness_blocks_react_cases_before_command_invocation_when_preflight_fails(
    ) {
        for scenario in [
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
        ] {
            let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
            {
                let mut config = state.config.lock().await;
                config.llm.provider = "custom".into();
                config.llm.openai_base = "https://example.invalid/v1".into();
                config.llm.openai_key.clear();
                config.system.network_policy.enabled = false;
            }
            {
                let mut scheduler = state.scheduler.lock().await;
                *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                    "unused-local-model".into(),
                    false,
                    "custom".into(),
                    "https://example.invalid/v1".into(),
                    String::new(),
                    "gpt-command-surface-live-preflight".into(),
                    "text-embedding-test".into(),
                    false,
                )
                .with_scripted_generation_response("scripted response must block live eval");
            }

            let report = run_main_chat_live_provider_eval_harness(
                state.clone(),
                MainChatLiveProviderEvalHarnessInput {
                    scenario,
                    session_id: format!("live-provider-eval-blocked-{}", scenario.as_str()),
                    prompt: scenario.prompt().into(),
                    explicit_live_eval_requested: false,
                    local_only_required: false,
                },
            )
            .await
            .expect("live provider harness report");

            assert!(!report.ready, "{scenario:?}");
            assert_eq!(report.status, "blocked");
            assert_eq!(report.provider_endpoint_kind, "scripted_scheduler_response");
            assert!(!report.main_chat_invoked);
            assert!(!report.model_invoked);
            assert!(!report.agent_loop_succeeded);
            assert!(!report.direct_writes_executed);
            assert!(report
                .required_evidence
                .contains(&"provider_backed_web_mcp_agent_loop".to_string()));
            let run_count = state
                .agent_run_store
                .as_ref()
                .expect("agent run store")
                .lock()
                .await
                .run_count()
                .expect("run count");
            assert_eq!(run_count, 0, "{scenario:?}");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn main_chat_live_provider_eval_harness_executes_local_http_provider_without_external_live_credit(
    ) {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        configure_live_provider_eval_state_with_local_http_provider(
            &state,
            "local provider harness direct answer",
        )
        .await;

        let report = run_main_chat_live_provider_eval_harness(
            state,
            MainChatLiveProviderEvalHarnessInput {
                scenario: MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
                session_id: "local-http-provider-harness-direct".into(),
                prompt: "Answer in one short sentence: what is this local provider eval proving?"
                    .into(),
                explicit_live_eval_requested: true,
                local_only_required: false,
            },
        )
        .await
        .expect("local provider harness report");

        assert!(
            report.ready,
            "local provider harness blocked: {:?}",
            report.blockers
        );
        assert_eq!(report.status, "completed");
        assert_eq!(report.provider_endpoint_kind, "local_test_http");
        assert!(report.live_provider_invocation_allowed);
        assert!(report.main_chat_invoked);
        assert!(report.model_invoked);
        assert!(!report.direct_writes_executed);
        assert!(!report.legacy_fallback_used);
        assert!(report
            .response_preview
            .as_ref()
            .is_some_and(|preview| preview.contains("local provider harness direct answer")));

        let evidence = main_chat_live_provider_acceptance_evidence(&[report]);
        assert!(!evidence.generation_eval_executed);
        assert!(evidence.no_silent_writes);
    }

    #[tokio::test]
    #[ignore = "requires OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1, network, and a real provider API key"]
    async fn main_chat_live_provider_eval_harness_invokes_external_direct_answer_when_opted_in() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            config.llm.provider =
                std::env::var("OPENLIFE_LIVE_EVAL_PROVIDER").unwrap_or_else(|_| "openai".into());
            config.llm.openai_base = std::env::var("OPENLIFE_LIVE_EVAL_BASE")
                .unwrap_or_else(|_| "https://api.openai.com/v1".into());
            config.llm.chat_model =
                std::env::var("OPENLIFE_LIVE_EVAL_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
            config.llm.openai_key = std::env::var("OPENLIFE_LIVE_EVAL_API_KEY")
                .unwrap_or_else(|_| std::env::var("OPENAI_API_KEY").unwrap_or_default());
            config.system.network_policy.enabled = true;
        }
        {
            let config = state.config.lock().await.clone();
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                config.local_model.clone(),
                false,
                config.llm.provider.clone(),
                config.llm.openai_base.clone(),
                config.llm.openai_key.clone(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                false,
            );
        }

        let report = run_main_chat_live_provider_eval_harness(
            state,
            MainChatLiveProviderEvalHarnessInput {
                scenario: MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
                session_id: "live-provider-eval-direct-answer".into(),
                prompt: "Answer in one short sentence: what is this live provider eval proving?"
                    .into(),
                explicit_live_eval_requested: true,
                local_only_required: false,
            },
        )
        .await
        .expect("live provider harness report");

        assert!(
            report.ready,
            "live provider harness blocked: {:?}",
            report.blockers
        );
        assert_eq!(report.status, "completed");
        assert_eq!(report.provider_endpoint_kind, "external_provider");
        assert!(report.live_provider_invocation_allowed);
        assert!(report.main_chat_invoked);
        assert!(report.model_invoked);
        assert!(!report.direct_writes_executed);
        assert!(!report.legacy_fallback_used);
        assert!(report.run_id.is_some());
        assert!(report.task_session_id.is_some());
        assert!(report
            .response_preview
            .as_ref()
            .is_some_and(|preview| !preview.trim().is_empty()));
        let evidence = main_chat_live_provider_acceptance_evidence(&[report]);
        assert!(evidence.generation_eval_executed);
        assert!(!evidence.web_agent_loop_eval_executed);
        assert!(!evidence.mcp_agent_loop_eval_executed);
        assert!(!evidence.web_mcp_agent_loop_eval_executed);
        assert!(!evidence.proposal_permission_eval_executed);
        assert!(evidence.no_silent_writes);
    }

    #[tokio::test]
    #[ignore = "requires OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1, network, and a real provider API key"]
    async fn main_chat_live_provider_eval_harness_invokes_external_react_web_and_mcp_when_opted_in()
    {
        let mut reports = Vec::new();
        for scenario in [
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
        ] {
            let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
            configure_live_provider_eval_state(&state).await;

            let report = run_main_chat_live_provider_eval_harness(
                state,
                MainChatLiveProviderEvalHarnessInput {
                    scenario,
                    session_id: format!("live-provider-eval-{}", scenario.as_str()),
                    prompt: scenario.prompt().into(),
                    explicit_live_eval_requested: true,
                    local_only_required: false,
                },
            )
            .await
            .expect("live provider harness report");

            assert!(report.ready, "{scenario:?} blocked: {:?}", report.blockers);
            assert_eq!(report.status, "completed");
            assert_eq!(report.provider_endpoint_kind, "external_provider");
            assert!(report.live_provider_invocation_allowed);
            assert!(report.main_chat_invoked);
            assert!(report.model_invoked);
            assert!(report.agent_loop_succeeded, "{scenario:?}");
            assert!(!report.single_step_fallback_used, "{scenario:?}");
            assert!(!report.direct_writes_executed);
            assert!(!report.legacy_fallback_used);
            match scenario {
                MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => {
                    assert_eq!(
                        report.agent_loop_action_status.as_deref(),
                        Some("succeeded")
                    );
                }
                MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
                    assert_eq!(
                        report.agent_loop_action_status.as_deref(),
                        Some("succeeded")
                    );
                    assert!(report.mcp_read_target_resolved);
                }
                MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
                    assert_eq!(
                        report.agent_loop_action_status.as_deref(),
                        Some("needs_confirmation")
                    );
                    assert!(report.tool_permission_proposal_created);
                }
                MainChatLiveProviderEvalHarnessScenario::DirectAnswer => unreachable!(),
            }
            reports.push(report);
        }

        let evidence = main_chat_live_provider_acceptance_evidence(&reports);
        assert!(!evidence.generation_eval_executed);
        assert!(evidence.web_agent_loop_eval_executed);
        assert!(evidence.mcp_agent_loop_eval_executed);
        assert!(evidence.web_mcp_agent_loop_eval_executed);
        assert!(evidence.proposal_permission_eval_executed);
        assert!(evidence.no_silent_writes);
    }

    #[tokio::test]
    async fn send_message_command_surface_runs_governed_proposal_path() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-send-proposal";

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "send_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": [
                        {
                            "role": "user",
                            "content": "Please remember that I prefer morning writing blocks."
                        }
                    ]
                }),
            ),
        )
        .expect("send_message response")
        .deserialize::<serde_json::Value>()
        .expect("deserialize send_message response");

        assert_eq!(response["legacy_fallback_used"], false);
        assert_eq!(
            response["agent_ingress"]["selectedStrategy"],
            "memory_proposal"
        );
        let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
            .as_str()
            .expect("agent task session id");
        assert!(response["execution_transcript"]
            .as_array()
            .expect("transcript array")
            .iter()
            .any(|entry| entry["kind"] == "proposal_request"));

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load task session")
                .expect("task session exists")
        };
        assert_eq!(session.chat_session_id, session_id);
        assert_eq!(session.selected_strategy.as_str(), "memory_proposal");
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        );
        assert!(session
            .pending_blockers
            .iter()
            .any(|blocker| blocker.starts_with("proposal:")));

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list command actions")
        };
        let proposal_action = actions
            .iter()
            .find(|action| action.action.action_type == "proposal.create")
            .expect("proposal create action");
        assert_eq!(
            proposal_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
        );
        assert_eq!(
            proposal_action.policy.level,
            openlife_core::agent::main_chat_agent_v1::MainChatPolicyLevel::L1GovernedProposalCreate
        );
        assert!(proposal_action.policy.execution_allowed);
        assert!(!proposal_action.policy.requires_proposal);
        assert!(!proposal_action.policy.silent_write_allowed);

        let proposals = {
            let proposal_arc = state.proposal_store.as_ref().expect("proposal store");
            let proposal_store = proposal_arc.lock().await;
            proposal_store
                .list_pending_proposals(10)
                .expect("list pending proposals")
        };
        assert!(proposals.iter().any(|proposal| {
            proposal.source == openlife_core::agent::ProposalSource::ChatConversation
                && proposal.source_detail.as_deref()
                    == Some(format!("main_chat_agent_task_session:{task_session_id}").as_str())
        }));
    }

    #[tokio::test]
    async fn start_stream_message_command_surface_runs_governed_proposal_path() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![start_stream_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-stream-proposal";
        let messages = serde_json::json!([
            {
                "role": "user",
                "content": "Please remember that I prefer async writing review on Fridays."
            }
        ]);

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "start_stream_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages,
                    "args": {
                        "sessionId": session_id,
                        "session_id": session_id,
                        "messages": messages
                    }
                }),
            ),
        );
        assert!(response.is_ok(), "stream command failed: {response:?}");

        let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
        let decision = ingress.decide(
            session_id,
            "Please remember that I prefer async writing review on Fridays.",
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let task_session_id = decision
            .agent_task_session_id
            .as_deref()
            .expect("expected stream task session id");

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load stream task session")
                .expect("stream task session exists")
        };
        assert_eq!(session.chat_session_id, session_id);
        assert_eq!(session.selected_strategy.as_str(), "memory_proposal");
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        );

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list stream command actions")
        };
        let proposal_action = actions
            .iter()
            .find(|action| action.action.action_type == "proposal.create")
            .expect("stream proposal create action");
        assert_eq!(
            proposal_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
        );
        assert_eq!(
            proposal_action.policy.level,
            openlife_core::agent::main_chat_agent_v1::MainChatPolicyLevel::L1GovernedProposalCreate
        );
        assert!(!proposal_action.policy.silent_write_allowed);

        let proposals = {
            let proposal_arc = state.proposal_store.as_ref().expect("proposal store");
            let proposal_store = proposal_arc.lock().await;
            proposal_store
                .list_pending_proposals(10)
                .expect("list stream pending proposals")
        };
        assert!(proposals.iter().any(|proposal| {
            proposal.source == openlife_core::agent::ProposalSource::ChatConversation
                && proposal.source_detail.as_deref()
                    == Some(format!("main_chat_agent_task_session:{task_session_id}").as_str())
        }));
    }

    #[tokio::test]
    async fn send_message_direct_answer_records_main_chat_run_and_completes_task() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-direct-answer";
        let user_text = "hello";

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "send_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": [{ "role": "user", "content": user_text }]
                }),
            ),
        )
        .expect("send_message direct response")
        .deserialize::<serde_json::Value>()
        .expect("deserialize direct response");

        assert_eq!(response["legacy_fallback_used"], false);
        assert_eq!(response["tool_calls"].as_array().map(Vec::len), Some(0));
        assert_eq!(
            response["agent_ingress"]["selectedStrategy"],
            "direct_answer"
        );
        let run_id = response["run_id"].as_str().expect("direct answer run id");
        let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
            .as_str()
            .expect("direct answer task session id");

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load direct answer task session")
                .expect("direct answer task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        );
        assert_eq!(session.selected_strategy.as_str(), "direct_answer");
        assert!(session.pending_blockers.is_empty());

        let run = {
            let run_store_arc = state.agent_run_store.as_ref().expect("agent run store");
            let run_store = run_store_arc.lock().await;
            run_store
                .get_run(run_id)
                .expect("get direct answer run")
                .expect("direct answer run exists")
        };
        assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Completed);
        assert_eq!(
            run.reasoning_strategy.as_deref(),
            Some("main_chat_agent_v1_direct_answer")
        );
        assert_eq!(
            run.model_route
                .as_ref()
                .map(|route| route.route_type.as_str()),
            Some("direct")
        );
        assert_eq!(run.tool_call_count, 0);

        let transcript = response["execution_transcript"]
            .as_array()
            .expect("direct answer transcript");
        assert!(transcript.iter().any(|entry| {
            entry["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("DirectAnswer prompt contract"))
        }));
        assert!(transcript.iter().any(|entry| {
            entry["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("Bounded context"))
        }));
        assert!(transcript.iter().any(|entry| {
            entry["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("DirectAnswer completed"))
        }));
    }

    #[tokio::test]
    async fn send_message_l2_direct_answer_records_scheduler_provider_generation_trace() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "openai".into(),
                "https://example.invalid/v1".into(),
                "test-key".into(),
                "gpt-provider-trace".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response("scripted provider-backed direct answer");
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-direct-answer-provider-trace";
        let user_text = "Explain focused work in one concise paragraph for a teammate.";

        let ingress_decision = openlife_core::agent::main_chat_agent_v1::AgentIngress::default()
            .decide(
                session_id,
                user_text,
                None,
                openlife_core::agent::AgentTaskKind::Conversation,
            );
        assert_eq!(
            ingress_decision.selected_strategy,
            openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer
        );

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "send_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": [{ "role": "user", "content": user_text }]
                }),
            ),
        )
        .expect("send_message provider-backed direct response")
        .deserialize::<serde_json::Value>()
        .expect("deserialize provider-backed direct response");

        assert_eq!(response["reply"], "scripted provider-backed direct answer");
        assert_eq!(response["legacy_fallback_used"], false);
        assert_eq!(
            response["agent_ingress"]["selectedStrategy"],
            "direct_answer"
        );
        let run_id = response["run_id"]
            .as_str()
            .expect("provider-backed direct answer run id");

        let generation = response["reasoning_trace"]["generation_result"]
            .as_object()
            .expect("generation result metadata");
        assert_eq!(
            generation
                .get("providerGenerationPath")
                .and_then(serde_json::Value::as_str),
            Some("main_chat_direct_answer_scheduler")
        );
        assert_eq!(
            generation
                .get("modelGenerated")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            generation
                .get("provider")
                .and_then(serde_json::Value::as_str),
            Some("openai")
        );
        assert_eq!(
            generation.get("model").and_then(serde_json::Value::as_str),
            Some("gpt-provider-trace")
        );
        assert_eq!(
            generation
                .get("routeType")
                .and_then(serde_json::Value::as_str),
            Some("cloud")
        );
        assert_eq!(
            generation
                .get("legacyFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );

        let run = {
            let run_store_arc = state.agent_run_store.as_ref().expect("agent run store");
            let run_store = run_store_arc.lock().await;
            run_store
                .get_run(run_id)
                .expect("get provider-backed direct answer run")
                .expect("provider-backed direct answer run exists")
        };
        let model_route = run
            .model_route
            .as_ref()
            .expect("provider-backed model route");
        assert_eq!(model_route.provider, "openai");
        assert_eq!(model_route.model, "gpt-provider-trace");
        assert_eq!(model_route.route_type, "cloud");
        assert_eq!(
            run.reasoning_strategy.as_deref(),
            Some("main_chat_agent_v1_direct_answer")
        );

        let transcript = response["execution_transcript"]
            .as_array()
            .expect("provider-backed direct answer transcript");
        let generation_entry = transcript
            .iter()
            .find(|entry| {
                entry["summary"].as_str().is_some_and(|summary| {
                    summary.contains("DirectAnswer generated a model response")
                })
            })
            .expect("provider generation transcript entry");
        assert_eq!(
            generation_entry["metadata"]["providerGenerationPath"].as_str(),
            Some("main_chat_direct_answer_scheduler")
        );
        assert_eq!(
            generation_entry["metadata"]["provider"].as_str(),
            Some("openai")
        );
        assert_eq!(
            generation_entry["metadata"]["model"].as_str(),
            Some("gpt-provider-trace")
        );
        assert_eq!(
            generation_entry["metadata"]["routeType"].as_str(),
            Some("cloud")
        );
        assert_eq!(
            generation_entry["metadata"]["directWritesExecuted"].as_bool(),
            Some(false)
        );
    }

    #[tokio::test]
    async fn start_stream_message_direct_answer_records_main_chat_run_and_completes_task() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![start_stream_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-stream-direct-answer";
        let user_text = "hello";
        let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "start_stream_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages,
                    "args": {
                        "sessionId": session_id,
                        "session_id": session_id,
                        "messages": messages
                    }
                }),
            ),
        );
        assert!(
            response.is_ok(),
            "stream direct answer failed: {response:?}"
        );

        let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
        let decision = ingress.decide(
            session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let task_session_id = decision
            .agent_task_session_id
            .as_deref()
            .expect("expected stream direct answer task session id");

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load stream direct answer task session")
                .expect("stream direct answer task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        );
        assert_eq!(session.selected_strategy.as_str(), "direct_answer");
        assert!(session.pending_blockers.is_empty());

        let runs = {
            let run_store_arc = state.agent_run_store.as_ref().expect("agent run store");
            let run_store = run_store_arc.lock().await;
            run_store
                .list_runs_for_session(session_id, 10)
                .expect("list stream direct answer runs")
        };
        let run = runs
            .iter()
            .find(|run| {
                run.reasoning_strategy.as_deref() == Some("main_chat_agent_v1_direct_answer")
            })
            .expect("stream direct answer main chat run");
        assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Completed);
        assert_eq!(
            run.model_route
                .as_ref()
                .map(|route| route.route_type.as_str()),
            Some("direct")
        );
        assert_eq!(run.tool_call_count, 0);

        let transcript = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .list_transcript_entries(task_session_id)
                .expect("list stream direct answer transcript")
        };
        assert!(transcript
            .iter()
            .any(|entry| entry.summary.contains("DirectAnswer prompt contract")));
        assert!(transcript
            .iter()
            .any(|entry| entry.summary.contains("Bounded context")));
        assert!(transcript
            .iter()
            .any(|entry| entry.summary.contains("DirectAnswer completed")));
    }

    #[tokio::test]
    async fn start_stream_message_l2_direct_answer_records_scheduler_provider_generation_trace() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "openai".into(),
                "https://example.invalid/v1".into(),
                "test-key".into(),
                "gpt-stream-provider-trace".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response("scripted stream provider direct answer");
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![start_stream_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-stream-direct-answer-provider-trace";
        let user_text = "Explain focused work in one concise paragraph for a teammate.";
        let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "start_stream_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages,
                    "args": {
                        "sessionId": session_id,
                        "session_id": session_id,
                        "messages": messages
                    }
                }),
            ),
        );
        assert!(
            response.is_ok(),
            "stream provider-backed direct answer failed: {response:?}"
        );

        let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
        let decision = ingress.decide(
            session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        assert_eq!(
            decision.selected_strategy,
            openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer
        );
        let task_session_id = decision
            .agent_task_session_id
            .as_deref()
            .expect("expected stream provider direct answer task session id");

        let run = {
            let run_store_arc = state.agent_run_store.as_ref().expect("agent run store");
            let run_store = run_store_arc.lock().await;
            run_store
                .list_runs_for_session(session_id, 10)
                .expect("list stream provider direct answer runs")
                .into_iter()
                .find(|run| {
                    run.reasoning_strategy.as_deref() == Some("main_chat_agent_v1_direct_answer")
                })
                .expect("stream provider direct answer main chat run")
        };
        assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Completed);
        let model_route = run
            .model_route
            .as_ref()
            .expect("stream provider-backed model route");
        assert_eq!(model_route.provider, "openai");
        assert_eq!(model_route.model, "gpt-stream-provider-trace");
        assert_eq!(model_route.route_type, "cloud");
        let generation = run
            .reasoning_trace
            .as_ref()
            .and_then(|trace| trace.generation_result.as_ref())
            .and_then(serde_json::Value::as_object)
            .expect("stream provider generation trace");
        assert_eq!(
            generation
                .get("providerGenerationPath")
                .and_then(serde_json::Value::as_str),
            Some("main_chat_direct_answer_scheduler")
        );
        assert_eq!(
            generation
                .get("provider")
                .and_then(serde_json::Value::as_str),
            Some("openai")
        );
        assert_eq!(
            generation.get("model").and_then(serde_json::Value::as_str),
            Some("gpt-stream-provider-trace")
        );
        assert_eq!(
            generation
                .get("routeType")
                .and_then(serde_json::Value::as_str),
            Some("cloud")
        );
        assert_eq!(
            generation
                .get("legacyFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );

        let transcript = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .list_transcript_entries(task_session_id)
                .expect("list stream provider direct answer transcript")
        };
        let generation_entry = transcript
            .iter()
            .find(|entry| {
                entry
                    .summary
                    .contains("DirectAnswer generated a model response")
            })
            .expect("stream provider generation transcript entry");
        assert_eq!(
            generation_entry
                .metadata
                .get("providerGenerationPath")
                .and_then(serde_json::Value::as_str),
            Some("main_chat_direct_answer_scheduler")
        );
        assert_eq!(
            generation_entry
                .metadata
                .get("provider")
                .and_then(serde_json::Value::as_str),
            Some("openai")
        );
        assert_eq!(
            generation_entry
                .metadata
                .get("model")
                .and_then(serde_json::Value::as_str),
            Some("gpt-stream-provider-trace")
        );
        assert_eq!(
            generation_entry
                .metadata
                .get("routeType")
                .and_then(serde_json::Value::as_str),
            Some("cloud")
        );
        assert_eq!(
            generation_entry
                .metadata
                .get("directWritesExecuted")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
    }

    #[tokio::test]
    async fn send_message_command_surface_preserves_web_policy_blocker() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            config.system.network_policy.enabled = false;
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-web-blocker";
        let user_text = "Please web search OpenLife release notes.";

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "send_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": [{ "role": "user", "content": user_text }]
                }),
            ),
        )
        .expect("send_message web blocker response")
        .deserialize::<serde_json::Value>()
        .expect("deserialize web blocker response");

        assert_eq!(response["legacy_fallback_used"], false);
        assert_eq!(
            response["agent_ingress"]["selectedStrategy"],
            "re_act_tool_execution"
        );
        let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
            .as_str()
            .expect("web blocker task session id");

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load web blocker task session")
                .expect("web blocker task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
        );
        assert!(session
            .pending_blockers
            .iter()
            .any(|blocker| blocker.contains("network_policy_blocked")));

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list web blocker actions")
        };
        let web_action = actions
            .iter()
            .find(|action| action.action.action_type == "web.search")
            .expect("web search action");
        assert_eq!(
            web_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
        );
        assert_eq!(
            web_action
                .observation_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("structuredResult"))
                .and_then(|value| value.get("network_policy_blocked"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn send_message_command_surface_preserves_missing_mcp_blocker() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-mcp-blocker";
        let user_text = "Use mcp missing.status read-only now.";

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "send_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": [{ "role": "user", "content": user_text }]
                }),
            ),
        )
        .expect("send_message mcp blocker response")
        .deserialize::<serde_json::Value>()
        .expect("deserialize mcp blocker response");

        assert_eq!(response["legacy_fallback_used"], false);
        assert_eq!(
            response["agent_ingress"]["selectedStrategy"],
            "re_act_tool_execution"
        );
        let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
            .as_str()
            .expect("mcp blocker task session id");

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load mcp blocker task session")
                .expect("mcp blocker task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
        );
        assert!(session
            .pending_blockers
            .iter()
            .any(|blocker| blocker.contains("mcp_read_tool_not_registered")));

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list mcp blocker actions")
        };
        let mcp_action = actions
            .iter()
            .find(|action| action.action.action_type == "mcp.read_only")
            .expect("mcp read action");
        assert_eq!(
            mcp_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
        );
        assert_eq!(
            mcp_action
                .observation_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("blockerReason"))
                .and_then(serde_json::Value::as_str),
            Some("mcp_read_tool_not_registered")
        );
    }

    #[tokio::test]
    async fn send_message_command_surface_preserves_registered_mcp_read_success() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let store = state.tool_permission_store.lock().await;
            store
                .grant(
                    "builtin_echo",
                    "builtin",
                    "low",
                    "read",
                    openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                    None,
                )
                .expect("grant builtin echo permission");
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-mcp-success";
        let user_text = "Use mcp builtin_echo read-only now.";

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "send_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": [{ "role": "user", "content": user_text }]
                }),
            ),
        )
        .expect("send_message mcp success response")
        .deserialize::<serde_json::Value>()
        .expect("deserialize mcp success response");

        assert_eq!(response["legacy_fallback_used"], false);
        assert_eq!(
            response["agent_ingress"]["selectedStrategy"],
            "re_act_tool_execution"
        );
        let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
            .as_str()
            .expect("mcp success task session id");

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load mcp success task session")
                .expect("mcp success task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        );
        assert!(session.pending_blockers.is_empty());

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list mcp success actions")
        };
        let mcp_action = actions
            .iter()
            .find(|action| action.action.action_type == "mcp.read_only")
            .expect("mcp read action");
        assert_eq!(
            mcp_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
        );
        let metadata = mcp_action
            .observation_metadata
            .as_ref()
            .expect("mcp read observation metadata");
        assert_eq!(metadata["target"], serde_json::json!("builtin_echo"));
        assert_eq!(
            metadata["requestedTarget"],
            serde_json::json!("mcp.call_tool")
        );
        assert_eq!(metadata["mcpReadTargetResolved"], serde_json::json!(true));
        assert_eq!(metadata["executorStatus"], serde_json::json!("succeeded"));
        assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
        assert_eq!(
            metadata["structuredResult"]["directWritesExecuted"],
            serde_json::json!(false)
        );
    }

    #[tokio::test]
    async fn send_message_registered_mcp_read_completes_through_agent_loop_not_fallback() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let store = state.tool_permission_store.lock().await;
            store
                .grant(
                    "builtin_echo",
                    "builtin",
                    "low",
                    "read",
                    openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                    None,
                )
                .expect("grant builtin echo permission");
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "openai".into(),
                "https://example.invalid/v1".into(),
                "test-key".into(),
                "gpt-react-mcp-loop".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response(
                serde_json::json!({
                    "final": "I will run the registered MCP read first.",
                    "actions": [{
                        "name": "builtin_echo",
                        "action_type": "mcp_tool",
                        "arguments": {}
                    }],
                    "thought_summary": "Need a governed read-only MCP observation.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-mcp-agent-loop-success";
        let user_text = "Use mcp builtin_echo read-only now.";

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "send_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": [{ "role": "user", "content": user_text }]
                }),
            ),
        )
        .expect("send_message mcp AgentLoop response")
        .deserialize::<serde_json::Value>()
        .expect("deserialize mcp AgentLoop response");

        assert_eq!(response["legacy_fallback_used"], false);
        assert_eq!(
            response["agent_ingress"]["selectedStrategy"],
            "re_act_tool_execution"
        );
        let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
            .as_str()
            .expect("mcp AgentLoop task session id");

        let transcript = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .list_transcript_entries(task_session_id)
                .expect("list mcp AgentLoop transcript")
        };
        let completed_entry = transcript
            .iter()
            .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
            .expect("mcp AgentLoop completion transcript entry");
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("plannedActionObserved")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("mcpReadTargetResolved")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("resolvedTarget")
                .and_then(serde_json::Value::as_str),
            Some("builtin_echo")
        );

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list mcp AgentLoop actions")
        };
        let mcp_action = actions
            .iter()
            .find(|action| action.action.action_type == "mcp.read_only")
            .expect("mcp read action");
        assert_eq!(
            mcp_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
        );
        let observation = mcp_action
            .observation_metadata
            .as_ref()
            .expect("mcp AgentLoop observation metadata");
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
        assert_eq!(
            observation["mcpReadTargetResolved"],
            serde_json::json!(true)
        );
        assert_eq!(
            observation["resolvedTarget"],
            serde_json::json!("builtin_echo")
        );
        assert_eq!(
            observation["directWritesExecuted"],
            serde_json::json!(false)
        );
    }

    #[tokio::test]
    async fn send_message_registered_mcp_multi_candidate_agent_loop_selects_allowed_manifest() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let store = state.tool_permission_store.lock().await;
            store
                .grant(
                    "builtin_echo",
                    "builtin",
                    "low",
                    "read",
                    openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                    None,
                )
                .expect("grant builtin echo permission");
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "openai".into(),
                "https://example.invalid/v1".into(),
                "test-key".into(),
                "gpt-react-mcp-loop-multi-candidate".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response(
                serde_json::json!({
                    "final": "I will run one allowed registered MCP read first.",
                    "actions": [{
                        "name": "builtin_echo",
                        "action_type": "mcp_tool",
                        "arguments": {
                            "text": "multi candidate selected"
                        }
                    }],
                    "thought_summary": "Select one governed read-only manifest from the candidate set.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-mcp-agent-loop-multi-candidate";
        let user_text = "Use an mcp read-only utility tool now.";

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "send_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": [{ "role": "user", "content": user_text }]
                }),
            ),
        )
        .expect("send_message mcp multi-candidate AgentLoop response")
        .deserialize::<serde_json::Value>()
        .expect("deserialize mcp multi-candidate AgentLoop response");

        assert_eq!(response["legacy_fallback_used"], false);
        assert_eq!(
            response["agent_ingress"]["selectedStrategy"],
            "re_act_tool_execution"
        );
        let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
            .as_str()
            .expect("mcp multi-candidate AgentLoop task session id");

        let completed_entry = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .list_transcript_entries(task_session_id)
                .expect("list mcp multi-candidate AgentLoop transcript")
                .into_iter()
                .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
                .expect("mcp multi-candidate AgentLoop completion transcript entry")
        };
        let metadata = completed_entry.metadata;
        let candidate_count = metadata
            .get("toolSelectionCandidateCount")
            .and_then(serde_json::Value::as_u64)
            .expect("candidate count metadata");
        assert!(
            candidate_count >= 2,
            "AgentLoop completion metadata must preserve the multi-candidate contract"
        );
        let candidate_ids = metadata
            .get("toolSelectionCandidateIds")
            .and_then(serde_json::Value::as_array)
            .expect("candidate ids metadata");
        assert!(candidate_ids
            .iter()
            .any(|candidate| candidate == "builtin_echo"));
        assert_eq!(
            metadata
                .get("toolSelectionCandidateId")
                .and_then(serde_json::Value::as_str),
            Some("builtin_echo")
        );
        assert_eq!(
            metadata
                .get("toolSelectionCandidateTarget")
                .and_then(serde_json::Value::as_str),
            Some("builtin_echo")
        );
        assert_eq!(
            metadata
                .get("modelSelectedAllowedTool")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            metadata
                .get("directWritesExecuted")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list mcp multi-candidate AgentLoop actions")
        };
        let mcp_action = actions
            .iter()
            .find(|action| action.action.action_type == "mcp.read_only")
            .expect("mcp read action");
        assert_eq!(
            mcp_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
        );
        let observation = mcp_action
            .observation_metadata
            .as_ref()
            .expect("mcp multi-candidate AgentLoop observation metadata");
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["toolSelectionCandidateId"],
            serde_json::json!("builtin_echo")
        );
        assert_eq!(
            observation["toolSelectionCandidateTarget"],
            serde_json::json!("builtin_echo")
        );
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
        assert_eq!(
            observation["directWritesExecuted"],
            serde_json::json!(false)
        );
    }

    #[tokio::test]
    async fn start_stream_message_command_surface_preserves_web_policy_blocker() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            config.system.network_policy.enabled = false;
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![start_stream_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-stream-web-blocker";
        let user_text = "Please web search OpenLife release notes.";
        let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "start_stream_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages,
                    "args": {
                        "sessionId": session_id,
                        "session_id": session_id,
                        "messages": messages
                    }
                }),
            ),
        );
        assert!(response.is_ok(), "stream web blocker failed: {response:?}");

        let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
        let decision = ingress.decide(
            session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let task_session_id = decision
            .agent_task_session_id
            .as_deref()
            .expect("expected stream web blocker task session id");

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load stream web blocker task session")
                .expect("stream web blocker task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
        );
        assert!(session
            .pending_blockers
            .iter()
            .any(|blocker| blocker.contains("network_policy_blocked")));

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list stream web blocker actions")
        };
        let web_action = actions
            .iter()
            .find(|action| action.action.action_type == "web.search")
            .expect("stream web search action");
        assert_eq!(
            web_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
        );
        assert_eq!(
            web_action
                .observation_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("structuredResult"))
                .and_then(|value| value.get("network_policy_blocked"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn start_stream_message_command_surface_preserves_missing_mcp_blocker() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![start_stream_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-stream-mcp-blocker";
        let user_text = "Use mcp missing.status read-only now.";
        let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "start_stream_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages,
                    "args": {
                        "sessionId": session_id,
                        "session_id": session_id,
                        "messages": messages
                    }
                }),
            ),
        );
        assert!(response.is_ok(), "stream mcp blocker failed: {response:?}");

        let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
        let decision = ingress.decide(
            session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let task_session_id = decision
            .agent_task_session_id
            .as_deref()
            .expect("expected stream mcp blocker task session id");

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load stream mcp blocker task session")
                .expect("stream mcp blocker task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
        );
        assert!(session
            .pending_blockers
            .iter()
            .any(|blocker| blocker.contains("mcp_read_tool_not_registered")));

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list stream mcp blocker actions")
        };
        let mcp_action = actions
            .iter()
            .find(|action| action.action.action_type == "mcp.read_only")
            .expect("stream mcp read action");
        assert_eq!(
            mcp_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
        );
        assert_eq!(
            mcp_action
                .observation_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("blockerReason"))
                .and_then(serde_json::Value::as_str),
            Some("mcp_read_tool_not_registered")
        );
    }

    #[tokio::test]
    async fn start_stream_message_command_surface_preserves_registered_mcp_read_success() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let store = state.tool_permission_store.lock().await;
            store
                .grant(
                    "builtin_echo",
                    "builtin",
                    "low",
                    "read",
                    openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                    None,
                )
                .expect("grant builtin echo permission");
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![start_stream_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-stream-mcp-success";
        let user_text = "Use mcp builtin_echo read-only now.";
        let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "start_stream_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages,
                    "args": {
                        "sessionId": session_id,
                        "session_id": session_id,
                        "messages": messages
                    }
                }),
            ),
        );
        assert!(response.is_ok(), "stream mcp success failed: {response:?}");

        let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
        let decision = ingress.decide(
            session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let task_session_id = decision
            .agent_task_session_id
            .as_deref()
            .expect("expected stream mcp success task session id");

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load stream mcp success task session")
                .expect("stream mcp success task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        );
        assert!(session.pending_blockers.is_empty());

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list stream mcp success actions")
        };
        let mcp_action = actions
            .iter()
            .find(|action| action.action.action_type == "mcp.read_only")
            .expect("stream mcp read action");
        assert_eq!(
            mcp_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
        );
        let metadata = mcp_action
            .observation_metadata
            .as_ref()
            .expect("stream mcp read observation metadata");
        assert_eq!(metadata["target"], serde_json::json!("builtin_echo"));
        assert_eq!(
            metadata["requestedTarget"],
            serde_json::json!("mcp.call_tool")
        );
        assert_eq!(metadata["mcpReadTargetResolved"], serde_json::json!(true));
        assert_eq!(metadata["executorStatus"], serde_json::json!("succeeded"));
        assert_eq!(metadata["directWritesExecuted"], serde_json::json!(false));
        assert_eq!(
            metadata["structuredResult"]["directWritesExecuted"],
            serde_json::json!(false)
        );
    }

    #[tokio::test]
    async fn start_stream_message_registered_mcp_read_completes_through_agent_loop_not_fallback() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let store = state.tool_permission_store.lock().await;
            store
                .grant(
                    "builtin_echo",
                    "builtin",
                    "low",
                    "read",
                    openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                    None,
                )
                .expect("grant builtin echo permission");
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "openai".into(),
                "https://example.invalid/v1".into(),
                "test-key".into(),
                "gpt-react-mcp-loop-stream".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response(
                serde_json::json!({
                    "final": "I will run the registered MCP read first.",
                    "actions": [{
                        "name": "builtin_echo",
                        "action_type": "mcp_tool",
                        "arguments": {}
                    }],
                    "thought_summary": "Need a governed read-only MCP observation.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![start_stream_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-stream-mcp-agent-loop-success";
        let user_text = "Use mcp builtin_echo read-only now.";
        let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "start_stream_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages,
                    "args": {
                        "sessionId": session_id,
                        "session_id": session_id,
                        "messages": messages
                    }
                }),
            ),
        );
        assert!(
            response.is_ok(),
            "stream mcp AgentLoop success failed: {response:?}"
        );

        let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
        let decision = ingress.decide(
            session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let task_session_id = decision
            .agent_task_session_id
            .as_deref()
            .expect("expected stream mcp AgentLoop task session id");

        let transcript = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .list_transcript_entries(task_session_id)
                .expect("list stream mcp AgentLoop transcript")
        };
        let completed_entry = transcript
            .iter()
            .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
            .expect("stream mcp AgentLoop completion transcript entry");
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("mcpReadTargetResolved")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("resolvedTarget")
                .and_then(serde_json::Value::as_str),
            Some("builtin_echo")
        );

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list stream mcp AgentLoop actions")
        };
        let mcp_action = actions
            .iter()
            .find(|action| action.action.action_type == "mcp.read_only")
            .expect("stream mcp read action");
        assert_eq!(
            mcp_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
        );
        let observation = mcp_action
            .observation_metadata
            .as_ref()
            .expect("stream mcp AgentLoop observation metadata");
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
        assert_eq!(
            observation["mcpReadTargetResolved"],
            serde_json::json!(true)
        );
        assert_eq!(
            observation["resolvedTarget"],
            serde_json::json!("builtin_echo")
        );
        assert_eq!(
            observation["directWritesExecuted"],
            serde_json::json!(false)
        );
    }

    #[tokio::test]
    async fn send_message_web_policy_blocker_completes_through_agent_loop_not_fallback() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            config.system.network_policy.enabled = false;
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "openai".into(),
                "https://example.invalid/v1".into(),
                "test-key".into(),
                "gpt-react-web-blocker-loop".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response(
                serde_json::json!({
                    "final": "I will run the governed web read first.",
                    "actions": [{
                        "name": "web.search",
                        "action_type": "mcp_tool",
                        "arguments": {
                            "query": "OpenLife release notes",
                            "max_results": 3
                        }
                    }],
                    "thought_summary": "Need a governed network-policy checked web observation.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![send_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-web-agent-loop-blocker";
        let user_text = "Please web search OpenLife release notes.";

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "send_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": [{ "role": "user", "content": user_text }]
                }),
            ),
        )
        .expect("send_message web AgentLoop blocker response")
        .deserialize::<serde_json::Value>()
        .expect("deserialize web AgentLoop blocker response");

        assert_eq!(response["legacy_fallback_used"], false);
        assert_eq!(
            response["agent_ingress"]["selectedStrategy"],
            "re_act_tool_execution"
        );
        let task_session_id = response["agent_ingress"]["agentTaskSessionId"]
            .as_str()
            .expect("web AgentLoop blocker task session id");

        let transcript = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .list_transcript_entries(task_session_id)
                .expect("list web AgentLoop transcript")
        };
        let completed_entry = transcript
            .iter()
            .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
            .expect("web AgentLoop completion transcript entry");
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopActionStatus")
                .and_then(serde_json::Value::as_str),
            Some("blocked")
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("permissionDecision")
                .and_then(serde_json::Value::as_str),
            Some("network_policy_blocked")
        );

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load web AgentLoop blocker task session")
                .expect("web AgentLoop blocker task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
        );
        assert!(session
            .pending_blockers
            .iter()
            .any(|blocker| blocker.contains("network_policy_blocked")));

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list web AgentLoop blocker actions")
        };
        let web_action = actions
            .iter()
            .find(|action| action.action.action_type == "web.search")
            .expect("web search action");
        assert_eq!(
            web_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
        );
        let observation = web_action
            .observation_metadata
            .as_ref()
            .expect("web AgentLoop observation metadata");
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
        assert_eq!(
            observation["agentLoopActionStatus"],
            serde_json::json!("blocked")
        );
        assert_eq!(
            observation["permissionDecision"],
            serde_json::json!("network_policy_blocked")
        );
        assert_eq!(
            observation["directWritesExecuted"],
            serde_json::json!(false)
        );
    }

    #[tokio::test]
    async fn start_stream_message_web_policy_blocker_completes_through_agent_loop_not_fallback() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            config.system.network_policy.enabled = false;
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                "unused-local-model".into(),
                false,
                "openai".into(),
                "https://example.invalid/v1".into(),
                "test-key".into(),
                "gpt-react-web-blocker-loop-stream".into(),
                "text-embedding-test".into(),
                false,
            )
            .with_scripted_generation_response(
                serde_json::json!({
                    "final": "I will run the governed web read first.",
                    "actions": [{
                        "name": "web.search",
                        "action_type": "mcp_tool",
                        "arguments": {
                            "query": "OpenLife release notes",
                            "max_results": 3
                        }
                    }],
                    "thought_summary": "Need a governed network-policy checked web observation.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![start_stream_message])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");
        let session_id = "command-surface-stream-web-agent-loop-blocker";
        let user_text = "Please web search OpenLife release notes.";
        let messages = serde_json::json!([{ "role": "user", "content": user_text }]);

        let response = tauri::test::get_ipc_response(
            &webview,
            main_chat_invoke_request(
                "start_stream_message",
                serde_json::json!({
                    "sessionId": session_id,
                    "session_id": session_id,
                    "messages": messages,
                    "args": {
                        "sessionId": session_id,
                        "session_id": session_id,
                        "messages": messages
                    }
                }),
            ),
        );
        assert!(
            response.is_ok(),
            "stream web AgentLoop blocker failed: {response:?}"
        );

        let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default();
        let decision = ingress.decide(
            session_id,
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let task_session_id = decision
            .agent_task_session_id
            .as_deref()
            .expect("expected stream web AgentLoop blocker task session id");

        let transcript = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .list_transcript_entries(task_session_id)
                .expect("list stream web AgentLoop transcript")
        };
        let completed_entry = transcript
            .iter()
            .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
            .expect("stream web AgentLoop completion transcript entry");
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("agentLoopActionStatus")
                .and_then(serde_json::Value::as_str),
            Some("blocked")
        );
        assert_eq!(
            completed_entry
                .metadata
                .get("permissionDecision")
                .and_then(serde_json::Value::as_str),
            Some("network_policy_blocked")
        );

        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id)
                .expect("load stream web AgentLoop blocker task session")
                .expect("stream web AgentLoop blocker task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
        );
        assert!(session
            .pending_blockers
            .iter()
            .any(|blocker| blocker.contains("network_policy_blocked")));

        let actions = {
            let queue_arc = state
                .main_chat_action_queue_store
                .as_ref()
                .expect("main chat action queue store");
            let queue = queue_arc.lock().await;
            queue
                .list_for_session(task_session_id)
                .expect("list stream web AgentLoop blocker actions")
        };
        let web_action = actions
            .iter()
            .find(|action| action.action.action_type == "web.search")
            .expect("stream web search action");
        assert_eq!(
            web_action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
        );
        let observation = web_action
            .observation_metadata
            .as_ref()
            .expect("stream web AgentLoop observation metadata");
        assert_eq!(observation["agentLoopSucceeded"], serde_json::json!(true));
        assert_eq!(
            observation["singleStepFallbackUsed"],
            serde_json::json!(false)
        );
        assert_eq!(
            observation["agentLoopActionStatus"],
            serde_json::json!("blocked")
        );
        assert_eq!(
            observation["permissionDecision"],
            serde_json::json!("network_policy_blocked")
        );
        assert_eq!(
            observation["directWritesExecuted"],
            serde_json::json!(false)
        );
    }

    #[tokio::test]
    async fn chat_runtime_hs_packet_uses_sanitized_inputs_and_seeded_stores() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
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
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
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
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
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
