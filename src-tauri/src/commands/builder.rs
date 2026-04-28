use crate::{persist_life_model, AppState};
use openlife_core::agent::{
    AgentProposal, ProposalSource, ProposalType, RiskLevel as ProposalRiskLevel,
};
use openlife_core::builder::{
    BuilderDimension, BuilderEngine, BuilderMode, BuilderSession, BuilderSummary, SignalUserStatus,
};
use std::sync::Arc;
use tauri::State;

fn value_at_path(root: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let mut current = root;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current.clone())
}

fn proposal_risk_level(signal_risk: &openlife_core::builder::RiskLevel) -> ProposalRiskLevel {
    match signal_risk {
        openlife_core::builder::RiskLevel::Low => ProposalRiskLevel::Low,
        openlife_core::builder::RiskLevel::Medium => ProposalRiskLevel::Medium,
        openlife_core::builder::RiskLevel::High => ProposalRiskLevel::High,
    }
}

async fn builder_start_with_state(
    mode: String,
    session_id: String,
    state: &Arc<AppState>,
    target_dimension: Option<String>,
) -> Result<serde_json::Value, String> {
    let mode = match mode.as_str() {
        "quick" => BuilderMode::Quick,
        "incremental" => BuilderMode::Incremental,
        "socratic" => BuilderMode::Socratic,
        _ => BuilderMode::Quick,
    };
    let mut session = BuilderSession::new(&session_id, mode);
    if let Some(dim) = target_dimension {
        session.target_dimension = Some(
            dim.parse::<openlife_core::builder::BuilderDimension>()
                .map_err(|e| e.to_string())?,
        );
    }
    // Check if there's a persisted session to resume
    {
        let store = state.builder_session_store.lock().await;
        if let Some(existing) = store.get_session(&session_id).map_err(|e| e.to_string())? {
            session = existing;
        }
    }
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    if session.finished && !session.pending_signals.is_empty() {
        let analysis = session
            .analysis
            .clone()
            .unwrap_or_else(|| BuilderEngine::build_analysis(&model));
        let pending_signals: Vec<serde_json::Value> = session
            .pending_signals
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "source_step": s.source_step,
                    "source_question_id": s.source_question_id,
                    "dimension": format!("{:?}", s.dimension),
                    "affected_path": s.affected_path,
                    "proposed_value": s.proposed_value.clone(),
                    "confidence": s.confidence,
                    "reason": s.reason,
                    "risk_level": format!("{}", s.risk_level),
                    "user_status": format!("{:?}", s.user_status),
                })
            })
            .collect();
        {
            let mut sessions = state.builder_sessions.lock().await;
            sessions.insert(session_id.clone(), session.clone());
        }
        {
            let store = state.builder_session_store.lock().await;
            store.save_session(&session).map_err(|e| e.to_string())?;
        }
        return Ok(serde_json::json!({
            "prompt": session.current_prompt,
            "progress": session.progress(),
            "analysis": analysis,
            "finished": true,
            "pending_signals": pending_signals,
            "mode": format!("{:?}", session.mode),
            "target_dimension": session.target_dimension.as_ref().map(|d| format!("{:?}", d)),
        }));
    }

    if !session.current_prompt.is_empty() && !session.finished && session.step_index > 0 {
        let progress = session.progress();
        let analysis = session
            .analysis
            .clone()
            .unwrap_or_else(|| BuilderEngine::build_analysis(&model));
        {
            let mut sessions = state.builder_sessions.lock().await;
            sessions.insert(session_id.clone(), session.clone());
        }
        {
            let store = state.builder_session_store.lock().await;
            store.save_session(&session).map_err(|e| e.to_string())?;
        }
        return Ok(serde_json::json!({
            "prompt": session.current_prompt,
            "progress": progress,
            "analysis": analysis,
        }));
    }
    {
        let mut sessions = state.builder_sessions.lock().await;
        sessions.insert(session_id.clone(), session.clone());
    }
    {
        let store = state.builder_session_store.lock().await;
        store.save_session(&session).map_err(|e| e.to_string())?;
    }
    let scheduler = {
        let scheduler = state.scheduler.lock().await;
        scheduler.clone()
    };
    let mut session = {
        let mut sessions = state.builder_sessions.lock().await;
        sessions
            .remove(&session_id)
            .ok_or_else(|| "Session not found".to_string())?
    };
    let engine = BuilderEngine::new(&scheduler);
    let (prompt, _) = engine.next_prompt(&mut session, "", &model).await;
    let progress = session.progress();
    let analysis = session
        .analysis
        .clone()
        .unwrap_or_else(|| BuilderEngine::build_analysis(&model));
    {
        let mut sessions = state.builder_sessions.lock().await;
        sessions.insert(session_id.clone(), session.clone());
    }
    {
        let store = state.builder_session_store.lock().await;
        store.save_session(&session).map_err(|e| e.to_string())?;
    }
    Ok(serde_json::json!({
        "prompt": prompt,
        "progress": progress,
        "analysis": analysis,
    }))
}

