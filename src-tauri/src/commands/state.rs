use crate::errors::AppError;
use crate::AppState;
use openlife_core::life_model::{AlertLevel, DailyGoal, StateAlert};
use openlife_core::state_store::StateHistoryEntry;
use std::sync::Arc;
use tauri::State;

fn canonical_state_store(
    state: &Arc<AppState>,
) -> Result<&openlife_core::state_store::StateStore, AppError> {
    state
        .state_store
        .as_deref()
        .ok_or_else(|| {
            AppError::db_with_hint(
                "StateStore is unavailable; state history truth is degraded and no temporary fallback is allowed.",
                "restart_after_repairing_state_db",
            )
        })
}

fn state_history_error(error: anyhow::Error) -> AppError {
    let message = error.to_string();
    if message.contains("state_history_product_owner_") {
        AppError::db_with_hint(
            format!("State history canonical owner is unavailable: {message}"),
            "restart_after_repairing_state_db",
        )
    } else {
        AppError::from(error)
    }
}

pub(crate) fn get_state_history_with_state(
    state: &Arc<AppState>,
    dimension_name: &str,
    limit: usize,
) -> Result<Vec<StateHistoryEntry>, AppError> {
    canonical_state_store(state)?
        .get_product_state_history(dimension_name, limit)
        .map_err(state_history_error)
}

