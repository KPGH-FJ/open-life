//! Application bootstrap: store initialization and AppState assembly.
//! Extracted from lib.rs to keep the main entry point focused on Tauri lifecycle.

use crate::credential_bootstrap::initialize_fresh_profile_credentials;
use crate::persistence_coordinator::PersistenceCoordinator;
use crate::secret_store::{
    hydrate_config_secrets_read_only, inspect_and_hydrate_integrity_key,
    inspect_existing_mcp_audit_keys, selected_secret_store_classification, IntegrityKeyHydration,
    McpAuditKeyHydrationInspection, ProfileSecretStore, ProviderCredentialHydrationStatus,
    SecretReader, StartupProfileSecretStore, CANONICAL_TASK_RECEIPT_KEY_REF,
};
use crate::state::{AppState, CredentialBootstrapSnapshot, CredentialBootstrapStatus};
use crate::storage::{load_mcp_audit_keyring_from_path, privacy_policy_path, McpAuditKeyringLoad};
use openlife_core::agent::{CanonicalTaskReceiptKey, MemoryLifecycleStore, ProposalStore};
use openlife_core::config::AppConfig;
use openlife_core::conversation::ConversationStore;
use openlife_core::feedback::FeedbackStore;
use openlife_core::life_model::LifeModelManager;
use openlife_core::mcp::McpRegistry;
use openlife_core::mcp_audit::McpAuditStore;
use openlife_core::memory::KnowledgeNoteProjectionStore;
use openlife_core::privacy::PrivacyEngine;
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::vectors::VectorStore;
use openlife_core::versioning::VersionManager;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Result of the bootstrap process: assembled application state and startup warnings.
pub struct BootstrapResult {
    pub state: Arc<AppState>,
}