#[tauri::command]
pub async fn builder_start(
    mode: String,
    session_id: String,
    state: State<'_, Arc<AppState>>,
    target_dimension: Option<String>,
) -> Result<serde_json::Value, String> {
    builder_start_with_state(mode, session_id, state.inner(), target_dimension).await
}

#[tauri::command]
pub async fn builder_step(
    session_id: String,
    user_reply: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    builder_step_with_state(session_id, user_reply, state.inner()).await
}

async fn builder_step_with_state(
    session_id: String,
    user_reply: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    // Create AgentRun for Builder tracking
    let mut agent_run = openlife_core::agent::AgentRun::new_builder_run(&session_id);
    
    let mut session = {
        let mut sessions = state.builder_sessions.lock().await;
        sessions
            .remove(&session_id)
            .ok_or_else(|| "Session not found".to_string())?
    };
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let scheduler = {
        let scheduler = state.scheduler.lock().await;
        scheduler.clone()
    };
    let engine = BuilderEngine::new(&scheduler);
    let (prompt, updated_model) = engine.next_prompt(&mut session, &user_reply, &model).await;
    let finished = updated_model.is_some();
    let response_model = updated_model
        .as_ref()
        .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null));
    let progress = session.progress();
    let analysis = session
        .analysis
        .clone()
        .unwrap_or_else(|| BuilderEngine::build_analysis(&model));

    // Convert pending signals to JSON for frontend review
    let pending_signals: Vec<serde_json::Value> = session
        .pending_signals
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "source_step": s.source_step,
                "source_question_id": s.source_question_id,
                "dimension": format!("{:?}", s.dimension),
                "affected_path": s.affected_path,
                "proposed_value": s.proposed_value.clone(),
                "confidence": s.confidence,
                "reason": s.reason,
                "risk_level": format!("{}", s.risk_level),
                "user_status": format!("{:?}", s.user_status),
            })
        })
        .collect();

    let waiting_for_review = finished && !session.pending_signals.is_empty();
    if !finished || waiting_for_review {
        let mut sessions = state.builder_sessions.lock().await;
        if !finished {
            sessions.insert(session_id.clone(), session.clone());
        }
        let store = state.builder_session_store.lock().await;
        store.save_session(&session).map_err(|e| e.to_string())?;
    } else {
        if let Some(new_model) = updated_model {
            persist_life_model(state, new_model.clone(), true).await?;
        }
        let store = state.builder_session_store.lock().await;
        store
            .remove_session(&session_id)
            .map_err(|e| e.to_string())?;
    }
    // Complete AgentRun
    agent_run.output_preview = Some(prompt.clone());
    agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    agent_run.finished_at = Some(chrono::Utc::now());
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    Ok(serde_json::json!({
        "prompt": prompt,
        "finished": finished,
        "model": response_model,
        "progress": progress,
        "analysis": analysis,
        "pending_signals": pending_signals,
        "mode": format!("{:?}", session.mode),
        "target_dimension": session.target_dimension.as_ref().map(|d| format!("{:?}", d)),
    }))
}