#[tauri::command]
pub async fn get_state_history(
    dimension_name: String,
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<StateHistoryEntry>, AppError> {
    get_state_history_with_state(&state.inner().clone(), &dimension_name, limit)
}

pub(crate) async fn get_state_alerts_with_state(
    state: &Arc<AppState>,
) -> Result<Vec<StateAlert>, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    drop(manager);
    let store = canonical_state_store(state)?;
    let mut alerts = Vec::new();
    for dim in &model.state.custom_dimensions {
        let entries = store
            .get_product_state_history(&dim.name, (dim.alert_days.max(1) as usize) * 2)
            .map_err(state_history_error)?;
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

#[tauri::command]
pub async fn get_state_alerts(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<StateAlert>, AppError> {
    get_state_alerts_with_state(&state.inner().clone()).await
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
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    crate::state_projection::validate_legacy_yaml_daily_task_cutover_source(store, &model)
        .map_err(|error| {
            AppError::db_with_hint(
                format!("daily task StateStore authority is degraded: {error}"),
                "restart_after_repairing_state_db",
            )
        })?;
    let canonical = store.get_product_daily_tasks().map_err(AppError::from)?;
    Ok(canonical
        .iter()
        .map(crate::state_projection::projected_daily_goal)
        .collect())
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
            governed_data_import_journal: None,
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
            a2a_sidecar: Arc::new(tokio::sync::Mutex::new(
                crate::a2a_sidecar::A2ASidecar::new(crate::a2a_server::configured_a2a_port()),
            )),
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp_audit::McpAuditStore::new(temp_dir.path().join("mcp_audit.db")),
            )),
            agent_run_store: None,
            canonical_task_runtime_store: None,
            evidence_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
            )),
            life_event_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::LifeEventStore::new_in_memory().unwrap(),
            ))),
            policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
            ))),
            memory_lifecycle_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
            ))),
            life_model_learning_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::LifeModelLearningStore::new_in_memory().unwrap(),
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
            credential_bootstrap_snapshot: Default::default(),
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
    async fn daily_goal_product_read_fails_closed_then_uses_imported_statestore_owner() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        // Add a daily goal directly via LifeModel
        {
            let manager = state.life_model_manager.lock().await;
            let mut model = manager.load().unwrap_or_default();
            model.goals.daily.push(DailyGoal {
                name: "Exercise".to_string(),
                done: false,
                time_block: Some(openlife_core::life_model::TimeBlock {
                    start: "09:00".into(),
                    end: "10:00".into(),
                }),
                due_at: None,
                operation_id: None,
                operation_digest: None,
            });
            manager.save(&model).unwrap();
        }

        let blocked = get_daily_goals_with_state(&state).await.unwrap_err();
        assert!(matches!(&blocked, AppError::Database { .. }));
        assert!(blocked
            .message()
            .contains("daily_task_product_owner_not_ready"));

        let model = state.life_model_manager.lock().await.load().unwrap();
        crate::state_projection::reconcile_and_import_legacy_yaml_daily_tasks(
            state.state_store.as_ref().unwrap(),
            &model,
            chrono::Utc::now(),
        )
        .unwrap();
        let goals = get_daily_goals_with_state(&state).await.unwrap();
        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].name, "Exercise");
        assert!(!goals[0].done);
        assert_eq!(
            goals[0]
                .time_block
                .as_ref()
                .map(|block| block.start.as_str()),
            Some("09:00")
        );

        crate::state_projection::reconcile_state_store_lifemodel_projection(&state)
            .await
            .unwrap();
        let projected = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(projected.goals.daily.len(), 1);
        assert!(
            crate::state_projection::is_state_store_projected_daily_goal(&projected.goals.daily[0])
        );
        assert_eq!(
            projected.goals.daily[0]
                .time_block
                .as_ref()
                .map(|block| block.end.as_str()),
            Some("10:00")
        );
        assert_eq!(
            get_daily_goals_with_state(&state).await.unwrap()[0].name,
            "Exercise"
        );

        {
            let manager = state.life_model_manager.lock().await;
            let mut drifted = manager.load().unwrap();
            drifted.goals.daily.push(DailyGoal {
                name: "切换后旧路线写入".into(),
                done: false,
                time_block: None,
                due_at: None,
                operation_id: None,
                operation_digest: None,
            });
            manager.save(&drifted).unwrap();
        }
        let drift = get_daily_goals_with_state(&state).await.unwrap_err();
        assert!(drift
            .message()
            .contains("legacy_daily_task_source_changed_after_cutover"));
    }

    #[test]
    fn state_history_product_read_fails_closed_without_import_receipt() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let error = get_state_history_with_state(&state, "focus", 10).unwrap_err();
        assert!(matches!(&error, AppError::Database { .. }));
        assert!(error
            .message()
            .contains("state_history_product_owner_not_ready"));
    }

    #[tokio::test]
    async fn state_history_and_alerts_read_only_canonical_statestore_after_cutover() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let store = state.state_store.as_ref().unwrap();
        store
            .reconcile_legacy_state_history_shadow(
                openlife_core::persistence_outbox::metadata_digest(
                    "commands-state-history-cutover",
                ),
                vec![
                    openlife_core::state_store::LegacyStateHistoryShadowCandidate {
                        legacy_id: 1,
                        dimension_name: "focus".into(),
                        value: 3.0,
                        unit: "/10".into(),
                        recorded_at: chrono::Utc::now() - chrono::Duration::days(1),
                        note: Some("legacy day one".into()),
                        legacy_operation_id: None,
                        legacy_operation_digest: None,
                    },
                    openlife_core::state_store::LegacyStateHistoryShadowCandidate {
                        legacy_id: 2,
                        dimension_name: "focus".into(),
                        value: 4.0,
                        unit: "/10".into(),
                        recorded_at: chrono::Utc::now(),
                        note: Some("legacy day two".into()),
                        legacy_operation_id: None,
                        legacy_operation_digest: None,
                    },
                ],
                chrono::Utc::now(),
            )
            .unwrap();
        store
            .import_legacy_state_history_shadow(chrono::Utc::now())
            .unwrap();
        {
            let manager = state.life_model_manager.lock().await;
            let mut model = manager.load().unwrap_or_default();
            model
                .state
                .custom_dimensions
                .push(openlife_core::life_model::CustomStateDimension {
                    name: "focus".into(),
                    current_value: 4.0,
                    unit: "/10".into(),
                    min_threshold: Some(5.0),
                    max_threshold: None,
                    alert_days: 2,
                });
            manager.save(&model).unwrap();
        }

        let history = get_state_history_with_state(&state, "focus", 10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].note.as_deref(), Some("legacy day one"));
        assert_eq!(history[1].note.as_deref(), Some("legacy day two"));
        let alerts = get_state_alerts_with_state(&state).await.unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].dimension_name, "focus");
        assert!(alerts[0].message.contains("连续 2 天低于阈值 5"));
    }
}
