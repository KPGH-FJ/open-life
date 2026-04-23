use crate::{persist_life_model, AppState};
use openlife_core::builder::{
    BuilderDimension, BuilderEngine, BuilderMode, BuilderSession, BuilderSummary, SignalUserStatus,
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn builder_start(
    mode: String,
    session_id: String,
    state: State<'_, Arc<AppState>>,
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
pub async fn builder_step(
    session_id: String,
    user_reply: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
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

    // For Quick mode: don't auto-save on finish, wait for signal confirmation
    // For Socratic mode: keep existing behavior with value extraction logging
    if let Some(new_model) = updated_model {
        if session.mode == BuilderMode::Socratic && !session.extracted_values.is_empty() {
            let store = state.feedback_store.lock().await;
            for value in new_model.identity.values.iter().take(3) {
                let normalized = (value.weight as f32 / 10.0).clamp(0.3, 1.0);
                let delta = (0.03 * normalized).min(0.03);
                let _ = store.log_event(
                    &format!("value_focus:{}", value.name),
                    Some(&session_id),
                    Some("builder_confirmed"),
                );
                let _ = store.save_conversation_inference(
                    Some(&session_id),
                    "identity.values",
                    &value.name,
                    delta,
                    0.72,
                    "Builder 苏格拉底构建中确认了该价值观的重要性",
                );
            }
        }
        // Don't auto-save for Quick or Incremental mode - wait for signal confirmation
        if session.mode != BuilderMode::Quick && session.mode != BuilderMode::Incremental {
            let _ = persist_life_model(&state.inner().clone(), new_model, true).await?;
        }
    }

    if !finished {
        let mut sessions = state.builder_sessions.lock().await;
        sessions.insert(session_id.clone(), session.clone());
    } else if session.mode == BuilderMode::Socratic {
        // Only clean up for Socratic mode - Quick and Incremental wait for signal confirmation
        let store = state.builder_session_store.lock().await;
        let _ = store.remove_session(&session_id);
    }
    {
        let store = state.builder_session_store.lock().await;
        store.save_session(&session).map_err(|e| e.to_string())?;
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
    let sessions = state.builder_sessions.lock().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;

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
#[tauri::command]
pub async fn builder_apply_signals(
    session_id: String,
    decisions: Vec<openlife_core::builder::BuilderSignalDecision>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let mut session = {
        let mut sessions = state.builder_sessions.lock().await;
        sessions
            .remove(&session_id)
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
                "rejected" | _ => {
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
    persist_life_model(&state.inner().clone(), model.clone(), true).await?;

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
    let store = state.builder_session_store.lock().await;
    let _ = store.remove_session(&session_id);

    Ok(serde_json::json!({
        "success": true,
        "applied_fields": applied,
        "merged_fields": merged,
        "skipped_fields": skipped,
        "edited_count": edited_count,
        "rejected_count": rejected_count,
        "model": model,
    }))
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
    Ok(serde_json::to_value(completion).map_err(|e| e.to_string())?)
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