#[tauri::command]
pub async fn builder_list_unfinished(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<BuilderSession>, String> {
    let store = state.builder_session_store.lock().await;
    store.list_unfinished_sessions().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn builder_delete_session(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut sessions = state.builder_sessions.lock().await;
        sessions.remove(&session_id);
    }
    let store = state.builder_session_store.lock().await;
    store.remove_session(&session_id).map_err(|e| e.to_string())
}

/// Get pending signals for a Quick Build session
#[tauri::command]
pub async fn builder_get_pending_signals(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let in_memory = {
        let sessions = state.builder_sessions.lock().await;
        sessions.get(&session_id).cloned()
    };
    let session = if let Some(session) = in_memory {
        session
    } else {
        let store = state.builder_session_store.lock().await;
        store
            .get_session(&session_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Session not found".to_string())?
    };

    let pending_signals: Vec<serde_json::Value> = session
        .pending_signals
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "source_step": s.source_step,
                "source_question_id": s.source_question_id,
                "dimension": format!("{:?}", s.dimension),
                "affected_path": s.affected_path,
                "proposed_value": s.proposed_value.clone(),
                "confidence": s.confidence,
                "reason": s.reason,
                "risk_level": format!("{}", s.risk_level),
                "user_status": format!("{:?}", s.user_status),
            })
        })
        .collect();

    let summary = BuilderSummary {
        identity_summary: format!(
            "基于 {} 个信号",
            session
                .pending_signals
                .iter()
                .filter(|s| s.dimension == BuilderDimension::Identity)
                .count()
        ),
        goals_summary: format!(
            "基于 {} 个信号",
            session
                .pending_signals
                .iter()
                .filter(|s| s.dimension == BuilderDimension::Goals)
                .count()
        ),
        capabilities_summary: format!(
            "基于 {} 个信号",
            session
                .pending_signals
                .iter()
                .filter(|s| s.dimension == BuilderDimension::Capabilities)
                .count()
        ),
        state_summary: format!(
            "基于 {} 个信号",
            session
                .pending_signals
                .iter()
                .filter(|s| s.dimension == BuilderDimension::State)
                .count()
        ),
        assumptions: vec!["用户通过快速构建流程提供".to_string()],
        unresolved_questions: vec![],
        recommended_next_steps: vec![
            "审阅并确认信号".to_string(),
            "可选择进入渐进构建继续完善".to_string(),
        ],
    };

    Ok(serde_json::json!({
        "session_id": session_id,
        "signals": pending_signals,
        "summary": summary,
        "finished": session.finished,
    }))
}

