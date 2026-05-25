use crate::errors::AppError;
use crate::{persist_life_model, AppState};
use chrono::Datelike;
use openlife_core::agent::{
    AgentEventActor, AgentProposal, AgentRunEvent, AgentRunEventType, ProposalSource, ProposalType,
    RiskLevel,
};
use openlife_core::evolution::{EvolutionChange, MicroEvolutionEngine};
use std::sync::Arc;
use tauri::State;

/// 评估 calibration change 的风险级别
fn assess_change_risk(change: &EvolutionChange) -> RiskLevel {
    let path = change.dimension.to_lowercase();
    if path.starts_with("identity.") {
        if path.contains("mission") || path.contains("values") || path.contains("philosophy") {
            RiskLevel::High
        } else {
            RiskLevel::Medium
        }
    } else if path.starts_with("goals.") {
        if path.contains("long_term") || path.contains("life_goals") {
            RiskLevel::High
        } else {
            RiskLevel::Medium
        }
    } else if path.starts_with("capabilities.") {
        RiskLevel::Medium
    } else if path.starts_with("state.") {
        RiskLevel::Low
    } else {
        RiskLevel::Medium
    }
}

fn calibration_score_value(value: f32) -> serde_json::Value {
    serde_json::json!(value as u8)
}

fn calibration_patch_target(
    change: &EvolutionChange,
    before_model: &openlife_core::life_model::LifeModel,
) -> Result<(String, serde_json::Value, serde_json::Value), AppError> {
    match change.dimension.as_str() {
        "identity.values" => {
            let (idx, item) = before_model
                .identity
                .values
                .iter()
                .enumerate()
                .find(|(_, item)| item.name == change.target_name)
                .ok_or_else(|| {
                    format!(
                        "无法创建 calibration Proposal：identity.values 中不存在 {}",
                        change.target_name
                    )
                })?;
            Ok((
                format!("identity.values.{}.weight", idx),
                serde_json::json!(item.weight),
                calibration_score_value(change.new_value),
            ))
        }
        "goals" => {
            let goal_lists = [
                ("short_term", &before_model.goals.short_term),
                ("medium_term", &before_model.goals.medium_term),
                ("long_term", &before_model.goals.long_term),
                ("life_goals", &before_model.goals.life_goals),
            ];
            let (list_name, idx, goal) = goal_lists
                .iter()
                .find_map(|(list_name, goals)| {
                    goals
                        .iter()
                        .enumerate()
                        .find(|(_, goal)| goal.name == change.target_name)
                        .map(|(idx, goal)| (*list_name, idx, goal))
                })
                .ok_or_else(|| {
                    format!(
                        "无法创建 calibration Proposal：goals 中不存在 {}",
                        change.target_name
                    )
                })?;
            Ok((
                format!("goals.{}.{}.priority", list_name, idx),
                serde_json::json!(goal.priority),
                calibration_score_value(change.new_value),
            ))
        }
        "capabilities.skills" => {
            let (idx, skill) = before_model
                .capabilities
                .skills
                .iter()
                .enumerate()
                .find(|(_, skill)| skill.name == change.target_name)
                .ok_or_else(|| {
                    format!(
                        "无法创建 calibration Proposal：capabilities.skills 中不存在 {}",
                        change.target_name
                    )
                })?;
            Ok((
                format!("capabilities.skills.{}.proficiency", idx),
                serde_json::json!(skill.proficiency),
                calibration_score_value(change.new_value),
            ))
        }
        _ => Err(format!(
            "无法创建 calibration Proposal：不支持的维度 {}",
            change.dimension
        )
        .into()),
    }
}

