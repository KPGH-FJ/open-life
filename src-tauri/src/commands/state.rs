use crate::errors::AppError;
use crate::life_model_materializer_guard::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::memory_gateway;
use crate::{persist_life_model, AppState};
use openlife_core::life_model::{
    AlertLevel, CustomStateDimension, DailyGoal, StateAlert, TimeBlock,
};
use openlife_core::memory::StateHistoryEntry;
use std::sync::Arc;
use tauri::State;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_state_with_state(
    dimension_name: String,
    value: f64,
    unit: String,
    note: Option<String>,
    min_threshold: Option<f32>,
    max_threshold: Option<f32>,
    alert_days: Option<u32>,
    state: &Arc<AppState>,
) -> Result<i64, AppError> {
    let id = memory_gateway::record_state_entry_with_state(
        &dimension_name,
        value,
        &unit,
        note.as_deref(),
        state,
    )
    .await?;
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(AppError::from)?;
    if let Some(dim) = model
        .state
        .custom_dimensions
        .iter_mut()
        .find(|d| d.name == dimension_name)
    {
        dim.current_value = value as f32;
        if let Some(min) = min_threshold {
            dim.min_threshold = Some(min);
        }
        if let Some(max) = max_threshold {
            dim.max_threshold = Some(max);
        }
        if let Some(days) = alert_days {
            dim.alert_days = days;
        }
    } else {
        model.state.custom_dimensions.push(CustomStateDimension {
            name: dimension_name,
            unit,
            current_value: value as f32,
            min_threshold,
            max_threshold,
            alert_days: alert_days.unwrap_or(3),
        });
    }
    model.state.last_updated = Some(chrono::Utc::now().to_rfc3339());
    drop(manager);
    persist_life_model(
        &state.clone(),
        model,
        true,
        LifeModelMaterializerCallerContext::new(
            "state_record_state_source_data",
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
        ),
    )
    .await
    .map_err(AppError::from)?;
    Ok(id)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn record_state(
    dimension_name: String,
    value: f64,
    unit: String,
    note: Option<String>,
    min_threshold: Option<f32>,
    max_threshold: Option<f32>,
    alert_days: Option<u32>,
    state: State<'_, Arc<AppState>>,
) -> Result<i64, AppError> {
    record_state_with_state(
        dimension_name,
        value,
        unit,
        note,
        min_threshold,
        max_threshold,
        alert_days,
        &state.inner().clone(),
    )
    .await
}

#[tauri::command]
pub async fn get_state_history(
    dimension_name: String,
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<StateHistoryEntry>, AppError> {
    let store = state.memory_store.lock().await;
    store
        .get_state_history(&dimension_name, limit)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn get_state_alerts(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<StateAlert>, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let store = state.memory_store.lock().await;
    let mut alerts = Vec::new();
    for dim in &model.state.custom_dimensions {
        let entries = store
            .get_state_history(&dim.name, (dim.alert_days.max(1) as usize) * 2)
            .map_err(AppError::from)?;
        if entries.len() < dim.alert_days.max(1) as usize {
            continue;
        }
        let recent: Vec<_> = entries
            .iter()
            .rev()
            .take(dim.alert_days.max(1) as usize)
            .collect();
        let mut out_of_range_count = 0u32;
        for e in &recent {
            let out = match (dim.min_threshold, dim.max_threshold) {
                (Some(min), Some(max)) => e.value < min as f64 || e.value > max as f64,
                (Some(min), None) => e.value < min as f64,
                (None, Some(max)) => e.value > max as f64,
                (None, None) => false,
            };
            if out {
                out_of_range_count += 1;
            }
        }
        if out_of_range_count >= dim.alert_days {
            let msg = match (dim.min_threshold, dim.max_threshold) {
                (Some(min), Some(max)) => format!(
                    "{} 连续 {} 天超出阈值范围 [{}, {}]，当前 {:.1} {}",
                    dim.name, dim.alert_days, min, max, dim.current_value, dim.unit
                ),
                (Some(min), None) => format!(
                    "{} 连续 {} 天低于阈值 {}，当前 {:.1} {}",
                    dim.name, dim.alert_days, min, dim.current_value, dim.unit
                ),
                (None, Some(max)) => format!(
                    "{} 连续 {} 天高于阈值 {}，当前 {:.1} {}",
                    dim.name, dim.alert_days, max, dim.current_value, dim.unit
                ),
                _ => format!(
                    "{} 连续 {} 天异常，当前 {:.1} {}",
                    dim.name, dim.alert_days, dim.current_value, dim.unit
                ),
            };
            alerts.push(StateAlert {
                dimension_name: dim.name.clone(),
                level: AlertLevel::Warning,
                message: msg,
                triggered_at: chrono::Utc::now().to_rfc3339(),
            });
        }
    }
    Ok(alerts)
}

pub(crate) async fn get_daily_goals_with_state(
    state: &Arc<AppState>,
) -> Result<Vec<DailyGoal>, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    Ok(model.goals.daily)
}

#[tauri::command]
pub async fn get_daily_goals(state: State<'_, Arc<AppState>>) -> Result<Vec<DailyGoal>, AppError> {
    get_daily_goals_with_state(&state.inner().clone()).await
}

#[tauri::command]
pub async fn add_daily_goal(
    name: String,
    time_block: Option<TimeBlock>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(AppError::from)?;
    model.goals.daily.push(DailyGoal {
        name,
        done: false,
        time_block,
    });
    drop(manager);
    persist_life_model(
        &state.inner().clone(),
        model,
        true,
        LifeModelMaterializerCallerContext::new(
            "state_add_daily_goal_source_data",
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
        ),
    )
    .await
    .map_err(AppError::from)
    .map(|_| ())
}

#[tauri::command]
pub async fn update_daily_goal(
    index: usize,
    name: String,
    time_block: Option<TimeBlock>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(AppError::from)?;
    if let Some(goal) = model.goals.daily.get_mut(index) {
        goal.name = name;
        goal.time_block = time_block;
        drop(manager);
        persist_life_model(
            &state.inner().clone(),
            model,
            true,
            LifeModelMaterializerCallerContext::new(
                "state_update_daily_goal_source_data",
                LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
                LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
            ),
        )
        .await
        .map_err(AppError::from)
        .map(|_| ())
    } else {
        Err(AppError::not_found("invalid index"))
    }
}

#[tauri::command]
pub async fn delete_daily_goal(
    index: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(AppError::from)?;
    if index < model.goals.daily.len() {
        model.goals.daily.remove(index);
        drop(manager);
        persist_life_model(
            &state.inner().clone(),
            model,
            true,
            LifeModelMaterializerCallerContext::new(
                "state_delete_daily_goal_source_data",
                LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
                LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
            ),
        )
        .await
        .map_err(AppError::from)
        .map(|_| ())
    } else {
        Err(AppError::not_found("invalid index"))
    }
}

pub(crate) async fn toggle_daily_goal_with_state(
    index: usize,
    state: &Arc<AppState>,
) -> Result<bool, AppError> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(AppError::from)?;
    if index >= model.goals.daily.len() {
        return Err(AppError::not_found("invalid index"));
    }
    model.goals.daily[index].done = !model.goals.daily[index].done;
    let completed = model.goals.daily[index].done;
    drop(manager);
    persist_life_model(
        &state.clone(),
        model,
        true,
        LifeModelMaterializerCallerContext::new(
            "state_toggle_daily_goal_source_data",
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
        ),
    )
    .await
    .map_err(AppError::from)?;
    Ok(completed)
}

#[tauri::command]
pub async fn toggle_daily_goal(
    index: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, AppError> {
    toggle_daily_goal_with_state(index, &state.inner().clone()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = openlife_core::config::AppConfig::default();
        let hot_cache: openlife_core::memory_cache::SharedHotCache = Arc::new(
            tokio::sync::RwLock::new(openlife_core::memory_cache::HotMemoryCache::default()),
        );
        Arc::new(AppState {
            config: Arc::new(tokio::sync::Mutex::new(config.clone())),
            life_model_manager: Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::LifeModelManager::new(
                    temp_dir.path().join("life-model").join("current"),
                ),
            )),
            memory_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::memory::MemoryStore::new_in_memory().unwrap(),
            )),
            mcp_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp::McpRegistry::new(),
            )),
            scheduler: Arc::new(tokio::sync::Mutex::new(
                openlife_core::scheduler::InferenceScheduler::new(
                    config.local_model.clone(),
                    config.prefer_local_model,
                    config.llm.provider.clone(),
                    config.llm.openai_base.clone(),
                    config.llm.openai_key.clone(),
                    config.llm.chat_model.clone(),
                    config.llm.embedding_model.clone(),
                    config.llm.embedding_enabled,
                ),
            )),
            privacy_engine: Arc::new(tokio::sync::Mutex::new(
                openlife_core::privacy::PrivacyEngine::new(),
            )),
            version_manager: Arc::new(tokio::sync::Mutex::new(
                openlife_core::versioning::VersionManager::new(
                    temp_dir.path().join("life-model").join("versions"),
                ),
            )),
            feedback_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::feedback::FeedbackStore::new_in_memory().unwrap(),
            )),
            vector_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::vectors::VectorStore::new_in_memory().unwrap(),
            )),
            vector_persistence_mode: crate::state::VectorPersistenceMode::Enabled,
            builder_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            builder_session_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::builder::BuilderSessionStore::new(
                    temp_dir.path().join("builder_sessions.json"),
                ),
            )),
            a2a_sidecar: Arc::new(tokio::sync::Mutex::new(
                crate::a2a_sidecar::A2ASidecar::new(crate::a2a_server::configured_a2a_port()),
            )),
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp_audit::McpAuditStore::new(temp_dir.path().join("mcp_audit.db")),
            )),
            agent_run_store: None,
            evidence_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
            )),
            life_event_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::LifeEventStore::new_in_memory().unwrap(),
            ))),
            heuristic_store: Arc::new(tokio::sync::Mutex::new({
                let store = openlife_core::agent::HeuristicStore::new_in_memory().unwrap();
                store.seed_mvp_heuristics().unwrap();
                store
            })),
            policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
            ))),
            memory_lifecycle_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
            ))),
            plan_execute_session_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::PlanExecuteSessionStore::new_in_memory().unwrap(),
            ))),
            main_chat_agent_session_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_action_queue_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::main_chat_agent_v1::ActionQueueStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_agent_event_store: None,
            main_chat_selected_skill_ids: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
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
            startup_warnings: vec![],
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_mutex: Arc::new(tokio::sync::Mutex::new(())),
            runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
                crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
            )),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        })
    }

    #[tokio::test]
    async fn add_and_get_daily_goal() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        // Add a daily goal directly via LifeModel
        {
            let manager = state.life_model_manager.lock().await;
            let mut model = manager.load().unwrap_or_default();
            model.goals.daily.push(DailyGoal {
                name: "Exercise".to_string(),
                done: false,
                time_block: None,
            });
            manager.save(&model).unwrap();
        }

        // Get daily goals
        let goals = get_daily_goals_with_state(&state).await.unwrap();
        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].name, "Exercise");
        assert!(!goals[0].done);
    }

    #[tokio::test]
    async fn toggle_daily_goal_changes_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        // Add a goal directly
        {
            let manager = state.life_model_manager.lock().await;
            let mut model = manager.load().unwrap_or_default();
            model.goals.daily.push(DailyGoal {
                name: "Read".to_string(),
                done: false,
                time_block: None,
            });
            manager.save(&model).unwrap();
        }

        // Toggle it
        let completed = toggle_daily_goal_with_state(0, &state).await.unwrap();
        assert!(completed);

        // Verify
        let goals = get_daily_goals_with_state(&state).await.unwrap();
        assert!(goals[0].done);

        // Toggle back
        let completed = toggle_daily_goal_with_state(0, &state).await.unwrap();
        assert!(!completed);
    }
}