fn protected_paths_are_absent(data_dir: &Path, relative_paths: &[&str]) -> std::io::Result<bool> {
    for relative_path in relative_paths {
        match std::fs::symlink_metadata(data_dir.join(relative_path)) {
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

fn inspect_fixed_credential<R: SecretReader + ?Sized>(
    data_dir: &Path,
    secret_ref: &'static str,
    protected_paths: &[&str],
    store: &R,
) -> (CredentialBootstrapStatus, Option<[u8; 32]>) {
    match inspect_and_hydrate_integrity_key(secret_ref, store) {
        IntegrityKeyHydration::Available(key) => (CredentialBootstrapStatus::Available, Some(key)),
        IntegrityKeyHydration::Missing => {
            match protected_paths_are_absent(data_dir, protected_paths) {
                Ok(true) => (CredentialBootstrapStatus::InitializationRequired, None),
                Ok(false) => (CredentialBootstrapStatus::MissingExistingData, None),
                Err(_) => (CredentialBootstrapStatus::Unknown, None),
            }
        }
        IntegrityKeyHydration::Invalid => (CredentialBootstrapStatus::Invalid, None),
        IntegrityKeyHydration::Unavailable => (CredentialBootstrapStatus::Unavailable, None),
    }
}

const STARTUP_PROPOSAL_RECONCILIATION_BATCH: i64 = 200;
const STARTUP_PROPOSAL_RECONCILIATION_SYNC_PASSES: usize = 5;
const STARTUP_CANONICAL_OUTBOX_BATCH: usize = 500;
const STARTUP_CANONICAL_OUTBOX_PASSES: usize = 20;
/// Drain canonical-owner outboxes before the product becomes interactive.
/// A deletion projection that cannot be reconciled fails closed at startup so
/// stale content is never presented as if deletion had completed.
pub(crate) async fn reconcile_startup_canonical_outboxes(
    state: &Arc<AppState>,
) -> Result<(), String> {
    for _ in 0..STARTUP_CANONICAL_OUTBOX_PASSES {
        let report = crate::memory_gateway::reconcile_blocking_canonical_outboxes_with_state(
            state,
            STARTUP_CANONICAL_OUTBOX_BATCH,
        )
        .await?;
        if report.blocking_degraded > 0 {
            return Err(format!(
                "canonical deletion/restore projection reconciliation degraded: {} delivery attempts",
                report.blocking_degraded
            ));
        }
        if !report.blocking_backlog_may_remain {
            return Ok(());
        }
    }
    Err("canonical projection reconciliation backlog exceeded startup bound".into())
}

/// Reconcile a bounded amount of already-confirmed Proposal truth before the
/// product window becomes interactive. `true` means a durable indexed backlog
/// remains and must be drained by the async continuation; it never means the
/// effects should be replayed.
pub(crate) async fn reconcile_startup_proposal_projections(
    state: &Arc<AppState>,
) -> Result<bool, String> {
    for _ in 0..STARTUP_PROPOSAL_RECONCILIATION_SYNC_PASSES {
        let report =
            crate::commands::proposal::reconcile_startup_durable_proposal_projections_with_state(
                state,
                STARTUP_PROPOSAL_RECONCILIATION_BATCH,
            )
            .await?;
        let backlog = report.artifact_backlog_may_remain || report.projection_backlog_may_remain;
        if !backlog {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) async fn drain_startup_proposal_projection_backlog(state: Arc<AppState>) {
    const MAX_BACKGROUND_PASSES: usize = 100;
    for _ in 0..MAX_BACKGROUND_PASSES {
        match crate::commands::proposal::reconcile_durable_proposal_projections_with_state(
            &state,
            STARTUP_PROPOSAL_RECONCILIATION_BATCH,
        )
        .await
        {
            Ok(report)
                if !report.artifact_backlog_may_remain && !report.projection_backlog_may_remain =>
            {
                return;
            }
            Ok(_) => tokio::task::yield_now().await,
            Err(error) => {
                log::warn!(
                    "[startup] Proposal projection reconciliation remains degraded: {}",
                    error
                );
                return;
            }
        }
    }
    log::warn!(
        "[startup] Proposal projection reconciliation backlog remains after bounded background passes"
    );
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

fn ephemeral_store_fallback_allowed() -> bool {
    cfg!(feature = "dev-extensions")
}

/// Helper to initialize a store with file-based fallback to in-memory.
fn init_store<T, F, G>(
    file_init: F,
    read_only_init: impl FnOnce() -> Result<T, String>,
    ephemeral_init: G,
    name: &str,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
    persistence: &PersistenceCoordinator,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
    G: FnOnce() -> Result<T, String>,
{
    let warning_count_before = startup_warnings.borrow().len();
    match file_init() {
        Ok(store) => {
            if startup_warnings.borrow().len() > warning_count_before {
                persistence.register_ephemeral_development(
                    name,
                    "dev_ephemeral_store_fallback",
                    "primary durable store failed; specialized development fallback was used",
                );
            } else {
                persistence.register_read_write(name);
            }
            Ok(store)
        }
        Err(e) => {
            startup_warnings
                .borrow_mut()
                .push(format!("{} file init failed: {}", name, e));
            if !ephemeral_store_fallback_allowed() {
                return match read_only_init() {
                    Ok(store) => {
                        persistence.register_read_only(name, "durable_store_write_open_failed", &e);
                        startup_warnings.borrow_mut().push(format!(
                            "{name} entered explicit read-only canonical recovery; all provider, tool, and canonical-write effects are disabled"
                        ));
                        Ok(store)
                    }
                    Err(read_only_error) => {
                        persistence.register_unavailable(
                            name,
                            "durable_store_unavailable",
                            &format!("primary={e}; read_only={read_only_error}"),
                        );
                        Err(format!(
                            "{name} canonical store is unavailable: primary={e}; read_only={read_only_error}"
                        ))
                    }
                };
            }
            ephemeral_init()
                .inspect(|_| {
                    persistence.register_ephemeral_development(
                        name,
                        "dev_ephemeral_store_fallback",
                        &e,
                    );
                })
                .map_err(|e| {
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

fn optional_store<T>(
    result: Result<T, String>,
    name: &str,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Option<T> {
    match result {
        Ok(store) => Some(store),
        Err(error) => {
            log::warn!("[startup] {name} unavailable: {error}");
            startup_warnings.borrow_mut().push(format!(
                "{name} canonical state is unavailable/unknown; the product remains degraded and all effects are disabled"
            ));
            None
        }
    }
}

fn required_store_or_unavailable<T>(
    result: Result<T, String>,
    name: &str,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
    unavailable_sentinel: impl FnOnce() -> Result<T, String>,
) -> T {
    match result {
        Ok(store) => store,
        Err(error) => {
            log::error!("[startup] {name} unavailable: {error}");
            startup_warnings.borrow_mut().push(format!(
                "{name} canonical state is unavailable/unknown; a schema-less query-only sentinel is active and all effects are disabled"
            ));
            unavailable_sentinel().unwrap_or_else(|sentinel_error| {
                panic!(
                    "{name} unavailable sentinel allocation failed after canonical open failure: {sentinel_error}"
                )
            })
        }
    }
}

fn init_memory_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<KnowledgeNoteProjectionStore, String> {
    match KnowledgeNoteProjectionStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "memory.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("memory.db");
            startup_warnings.borrow_mut().push(format!(
                "memory.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match KnowledgeNoteProjectionStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 memory.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    KnowledgeNoteProjectionStore::new_in_memory().map_err(|memory_err| {
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
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "feedback.db durable initialization failed: {primary_err}"
                ));
            }
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
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "vectors.db durable initialization failed: {primary_err}"
                ));
            }
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

fn init_proposal_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<ProposalStore, String> {
    match ProposalStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "proposals.db durable initialization failed: {primary_err}"
                ));
            }
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
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "memory_lifecycle.db durable initialization failed: {primary_err}"
                ));
            }
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

/// Bootstrap the entire application: config, stores, routers, engines, AppState.
/// Returns assembled AppState along with startup warnings.
pub fn bootstrap(data_dir: PathBuf) -> BootstrapResult {
    match selected_secret_store_classification() {
        Ok(classification) => {
            log::info!("[startup] credential store class: {classification}")
        }
        Err(error) => log::error!("[startup] credential store selection blocked: {error}"),
    }
    let startup_store = StartupProfileSecretStore::default();
    if let Err(error) =
        initialize_fresh_profile_credentials(&data_dir, &startup_store, &ProfileSecretStore)
    {
        log::warn!("[startup] fresh internal credential initialization skipped: {error}");
    }
    bootstrap_with_secret_store(data_dir, &startup_store)
}

fn bootstrap_with_secret_store(
    data_dir: PathBuf,
    secret_store: &dyn SecretReader,
) -> BootstrapResult {
    let startup_warnings = std::cell::RefCell::new(Vec::new());
    let persistence = Arc::new(PersistenceCoordinator::for_release_bootstrap());

    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        persistence.degrade_globally("application_data_directory_unavailable");
        startup_warnings.borrow_mut().push(format!(
            "应用数据目录创建失败：{} ({})",
            data_dir.display(),
            e
        ));
    }

    let config_path = data_dir.join("config.yaml");
    let (mut config, config_warning) = AppConfig::load_or_default_with_warning(&config_path);
    if let Some(warning) = config_warning {
        persistence.register_unavailable("ConfigStore", "config_load_failed", &warning);
        startup_warnings.borrow_mut().push(warning);
    } else {
        persistence.register_read_write("ConfigStore");
    }
    let secret_hydration = hydrate_config_secrets_read_only(&mut config, secret_store);
    let provider_credential_hydration_status = secret_hydration.provider_credential_status;
    for capability in &secret_hydration.fail_closed_capabilities {
        let owner = match capability.as_str() {
            "provider_credential" => "ProviderCredentialStore",
            "search_provider_credential" => "SearchProviderCredentialStore",
            _ => "CredentialStore",
        };
        persistence.register_unavailable(
            owner,
            "secret_hydration_failed_closed",
            &format!("{capability} is disabled because OS credential hydration did not complete"),
        );
    }
    startup_warnings
        .borrow_mut()
        .extend(secret_hydration.warnings);
    if secret_hydration.rewrite_config_without_plaintext {
        if let Err(error) = config.save(&config_path) {
            persistence.register_unavailable(
                "ConfigStore",
                "config_secret_rewrite_failed",
                &error.to_string(),
            );
            startup_warnings.borrow_mut().push(format!(
                "credential migration succeeded but plaintext config rewrite failed: {error}"
            ));
        }
    }

    let (canonical_task_credential_status, canonical_task_receipt_key_material) =
        inspect_fixed_credential(
            &data_dir,
            CANONICAL_TASK_RECEIPT_KEY_REF,
            &["task_runtime.db"],
            secret_store,
        );
    // Apply system configuration
    openlife_core::ollama::set_ollama_cache_ttl_seconds(config.system.ollama_cache_ttl_seconds);

    let life_model_manager = LifeModelManager::new(data_dir.join("life-model").join("current"));
    // A missing canonical store is a valid empty profile. An existing store
    // must be readable before reviewed LifeModel materialization is admitted.
    match life_model_manager
        .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
    {
        Ok(_) => persistence.register_read_write("LifeModelFileStore"),
        Err(error) => persistence.register_unavailable(
            "LifeModelFileStore",
            "lifemodel_load_failed",
            &error.to_string(),
        ),
    }
    let db_path = data_dir.join("memory.db");
    let memory_store = init_store(
        || init_memory_store(&db_path, &startup_warnings),
        || {
            KnowledgeNoteProjectionStore::open_read_only_existing(&db_path)
                .map_err(|e| e.to_string())
        },
        || KnowledgeNoteProjectionStore::new_in_memory().map_err(|e| e.to_string()),
        "MemoryStore",
        &startup_warnings,
        &persistence,
    );
    let memory_store =
        required_store_or_unavailable(memory_store, "MemoryStore", &startup_warnings, || {
            KnowledgeNoteProjectionStore::unavailable_sentinel().map_err(|error| error.to_string())
        });

    let conversation_db_path = data_dir.join("conversations.db");
    let conversation_store = init_store(
        || ConversationStore::new(&conversation_db_path).map_err(|error| error.to_string()),
        || {
            ConversationStore::open_read_only_existing(&conversation_db_path)
                .map_err(|error| error.to_string())
        },
        || ConversationStore::new_in_memory().map_err(|error| error.to_string()),
        "ConversationStore",
        &startup_warnings,
        &persistence,
    );
    let conversation_store = optional_store(
        conversation_store,
        "ConversationStore",
        &startup_warnings,
    )
    .and_then(|store| match store.interrupt_incomplete_turns() {
        Ok(interrupted) => {
            if interrupted > 0 {
                log::info!("[startup] marked {interrupted} incomplete Chat turns interrupted");
            }
            Some(store)
        }
        Err(error) => {
            persistence.register_unavailable(
                "ConversationStore",
                "incomplete_turn_recovery_failed",
                &error.to_string(),
            );
            startup_warnings.borrow_mut().push(format!(
                "ConversationStore recovery failed; Chat is unavailable: {error}"
            ));
            None
        }
    });

    let feedback_db_path = data_dir.join("feedback.db");
    let feedback_store = init_store(
        || init_feedback_store(&feedback_db_path, &startup_warnings),
        || FeedbackStore::open_read_only_existing(&feedback_db_path).map_err(|e| e.to_string()),
        || FeedbackStore::new_in_memory().map_err(|e| e.to_string()),
        "FeedbackStore",
        &startup_warnings,
        &persistence,
    );
    let feedback_store =
        required_store_or_unavailable(feedback_store, "FeedbackStore", &startup_warnings, || {
            FeedbackStore::unavailable_sentinel().map_err(|error| error.to_string())
        });

    let vector_db_path = data_dir.join("vectors.db");
    let vector_store = init_store(
        || init_vector_store(&vector_db_path, &startup_warnings),
        || VectorStore::open_read_only_existing(&vector_db_path).map_err(|e| e.to_string()),
        || VectorStore::new_in_memory().map_err(|e| e.to_string()),
        "VectorStore",
        &startup_warnings,
        &persistence,
    );
    let vector_store =
        required_store_or_unavailable(vector_store, "VectorStore", &startup_warnings, || {
            VectorStore::unavailable_sentinel().map_err(|error| error.to_string())
        });

    let canonical_task_receipt_key = match canonical_task_receipt_key_material {
        Some(key) => match CanonicalTaskReceiptKey::from_bytes(key) {
            Ok(key) => Some(key),
            Err(error) => {
                startup_warnings.borrow_mut().push(format!(
                    "Canonical Task receipt key is invalid; Work persistence is disabled: {error}"
                ));
                None
            }
        },
        None => {
            startup_warnings.borrow_mut().push(format!(
                "Canonical Task receipt key is unavailable; Work persistence is disabled: {}",
                canonical_task_credential_status.as_str()
            ));
            None
        }
    };
    let canonical_task_runtime_db_path = data_dir.join("task_runtime.db");
    let canonical_task_runtime_store = init_store(
        || {
            let key = canonical_task_receipt_key
                .as_ref()
                .ok_or_else(|| "canonical_task_receipt_key_unavailable".to_string())?;
            openlife_core::task_runtime::CanonicalTaskRuntimeStore::new_with_receipt_key(
                &canonical_task_runtime_db_path,
                key.clone(),
            )
            .map_err(|error| error.to_string())
        },
        || {
            openlife_core::task_runtime::CanonicalTaskRuntimeStore::open_read_only_existing(
                &canonical_task_runtime_db_path,
            )
            .map_err(|error| error.to_string())
        },
        || {
            let key = canonical_task_receipt_key
                .as_ref()
                .ok_or_else(|| "canonical_task_receipt_key_unavailable".to_string())?;
            openlife_core::task_runtime::CanonicalTaskRuntimeStore::new_in_memory_with_receipt_key(
                key.clone(),
            )
            .map_err(|error| error.to_string())
        },
        "CanonicalTaskRuntimeStore",
        &startup_warnings,
        &persistence,
    );
    let canonical_task_runtime_store = optional_store(
        canonical_task_runtime_store,
        "CanonicalTaskRuntimeStore",
        &startup_warnings,
    )
    .and_then(|store| match store.recover_interrupted_general_runs() {
        Ok(interrupted) => {
            if interrupted > 0 {
                log::info!("[startup] marked {interrupted} incomplete Work runs interrupted");
            }
            Some(store)
        }
        Err(error) => {
            persistence.register_unavailable(
                "CanonicalTaskRuntimeStore",
                "incomplete_work_recovery_failed",
                &error.to_string(),
            );
            startup_warnings.borrow_mut().push(format!(
                "Canonical Work recovery failed; Work is unavailable: {error}"
            ));
            None
        }
    });

    let proposals_db_path = data_dir.join("proposals.db");
    let proposal_store = init_store(
        || init_proposal_store(&proposals_db_path, &startup_warnings),
        || ProposalStore::open_read_only_existing(&proposals_db_path).map_err(|e| e.to_string()),
        || ProposalStore::new_in_memory().map_err(|e| e.to_string()),
        "ProposalStore",
        &startup_warnings,
        &persistence,
    );
    let proposal_store = optional_store(proposal_store, "ProposalStore", &startup_warnings);

    let memory_lifecycle_db_path = data_dir.join("memory_lifecycle.db");
    let memory_lifecycle_store = init_store(
        || init_memory_lifecycle_store(&memory_lifecycle_db_path, &startup_warnings),
        || {
            MemoryLifecycleStore::open_read_only_existing(&memory_lifecycle_db_path)
                .map_err(|e| e.to_string())
        },
        || MemoryLifecycleStore::new_in_memory().map_err(|e| e.to_string()),
        "MemoryLifecycleStore",
        &startup_warnings,
        &persistence,
    );
    let memory_lifecycle_store = optional_store(
        memory_lifecycle_store,
        "MemoryLifecycleStore",
        &startup_warnings,
    );

    let life_model_learning_db_path = data_dir.join("life_model_learning.db");
    let life_model_learning_store = init_store(
        || {
            openlife_core::agent::LifeModelLearningStore::new(&life_model_learning_db_path)
                .map_err(|error| error.to_string())
        },
        || {
            openlife_core::agent::LifeModelLearningStore::open_read_only_existing(
                &life_model_learning_db_path,
            )
            .map_err(|error| error.to_string())
        },
        || {
            openlife_core::agent::LifeModelLearningStore::new_in_memory()
                .map_err(|error| error.to_string())
        },
        "LifeModelLearningStore",
        &startup_warnings,
        &persistence,
    );
    let life_model_learning_store = optional_store(
        life_model_learning_store,
        "LifeModelLearningStore",
        &startup_warnings,
    );

    let scheduler = InferenceScheduler::new(
        config.local_model.clone(),
        config.prefer_local_model,
        config.llm.provider.clone(),
        config.llm.openai_base.clone(),
        config.llm.openai_key.clone(),
        config.llm.chat_model.clone(),
        config.llm.embedding_model.clone(),
        config.llm.embedding_enabled,
    )
    .with_provider_credential_version(config.llm.credential_version);
    let privacy_policy_path = privacy_policy_path();
    let privacy_policy = match std::fs::read_to_string(&privacy_policy_path) {
        Ok(text) => match openlife_core::privacy::PrivacyPolicy::from_yaml(&text) {
            Ok(policy) => {
                persistence.register_read_write("PrivacyPolicyStore");
                policy
            }
            Err(error) => {
                persistence.register_unavailable(
                    "PrivacyPolicyStore",
                    "privacy_policy_parse_failed",
                    &error.to_string(),
                );
                openlife_core::privacy::PrivacyPolicy::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            persistence.register_read_write("PrivacyPolicyStore");
            openlife_core::privacy::PrivacyPolicy::default()
        }
        Err(error) => {
            persistence.register_unavailable(
                "PrivacyPolicyStore",
                "privacy_policy_read_failed",
                &error.to_string(),
            );
            openlife_core::privacy::PrivacyPolicy::default()
        }
    };
    let privacy_engine = PrivacyEngine::with_policy(privacy_policy);
    let version_manager = VersionManager::new(data_dir.join("life-model").join("versions"));
    let audit_keyring_path = data_dir.join("mcp_audit_keys.json");
    let mcp_audit_db_path = data_dir.join("mcp_audit.db");
    let (audit_key_hydration, mcp_audit_credential_status, mcp_warning) =
        match load_mcp_audit_keyring_from_path(&audit_keyring_path) {
            McpAuditKeyringLoad::Absent => {
                match McpAuditStore::inspect_existing_database(&mcp_audit_db_path) {
                    Ok(inspection) if inspection.is_empty_or_absent() => (
                        None,
                        CredentialBootstrapStatus::InitializationRequired,
                        None,
                    ),
                    Ok(inspection) => (
                        None,
                        CredentialBootstrapStatus::MissingExistingData,
                        Some(format!(
                            "MCP audit keyring is missing while the canonical audit database contains {} rows",
                            inspection.row_count
                        )),
                    ),
                    Err(error) => (
                        None,
                        CredentialBootstrapStatus::Unknown,
                        Some(format!(
                            "missing MCP audit keyring beside an untrusted audit database: {error}"
                        )),
                    ),
                }
            }
            McpAuditKeyringLoad::Present(configs) => {
                match inspect_existing_mcp_audit_keys(configs, secret_store) {
                    McpAuditKeyHydrationInspection::Available(hydration) => {
                        match McpAuditStore::preflight_existing_database_key_materials(
                            &mcp_audit_db_path,
                            &hydration.materials,
                        ) {
                            Ok(_) => {
                                let latest_is_keychain = hydration.configs.last().is_some_and(
                                    |config| {
                                        config.mode
                                            == openlife_core::mcp_audit::KeyMode::Keychain
                                    },
                                );
                                let has_keychain_epoch = hydration.configs.iter().any(|config| {
                                    config.mode == openlife_core::mcp_audit::KeyMode::Keychain
                                });
                                if latest_is_keychain {
                                    (
                                        Some(hydration),
                                        CredentialBootstrapStatus::Available,
                                        None,
                                    )
                                } else if !has_keychain_epoch {
                                    (
                                        Some(hydration),
                                        CredentialBootstrapStatus::InitializationRequired,
                                        None,
                                    )
                                } else {
                                    (
                                        Some(hydration),
                                        CredentialBootstrapStatus::Invalid,
                                        Some(
                                            "MCP audit keyring has a legacy epoch after a Keychain write epoch; initialization is blocked"
                                                .into(),
                                        ),
                                    )
                                }
                            }
                            Err(error)
                                if openlife_core::mcp_audit::is_payload_integrity_failure(
                                    &error,
                                ) =>
                            {
                                (
                                    Some(hydration),
                                    CredentialBootstrapStatus::Invalid,
                                    Some(format!(
                                        "audit payload integrity is invalid; MCP audit effects remain disabled: {error}"
                                    )),
                                )
                            }
                            Err(error) => (
                                Some(hydration),
                                CredentialBootstrapStatus::Unknown,
                                Some(format!(
                                    "MCP audit database preflight is unavailable; effects remain disabled: {error}"
                                )),
                            ),
                        }
                    }
                    McpAuditKeyHydrationInspection::MissingExistingData => (
                        None,
                        CredentialBootstrapStatus::MissingExistingData,
                        Some("MCP audit keychain reference has no credential".into()),
                    ),
                    McpAuditKeyHydrationInspection::Invalid => (
                        None,
                        CredentialBootstrapStatus::Invalid,
                        Some("MCP audit key material is invalid".into()),
                    ),
                    McpAuditKeyHydrationInspection::Unavailable => (
                        None,
                        CredentialBootstrapStatus::Unavailable,
                        Some("MCP audit key material is unavailable".into()),
                    ),
                }
            }
            McpAuditKeyringLoad::PresentInvalid { error } => (
                None,
                CredentialBootstrapStatus::Invalid,
                Some(format!("MCP audit keyring is present but invalid: {error}")),
            ),
            McpAuditKeyringLoad::Unreadable { error } => (
                None,
                CredentialBootstrapStatus::Unavailable,
                Some(format!("MCP audit keyring is unreadable: {error}")),
            ),
        };
    let mcp_reference_available = audit_key_hydration.is_some();
    if mcp_reference_available {
        persistence.register_read_write("McpAuditKeyReferenceStore");
    } else {
        persistence.register_unavailable(
            "McpAuditKeyReferenceStore",
            "mcp_audit_key_hydration_failed",
            mcp_audit_credential_status.as_str(),
        );
    }
    if mcp_audit_credential_status != CredentialBootstrapStatus::Available {
        let warning = mcp_warning.unwrap_or_else(|| {
            "MCP audit credential initialization is required; audit effects remain disabled".into()
        });
        startup_warnings.borrow_mut().push(warning);
    }
    let audit_materials = audit_key_hydration
        .map(|hydration| hydration.materials)
        .unwrap_or_default();
    let mcp_audit_store = if mcp_audit_credential_status == CredentialBootstrapStatus::Available {
        let store = init_store(
            || {
                McpAuditStore::with_key_materials(&mcp_audit_db_path, audit_materials.clone())
                    .map_err(|error| error.to_string())
            },
            || {
                McpAuditStore::open_read_only_existing_with_key_materials(
                    &mcp_audit_db_path,
                    audit_materials.clone(),
                )
                .map_err(|error| error.to_string())
            },
            || {
                McpAuditStore::with_key_materials(
                    recovery_db_path("mcp_audit.db"),
                    audit_materials.clone(),
                )
                .map_err(|error| error.to_string())
            },
            "McpAuditStore",
            &startup_warnings,
            &persistence,
        );
        required_store_or_unavailable(store, "McpAuditStore", &startup_warnings, || {
            Ok(McpAuditStore::unavailable_sentinel(
                "canonical and read-only audit store open failed",
            ))
        })
    } else {
        persistence.register_unavailable(
            "McpAuditStore",
            "mcp_audit_credential_unavailable",
            mcp_audit_credential_status.as_str(),
        );
        McpAuditStore::unavailable_sentinel(
            "credential bootstrap did not prove an available MCP audit write epoch",
        )
    };

    #[cfg(feature = "dev-extensions")]
    let mcp_registry = {
        let mut registry = McpRegistry::new();
        registry
    };
    #[cfg(not(feature = "dev-extensions"))]
    let mcp_registry = McpRegistry::new_release_product();
    let tool_permission_store = init_store(
        || {
            openlife_core::tool_permissions::ToolPermissionStore::new(
                data_dir.join("tool_permissions.db"),
            )
            .map_err(|e| e.to_string())
        },
        || {
            openlife_core::tool_permissions::ToolPermissionStore::open_read_only_existing(
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
        &persistence,
    );
    let tool_permission_store = required_store_or_unavailable(
        tool_permission_store,
        "ToolPermissionStore",
        &startup_warnings,
        || {
            openlife_core::tool_permissions::ToolPermissionStore::unavailable_sentinel()
                .map_err(|error| error.to_string())
        },
    );
    let skill_registry = openlife_core::skills::SkillRegistry::built_in();

    let resource_runtime = {
        let store_path = data_dir.join("resources.db");
        let runtime = openlife_core::resource::ResourceStore::new(&store_path).and_then(|store| {
            let parser =
                openlife_core::resource_gateway::ResourceParserProcess::for_current_executable()?;
            Ok(crate::resource_commands::ResourceRuntime::new(
                openlife_core::resource_gateway::ResourceGateway::new(store, parser),
            ))
        });
        match runtime {
            Ok(runtime) => {
                persistence.register_read_write("ResourceStore");
                Some(Arc::new(runtime))
            }
            Err(error) => {
                persistence.register_unavailable(
                    "ResourceStore",
                    "resource_runtime_initialization_failed",
                    &error.to_string(),
                );
                startup_warnings
                    .borrow_mut()
                    .push(format!("resources.db 初始化失败: {error}"));
                None
            }
        }
    };

    let provider_credential_status = match provider_credential_hydration_status {
        ProviderCredentialHydrationStatus::NotReferenced
        | ProviderCredentialHydrationStatus::Missing => {
            CredentialBootstrapStatus::MissingExistingData
        }
        ProviderCredentialHydrationStatus::Available => CredentialBootstrapStatus::Available,
        ProviderCredentialHydrationStatus::Invalid => CredentialBootstrapStatus::Invalid,
        ProviderCredentialHydrationStatus::Unavailable => CredentialBootstrapStatus::Unavailable,
    };
    let credential_bootstrap_snapshot = CredentialBootstrapSnapshot::from_statuses([
        canonical_task_credential_status,
        mcp_audit_credential_status,
    ])
    .with_provider_status(provider_credential_status);
    let app_state = Arc::new(AppState {
        persistence_coordinator: Arc::clone(&persistence),
        config: Arc::new(Mutex::new(config)),
        life_model_manager: Arc::new(Mutex::new(life_model_manager)),
        life_model_write_coordinator: Arc::new(Mutex::new(())),
        memory_store: Arc::new(Mutex::new(memory_store)),
        conversation_store: conversation_store.map(|store| Arc::new(Mutex::new(store))),
        mcp_registry: Arc::new(Mutex::new(mcp_registry)),
        scheduler: Arc::new(Mutex::new(scheduler)),
        privacy_engine: Arc::new(Mutex::new(privacy_engine)),
        version_manager: Arc::new(Mutex::new(version_manager)),
        feedback_store: Arc::new(Mutex::new(feedback_store)),
        vector_store: Arc::new(Mutex::new(vector_store)),
        last_snapshot_date: Arc::new(Mutex::new(None)),
        mcp_audit_store: Arc::new(Mutex::new(mcp_audit_store)),
        canonical_task_runtime_store: canonical_task_runtime_store
            .map(|store| Arc::new(Mutex::new(store))),
        proposal_store: proposal_store.map(|store| Arc::new(Mutex::new(store))),
        memory_lifecycle_store: memory_lifecycle_store.map(|store| Arc::new(Mutex::new(store))),
        life_model_learning_store: life_model_learning_store
            .map(|store| Arc::new(Mutex::new(store))),
        main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
        tool_permission_store: Arc::new(Mutex::new(tool_permission_store)),
        skill_registry: Arc::new(Mutex::new(skill_registry)),
        startup_warnings: startup_warnings.into_inner(),
        credential_bootstrap_snapshot,
        #[cfg(test)]
        web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
        #[cfg(test)]
        work_initial_decision_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
        #[cfg(test)]
        work_agent_step_fixture_outputs: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        #[cfg(test)]
        work_semantic_verification_fixture_outputs: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        resource_runtime,
    });

    BootstrapResult { state: app_state }
}