/// 将 EvolutionChange 转换为 AgentProposal
fn change_to_proposal(
    change: &EvolutionChange,
    source: ProposalSource,
    before_model: &openlife_core::life_model::LifeModel,
) -> Result<AgentProposal, AppError> {
    let risk_level = assess_change_risk(change);
    let proposal_type = if change.dimension.starts_with("goals.") {
        ProposalType::GoalUpdate
    } else if change.dimension.starts_with("state.") {
        ProposalType::StateUpdate
    } else if change.dimension.starts_with("capabilities.") {
        ProposalType::CapabilityUpdate
    } else if change.dimension.starts_with("preferences.") {
        ProposalType::PreferenceUpdate
    } else {
        ProposalType::LifeModelUpdate
    };

    let (affected_path, before_value, after_value) =
        calibration_patch_target(change, before_model)?;

    let mut proposal = AgentProposal::new(
        proposal_type,
        &affected_path,
        after_value,
        &format!("Calibration 建议：{}", change.reason),
        change.confidence,
        risk_level,
        source,
    );
    proposal.before = Some(before_value);
    Ok(proposal)
}

fn proposal_created_payload(proposal: &AgentProposal) -> serde_json::Value {
    openlife_core::agent::trace_payloads::build_proposal_created_payload(
        proposal.id.clone(),
        proposal.source.to_string(),
        proposal.proposal_type.to_string(),
        proposal.affected_path.clone(),
        proposal.risk_level.to_string(),
        proposal.status.to_string(),
        proposal.source_detail.clone(),
    )
}

fn record_proposal_created_event(state: &Arc<AppState>, run_id: &str, proposal: &AgentProposal) {
    if let Some(ref event_store) = state.agent_run_event_store {
        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ProposalCreated,
            AgentEventActor::System,
            format!(
                "calibration proposal created for {}",
                proposal.affected_path
            ),
            proposal_created_payload(proposal),
        );
        let _ = event_store.append_event(&event);
    }
}

#[tauri::command]
pub async fn run_micro_evolution(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let engine = MicroEvolutionEngine::new(&store);
    let (result, signals) = engine.run_with_signals(&model).map_err(AppError::from)?;
    let signal_summary = signals.summary();
    let mut snapshot_version = None;
    if result.applied {
        let mut new_model = model.clone();
        MicroEvolutionEngine::apply_changes(&mut new_model, &result.changes)
            .map_err(AppError::from)?;
        drop(manager);
        let new_model = persist_life_model(&state.inner().clone(), new_model, false).await?;
        // auto snapshot after evolution
        let vm = state.version_manager.lock().await;
        if let Ok(snap) = vm.snapshot(&new_model, "auto:evolution", &result.message) {
            snapshot_version = Some(snap.version);
        }
    }
    Ok(serde_json::json!({
        "changes": result.changes,
        "applied": result.applied,
        "message": result.message,
        "snapshot_version": snapshot_version,
        "signal_summary": signal_summary,
    }))
}

#[tauri::command]
pub async fn generate_calibration_report(
    period_days: u64,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let report = store
        .generate_calibration_report(&model, period_days as i64)
        .map_err(AppError::from)?;
    Ok(serde_json::json!({
        "period_days": report.period_days,
        "feedback_up": report.feedback_up,
        "feedback_down": report.feedback_down,
        "top_liked_patterns": report.top_liked_patterns,
        "top_disliked_patterns": report.top_disliked_patterns,
        "value_changes": report.value_changes,
        "suggested_actions": report.suggested_actions,
        "summary_text": report.summary_text,
    }))
}

#[tauri::command]
pub async fn generate_micro_evolution_changes(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let mut agent_run = openlife_core::agent::AgentRun::new_calibration_run();

    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let engine = MicroEvolutionEngine::new(&store);
    let (result, signals) = engine.run_with_signals(&model).map_err(AppError::from)?;
    let signal_summary = signals.summary();
    let mut after_model = model.clone();
    let _ = MicroEvolutionEngine::apply_changes(&mut after_model, &result.changes);

    // Complete AgentRun
    agent_run.output_preview = Some(result.message.clone());
    agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    agent_run.finished_at = Some(chrono::Utc::now());
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    Ok(serde_json::json!({
        "applied": result.applied,
        "message": result.message,
        "changes": result.changes,
        "before": model.calculate_4d_completion(),
        "after": after_model.calculate_4d_completion(),
        "requires_confirmation": !result.changes.is_empty(),
        "signal_summary": signal_summary,
    }))
}

