use crate::AppState;
use openlife_core::config::AppConfig;
use std::sync::Arc;
use tokio::sync::Mutex;

fn isolated_eval_mcp_audit_store(
    path: std::path::PathBuf,
) -> openlife_core::mcp_audit::McpAuditStore {
    let key_ref = format!("eval:mcp-audit:{}", uuid::Uuid::new_v4());
    openlife_core::mcp_audit::McpAuditStore::with_key_materials(
        path,
        vec![openlife_core::mcp_audit::AuditKeyMaterial {
            config: openlife_core::mcp_audit::AuditKeyConfig {
                mode: openlife_core::mcp_audit::KeyMode::Keychain,
                salt_b64: None,
                env_var: None,
                key_ref: Some(key_ref),
                epoch: 1,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
            key: rand::random(),
        }],
    )
    .expect("isolated eval audit key material")
}

pub(crate) fn build_isolated_main_chat_eval_state() -> Arc<AppState> {
    let config = AppConfig::default();
    let base =
        std::env::temp_dir().join(format!("openlife-main-chat-eval-{}", uuid::Uuid::new_v4()));
    let memory_store = openlife_core::memory::MemoryStore::new_in_memory().unwrap();
    let life_model_manager =
        openlife_core::life_model::LifeModelManager::new(base.join("life-model").join("current"));
    openlife_core::persistence_outbox::FileMutationJournal::new(
        life_model_manager.mutation_journal_path(),
    )
    .expect("isolated eval LifeModel file-mutation journal");
    let state = Arc::new(AppState {
        persistence_coordinator: Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::isolated_evaluation(),
        ),
        config: Arc::new(Mutex::new(config.clone())),
        life_model_manager: Arc::new(Mutex::new(life_model_manager)),
        life_model_write_coordinator: Arc::new(Mutex::new(())),
        memory_store: Arc::new(Mutex::new(memory_store)),
        conversation_store: Some(Arc::new(Mutex::new(
            openlife_core::conversation::ConversationStore::new_in_memory()
                .expect("initialize isolated ConversationStore"),
        ))),
        mcp_registry: Arc::new(Mutex::new(openlife_core::mcp::McpRegistry::new())),
        scheduler: Arc::new(Mutex::new(
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
        privacy_engine: Arc::new(Mutex::new(openlife_core::privacy::PrivacyEngine::new())),
        version_manager: Arc::new(Mutex::new(openlife_core::versioning::VersionManager::new(
            base.join("life-model").join("versions"),
        ))),
        feedback_store: Arc::new(Mutex::new(
            openlife_core::feedback::FeedbackStore::new_in_memory().unwrap(),
        )),
        vector_store: Arc::new(Mutex::new(
            openlife_core::vectors::VectorStore::new_in_memory().unwrap(),
        )),
        vector_persistence_mode: crate::state::VectorPersistenceMode::EvalDisabled,
        last_snapshot_date: Arc::new(Mutex::new(None)),
        mcp_audit_store: Arc::new(Mutex::new(isolated_eval_mcp_audit_store(
            base.join("mcp_audit.db"),
        ))),
        canonical_task_runtime_store: Some(Arc::new(Mutex::new(
            openlife_core::task_runtime::CanonicalTaskRuntimeStore::new_in_memory_with_receipt_key(
                openlife_core::agent::CanonicalTaskReceiptKey::from_bytes([0xC7; 32]).unwrap(),
            )
            .unwrap(),
        ))),
        evidence_store: Arc::new(Mutex::new(
            openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
        )),
        policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
        proposal_store: Some(Arc::new(Mutex::new(
            openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
        ))),
        memory_lifecycle_store: Some(Arc::new(Mutex::new(
            openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
        ))),
        life_model_learning_store: Some(Arc::new(Mutex::new(
            openlife_core::agent::LifeModelLearningStore::new_in_memory().unwrap(),
        ))),
        main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
        patch_store: Some(Arc::new(Mutex::new(
            openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
        ))),
        tool_permission_store: Arc::new(Mutex::new(
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
        )),
        skill_registry: Arc::new(Mutex::new(openlife_core::skills::SkillRegistry::built_in())),
        hot_cache: Arc::new(tokio::sync::RwLock::new(
            openlife_core::memory_cache::HotMemoryCache::default(),
        )),
        startup_warnings: vec![],
        credential_bootstrap_snapshot: Default::default(),
        scheduled_task_store: Arc::new(openlife_core::tasks::TaskStore::new_in_memory().unwrap()),
        web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
        resource_runtime: None,
        state_store: Some(Arc::new(
            openlife_core::state_store::StateStore::new_in_memory().unwrap(),
        )),
        shutdown_notify: Arc::new(tokio::sync::Notify::new()),
    });

    {
        let manager = state
            .life_model_manager
            .try_lock()
            .expect("isolated eval LifeModel manager must remain uncontended");
        let model = manager
            .load()
            .expect("isolated eval daily-task migration source");
        crate::state_projection::reconcile_and_import_legacy_yaml_daily_tasks(
            state
                .state_store
                .as_ref()
                .expect("isolated eval canonical StateStore"),
            &model,
            chrono::Utc::now(),
        )
        .expect("isolated eval daily-task product owner cutover fixture");
    }
    state
}