/// Apply accepted signals from Quick Build
async fn builder_apply_signals_with_state(
    session_id: String,
    decisions: Vec<openlife_core::builder::BuilderSignalDecision>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    // Create AgentRun for this direct apply
    let mut agent_run = openlife_core::agent::AgentRun::new_builder_run(&session_id);
    let run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    let in_memory_session = {
        let mut sessions = state.builder_sessions.lock().await;
        sessions.remove(&session_id)
    };
    let mut session = if let Some(session) = in_memory_session {
        session
    } else {
        let store = state.builder_session_store.lock().await;
        store
            .get_session(&session_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Session not found".to_string())?
    };

    // Load current model
    let mut model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };

    let mut edited_count = 0usize;
    let mut rejected_count = 0usize;
    let mut edited_fields = Vec::new();

    // Build a lookup from decision id to decision
    let decision_map: std::collections::HashMap<
        String,
        &openlife_core::builder::BuilderSignalDecision,
    > = decisions.iter().map(|d| (d.id.clone(), d)).collect();

    // Update signal statuses based on decisions
    for signal in &mut session.pending_signals {
        if let Some(decision) = decision_map.get(&signal.id) {
            match decision.status.as_str() {
                "accepted" => {
                    signal.user_status = SignalUserStatus::Accepted;
                }
                "edited" => {
                    signal.user_status = SignalUserStatus::Edited;
                    if let Some(new_value) = &decision.proposed_value {
                        signal.proposed_value = new_value.clone();
                        edited_count += 1;
                        edited_fields.push(format!("{}: edited", signal.affected_path));
                    }
                }
                _ => {
                    signal.user_status = SignalUserStatus::Rejected;
                    rejected_count += 1;
                }
            }
        } else {
            // No decision for this signal -> reject by default
            signal.user_status = SignalUserStatus::Rejected;
            rejected_count += 1;
        }
    }

    // Apply accepted and edited signals
    let (applied, skipped) =
        BuilderEngine::apply_signals_to_model(&mut model, &session.pending_signals);
    let merged: Vec<String> = applied
        .iter()
        .filter(|field| field.contains("(merged)"))
        .cloned()
        .collect();

    // Save the updated model
    persist_life_model(state, model.clone(), true).await?;

    // Create snapshot
    {
        let store = state.memory_store.lock().await;
        let _ = store.save_snapshot(&session_id, &model);
    }

    // Log the completion with audit info
    {
        let feedback = state.feedback_store.lock().await;
        let audit = format!(
            "applied: {}, skipped: {}, edited: {:?}, rejected: {}",
            applied.len(),
            skipped.len(),
            edited_fields,
            rejected_count
        );
        let _ = feedback.log_event("builder_apply_signals", Some(&session_id), Some(&audit));
    }

    // Clean up session
    let mut warnings = Vec::new();
    {
        let store = state.builder_session_store.lock().await;
        if let Err(e) = store.remove_session(&session_id) {
            warnings.push(format!("模型已写入，但构建会话清理失败: {}", e));
        }
    }

    // Mark AgentRun as completed
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
        agent_run.finished_at = Some(chrono::Utc::now());
        let _ = store.update_run(&agent_run);
    }

    let mut result = serde_json::json!({
        "success": true,
        "applied_fields": applied,
        "merged_fields": merged,
        "skipped_fields": skipped,
        "edited_count": edited_count,
        "rejected_count": rejected_count,
        "model": model,
        "run_id": run_id,
    });
    if !warnings.is_empty() {
        result["warnings"] = serde_json::Value::Array(
            warnings
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        );
    }
    Ok(result)
}

#[tauri::command]
pub async fn builder_apply_signals(
    session_id: String,
    decisions: Vec<openlife_core::builder::BuilderSignalDecision>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    builder_apply_signals_with_state(session_id, decisions, state.inner()).await
}