#[tauri::command]
pub async fn apply_calibration(
    changes: Vec<EvolutionChange>,
    mode: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let mode = mode.as_deref().unwrap_or("direct");

    if mode == "proposal" {
        // 创建 Proposal 而不是直接应用
        return calibration_create_proposals_with_state(changes, state.inner()).await;
    }

    // direct 模式：直接应用变更
    let mut agent_run = openlife_core::agent::AgentRun::new_calibration_run();

    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(AppError::from)?;
    MicroEvolutionEngine::apply_changes(&mut model, &changes).map_err(AppError::from)?;
    drop(manager);
    let model = persist_life_model(&state.inner().clone(), model, false).await?;
    let vm = state.version_manager.lock().await;
    let snap = vm
        .snapshot(&model, "auto:calibration", "用户确认并应用校准确认变更")
        .map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let _ = store.log_event(
        "calibration_applied",
        None,
        Some(&format!("applied_changes={}", changes.len())),
    );

    // Complete AgentRun
    agent_run.output_preview = Some(format!("Applied {} calibration changes", changes.len()));
    agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    agent_run.finished_at = Some(chrono::Utc::now());
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    Ok(serde_json::json!({
        "success": true,
        "snapshot_version": snap.version,
        "applied_count": changes.len(),
        "message": format!("已应用 {} 项校准变更，并创建快照 {}", changes.len(), snap.version),
    }))
}

#[tauri::command]
pub async fn should_show_calibration(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let now = chrono::Local::now();
    let is_monday = now.weekday() == chrono::Weekday::Mon;
    let is_first_day = now.day() == 1;
    let today = now.format("%Y-%m-%d").to_string();
    let store = state.feedback_store.lock().await;
    let already_weekly = store
        .count_event_today("calibration_prompt_weekly")
        .unwrap_or(1);
    let already_monthly = store
        .count_event_today("calibration_prompt_monthly")
        .unwrap_or(1);
    Ok(serde_json::json!({
        "weekly": is_monday && already_weekly == 0,
        "monthly": is_first_day && already_monthly == 0,
        "today": today,
    }))
}

#[tauri::command]
pub async fn calibration_create_proposals(
    changes: Vec<EvolutionChange>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    calibration_create_proposals_with_state(changes, state.inner()).await
}

async fn calibration_create_proposals_with_state(
    changes: Vec<EvolutionChange>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    // Create AgentRun for this calibration
    let mut agent_run = openlife_core::agent::AgentRun::new_calibration_run();
    let run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    drop(manager);

    let proposal_store_opt = state.proposal_store.clone();
    let store = proposal_store_opt
        .as_ref()
        .ok_or_else(|| "Proposal store 不可用".to_string())?;
    let store = store.lock().await;

    let mut created_ids = Vec::new();
    let mut errors = Vec::new();

    for change in &changes {
        match change_to_proposal(change, ProposalSource::CalibrationRun, &model) {
            Ok(mut proposal) => {
                proposal.run_id = Some(run_id.clone());
                proposal.source_detail = Some("evolution".to_string());
                let id = proposal.id.clone();
                if let Err(e) = store.create_proposal(&proposal) {
                    errors.push(format!("{}: {}", proposal.affected_path, e));
                } else {
                    record_proposal_created_event(state, &run_id, &proposal);
                    created_ids.push(id);
                }
            }
            Err(e) => {
                errors.push(format!("{}: {}", change.dimension, e));
            }
        }
    }

    // Update AgentRun with generated proposal IDs and mark as completed
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        for pid in &created_ids {
            let _ = store.add_generated_proposal(&run_id, pid);
        }
        agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
        agent_run.finished_at = Some(chrono::Utc::now());
        let _ = store.update_run(&agent_run);
    }

    Ok(serde_json::json!({
        "created_count": created_ids.len(),
        "created_ids": created_ids,
        "run_id": run_id,
        "error_count": errors.len(),
        "errors": errors,
        "message": format!("已创建 {} 个 Proposal 到 Review Center", created_ids.len()),
    }))
}

