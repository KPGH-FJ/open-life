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

#[cfg(test)]
pub(crate) fn isolated_mcp_audit_store_for_test(
    path: std::path::PathBuf,
) -> openlife_core::mcp_audit::McpAuditStore {
    isolated_eval_mcp_audit_store(path)
}

pub(crate) fn build_isolated_main_chat_eval_state() -> Arc<AppState> {
    let config = AppConfig::default();
    let base =
        std::env::temp_dir().join(format!("openlife-main-chat-eval-{}", uuid::Uuid::new_v4()));
    let memory_store = openlife_core::memory::MemoryStore::new_in_memory().unwrap();
    let agent_run_receipt_key = loop {
        if let Ok(key) =
            openlife_core::agent::AgentRunReceiptKey::from_bytes(rand::random::<[u8; 32]>())
        {
            break key;
        }
    };
    let agent_run_store = openlife_core::agent::AgentRunStore::new_in_memory_with_receipt_key(
        agent_run_receipt_key.clone(),
    )
    .unwrap();
    agent_run_store
        .bind_canonical_memory_store(&memory_store)
        .expect("isolated eval AgentRunStore must bind canonical MemoryStore");
    let main_chat_agent_session_store =
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStore::new_in_memory_with_receipt_key(
            agent_run_receipt_key.clone(),
        )
        .unwrap();
    main_chat_agent_session_store
        .bind_canonical_memory_store(&memory_store)
        .expect("isolated eval task sessions must bind canonical MemoryStore");
    let main_chat_action_queue_store =
        openlife_core::agent::main_chat_agent_v1::ActionQueueStore::new_in_memory().unwrap();
    let main_chat_agent_event_store =
        crate::main_chat_event_stream::MainChatAgentEventStore::new_in_memory().unwrap();
    let reconciliation_public_key = main_chat_agent_event_store
        .reconciliation_attestation_public_key()
        .expect("isolated eval EventStore reconciliation public key");
    main_chat_action_queue_store
        .install_event_store_reconciliation_public_key(&reconciliation_public_key)
        .expect("isolated eval ActionQueue must trust its EventStore");
    let life_model_manager =
        openlife_core::life_model::LifeModelManager::new(base.join("life-model").join("current"));
    // Mirror release bootstrap ordering because both metadata owners share one
    // SQLite file. Recovery preflight opens the file read-only and therefore
    // requires the canonical file-journal schema to exist before the governed
    // import journal makes the path observable.
    openlife_core::persistence_outbox::FileMutationJournal::new(
        life_model_manager.mutation_journal_path(),
    )
    .expect("isolated eval LifeModel file-mutation journal");
    let governed_data_import_journal = Arc::new(
        openlife_core::persistence_outbox::GovernedDataImportJournal::new(
            life_model_manager.mutation_journal_path(),
        )
        .expect("isolated eval governed data-import journal"),
    );
    let state = Arc::new(AppState {
        persistence_coordinator: Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::isolated_evaluation(),
        ),
        governed_data_import_journal: Some(governed_data_import_journal),
        config: Arc::new(Mutex::new(config.clone())),
        life_model_manager: Arc::new(Mutex::new(life_model_manager)),
        life_model_write_coordinator: Arc::new(Mutex::new(())),
        memory_store: Arc::new(Mutex::new(memory_store)),
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
        builder_session_store: Arc::new(Mutex::new(
            openlife_core::builder::BuilderSessionStore::new(base.join("builder_sessions.json")),
        )),
        a2a_sidecar: Arc::new(Mutex::new(crate::a2a_sidecar::A2ASidecar::new(
            crate::a2a_server::configured_a2a_port(),
        ))),
        last_snapshot_date: Arc::new(Mutex::new(None)),
        mcp_audit_store: Arc::new(Mutex::new(isolated_eval_mcp_audit_store(
            base.join("mcp_audit.db"),
        ))),
        agent_run_store: Some(Arc::new(Mutex::new(agent_run_store))),
        evidence_store: Arc::new(Mutex::new(
            openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
        )),
        life_event_store: Some(Arc::new(Mutex::new(
            openlife_core::agent::LifeEventStore::new_in_memory_with_receipt_key(
                agent_run_receipt_key,
            )
            .unwrap(),
        ))),
        heuristic_store: Arc::new(Mutex::new({
            let store = openlife_core::agent::HeuristicStore::new_in_memory().unwrap();
            store.seed_mvp_heuristics().unwrap();
            store
        })),
        policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
        proposal_store: Some(Arc::new(Mutex::new(
            openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
        ))),
        memory_lifecycle_store: Some(Arc::new(Mutex::new(
            openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
        ))),
        plan_execute_session_store: Some(Arc::new(Mutex::new(
            openlife_core::agent::PlanExecuteSessionStore::new_in_memory().unwrap(),
        ))),
        main_chat_agent_session_store: Some(Arc::new(Mutex::new(main_chat_agent_session_store))),
        main_chat_action_queue_store: Some(Arc::new(Mutex::new(main_chat_action_queue_store))),
        main_chat_agent_event_store: Some(Arc::new(Mutex::new(main_chat_agent_event_store))),
        main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
        patch_store: Some(Arc::new(Mutex::new(
            openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
        ))),
        rollout_metrics_store: None,
        tool_permission_store: Arc::new(Mutex::new(
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
        )),
        skill_registry: Arc::new(Mutex::new(openlife_core::skills::SkillRegistry::built_in())),
        plugin_registry: Arc::new(Mutex::new(openlife_core::plugins::PluginRegistry::new(
            base.join("plugins"),
        ))),
        hot_cache: Arc::new(tokio::sync::RwLock::new(
            openlife_core::memory_cache::HotMemoryCache::default(),
        )),
        startup_warnings: vec![],
        credential_bootstrap_snapshot: Default::default(),
        provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
        scheduled_task_store: Arc::new(openlife_core::tasks::TaskStore::new_in_memory().unwrap()),
        runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
            crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
        )),
        web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
        resource_runtime: None,
        state_store: Some(Arc::new(
            openlife_core::state_store::StateStore::new_in_memory().unwrap(),
        )),
        shutdown_notify: Arc::new(tokio::sync::Notify::new()),
    });

    // This isolated eval profile explicitly promotes the category with a
    // test-only receipt so existing runtime contract tests exercise the
    // post-cutover read path. It is local fixture evidence and must never be
    // counted as a real product-runtime or live-trial receipt.
    {
        let manager = state
            .life_model_manager
            .try_lock()
            .expect("isolated eval LifeModel manager must be uncontended");
        let heuristic_store = state
            .heuristic_store
            .try_lock()
            .expect("isolated eval heuristic store must be uncontended");
        let registry = openlife_core::agent::HSAssetAuthorityRegistry::new(
            manager.hs_asset_authority_registry_path(),
        )
        .expect("isolated eval HS authority registry");
        let scenario = registry
            .record_product_scenario(
                openlife_core::agent::HSAssetCategory::CollaborationGuidance,
                1,
                "test-fixture:isolated-main-chat-eval-state",
                openlife_core::agent::HSAssetOwner::AcceptedHsStore,
                &[openlife_core::agent::BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING.into()],
                openlife_core::agent::digest_string("isolated-eval-hs-runtime-audit"),
            )
            .expect("isolated eval product receipt shape");
        let model = manager.load().expect("isolated eval LifeModel");
        let report = openlife_core::agent::complete_collaboration_guidance_cutover(
            &registry,
            &model,
            &heuristic_store,
            &scenario,
        )
        .expect("isolated eval collaboration guidance cutover fixture");
        manager
            .save_hs_compatibility_view(&report.projection.yaml)
            .expect("isolated eval derived compatibility view");
    }
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
