//! Application bootstrap: store initialization and AppState assembly.
//! Extracted from lib.rs to keep the main entry point focused on Tauri lifecycle.

use crate::credential_bootstrap::initialize_fresh_profile_credentials;
use crate::persistence_coordinator::PersistenceCoordinator;
use crate::secret_store::{
    hydrate_config_secrets_read_only, inspect_and_hydrate_integrity_key,
    inspect_existing_mcp_audit_keys, selected_secret_store_classification, IntegrityKeyHydration,
    McpAuditKeyHydrationInspection, ProfileSecretStore, ProviderCredentialHydrationStatus,
    SecretReader, StartupProfileSecretStore, CANONICAL_TASK_RECEIPT_KEY_REF,
    TASK_STORE_AUTHORITY_KEY_REF,
};
use crate::state::{AppState, CredentialBootstrapSnapshot, CredentialBootstrapStatus};
use crate::storage::{load_mcp_audit_keyring_from_path, privacy_policy_path, McpAuditKeyringLoad};
use openlife_core::agent::{
    AgentProposal, CanonicalTaskReceiptKey, DurableWriteRequest, DurableWriteSource,
    DurableWriteSubject, MemoryLifecycleStore, ProposalSource, ProposalStore, ProposalType,
    ReviewWorkflow, RiskLevel,
};
use openlife_core::config::AppConfig;
use openlife_core::conversation::ConversationStore;
use openlife_core::feedback::FeedbackStore;
use openlife_core::life_model::LifeModelManager;
use openlife_core::mcp::McpRegistry;
use openlife_core::mcp_audit::McpAuditStore;
use openlife_core::memory::MemoryStore;
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
    let mut lifemodel_drained = false;
    for _ in 0..STARTUP_CANONICAL_OUTBOX_PASSES {
        let report =
            crate::life_model_write_gateway::reconcile_startup_lifemodel_file_mutations_with_state(
                state,
            )
            .await?;
        if report.degraded > 0 {
            return Err(format!(
                "LifeModel file projection reconciliation degraded: {} delivery attempts",
                report.degraded
            ));
        }
        if !report.backlog_may_remain {
            lifemodel_drained = true;
            break;
        }
    }
    if !lifemodel_drained {
        return Err("LifeModel file projection backlog exceeded startup bound".into());
    }
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
) -> Result<MemoryStore, String> {
    match MemoryStore::new(db_path) {
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

fn build_legacy_scheduled_task_review_proposal(
    candidate: &openlife_core::tasks::LegacyScheduledTaskReviewCandidate,
) -> Result<(AgentProposal, String), String> {
    let identity_digest =
        openlife_core::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "schema": "openlife.legacyScheduledReviewIdentity.v1",
            "sourceDigest": candidate.source_digest.clone(),
            "sourceOrdinal": candidate.source_ordinal,
            "itemDigest": candidate.item_digest.clone(),
        }))
        .1;
    let identity_suffix = identity_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| "legacy scheduled review identity digest is invalid".to_string())?;
    let proposal_id = format!("legacy-scheduled-review-{identity_suffix}");
    let source_detail = format!(
        "legacy_scheduled_review:{}:{}:{}",
        candidate.source_digest, candidate.source_ordinal, candidate.item_digest
    );
    let source_run_id_digest = candidate
        .source_run_id
        .as_deref()
        .map(|value| openlife_core::agent::metadata_safe::metadata_safe_text_digest(value).1);
    let source_proposal_id_digest = candidate
        .source_proposal_id
        .as_deref()
        .map(|value| openlife_core::agent::metadata_safe::metadata_safe_text_digest(value).1);
    let after = serde_json::json!({
        "title": candidate.title.clone(),
        "description": candidate.description.clone(),
        "due_date": candidate.due_at.clone(),
        "scheduled_at": candidate.due_at.clone(),
        "priority": candidate.priority.clone(),
        "tool": candidate.action_type.clone(),
        "legacy_migration": {
            "source_digest": candidate.source_digest.clone(),
            "source_ordinal": candidate.source_ordinal,
            "item_digest": candidate.item_digest.clone(),
            "effect_state": "review_required",
            "source_run_id_digest": source_run_id_digest,
            "source_proposal_id_digest": source_proposal_id_digest,
        }
    });
    let mut proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            &format!("tasks.legacy_review.{}", &identity_suffix[..16]),
            after,
            "A provably not-yet-due legacy scheduled task requires fresh Review Center approval before it can enter the canonical TaskStore.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
    proposal.id = proposal_id.clone();
    proposal.source_detail = Some(source_detail);
    proposal.created_at = chrono::DateTime::parse_from_rfc3339(&candidate.review_created_at)
        .map_err(|_| "legacy scheduled review creation snapshot is invalid".to_string())?
        .with_timezone(&chrono::Utc);
    proposal.expires_at = Some(
        chrono::DateTime::parse_from_rfc3339(&candidate.due_at)
            .map_err(|_| "legacy scheduled review expiry snapshot is invalid".to_string())?
            .with_timezone(&chrono::Utc),
    );
    if proposal
        .expires_at
        .is_some_and(|expiry| expiry <= proposal.created_at)
    {
        return Err("legacy scheduled review snapshot is not future-bounded".into());
    }
    Ok((proposal, proposal_id))
}

