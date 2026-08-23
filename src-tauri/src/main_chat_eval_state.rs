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
    let memory_store =
        openlife_core::memory::KnowledgeNoteProjectionStore::new_in_memory().unwrap();
    let life_model_manager =
        openlife_core::life_model::LifeModelManager::new(base.join("life-model").join("current"));
    Arc::new(AppState {
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
        tool_permission_store: Arc::new(Mutex::new(
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
        )),
        skill_registry: Arc::new(Mutex::new(openlife_core::skills::SkillRegistry::built_in())),
        startup_warnings: vec![],
        credential_bootstrap_snapshot: Default::default(),
        web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
        work_initial_decision_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
        work_agent_step_fixture_outputs: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        work_semantic_verification_fixture_outputs: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        resource_runtime: None,
    })
}
