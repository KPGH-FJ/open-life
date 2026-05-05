//! Application bootstrap: store initialization and AppState assembly.
//! Extracted from lib.rs to keep the main entry point focused on Tauri lifecycle.

use crate::a2a_sidecar;
use crate::state::AppState;
use crate::storage::{
    load_mcp_audit_keyring_from_path, load_privacy_policy_from_path, mcp_audit_keyring_path,
    privacy_policy_path,
};
use openlife_core::agent::{ProposalEngine, ProposalStore};
use openlife_core::builder::BuilderSessionStore;
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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Result of the bootstrap process: assembled application state and startup warnings.
pub struct BootstrapResult {
    pub state: Arc<AppState>,
}

fn recovery_db_path(file_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("openlife-recovery")
        .join(std::process::id().to_string());
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "failed to create OpenLife recovery database directory {}: {}",
            dir.display(),
            e
        );
    }
    dir.join(file_name)
}

/// Helper to initialize a store with file-based fallback to in-memory.
fn init_store<T, F, G>(
    file_init: F,
    memory_init: G,
    name: &str,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
    G: FnOnce() -> Result<T, String>,
{
    match file_init() {
        Ok(store) => Ok(store),
        Err(e) => {
            startup_warnings
                .borrow_mut()
                .push(format!("{} file init failed: {}", name, e));
            memory_init().map_err(|e| {
                let msg = format!(
                    "CRITICAL: {} in-memory fallback also failed: {}. \
                     System resources may be exhausted.",
                    name, e
                );
                log::warn!("[startup] {}", msg);
                msg
            })
        }
    }
}

fn init_memory_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<MemoryStore, String> {
    match MemoryStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("memory.db");
            startup_warnings.borrow_mut().push(format!(
                "memory.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match MemoryStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 memory.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    MemoryStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 memory store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_feedback_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<FeedbackStore, String> {
    match FeedbackStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("feedback.db");
            startup_warnings.borrow_mut().push(format!(
                "feedback.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match FeedbackStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 feedback.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    FeedbackStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 feedback store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_vector_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<VectorStore, String> {
    match VectorStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("vectors.db");
            startup_warnings.borrow_mut().push(format!(
                "vectors.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match VectorStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 vectors.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    VectorStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 vector store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_agent_run_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<openlife_core::agent::AgentRunStore, String> {
    match openlife_core::agent::AgentRunStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("agent_runs.db");
            startup_warnings.borrow_mut().push(format!(
                "agent_runs.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match openlife_core::agent::AgentRunStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 agent_runs.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    openlife_core::agent::AgentRunStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 agent run store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_proposal_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<ProposalStore, String> {
    match ProposalStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("proposals.db");
            startup_warnings.borrow_mut().push(format!(
                "proposals.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match ProposalStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 proposals.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    ProposalStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 proposal store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

/// Bootstrap the entire application: config, stores, routers, engines, AppState.
/// Returns assembled AppState along with startup warnings.
pub fn bootstrap(data_dir: PathBuf) -> BootstrapResult {
    let startup_warnings = std::cell::RefCell::new(Vec::new());

    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        startup_warnings.borrow_mut().push(format!(
            "应用数据目录创建失败：{} ({})",
            data_dir.display(),
            e
        ));
    }

    let config_path = data_dir.join("config.yaml");
    let (config, config_warning) = AppConfig::load_or_default_with_warning(&config_path);
    if let Some(warning) = config_warning {
        startup_warnings.borrow_mut().push(warning);
    }

    // Apply system configuration
    openlife_core::ollama::set_ollama_cache_ttl_seconds(config.system.ollama_cache_ttl_seconds);

    // Initialize web search provider configuration
    openlife_core::agent::action_executor::helpers::set_search_config(
        &config.system.search_provider,
        &config.system.search_provider_key,
        &config.system.searxng_url,
    );

    let life_model_manager = LifeModelManager::new(data_dir.join("life-model").join("current"));

    let db_path = data_dir.join("memory.db");
    let memory_store = init_store(
        || init_memory_store(&db_path, &startup_warnings),
        || MemoryStore::new_in_memory().map_err(|e| e.to_string()),
        "MemoryStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let feedback_db_path = data_dir.join("feedback.db");
    let feedback_store = init_store(
        || init_feedback_store(&feedback_db_path, &startup_warnings),
        || FeedbackStore::new_in_memory().map_err(|e| e.to_string()),
        "FeedbackStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let vector_db_path = data_dir.join("vectors.db");
    let vector_store = init_store(
        || init_vector_store(&vector_db_path, &startup_warnings),
        || VectorStore::new_in_memory().map_err(|e| e.to_string()),
        "VectorStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let agent_runs_db_path = data_dir.join("agent_runs.db");
    let agent_run_store = init_store(
        || init_agent_run_store(&agent_runs_db_path, &startup_warnings),
        || openlife_core::agent::AgentRunStore::new_in_memory().map_err(|e| e.to_string()),
        "AgentRunStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let proposals_db_path = data_dir.join("proposals.db");
    let proposal_store = init_store(
        || init_proposal_store(&proposals_db_path, &startup_warnings),
        || ProposalStore::new_in_memory().map_err(|e| e.to_string()),
        "ProposalStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let patches_db_path = data_dir.join("patches.db");
    let patch_store = init_store(
        || {
            openlife_core::life_model::patch_store::PatchStore::new(&patches_db_path)
                .map_err(|e| e.to_string())
        },
        || {
            openlife_core::life_model::patch_store::PatchStore::new_in_memory()
                .map_err(|e| e.to_string())
        },
        "PatchStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let model_dir = data_dir.join("models");
    let intent_router = IntentRouter::with_optional_onnx(Some(&model_dir));
    let layer_router = LayerRouter::new();
    let scheduler = InferenceScheduler::new(
        config.local_model.clone(),
        config.prefer_local_model,
        config.llm.provider.clone(),
        config.llm.openai_base.clone(),
        config.llm.openai_key.clone(),
        config.llm.chat_model.clone(),
        config.llm.embedding_model.clone(),
        config.llm.embedding_enabled,
    );
    let privacy_engine =
        PrivacyEngine::with_policy(load_privacy_policy_from_path(&privacy_policy_path()));
    let version_manager = VersionManager::new(data_dir.join("life-model").join("versions"));
    let mcp_audit_store = McpAuditStore::with_keyring(
        data_dir.join("mcp_audit.db"),
        load_mcp_audit_keyring_from_path(&mcp_audit_keyring_path()),
    );

    let hot_cache: SharedHotCache = {
        let initial_cache = match life_model_manager.load() {
            Ok(model) => HotMemoryCache::from_life_model(&model),
            Err(_) => HotMemoryCache::default(),
        };
        Arc::new(tokio::sync::RwLock::new(initial_cache))
    };

    let mcp_registry = McpRegistry::new();
    let tool_permission_store = init_store(
        || {
            openlife_core::tool_permissions::ToolPermissionStore::new(
                data_dir.join("tool_permissions.db"),
            )
            .map_err(|e| e.to_string())
        },
        || {
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory()
                .map_err(|e| e.to_string())
        },
        "ToolPermissionStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let mut plugin_registry = openlife_core::plugins::PluginRegistry::new(data_dir.join("plugins"));
    if let Err(e) = plugin_registry.reload() {
        startup_warnings
            .borrow_mut()
            .push(format!("plugins manifest reload failed: {}", e));
    }

    let rollout_metrics_store = {
        let store_path = data_dir.join("rollout_metrics.db");
        match openlife_core::agent::RolloutMetricsStore::new(&store_path) {
            Ok(store) => Some(Arc::new(Mutex::new(store))),
            Err(e) => {
                startup_warnings
                    .borrow_mut()
                    .push(format!("rollout_metrics.db 初始化失败: {}", e));
                None
            }
        }
    };

    let app_state = Arc::new(AppState {
        config: Arc::new(Mutex::new(config)),
        life_model_manager: Arc::new(Mutex::new(life_model_manager)),
        memory_store: Arc::new(Mutex::new(memory_store)),
        mcp_registry: Arc::new(Mutex::new(mcp_registry)),
        intent_router: Arc::new(Mutex::new(intent_router)),
        layer_router: Arc::new(Mutex::new(layer_router)),
        scheduler: Arc::new(Mutex::new(scheduler)),
        privacy_engine: Arc::new(Mutex::new(privacy_engine)),
        version_manager: Arc::new(Mutex::new(version_manager)),
        feedback_store: Arc::new(Mutex::new(feedback_store)),
        vector_store: Arc::new(Mutex::new(vector_store)),
        builder_sessions: Arc::new(Mutex::new(HashMap::new())),
        builder_session_store: Arc::new(Mutex::new(BuilderSessionStore::new(
            data_dir.join("builder_sessions.json"),
        ))),
        a2a_sidecar: Arc::new(Mutex::new(a2a_sidecar::A2ASidecar::new(8765))),
        last_snapshot_date: Arc::new(Mutex::new(None)),
        mcp_audit_store: Arc::new(Mutex::new(mcp_audit_store)),
        agent_run_store: Some(Arc::new(Mutex::new(agent_run_store))),
        proposal_store: Some(Arc::new(Mutex::new(proposal_store))),
        patch_store: Some(Arc::new(Mutex::new(patch_store))),
        rollout_metrics_store,
        tool_permission_store: Arc::new(Mutex::new(tool_permission_store)),
        skill_registry: Arc::new(Mutex::new(openlife_core::skills::SkillRegistry::built_in())),
        plugin_registry: Arc::new(Mutex::new(plugin_registry)),
        hot_cache,
        proposal_engine: Arc::new(tokio::sync::Mutex::new({
            let mut engine = ProposalEngine::new();
            engine.register(Box::new(
                openlife_core::agent::ChatProposalGeneratorAdapter::new(),
            ));
            engine.register(Box::new(
                openlife_core::agent::FeedbackProposalGenerator::new(None),
            ));
            engine.register(Box::new(openlife_core::agent::MemoryProposalGenerator));
            engine.register(Box::new(openlife_core::agent::ToolProposalGenerator));
            engine
        })),
        startup_warnings: startup_warnings.into_inner(),
        provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
        scheduled_task_mutex: Arc::new(tokio::sync::Mutex::new(())),
        shutdown_notify: Arc::new(tokio::sync::Notify::new()),
    });

    BootstrapResult { state: app_state }
}