async fn builder_create_proposals_with_state(
    session_id: String,
    decisions: Vec<openlife_core::builder::BuilderSignalDecision>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    // Create AgentRun for this builder session
    let mut agent_run = openlife_core::agent::AgentRun::new_builder_run(&session_id);
    let run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    let in_memory_session = {
        let mut sessions = state.builder_sessions.lock().await;
        sessions.remove(&session_id)
    };
    let session = if let Some(session) = in_memory_session {
        session
    } else {
        let store = state.builder_session_store.lock().await;
        store
            .get_session(&session_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Session not found".to_string())?
    };
    if session.pending_signals.is_empty() {
        return Err("当前构建会话没有待确认信号，无法创建 Proposal。".to_string());
    }

    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let model_value = serde_json::to_value(&model).map_err(|e| e.to_string())?;
    let decision_map: std::collections::HashMap<
        String,
        &openlife_core::builder::BuilderSignalDecision,
    > = decisions.iter().map(|d| (d.id.clone(), d)).collect();

    let mut proposals = Vec::new();
    let mut rejected_count = 0usize;
    for signal in &session.pending_signals {
        let Some(decision) = decision_map.get(&signal.id) else {
            rejected_count += 1;
            continue;
        };
        if decision.status == "rejected" {
            rejected_count += 1;
            continue;
        }
        let after = if decision.status == "edited" {
            decision
                .proposed_value
                .clone()
                .ok_or_else(|| format!("编辑后的信号缺少 proposed_value：{}", signal.id))?
        } else {
            signal.proposed_value.clone()
        };
        let mut proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            &signal.affected_path,
            after,
            &signal.reason,
            signal.confidence,
            proposal_risk_level(&signal.risk_level),
            ProposalSource::BuilderReview,
        );
        proposal.before = value_at_path(&model_value, &signal.affected_path);
        proposal.run_id = Some(run_id.clone());
        proposal.source_detail = Some(format!("{}:{}", session_id, signal.id));
        proposals.push(proposal);
    }

    if proposals.is_empty() {
        return Err("没有被接受或编辑的信号可转为 Proposal。".to_string());
    }

    {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(|| "Proposal store is unavailable.".to_string())?
            .lock()
            .await;
        for proposal in &proposals {
            store.create_proposal(proposal).map_err(|e| e.to_string())?;
        }
    }

    // Update AgentRun with generated proposal IDs and mark as completed
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        for proposal in &proposals {
            let _ = store.add_generated_proposal(&run_id, &proposal.id);
        }
        agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
        agent_run.finished_at = Some(chrono::Utc::now());
        let _ = store.update_run(&agent_run);
    }

    let mut warnings = Vec::new();
    {
        let store = state.builder_session_store.lock().await;
        if let Err(e) = store.remove_session(&session_id) {
            warnings.push(format!("Proposal 已创建，但构建会话清理失败: {}", e));
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "created_count": proposals.len(),
        "rejected_count": rejected_count,
        "proposal_ids": proposals.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
        "run_id": run_id,
        "warnings": warnings,
    }))
}

#[tauri::command]
pub async fn builder_create_proposals(
    session_id: String,
    decisions: Vec<openlife_core::builder::BuilderSignalDecision>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    builder_create_proposals_with_state(session_id, decisions, state.inner()).await
}