#[tauri::command]
pub async fn mark_calibration_shown(
    period: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let store = state.feedback_store.lock().await;
    let event = format!("calibration_prompt_{}", period);
    store
        .log_event(&event, None, None)
        .map_err(AppError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::config::AppConfig;
    use openlife_core::feedback::FeedbackStore;
    use openlife_core::layer_router::LayerRouter;
    use openlife_core::life_model::{GoalItem, LifeModel, LifeModelManager, Skill, ValueItem};
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
        let hot_cache: SharedHotCache =
            Arc::new(tokio::sync::RwLock::new(HotMemoryCache::default()));

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
            agent_run_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
            ))),
            agent_run_event_store: Some(Arc::new(
                openlife_core::agent::event_store::AgentRunEventStore::new_in_memory().unwrap(),
            )),
            plan_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::PlanStore::new_in_memory().unwrap(),
            ))),
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
            ))),
            patch_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
            ))),
            rollout_metrics_store: None,
            tool_permission_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            )),
            skill_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::skills::SkillRegistry::built_in(),
            )),
            plugin_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::plugins::PluginRegistry::new(temp_dir.path().join("plugins")),
            )),
            hot_cache,
            proposal_engine: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalEngine::new(),
            )),
            agent_spec_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::AgentSpecStore::new_in_memory().unwrap(),
            )),
            startup_warnings: vec![],
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_mutex: Arc::new(tokio::sync::Mutex::new(())),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        })
    }

    fn seeded_model() -> LifeModel {
        let mut model = LifeModel::default_model();
        model.identity.values.push(ValueItem {
            name: "成长".into(),
            weight: 5,
            description: "持续学习".into(),
        });
        model.goals.short_term.push(GoalItem {
            name: "跑通 OpenLife".into(),
            description: "".into(),
            priority: 4,
            status: "active".into(),
            progress: 0.0,
            deadline: None,
            milestones: vec![],
            related_memories: vec![],
            updated_at: None,
        });
        model.capabilities.skills.push(Skill {
            name: "Rust".into(),
            proficiency: 3,
            description: "".into(),
        });
        model
    }

    fn change(
        dimension: &str,
        target_name: &str,
        old_value: f32,
        new_value: f32,
    ) -> EvolutionChange {
        EvolutionChange {
            dimension: dimension.into(),
            target_name: target_name.into(),
            old_value,
            new_value,
            reason: "多源信号建议微调".into(),
            confidence: 0.82,
            sources: vec![openlife_core::evolution::SignalSource {
                source: "feedback".into(),
                score: 0.4,
                weight: 0.5,
            }],
        }
    }

    #[tokio::test]
    async fn calibration_create_proposals_preserves_review_metadata_and_records_redacted_events() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        {
            let manager = state.life_model_manager.lock().await;
            manager.save(&seeded_model()).unwrap();
        }

        let res = calibration_create_proposals_with_state(
            vec![
                change("identity.values", "成长", 5.0, 7.0),
                change("goals", "跑通 OpenLife", 4.0, 6.0),
                change("capabilities.skills", "Rust", 3.0, 8.0),
            ],
            &state,
        )
        .await
        .unwrap();

        assert_eq!(res.get("created_count").and_then(|v| v.as_u64()), Some(3));
        let proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert_eq!(proposals.len(), 3);
        let value_proposal = proposals
            .iter()
            .find(|p| p.affected_path == "identity.values.0.weight")
            .expect("identity value proposal should use patchable scalar path");
        assert_eq!(value_proposal.source, ProposalSource::CalibrationRun);
        assert_eq!(
            value_proposal.status,
            openlife_core::agent::ProposalStatus::Pending
        );
        assert_eq!(value_proposal.before, Some(serde_json::json!(5)));
        assert_eq!(value_proposal.after, serde_json::json!(7));
        assert!(value_proposal.reason.contains("Calibration 建议"));
        assert_eq!(value_proposal.risk_level, RiskLevel::High);

        let run_id = res
            .get("run_id")
            .and_then(|v| v.as_str())
            .expect("run id should be returned");
        let events = state
            .agent_run_event_store
            .as_ref()
            .unwrap()
            .list_events_by_run(run_id)
            .unwrap();
        assert_eq!(events.len(), 3);
        assert!(events
            .iter()
            .all(|e| e.event_type == AgentRunEventType::ProposalCreated));
        assert!(events.iter().all(|e| {
            let text = e.payload.to_string();
            e.payload["source"] == serde_json::json!("calibration_run")
                && e.payload["status"] == serde_json::json!("pending")
                && e.payload.get("prompt").is_none()
                && e.payload.get("before").is_none()
                && e.payload.get("after").is_none()
                && e.payload.get("life_model").is_none()
                && !text.contains("持续学习")
        }));

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.identity.values[0].weight, 5);
        assert_eq!(model.goals.short_term[0].priority, 4);
        assert_eq!(model.capabilities.skills[0].proficiency, 3);
    }

    #[tokio::test]
    async fn calibration_life_model_apply_requires_proposal_acceptance_status() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        {
            let manager = state.life_model_manager.lock().await;
            manager.save(&seeded_model()).unwrap();
        }

        let res = calibration_create_proposals_with_state(
            vec![change("identity.values", "成长", 5.0, 7.0)],
            &state,
        )
        .await
        .unwrap();
        let proposal_id = res["created_ids"][0].as_str().unwrap().to_string();

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(
            model.identity.values[0].weight, 5,
            "pending calibration proposal must not silently apply"
        );

        crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();
        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.identity.values[0].weight, 7);
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            stored.status,
            openlife_core::agent::ProposalStatus::Accepted
        );
        let patches = state
            .patch_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_patches_by_proposal(&proposal_id)
            .unwrap();
        assert_eq!(patches.len(), 1);
        assert_eq!(
            patches[0].source,
            openlife_core::life_model::patch::PatchSource::Calibration
        );

        for status in [
            openlife_core::agent::ProposalStatus::Rejected,
            openlife_core::agent::ProposalStatus::Edited,
        ] {
            let mut proposal = AgentProposal::new(
                ProposalType::LifeModelUpdate,
                "identity.values.0.weight",
                serde_json::json!(9),
                "must not apply unless pending and explicitly accepted",
                0.8,
                RiskLevel::Medium,
                ProposalSource::CalibrationRun,
            );
            proposal.before = Some(serde_json::json!(7));
            match status {
                openlife_core::agent::ProposalStatus::Rejected => proposal.reject(),
                openlife_core::agent::ProposalStatus::Edited => {
                    proposal.edit(serde_json::json!(9));
                }
                _ => unreachable!(),
            }
            let id = proposal.id.clone();
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .create_proposal(&proposal)
                .unwrap();

            let err = crate::commands::proposal::accept_proposal_with_state(id, &state)
                .await
                .expect_err("terminal proposal status must not be accepted again");
            assert!(
                err.contains("不能") || err.contains("已"),
                "unexpected error: {err}"
            );
            let model = state.life_model_manager.lock().await.load().unwrap();
            assert_eq!(model.identity.values[0].weight, 7);
        }
    }

    #[test]
    fn calibration_command_source_does_not_call_chat_facade_or_fallback() {
        let source = include_str!("calibration.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("calibration production source should exist");
        assert!(!production.contains("run_tauri_agent_task"));
        assert!(!production.contains("handle_agent_loop_fallback"));
        assert!(!production.contains("FallbackStarted"));
        assert!(!production.contains("FallbackCompleted"));
    }
}