fn stage_legacy_scheduled_task_review_proposals(
    task_store: &openlife_core::tasks::TaskStore,
    proposal_store: &ProposalStore,
    evidence_directory: &Path,
) -> Result<usize, String> {
    let candidates = task_store
        .pending_legacy_review_candidates(evidence_directory)
        .map_err(|error| error.to_string())?;
    let mut staged = 0;
    for candidate in candidates {
        let (proposal, proposal_id) = build_legacy_scheduled_task_review_proposal(&candidate)?;
        if let Some(existing) = proposal_store
            .get_proposal(&proposal_id)
            .map_err(|error| error.to_string())?
        {
            if existing.source_detail != proposal.source_detail
                || existing.run_id != proposal.run_id
                || existing.proposal_type != proposal.proposal_type
                || existing.source != proposal.source
                || existing.affected_path != proposal.affected_path
                || existing.base_hash != proposal.base_hash
                || existing.before != proposal.before
                || existing.after != proposal.after
                || existing.reason != proposal.reason
                || existing.confidence.to_bits() != proposal.confidence.to_bits()
                || existing.risk_level != proposal.risk_level
                || existing.created_at != proposal.created_at
                || existing.expires_at != proposal.expires_at
                || existing.resolved_at.is_some()
                || !matches!(
                    existing.status,
                    openlife_core::agent::ProposalStatus::Pending
                        | openlife_core::agent::ProposalStatus::Postponed
                        | openlife_core::agent::ProposalStatus::Edited
                )
                || existing.is_expired()
            {
                return Err(
                    "legacy scheduled review proposal id resolves to a non-exact snapshot".into(),
                );
            }
            if !task_store
                .mark_legacy_review_proposal_staged(&candidate, &proposal_id)
                .map_err(|error| error.to_string())?
            {
                return Err(
                    "legacy scheduled review migration journal rejected the exact proposal".into(),
                );
            }
            staged += 1;
            continue;
        }
        let outcome = ReviewWorkflow::new(proposal_store)
            .submit(
                DurableWriteRequest::from_agent_proposal(
                    DurableWriteSource::ManualOverride,
                    DurableWriteSubject::Calendar,
                    proposal,
                    "A legacy future scheduled task is pending fresh Review Center approval; it has not been scheduled or executed.",
                )
                .with_existing_proposal_id(Some(proposal_id.clone()))
                .with_idempotency_key(format!(
                    "legacy_scheduled_review:{}:{}",
                    candidate.source_digest, candidate.source_ordinal
                )),
            )
            .map_err(|error| error.to_string())?;
        if outcome.proposal_id() != proposal_id {
            return Err("legacy scheduled review did not preserve its deterministic id".into());
        }
        if !task_store
            .mark_legacy_review_proposal_staged(&candidate, &proposal_id)
            .map_err(|error| error.to_string())?
        {
            return Err("legacy scheduled review migration journal rejected the proposal".into());
        }
        staged += 1;
    }
    Ok(staged)
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
    let (task_store_credential_status, task_store_authority_key_material) =
        inspect_fixed_credential(
            &data_dir,
            TASK_STORE_AUTHORITY_KEY_REF,
            &["tasks.db"],
            secret_store,
        );

    // Apply system configuration
    openlife_core::ollama::set_ollama_cache_ttl_seconds(config.system.ollama_cache_ttl_seconds);

    let life_model_manager = LifeModelManager::new(data_dir.join("life-model").join("current"));
    // Bootstrap must remain read-only for an absent legacy model. Calling
    // `load()` here used to manufacture a default-filled YAML document on the
    // first launch, which then looked like user-authored migration input to the
    // v2 product path. Canonical creation is owned by an explicitly reviewed
    // v2 proposal, never by application startup.
    match life_model_manager.load_existing() {
        Ok(_) => persistence.register_read_write("LifeModelFileStore"),
        Err(error) => persistence.register_unavailable(
            "LifeModelFileStore",
            "lifemodel_load_failed",
            &error.to_string(),
        ),
    }
    match openlife_core::persistence_outbox::FileMutationJournal::new(
        life_model_manager.mutation_journal_path(),
    ) {
        Ok(_) => persistence.register_read_write("LifeModelFileJournal"),
        Err(error) => persistence.register_unavailable(
            "LifeModelFileJournal",
            "lifemodel_journal_open_failed",
            &error.to_string(),
        ),
    }
    let db_path = data_dir.join("memory.db");
    let memory_store = init_store(
        || init_memory_store(&db_path, &startup_warnings),
        || MemoryStore::open_read_only_existing(&db_path).map_err(|e| e.to_string()),
        || MemoryStore::new_in_memory().map_err(|e| e.to_string()),
        "MemoryStore",
        &startup_warnings,
        &persistence,
    );
    let memory_store =
        required_store_or_unavailable(memory_store, "MemoryStore", &startup_warnings, || {
            MemoryStore::unavailable_sentinel().map_err(|error| error.to_string())
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

    let task_store_db_path = data_dir.join("tasks.db");
    let task_store_authority_key = match task_store_authority_key_material {
        Some(key) => openlife_core::tasks::TaskStoreAuthorityKey::from_key_material(&key)
            .map(Some)
            .unwrap_or_else(|error| {
                startup_warnings.borrow_mut().push(format!(
                    "TaskStore authority key is invalid; scheduled execution is disabled: {error}"
                ));
                None
            }),
        None => {
            startup_warnings.borrow_mut().push(format!(
                "TaskStore authority key is unavailable; scheduled execution is disabled: {}",
                task_store_credential_status.as_str()
            ));
            None
        }
    };
    let patches_db_path = data_dir.join("patches.db");
    let patch_store = init_store(
        || {
            openlife_core::life_model::patch_store::PatchStore::new(&patches_db_path)
                .map_err(|e| e.to_string())
        },
        || {
            openlife_core::life_model::patch_store::PatchStore::open_read_only_existing(
                &patches_db_path,
            )
            .map_err(|e| e.to_string())
        },
        || {
            openlife_core::life_model::patch_store::PatchStore::new_in_memory()
                .map_err(|e| e.to_string())
        },
        "PatchStore",
        &startup_warnings,
        &persistence,
    );
    let patch_store = optional_store(patch_store, "PatchStore", &startup_warnings);

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
    let legacy_scheduled_task_report = std::cell::RefCell::new(None);
    let legacy_scheduled_task_path = data_dir.join("scheduled_tasks.json");
    let scheduled_task_store = init_store(
        || {
            let authority_key = task_store_authority_key
                .as_ref()
                .ok_or_else(|| "task_store_authority_key_unavailable".to_string())?;
            let store = openlife_core::tasks::TaskStore::new_with_authority_key(
                &task_store_db_path,
                authority_key,
            )
            .map_err(|e| e.to_string())?;
            let report = store
                .migrate_legacy_json_if_present(&legacy_scheduled_task_path)
                .map_err(|e| e.to_string())?;
            *legacy_scheduled_task_report.borrow_mut() = Some(report);
            Ok(store)
        },
        || {
            let authority_key = task_store_authority_key
                .as_ref()
                .ok_or_else(|| "task_store_authority_key_unavailable".to_string())?;
            openlife_core::tasks::TaskStore::open_read_only_existing_with_authority_key(
                &task_store_db_path,
                authority_key,
            )
            .map_err(|e| e.to_string())
        },
        || openlife_core::tasks::TaskStore::new_in_memory().map_err(|e| e.to_string()),
        "TaskStore",
        &startup_warnings,
        &persistence,
    );
    if legacy_scheduled_task_path.exists() {
        persistence.register_unavailable(
            "LegacyScheduledTaskOwner",
            "legacy_scheduled_task_quarantine_incomplete",
            "scheduled_tasks.json remains active because its metadata quarantine or atomic evidence retirement did not complete",
        );
        startup_warnings.borrow_mut().push(
            "Legacy scheduled-task state is unresolved/unknown; all effects remain disabled until scheduled_tasks.json is quarantined."
                .into(),
        );
    } else if let Some(report) = legacy_scheduled_task_report.into_inner() {
        if report.quarantined_count > 0 {
            log::warn!(
                "[startup] quarantined legacy scheduled-task source digest={} items={} unknown={}",
                report.source_digest.as_deref().unwrap_or("unknown"),
                report.item_count,
                report.quarantined_count,
            );
            startup_warnings.borrow_mut().push(format!(
                "Quarantined {} legacy scheduled-task record(s) as unknown; no legacy task was auto-executed.",
                report.quarantined_count
            ));
        }
        if report.historical_count > 0 {
            startup_warnings.borrow_mut().push(format!(
                "Imported {} legacy scheduled-task terminal label(s) as metadata-only history; they are not canonical completion receipts.",
                report.historical_count
            ));
        }
        if report.review_required_count > 0 {
            startup_warnings.borrow_mut().push(format!(
                "Identified {} future legacy scheduled task(s) requiring fresh Review Center review; none is executable before approval.",
                report.review_required_count
            ));
        }
    }
    let scheduled_task_store =
        required_store_or_unavailable(scheduled_task_store, "TaskStore", &startup_warnings, || {
            openlife_core::tasks::TaskStore::unavailable_sentinel()
                .map_err(|error| error.to_string())
        });
    let pending_reviewed_cloud_tasks = scheduled_task_store
        .list_tasks(Some("pending"))
        .unwrap_or_default()
        .into_iter()
        .filter(|task| {
            task.provider_grant.data_route == openlife_core::llm::ProviderDataRoute::PolicyAllowed
        })
        .collect::<Vec<_>>();
    for task in pending_reviewed_cloud_tasks {
        let restored = proposal_store
            .as_ref()
            .ok_or_else(|| "ProposalStore is unavailable".to_string())
            .and_then(|store| {
                let proof = ReviewWorkflow::new(store)
                    .materialized_acceptance_snapshot(&task.id)
                    .map_err(|error| error.to_string())?;
                scheduled_task_store
                    .restore_reviewed_cloud_authority(&proof)
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = restored {
            match scheduled_task_store.quarantine_unproven_reviewed_cloud_task(&task.id) {
                Ok(true) => log::warn!(
                    "[startup] scheduled cloud task {} lacks canonical ReviewWorkflow authority and now requires fresh review: {}",
                    task.id,
                    error
                ),
                Ok(false) | Err(_) => {
                    persistence.register_unavailable(
                        "ScheduledCloudAuthority",
                        "scheduled_cloud_authority_quarantine_failed",
                        &error,
                    );
                    startup_warnings.borrow_mut().push(format!(
                        "Scheduled cloud task {} could not prove ReviewWorkflow authority or enter fresh-review quarantine; all effects remain disabled: {}",
                        task.id, error
                    ));
                }
            }
        }
    }
    match proposal_store.as_ref() {
        Some(store) => match stage_legacy_scheduled_task_review_proposals(
            &scheduled_task_store,
            store,
            &data_dir,
        ) {
            Ok(staged) if staged > 0 => log::warn!(
                "[startup] staged {} legacy future scheduled task(s) for fresh review",
                staged
            ),
            Ok(_) => {}
            Err(error) => {
                persistence.register_unavailable(
                    "LegacyScheduledTaskReviewMigration",
                    "legacy_scheduled_review_staging_failed",
                    &error,
                );
                startup_warnings.borrow_mut().push(format!(
                    "Legacy future scheduled-task review staging failed; all effects remain disabled: {error}"
                ));
            }
        },
        None => match scheduled_task_store.pending_legacy_review_candidates(&data_dir) {
            Ok(candidates) if candidates.is_empty() => {}
            Ok(_) | Err(_) => {
                persistence.register_unavailable(
                    "LegacyScheduledTaskReviewMigration",
                    "proposal_store_unavailable_for_legacy_review",
                    "future legacy scheduled tasks require ReviewWorkflow but ProposalStore is unavailable",
                );
                startup_warnings.borrow_mut().push(
                    "Legacy future scheduled tasks await ReviewWorkflow, but ProposalStore is unavailable; all effects remain disabled."
                        .into(),
                );
            }
        },
    }

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

    let state_store = {
        let store_path = data_dir.join("state.db");
        match openlife_core::state_store::StateStore::new(&store_path) {
            Ok(store) => {
                persistence.register_read_write("StateStore");
                if persistence.bootstrap_mutations_safe() {
                    let daily_task_cutover_result = match life_model_manager.load_existing() {
                        Ok(Some(model)) => crate::state_projection::reconcile_and_import_legacy_yaml_daily_tasks(
                                &store,
                                &model,
                                chrono::Utc::now(),
                            )
                            .map(|_| ()),
                        Ok(None) => Ok(()),
                        Err(error) => Err(format!(
                            "LifeModel could not be loaded for legacy daily-task StateStore cutover: {error}"
                        )),
                    };
                    if let Err(error) = daily_task_cutover_result {
                        // Shipped product reads require the import receipt and
                        // fail closed. Never merge a partial StateStore view
                        // with the legacy YAML source after a blocked cutover.
                        startup_warnings.borrow_mut().push(format!(
                            "legacy daily-task StateStore cutover remains blocked: {error}"
                        ));
                    }
                    let history_cutover_result = memory_store
                        .list_legacy_state_history_migration_source()
                        .map_err(|error| {
                            format!(
                                "MemoryStore state history could not be loaded for StateStore cutover: {error}"
                            )
                        })
                        .and_then(|snapshot| {
                            crate::state_projection::reconcile_legacy_memory_state_history_shadow(
                                &store,
                                &snapshot,
                                chrono::Utc::now(),
                            )?;
                            store
                                .import_legacy_state_history_shadow(chrono::Utc::now())
                                .map_err(|error| {
                                    format!(
                                        "legacy state-history canonical import failed: {error}"
                                    )
                                })
                        });
                    if let Err(error) = history_cutover_result {
                        // Product reads fail closed on the absent import
                        // receipt. MemoryStore remains migration evidence, not
                        // a hidden product fallback.
                        startup_warnings.borrow_mut().push(format!(
                            "legacy state-history StateStore cutover remains blocked: {error}"
                        ));
                    }
                } else {
                    startup_warnings.borrow_mut().push(
                        "legacy daily-task and state-history StateStore shadow reconciliation skipped because canonical bootstrap mutations are unsafe"
                            .into(),
                    );
                }
                Some(Arc::new(store))
            }
            Err(error) => {
                persistence.register_unavailable(
                    "StateStore",
                    "state_store_initialization_failed",
                    &error.to_string(),
                );
                startup_warnings.borrow_mut().push(format!(
                    "state.db 初始化失败；transient-state 写入已禁用且不会降级到临时存储：{error}"
                ));
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
        task_store_credential_status,
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
        patch_store: patch_store.map(|store| Arc::new(Mutex::new(store))),
        tool_permission_store: Arc::new(Mutex::new(tool_permission_store)),
        skill_registry: Arc::new(Mutex::new(skill_registry)),
        startup_warnings: startup_warnings.into_inner(),
        credential_bootstrap_snapshot,
        scheduled_task_store: Arc::new(scheduled_task_store),
        #[cfg(test)]
        web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
        resource_runtime,
        state_store,
    });

    BootstrapResult { state: app_state }
}