#[tauri::command]
pub async fn get_model_4d_completion(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let completion = model.calculate_4d_completion();
    serde_json::to_value(completion).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn goal_capability_gap_analysis(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, String> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    Ok(model.goal_capability_gap_analysis())
}

#[tauri::command]
pub async fn goal_capability_gap_report(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::life_model::CapabilityGap>, String> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    Ok(model.goal_capability_gap_report())
}

#[tauri::command]
pub async fn identity_goal_alignment_check(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, String> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    Ok(model.identity_goal_alignment_check())
}

#[tauri::command]
pub async fn identity_goal_alignment_report(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::life_model::AlignmentIssue>, String> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    Ok(model.identity_goal_alignment_report())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::builder::{BuilderSignal, BuilderSignalDecision, RiskLevel};
    use openlife_core::config::AppConfig;
    use openlife_core::feedback::FeedbackStore;
    use openlife_core::layer_router::LayerRouter;
    use openlife_core::life_model::LifeModelManager;
    use openlife_core::mcp::McpRegistry;
    use openlife_core::mcp_audit::McpAuditStore;
    use openlife_core::memory::MemoryStore;
    use openlife_core::memory_cache::{HotMemoryCache, SharedHotCache};
    use openlife_core::privacy::PrivacyEngine;
    use openlife_core::router::IntentRouter;
    use openlife_core::scheduler::InferenceScheduler;
    use openlife_core::vectors::VectorStore;
    use openlife_core::versioning::VersionManager;
    use std::collections::HashMap;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = AppConfig::default();
        let life_model_manager =
            LifeModelManager::new(temp_dir.path().join("life-model").join("current"));
        let hot_cache: SharedHotCache = Arc::new(std::sync::Mutex::new(HotMemoryCache::default()));

        Arc::new(AppState {
            config: Arc::new(tokio::sync::Mutex::new(config.clone())),
            life_model_manager: Arc::new(tokio::sync::Mutex::new(life_model_manager)),
            memory_store: Arc::new(tokio::sync::Mutex::new(
                MemoryStore::new_in_memory().unwrap(),
            )),
            mcp_registry: Arc::new(tokio::sync::Mutex::new(McpRegistry::new())),
            intent_router: Arc::new(tokio::sync::Mutex::new(IntentRouter::new())),
            layer_router: Arc::new(tokio::sync::Mutex::new(LayerRouter::new())),
            scheduler: Arc::new(tokio::sync::Mutex::new(InferenceScheduler::new(
                config.local_model.clone(),
                config.prefer_local_model,
                config.llm.provider.clone(),
                config.llm.openai_base.clone(),
                config.llm.openai_key.clone(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                config.llm.embedding_enabled,
            ))),
            privacy_engine: Arc::new(tokio::sync::Mutex::new(PrivacyEngine::new())),
            version_manager: Arc::new(tokio::sync::Mutex::new(VersionManager::new(
                temp_dir.path().join("life-model").join("versions"),
            ))),
            feedback_store: Arc::new(tokio::sync::Mutex::new(
                FeedbackStore::new_in_memory().unwrap(),
            )),
            vector_store: Arc::new(tokio::sync::Mutex::new(
                VectorStore::new_in_memory().unwrap(),
            )),
            builder_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            builder_session_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::builder::BuilderSessionStore::new(
                    temp_dir.path().join("builder_sessions.json"),
                ),
            )),
            a2a_sidecar: Arc::new(tokio::sync::Mutex::new(
                crate::a2a_sidecar::A2ASidecar::new(8765),
            )),
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(McpAuditStore::new(
                temp_dir.path().join("mcp_audit.db"),
            ))),
            agent_run_store: None,
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
            ))),
            patch_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
            ))),
            rollout_metrics_store: None,
            hot_cache,
            startup_warnings: vec![],
        })
    }

    #[tokio::test]
    async fn builder_start_restores_finished_review_session_from_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("review-session", BuilderMode::Quick);
        session.finished = true;
        session.current_prompt = "快速构建问题已完成！接下来请审阅 AI 生成的模型建议。".into();
        session.pending_signals.push(BuilderSignal {
            id: "sig_name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::Value::String("fujing".into()),
            confidence: 0.95,
            reason: "用户直接提供的称呼".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let res = builder_start_with_state("quick".into(), "review-session".into(), &state, None)
            .await
            .unwrap();

        assert_eq!(res.get("finished").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            res.get("pending_signals")
                .and_then(|v| v.as_array())
                .map(|v| v.len()),
            Some(1)
        );
        assert_eq!(
            res.get("prompt").and_then(|v| v.as_str()),
            Some("快速构建问题已完成！接下来请审阅 AI 生成的模型建议。")
        );
    }

    #[tokio::test]
    async fn builder_apply_signals_updates_model_and_removes_persisted_session() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("apply-session", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals = vec![
            BuilderSignal {
                id: "sig_name".into(),
                source_step: 1,
                source_question_id: "name".into(),
                dimension: BuilderDimension::Identity,
                affected_path: "identity.name".into(),
                proposed_value: serde_json::Value::String("fujing".into()),
                confidence: 0.95,
                reason: "用户直接提供的称呼".into(),
                risk_level: RiskLevel::Low,
                user_status: SignalUserStatus::Pending,
            },
            BuilderSignal {
                id: "sig_goal".into(),
                source_step: 3,
                source_question_id: "short_term_goals".into(),
                dimension: BuilderDimension::Goals,
                affected_path: "goals.short_term".into(),
                proposed_value: serde_json::json!([
                    {
                        "name": "把 OpenLife 跑通",
                        "priority": 5,
                        "status": "pending",
                        "milestones": [],
                        "description": "",
                        "progress": 0.0
                    }
                ]),
                confidence: 0.8,
                reason: "用户描述的近期目标".into(),
                risk_level: RiskLevel::Medium,
                user_status: SignalUserStatus::Pending,
            },
            BuilderSignal {
                id: "sig_comm_style".into(),
                source_step: 7,
                source_question_id: "companion_style".into(),
                dimension: BuilderDimension::Identity,
                affected_path: "preferences.communication_style".into(),
                proposed_value: serde_json::Value::String("苏格拉底式追问型".into()),
                confidence: 0.9,
                reason: "用户选择的陪伴风格".into(),
                risk_level: RiskLevel::Low,
                user_status: SignalUserStatus::Pending,
            },
        ];
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let decisions = vec![
            BuilderSignalDecision {
                id: "sig_name".into(),
                status: "accepted".into(),
                proposed_value: None,
            },
            BuilderSignalDecision {
                id: "sig_goal".into(),
                status: "accepted".into(),
                proposed_value: None,
            },
            BuilderSignalDecision {
                id: "sig_comm_style".into(),
                status: "accepted".into(),
                proposed_value: None,
            },
        ];

        let res = builder_apply_signals_with_state("apply-session".into(), decisions, &state)
            .await
            .unwrap();

        assert_eq!(res.get("success").and_then(|v| v.as_bool()), Some(true));
        assert!(res
            .get("applied_fields")
            .and_then(|v| v.as_array())
            .is_some_and(|items| !items.is_empty()));

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.identity.name, "fujing");
        assert!(model
            .goals
            .short_term
            .iter()
            .any(|goal| goal.name == "把 OpenLife 跑通"));
        assert_eq!(model.preferences.communication_style, "苏格拉底式追问型");

        let persisted = state
            .builder_session_store
            .lock()
            .await
            .get_session("apply-session")
            .unwrap();
        assert!(persisted.is_none());
    }

    #[tokio::test]
    async fn builder_step_keeps_finished_review_session_without_persisting_draft_model() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("review-step-session", BuilderMode::Quick);
        session.step_index = 7;
        session.draft_yaml = [
            "# step 1\nfujing",
            "# step 2\n事业 / 学业",
            "# step 3\n- 把 OpenLife 跑通",
            "# step 4\n成为能持续创造产品的人",
            "# step 5\n- 编程\n- 产品设计",
            "# step 6\n- 精力不足",
            "# step 7\n苏格拉底追问型",
        ]
        .join("\n");

        state
            .builder_sessions
            .lock()
            .await
            .insert("review-step-session".into(), session);

        let res = builder_step_with_state(
            "review-step-session".into(),
            "最终确认前的回答".into(),
            &state,
        )
        .await
        .unwrap();

        assert_eq!(res.get("finished").and_then(|v| v.as_bool()), Some(true));
        assert!(res
            .get("pending_signals")
            .and_then(|v| v.as_array())
            .is_some_and(|items| !items.is_empty()));

        let persisted = state
            .builder_session_store
            .lock()
            .await
            .get_session("review-step-session")
            .unwrap();
        assert!(persisted.is_some());

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert!(model.is_effectively_empty());
    }

    #[tokio::test]
    async fn builder_create_proposals_moves_review_signals_to_proposal_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("proposal-session", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals.push(BuilderSignal {
            id: "sig_name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::Value::String("fujing".into()),
            confidence: 0.95,
            reason: "用户直接提供的称呼".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let decisions = vec![BuilderSignalDecision {
            id: "sig_name".into(),
            status: "accepted".into(),
            proposed_value: None,
        }];
        let res = builder_create_proposals_with_state("proposal-session".into(), decisions, &state)
            .await
            .unwrap();

        assert_eq!(res.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(res.get("created_count").and_then(|v| v.as_u64()), Some(1));
        let proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].affected_path, "identity.name");
        assert_eq!(proposals[0].after, serde_json::json!("fujing"));

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert!(model.is_effectively_empty());
        let persisted = state
            .builder_session_store
            .lock()
            .await
            .get_session("proposal-session")
            .unwrap();
        assert!(persisted.is_none());
    }
}
