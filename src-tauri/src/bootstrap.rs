//! Application bootstrap: store initialization and AppState assembly.
//! Extracted from lib.rs to keep the main entry point focused on Tauri lifecycle.

use crate::a2a_sidecar;
use crate::main_chat_event_stream::MainChatAgentEventStore;
use crate::state::AppState;
use crate::storage::{
    load_mcp_audit_keyring_from_path, load_privacy_policy_from_path, mcp_audit_keyring_path,
    privacy_policy_path,
};
use openlife_core::agent::{
    main_chat_agent_v1::{ActionQueueStore, AgentTaskSessionStore},
    MemoryLifecycleStore, PlanExecuteSessionStore, ProposalEngine, ProposalStore,
};
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

fn init_evidence_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<openlife_core::agent::EvidenceStore, String> {
    match openlife_core::agent::EvidenceStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("evidence.db");
            startup_warnings.borrow_mut().push(format!(
                "evidence.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match openlife_core::agent::EvidenceStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 evidence.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    openlife_core::agent::EvidenceStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 evidence store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_heuristic_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<openlife_core::agent::HeuristicStore, String> {
    match openlife_core::agent::HeuristicStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("heuristics.db");
            startup_warnings.borrow_mut().push(format!(
                "heuristics.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match openlife_core::agent::HeuristicStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 heuristics.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    openlife_core::agent::HeuristicStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 heuristic store 初始化失败: primary={}, fallback={}, in_memory={}",
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

fn init_memory_lifecycle_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<MemoryLifecycleStore, String> {
    match MemoryLifecycleStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("memory_lifecycle.db");
            startup_warnings.borrow_mut().push(format!(
                "memory_lifecycle.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match MemoryLifecycleStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 memory_lifecycle.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    MemoryLifecycleStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 memory lifecycle store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_plan_execute_session_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<PlanExecuteSessionStore, String> {
    match PlanExecuteSessionStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("plan_execute_sessions.db");
            startup_warnings.borrow_mut().push(format!(
                "plan_execute_sessions.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match PlanExecuteSessionStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 plan_execute_sessions.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    PlanExecuteSessionStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 Plan-Execute session store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_main_chat_agent_session_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<AgentTaskSessionStore, String> {
    match AgentTaskSessionStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("main_chat_agent_sessions.db");
            startup_warnings.borrow_mut().push(format!(
                "main_chat_agent_sessions.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match AgentTaskSessionStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 main_chat_agent_sessions.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    AgentTaskSessionStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 Main Chat Agent session store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_main_chat_action_queue_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<ActionQueueStore, String> {
    match ActionQueueStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("main_chat_action_queue.db");
            startup_warnings.borrow_mut().push(format!(
                "main_chat_action_queue.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match ActionQueueStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 main_chat_action_queue.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    ActionQueueStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 Main Chat action queue store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_main_chat_agent_event_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<MainChatAgentEventStore, String> {
    match MainChatAgentEventStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let fallback = recovery_db_path("main_chat_agent_events.db");
            startup_warnings.borrow_mut().push(format!(
                "main_chat_agent_events.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match MainChatAgentEventStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 main_chat_agent_events.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    MainChatAgentEventStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 Main Chat agent event store 初始化失败: primary={}, fallback={}, in_memory={}",
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

    let evidence_db_path = data_dir.join("evidence.db");
    let evidence_store = init_store(
        || init_evidence_store(&evidence_db_path, &startup_warnings),
        || openlife_core::agent::EvidenceStore::new_in_memory().map_err(|e| e.to_string()),
        "EvidenceStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let heuristics_db_path = data_dir.join("heuristics.db");
    let heuristic_store = init_store(
        || init_heuristic_store(&heuristics_db_path, &startup_warnings),
        || openlife_core::agent::HeuristicStore::new_in_memory().map_err(|e| e.to_string()),
        "HeuristicStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });
    if let Err(e) = heuristic_store.seed_mvp_heuristics() {
        startup_warnings
            .borrow_mut()
            .push(format!("initial heuristics seed failed: {}", e));
    }
    let policy_store = openlife_core::agent::PolicyStore::mvp_builtin();

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

    let memory_lifecycle_db_path = data_dir.join("memory_lifecycle.db");
    let memory_lifecycle_store = init_store(
        || init_memory_lifecycle_store(&memory_lifecycle_db_path, &startup_warnings),
        || MemoryLifecycleStore::new_in_memory().map_err(|e| e.to_string()),
        "MemoryLifecycleStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let plan_execute_sessions_db_path = data_dir.join("plan_execute_sessions.db");
    let plan_execute_session_store = init_store(
        || init_plan_execute_session_store(&plan_execute_sessions_db_path, &startup_warnings),
        || PlanExecuteSessionStore::new_in_memory().map_err(|e| e.to_string()),
        "PlanExecuteSessionStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let main_chat_agent_sessions_db_path = data_dir.join("main_chat_agent_sessions.db");
    let main_chat_agent_session_store = init_store(
        || init_main_chat_agent_session_store(&main_chat_agent_sessions_db_path, &startup_warnings),
        || AgentTaskSessionStore::new_in_memory().map_err(|e| e.to_string()),
        "MainChatAgentSessionStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let main_chat_action_queue_db_path = data_dir.join("main_chat_action_queue.db");
    let main_chat_action_queue_store = init_store(
        || init_main_chat_action_queue_store(&main_chat_action_queue_db_path, &startup_warnings),
        || ActionQueueStore::new_in_memory().map_err(|e| e.to_string()),
        "MainChatActionQueueStore",
        &startup_warnings,
    )
    .unwrap_or_else(|e| {
        log::warn!("[startup] Fatal: {}", e);
        std::process::exit(1);
    });

    let main_chat_agent_events_db_path = data_dir.join("main_chat_agent_events.db");
    let main_chat_agent_event_store = init_store(
        || init_main_chat_agent_event_store(&main_chat_agent_events_db_path, &startup_warnings),
        || MainChatAgentEventStore::new_in_memory().map_err(|e| e.to_string()),
        "MainChatAgentEventStore",
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
    let mut skill_registry = openlife_core::skills::SkillRegistry::built_in();
    for record in plugin_registry.list() {
        if record.enabled && record.error.is_none() {
            for skill in &record.manifest.skills {
                let mut skill_clone = skill
                    .clone()
                    .as_plugin_declarative_only(&record.manifest.id);
                skill_clone.id = format!("plugin:{}:{}", record.manifest.id, skill.id);
                skill_registry.register(skill_clone);
            }
        }
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
        a2a_sidecar: Arc::new(Mutex::new(a2a_sidecar::A2ASidecar::new(
            crate::a2a_server::configured_a2a_port(),
        ))),
        last_snapshot_date: Arc::new(Mutex::new(None)),
        mcp_audit_store: Arc::new(Mutex::new(mcp_audit_store)),
        agent_run_store: Some(Arc::new(Mutex::new(agent_run_store))),
        evidence_store: Arc::new(Mutex::new(evidence_store)),
        heuristic_store: Arc::new(Mutex::new(heuristic_store)),
        policy_store: Arc::new(policy_store),
        proposal_store: Some(Arc::new(Mutex::new(proposal_store))),
        memory_lifecycle_store: Some(Arc::new(Mutex::new(memory_lifecycle_store))),
        plan_execute_session_store: Some(Arc::new(Mutex::new(plan_execute_session_store))),
        main_chat_agent_session_store: Some(Arc::new(Mutex::new(main_chat_agent_session_store))),
        main_chat_action_queue_store: Some(Arc::new(Mutex::new(main_chat_action_queue_store))),
        main_chat_agent_event_store: Some(Arc::new(Mutex::new(main_chat_agent_event_store))),
        main_chat_selected_skill_ids: Arc::new(Mutex::new(std::collections::HashMap::new())),
        main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
        patch_store: Some(Arc::new(Mutex::new(patch_store))),
        rollout_metrics_store,
        tool_permission_store: Arc::new(Mutex::new(tool_permission_store)),
        skill_registry: Arc::new(Mutex::new(skill_registry)),
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
        runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
            crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
        )),
        web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
        shutdown_notify: Arc::new(tokio::sync::Notify::new()),
    });

    BootstrapResult { state: app_state }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{
        EvidenceQuery, HeuristicQuery, BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
    };

    #[tokio::test]
    async fn bootstrap_initializes_hs_stores_and_seeds_mvp_heuristics() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let result = bootstrap(temp_dir.path().to_path_buf());

        result
            .state
            .evidence_store
            .lock()
            .await
            .query(EvidenceQuery::default())
            .unwrap();

        let heuristic_store = result.state.heuristic_store.lock().await;
        let heuristics = heuristic_store
            .query(HeuristicQuery {
                domain: Some("planning".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(heuristics
            .iter()
            .any(|heuristic| heuristic.id == BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING));
        assert!(result
            .state
            .policy_store
            .is_hard_policy_id(openlife_core::agent::BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY));
    }
}
