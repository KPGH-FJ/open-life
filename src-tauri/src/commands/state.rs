use crate::errors::AppError;
use crate::AppState;
use openlife_core::life_model::{AlertLevel, DailyGoal, StateAlert};
use openlife_core::memory::StateHistoryEntry;
use std::sync::Arc;
use tauri::State;

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
    let store = state.state_store.as_ref().ok_or_else(|| {
        AppError::db_with_hint(
            "StateStore is unavailable; daily task truth is degraded and no temporary fallback is allowed.",
            "restart_after_repairing_state_db",
        )
    })?;
    let canonical = store.list_daily_tasks(false).map_err(AppError::from)?;
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let mut goals = model
        .goals
        .daily
        .into_iter()
        .filter(|goal| !crate::state_projection::is_state_store_projected_daily_goal(goal))
        .collect::<Vec<_>>();
    goals.extend(
        canonical
            .iter()
            .map(crate::state_projection::projected_daily_goal),
    );
    Ok(goals)
}

#[tauri::command]
pub async fn get_daily_goals(state: State<'_, Arc<AppState>>) -> Result<Vec<DailyGoal>, AppError> {
    get_daily_goals_with_state(&state.inner().clone()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = openlife_core::config::AppConfig::default();
        let hot_cache: openlife_core::memory_cache::SharedHotCache = Arc::new(
            tokio::sync::RwLock::new(openlife_core::memory_cache::HotMemoryCache::default()),
        );
        Arc::new(AppState {
            persistence_coordinator: Arc::new(
                crate::persistence_coordinator::PersistenceCoordinator::isolated_evaluation(),
            ),
            config: Arc::new(tokio::sync::Mutex::new(config.clone())),
            life_model_manager: Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::LifeModelManager::new(
                    temp_dir.path().join("life-model").join("current"),
                ),
            )),
            life_model_write_coordinator: Arc::new(tokio::sync::Mutex::new(())),
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
            startup_warnings: vec![],
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_store: Arc::new(
                openlife_core::tasks::TaskStore::new_in_memory().unwrap(),
            ),
            runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
                crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
            )),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            resource_runtime: None,
            state_store: Some(Arc::new(
                openlife_core::state_store::StateStore::new_in_memory().unwrap(),
            )),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        })
    }

    #[tokio::test]
    async fn legacy_yaml_daily_goal_remains_read_only_during_statestore_migration() {
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
                due_at: None,
                operation_id: None,
                operation_digest: None,
            });
            manager.save(&model).unwrap();
        }

        // Get daily goals
        let goals = get_daily_goals_with_state(&state).await.unwrap();
        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].name, "Exercise");
        assert!(!goals[0].done);
    }
}
