use crate::errors::AppError;
use base64::{engine::general_purpose, Engine as _};
use once_cell::sync::Lazy as LazyLock;
use openlife_core::config::AppConfig;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::{
    chat_completions_url, default_base_for_provider, effective_api_key_for_endpoint,
    provider_label, ProviderInvocationReceipt, ProviderInvocationStatus,
};
use openlife_core::mcp_audit::{AuditExport, McpAuditStore};
use openlife_core::network_client::resolve_network_policy_decision;
use openlife_core::privacy::PrivacyPolicy;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
#[cfg(test)]
use std::collections::HashMap;
use std::fs::File;
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex as StdMutex;
use tauri::State;
use uuid::{Uuid, Version};

use crate::danger_action_confirmation::{
    issue_danger_action_challenge, require_native_danger_action_confirmation,
    NativeDangerActionRequest,
};
use crate::life_model_materializer_guard::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::persistence_coordinator::{
    GovernedDataImportRecoveryAdmission, GovernedDataImportRecoveryOwner,
};
use crate::provider_network_consent::{
    authorize_explicit_provider_probe, ExplicitProviderProbeAuthorization,
};
use crate::secret_store::{
    create_mcp_audit_key_material, hydrate_or_create_canonical_store_integrity_key,
    hydrate_or_create_integrity_key, inspect_existing_mcp_audit_keys, inspect_integrity_key_access,
    stage_config_secrets, IntegrityKeyInspection, KeyringSecretStore,
    McpAuditKeyHydrationInspection, SecretStore, ACTION_QUEUE_AUTHORITY_KEY_REF,
    AGENT_RUN_RECEIPT_KEY_REF, MAIN_CHAT_EVENT_INTEGRITY_KEY_REF, MCP_AUDIT_KEY_REF_PREFIX,
    TASK_STORE_AUTHORITY_KEY_REF,
};
use crate::state::{CredentialBootstrapSnapshot, CredentialBootstrapStatus};
use crate::storage::{
    app_data_dir, load_mcp_audit_keyring_from_path, mcp_audit_keyring_bytes,
    mcp_audit_keyring_path, privacy_policy_path, save_mcp_audit_keyring_to_path,
    save_privacy_policy_to_path, McpAuditKeyringBytes, McpAuditKeyringLoad,
};
use crate::AppState;
use crate::{life_model_write_gateway, memory_gateway};
use openlife_core::persistence_outbox::{
    metadata_digest, GovernedDataImportJournal, GovernedDataImportOwnerObservation,
    GovernedDataImportOwnerPlan, GovernedDataImportOwnerReceipt, GovernedDataImportOwnerResolution,
    GovernedDataImportOwnerStatus, GovernedDataImportOwnerUpdate, GovernedDataImportPrepare,
    GovernedDataImportReceipt, GovernedDataImportResolutionClassification, GovernedDataImportStage,
    ProjectionDeliveryState, GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON,
};

static GOVERNED_DATA_IMPORT_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));
#[cfg(test)]
struct GovernedImportTerminalObservationBarrier {
    observed_first_owner: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}
#[cfg(test)]
static GOVERNED_IMPORT_TERMINAL_OBSERVATION_BARRIERS: LazyLock<
    StdMutex<HashMap<String, GovernedImportTerminalObservationBarrier>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));
const MAX_GOVERNED_IMPORT_JSON_BYTES: usize = 64 * 1024 * 1024;
const MAX_GOVERNED_IMPORT_STRING_BYTES: usize = 1024 * 1024;
const MAX_GOVERNED_IMPORT_CONTAINER_ITEMS: usize = 100_000;
const MAX_GOVERNED_IMPORT_MESSAGES: usize = 50_000;
const MAX_GOVERNED_IMPORT_VECTORS: usize = 50_000;
const MAX_GOVERNED_IMPORT_STATE_TASKS: usize = 512;
const MAX_GOVERNED_IMPORT_JSON_DEPTH: usize = 64;
pub(crate) static CONFIG_WRITE_COORDINATOR: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));
static CREDENTIAL_RECOVERY_ACTIVE: AtomicBool = AtomicBool::new(false);

struct CredentialRecoveryActivityGuard;

impl Drop for CredentialRecoveryActivityGuard {
    fn drop(&mut self) {
        CREDENTIAL_RECOVERY_ACTIVE.store(false, Ordering::Release);
    }
}

struct CredentialRecoveryProcessLock(File);

impl CredentialRecoveryProcessLock {
    fn acquire(data_dir: &Path) -> Result<Self, AppError> {
        let file = File::open(data_dir).map_err(|error| {
            AppError::permission(format!(
                "credential initialization cannot open its process-lock owner: {error}"
            ))
        })?;
        #[cfg(unix)]
        {
            // SAFETY: `file` owns this live descriptor for the full lifetime of
            // the guard. The lock is advisory and scoped to the data directory,
            // so parallel OpenLife processes serialize the entire
            // reinspection-write-compensation transaction without creating a
            // second durable authority file.
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result != 0 {
                return Err(AppError::permission(format!(
                    "credential initialization is already active in another OpenLife process: {}",
                    std::io::Error::last_os_error()
                )));
            }
            Ok(Self(file))
        }
        #[cfg(not(unix))]
        {
            let _ = file;
            Err(AppError::permission(
                "credential initialization cross-process locking is unavailable on this platform",
            ))
        }
    }
}

impl Drop for CredentialRecoveryProcessLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // SAFETY: the guard still owns the descriptor and drops it
            // immediately after releasing the advisory lock.
            unsafe {
                libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRecoveryItem {
    pub purpose: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRecoveryReport {
    pub items: Vec<CredentialRecoveryItem>,
    pub initialization_completed_for_restart: bool,
    pub restart_required: bool,
    pub cleanup_status: String,
    pub blocked_reason: Option<String>,
    pub bootstrap_snapshot_digest: String,
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

fn inspect_fixed_credential_status(
    data_dir: &Path,
    store: &dyn SecretStore,
    secret_ref: &'static str,
    protected_files: &[&str],
) -> CredentialBootstrapStatus {
    match inspect_integrity_key_access(secret_ref, store) {
        IntegrityKeyInspection::Available => CredentialBootstrapStatus::Available,
        IntegrityKeyInspection::Invalid => CredentialBootstrapStatus::Invalid,
        IntegrityKeyInspection::Unavailable => CredentialBootstrapStatus::Unavailable,
        IntegrityKeyInspection::Missing => {
            match protected_paths_are_absent(data_dir, protected_files) {
                Ok(true) => CredentialBootstrapStatus::InitializationRequired,
                Ok(false) => CredentialBootstrapStatus::MissingExistingData,
                Err(_) => CredentialBootstrapStatus::Unknown,
            }
        }
    }
}

fn inspect_mcp_audit_credential_status(
    data_dir: &Path,
    store: &dyn SecretStore,
) -> CredentialBootstrapStatus {
    let keyring_path = data_dir.join("mcp_audit_keys.json");
    let database_path = data_dir.join("mcp_audit.db");
    match load_mcp_audit_keyring_from_path(&keyring_path) {
        McpAuditKeyringLoad::Absent => {
            match McpAuditStore::inspect_existing_database(&database_path) {
                Ok(inspection) if inspection.is_empty_or_absent() => {
                    CredentialBootstrapStatus::InitializationRequired
                }
                Ok(_) => CredentialBootstrapStatus::MissingExistingData,
                Err(_) => CredentialBootstrapStatus::Unknown,
            }
        }
        McpAuditKeyringLoad::Present(configs) => {
            match inspect_existing_mcp_audit_keys(configs, store) {
                McpAuditKeyHydrationInspection::Available(hydration) => {
                    if let Err(error) = McpAuditStore::preflight_existing_database_key_materials(
                        &database_path,
                        &hydration.materials,
                    ) {
                        return if openlife_core::mcp_audit::is_payload_integrity_failure(&error) {
                            CredentialBootstrapStatus::Invalid
                        } else {
                            CredentialBootstrapStatus::Unknown
                        };
                    }
                    let latest_is_keychain = hydration.configs.last().is_some_and(|config| {
                        config.mode == openlife_core::mcp_audit::KeyMode::Keychain
                    });
                    let has_keychain_epoch = hydration
                        .configs
                        .iter()
                        .any(|config| config.mode == openlife_core::mcp_audit::KeyMode::Keychain);
                    if latest_is_keychain {
                        CredentialBootstrapStatus::Available
                    } else if has_keychain_epoch {
                        CredentialBootstrapStatus::Invalid
                    } else {
                        CredentialBootstrapStatus::InitializationRequired
                    }
                }
                McpAuditKeyHydrationInspection::MissingExistingData => {
                    CredentialBootstrapStatus::MissingExistingData
                }
                McpAuditKeyHydrationInspection::Invalid => CredentialBootstrapStatus::Invalid,
                McpAuditKeyHydrationInspection::Unavailable => {
                    CredentialBootstrapStatus::Unavailable
                }
            }
        }
        McpAuditKeyringLoad::PresentInvalid { .. } => CredentialBootstrapStatus::Invalid,
        McpAuditKeyringLoad::Unreadable { .. } => CredentialBootstrapStatus::Unavailable,
    }
}

fn inspect_required_credential_snapshot(
    data_dir: &Path,
    store: &dyn SecretStore,
) -> CredentialBootstrapSnapshot {
    CredentialBootstrapSnapshot::from_statuses([
        inspect_fixed_credential_status(
            data_dir,
            store,
            AGENT_RUN_RECEIPT_KEY_REF,
            &[
                "agent_runs.db",
                "life_events.db",
                "main_chat_agent_sessions.db",
            ],
        ),
        inspect_fixed_credential_status(
            data_dir,
            store,
            MAIN_CHAT_EVENT_INTEGRITY_KEY_REF,
            &["main_chat_agent_events.db"],
        ),
        inspect_fixed_credential_status(
            data_dir,
            store,
            ACTION_QUEUE_AUTHORITY_KEY_REF,
            &["main_chat_action_queue.db"],
        ),
        inspect_fixed_credential_status(
            data_dir,
            store,
            TASK_STORE_AUTHORITY_KEY_REF,
            &["tasks.db"],
        ),
        inspect_mcp_audit_credential_status(data_dir, store),
    ])
}

fn eligible_credential_purposes(snapshot: &CredentialBootstrapSnapshot) -> Vec<String> {
    let mut purposes = snapshot
        .purposes
        .iter()
        .filter(|item| item.status == CredentialBootstrapStatus::InitializationRequired)
        .map(|item| item.purpose.clone())
        .collect::<Vec<_>>();
    purposes.sort();
    purposes
}

fn recovery_report_from_snapshot(
    snapshot: &CredentialBootstrapSnapshot,
) -> CredentialRecoveryReport {
    CredentialRecoveryReport {
        items: snapshot
            .purposes
            .iter()
            .map(|item| CredentialRecoveryItem {
                purpose: item.purpose.clone(),
                status: item.status.as_str().into(),
            })
            .collect(),
        initialization_completed_for_restart: false,
        restart_required: false,
        cleanup_status: "not_required".into(),
        blocked_reason: None,
        bootstrap_snapshot_digest: snapshot.digest.clone(),
    }
}

fn set_recovery_item_status(report: &mut CredentialRecoveryReport, purpose: &str, status: &str) {
    if let Some(item) = report.items.iter_mut().find(|item| item.purpose == purpose) {
        item.status = status.into();
    }
}

struct CreatedCredential {
    purpose: &'static str,
    secret_ref: &'static str,
    exact_encoded_value: String,
}

fn delete_exact_created_credential(
    store: &dyn SecretStore,
    secret_ref: &str,
    exact_encoded_value: &str,
) -> bool {
    match store.get(secret_ref) {
        Ok(Some(current)) if current == exact_encoded_value => store.delete(secret_ref).is_ok(),
        _ => false,
    }
}

fn rollback_created_credentials(
    store: &dyn SecretStore,
    created: &[CreatedCredential],
    report: &mut CredentialRecoveryReport,
) -> bool {
    let mut complete = true;
    for credential in created.iter().rev() {
        if delete_exact_created_credential(
            store,
            credential.secret_ref,
            &credential.exact_encoded_value,
        ) {
            set_recovery_item_status(report, credential.purpose, "compensated");
        } else {
            complete = false;
            set_recovery_item_status(report, credential.purpose, "cleanup_unknown");
        }
    }
    complete
}

fn initialize_mcp_audit_credential(
    data_dir: &Path,
    store: &dyn SecretStore,
) -> Result<(), (String, bool)> {
    let path = data_dir.join("mcp_audit_keys.json");
    let prior_bytes = mcp_audit_keyring_bytes(&path);
    let mut configs = match &prior_bytes {
        McpAuditKeyringBytes::Present(bytes) => serde_json::from_slice(bytes).map_err(|error| {
            (
                format!("parse prior MCP key-reference bytes: {error}"),
                false,
            )
        })?,
        McpAuditKeyringBytes::Absent => Vec::new(),
        McpAuditKeyringBytes::Unreadable(error) => {
            return Err((
                format!("read prior MCP key-reference bytes: {error}"),
                false,
            ));
        }
    };
    let previous_epoch = configs
        .last()
        .map_or(0, |config: &openlife_core::mcp_audit::AuditKeyConfig| {
            config.epoch
        });
    let next_epoch = previous_epoch
        .checked_add(1)
        .map(|next| next.max(chrono::Utc::now().timestamp().max(0) as u64))
        .ok_or_else(|| ("MCP audit key epoch is exhausted".into(), false))?;
    let expected_secret_ref = format!("{MCP_AUDIT_KEY_REF_PREFIX}{next_epoch}");
    let material = create_mcp_audit_key_material(next_epoch, store).map_err(|error| {
        let cleanup_unknown = !matches!(store.get(&expected_secret_ref), Ok(None));
        (
            format!("create MCP audit credential: {error}"),
            cleanup_unknown,
        )
    })?;
    let secret_ref = material.config.key_ref.clone().unwrap_or_default();
    let exact_encoded_value = general_purpose::STANDARD.encode(material.key);
    configs.push(material.config);
    let intended_bytes = match serde_json::to_vec_pretty(&configs) {
        Ok(bytes) => bytes,
        Err(error) => {
            let cleanup_unknown =
                !delete_exact_created_credential(store, &secret_ref, &exact_encoded_value);
            return Err((
                format!("serialize MCP key-reference bytes: {error}"),
                cleanup_unknown,
            ));
        }
    };
    if let Err(error) = save_mcp_audit_keyring_to_path(&path, &configs) {
        let final_bytes = mcp_audit_keyring_bytes(&path);
        if final_bytes == prior_bytes {
            let delete_failed =
                !delete_exact_created_credential(store, &secret_ref, &exact_encoded_value);
            return Err((
                format!("save MCP key reference failed before authority changed: {error}"),
                delete_failed,
            ));
        }
        if final_bytes == McpAuditKeyringBytes::Present(intended_bytes.clone()) {
            return Err((
                format!(
                    "save MCP key reference reported an ambiguous error after intended authority became observable: {error}"
                ),
                true,
            ));
        }
        return Err((
            format!("save MCP key reference left an unknown authority state: {error}"),
            true,
        ));
    }
    let final_bytes = mcp_audit_keyring_bytes(&path);
    if final_bytes == McpAuditKeyringBytes::Present(intended_bytes) {
        return Ok(());
    }
    if final_bytes == prior_bytes {
        let cleanup_unknown =
            !delete_exact_created_credential(store, &secret_ref, &exact_encoded_value);
        return Err((
            "MCP key-reference save returned success without changing authority".into(),
            cleanup_unknown,
        ));
    }
    Err((
        "MCP key-reference authority drifted after save".into(),
        true,
    ))
}

fn initialize_required_credentials_after_confirmation(
    data_dir: &Path,
    store: &dyn SecretStore,
    expected_snapshot: &CredentialBootstrapSnapshot,
) -> Result<CredentialRecoveryReport, AppError> {
    let _process_lock = CredentialRecoveryProcessLock::acquire(data_dir)?;
    let current_snapshot = inspect_required_credential_snapshot(data_dir, store);
    if current_snapshot != *expected_snapshot {
        return Err(AppError::permission(
            "credential bootstrap snapshot changed after native confirmation; restart and retry",
        ));
    }
    let eligible = eligible_credential_purposes(&current_snapshot);
    if eligible.is_empty() {
        return Err(AppError::permission(
            "no required credential is explicitly eligible for initialization",
        ));
    }

    let mut report = recovery_report_from_snapshot(&current_snapshot);
    let mut created = Vec::<CreatedCredential>::new();
    for (purpose, secret_ref) in [
        ("agent_run_receipts", AGENT_RUN_RECEIPT_KEY_REF),
        ("main_chat_events", MAIN_CHAT_EVENT_INTEGRITY_KEY_REF),
        ("action_queue", ACTION_QUEUE_AUTHORITY_KEY_REF),
        ("task_store", TASK_STORE_AUTHORITY_KEY_REF),
    ] {
        if !eligible.iter().any(|eligible| eligible == purpose) {
            continue;
        }
        if !matches!(store.get(secret_ref), Ok(None)) {
            set_recovery_item_status(&mut report, purpose, "cleanup_unknown");
            let cleanup_complete = rollback_created_credentials(store, &created, &mut report);
            report.cleanup_status = "unknown".into();
            report.blocked_reason = Some(format!(
                "{purpose} credential ownership changed after the locked snapshot; cleanup was not attempted"
            ));
            if !cleanup_complete {
                report.cleanup_status = "unknown".into();
            }
            return Ok(report);
        }
        let result = if purpose == "task_store" {
            hydrate_or_create_canonical_store_integrity_key(
                secret_ref,
                &data_dir.join("tasks.db"),
                store,
            )
        } else {
            hydrate_or_create_integrity_key(secret_ref, store)
        };
        let key = match result {
            Ok(key) => key,
            Err(error) => {
                let ambiguous = !matches!(store.get(secret_ref), Ok(None));
                set_recovery_item_status(
                    &mut report,
                    purpose,
                    if ambiguous {
                        "cleanup_unknown"
                    } else {
                        "unavailable"
                    },
                );
                let cleanup_complete = rollback_created_credentials(store, &created, &mut report);
                report.cleanup_status = if ambiguous || !cleanup_complete {
                    "unknown"
                } else {
                    "compensated"
                }
                .into();
                report.blocked_reason = Some(format!("{purpose} initialization failed: {error}"));
                return Ok(report);
            }
        };
        let exact_encoded_value = general_purpose::STANDARD.encode(key);
        if !matches!(store.get(secret_ref), Ok(Some(current)) if current == exact_encoded_value) {
            set_recovery_item_status(&mut report, purpose, "cleanup_unknown");
            let cleanup_complete = rollback_created_credentials(store, &created, &mut report);
            report.cleanup_status = "unknown".into();
            report.blocked_reason = Some(format!(
                "{purpose} initialization did not produce an exact owned credential receipt"
            ));
            if !cleanup_complete {
                report.cleanup_status = "unknown".into();
            }
            return Ok(report);
        }
        created.push(CreatedCredential {
            purpose,
            secret_ref,
            exact_encoded_value,
        });
        set_recovery_item_status(&mut report, purpose, "created");
    }

    if eligible.iter().any(|purpose| purpose == "mcp_audit") {
        if let Err((error, cleanup_unknown)) = initialize_mcp_audit_credential(data_dir, store) {
            set_recovery_item_status(
                &mut report,
                "mcp_audit",
                if cleanup_unknown {
                    "cleanup_unknown"
                } else {
                    "unavailable"
                },
            );
            let fixed_cleanup_complete = rollback_created_credentials(store, &created, &mut report);
            report.cleanup_status = if cleanup_unknown || !fixed_cleanup_complete {
                "unknown"
            } else {
                "compensated"
            }
            .into();
            report.blocked_reason = Some(error);
            return Ok(report);
        }
        set_recovery_item_status(&mut report, "mcp_audit", "created");
    }

    report.initialization_completed_for_restart = true;
    report.restart_required = true;
    Ok(report)
}

fn initialize_required_credentials_with_confirmation_result(
    native_confirmed: bool,
    data_dir: &Path,
    store: &dyn SecretStore,
    expected_snapshot: &CredentialBootstrapSnapshot,
) -> Result<CredentialRecoveryReport, AppError> {
    if !native_confirmed {
        return Err(AppError::permission(
            "credential initialization was not confirmed in the native system dialog",
        ));
    }
    initialize_required_credentials_after_confirmation(data_dir, store, expected_snapshot)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GovernedDataImportRequest {
    pub operation_id: String,
    pub purpose: String,
    pub explicit_user_intent: bool,
    pub create_pre_change_snapshot: bool,
    pub import_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DangerActionConfirmationReference {
    /// Opaque, random challenge identifier issued by the Rust authority. All
    /// other client fields are scope hints only and can never authorize an action.
    pub preflight_id: String,
    #[serde(default)]
    pub action_type: String,
    #[serde(default)]
    pub target_ids: Vec<String>,
}

pub(crate) struct DangerActionConfirmationRequest<'a> {
    pub action_type: &'a str,
    pub target_ids_for_new_challenge: &'a [String],
    pub requested_target: Option<&'a str>,
    pub affected_count: Option<usize>,
    pub reference: Option<&'a DangerActionConfirmationReference>,
    pub arguments: &'a serde_json::Value,
    pub arguments_summary: &'a str,
    /// Unforgeable, journal-bound capability used only to let the same
    /// confirmed import operation enter its recovery path while every ordinary
    /// effect remains fail-closed.
    pub governed_data_import_recovery: Option<&'a GovernedDataImportRecoveryAdmission<'a>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DangerActionPreflightView {
    pub action_type: String,
    pub risk_tier: String,
    pub scope_summary: String,
    pub data_categories: Vec<String>,
    pub writes_durable_state: bool,
    pub privacy_sensitive: bool,
    pub external_transmission: String,
    pub dry_run_available: bool,
    pub backup_status: String,
    pub requires_typed_confirmation: bool,
    pub confirmation_required: bool,
    pub confirmation_phrase: Option<String>,
    pub confirmation_scope_digest: String,
    pub preflight_id: String,
    pub affected_item_count: usize,
    pub affected_item_digest: String,
    pub final_action_enabled: bool,
    pub safe_mode_blocked: bool,
    pub blocking_reasons: Vec<String>,
    pub source_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_stage: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DangerActionPreflightScope {
    target_ids: Vec<String>,
    affected_count: Option<usize>,
}

fn validate_scope_target_ids(target_ids: &[String]) -> Result<Vec<String>, AppError> {
    if target_ids.len() > 100 {
        return Err(AppError::permission(
            "danger action preflight target scope is too large",
        ));
    }
    let mut safe = Vec::with_capacity(target_ids.len());
    for target_id in target_ids {
        if target_id.is_empty()
            || target_id.len() > 128
            || target_id.trim() != target_id
            || target_id.chars().any(char::is_control)
        {
            return Err(AppError::permission(
                "danger action preflight target scope is not metadata-safe",
            ));
        }
        safe.push(target_id.clone());
    }
    safe.sort();
    safe.dedup();
    Ok(safe)
}

fn danger_action_requires_native_confirmation(action_type: &str) -> bool {
    matches!(
        action_type,
        "data_export"
            | "data_import_overwrite"
            | "data_import_abandon_recovery"
            | "mcp_audit_export"
            | "mcp_audit_cleanup"
            | "mcp_audit_key_rotation"
            | "agent_run_delete"
            | "agent_run_bulk_delete"
            | "vector_rebuild"
    )
}

fn danger_action_scope_digest(
    action_type: &str,
    target_ids: &[String],
    affected_count: usize,
) -> Result<String, AppError> {
    let canonical = serde_json::json!({
        "action_type": action_type,
        "affected_count": affected_count,
        "target_id_count": target_ids.len(),
        "target_ids": target_ids,
    });
    let bytes = serde_json::to_vec(&canonical)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!(
        "bytes:{} hash:sha256:{:x}",
        bytes.len(),
        hasher.finalize()
    ))
}

#[cfg(test)]
fn danger_action_preflight_for_action(
    action_type: &str,
    safe_mode: bool,
) -> Result<DangerActionPreflightView, AppError> {
    danger_action_preflight_for_action_scoped(
        action_type,
        safe_mode,
        DangerActionPreflightScope::default(),
    )
}

fn danger_action_preflight_for_action_scoped(
    action_type: &str,
    safe_mode: bool,
    scope: DangerActionPreflightScope,
) -> Result<DangerActionPreflightView, AppError> {
    let safe_target_ids = validate_scope_target_ids(&scope.target_ids)?;
    let affected_count = scope
        .affected_count
        .unwrap_or(safe_target_ids.len())
        .max(safe_target_ids.len());
    let scope_digest = danger_action_scope_digest(action_type, &safe_target_ids, affected_count)?;
    let confirmation_phrase = None;
    let requires_typed_confirmation = false;
    let confirmation_required = danger_action_requires_native_confirmation(action_type);
    // A usable preflight id is created only by `get_danger_action_preflight`
    // through the Rust-owned challenge authority. The deterministic view builder
    // intentionally cannot mint authorization state.
    let preflight_id = String::new();
    let mut view = match action_type {
        "data_export" => DangerActionPreflightView {
            action_type: "data_export".into(),
            risk_tier: "high".into(),
            scope_summary:
                "导出本地 LifeModel、StateStore 日任务、聊天记录和向量记忆到用户选择的本地 JSON 文件。".into(),
            data_categories: vec![
                "life_model".into(),
                "state_store".into(),
                "messages".into(),
                "vectors".into(),
            ],
            writes_durable_state: false,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "not_required_read_only".into(),
            requires_typed_confirmation,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:export_all_data".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
            recovery_operation_id: None,
            recovery_stage: None,
        },
        "data_import_overwrite" => DangerActionPreflightView {
            action_type: "data_import_overwrite".into(),
            risk_tier: "critical".into(),
            scope_summary:
                "读取用户选择的 OpenLife JSON 备份，并覆盖当前 LifeModel、StateStore 日任务、聊天记录和向量记忆。执行前只为 LifeModel 创建 durable snapshot；其他 owner 依赖 CAS、故障 journal 和前向恢复，不存在自动完整回滚副本。"
                    .into(),
            data_categories: vec![
                "life_model".into(),
                "state_store".into(),
                "messages".into(),
                "vectors".into(),
            ],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "lifemodel_snapshot_only_other_owners_forward_recovery".into(),
            requires_typed_confirmation,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:import_all_data".into(),
                "governed_request:create_lifemodel_pre_change_snapshot_on_execute".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
            recovery_operation_id: None,
            recovery_stage: None,
        },
        "data_import_abandon_recovery" => DangerActionPreflightView {
            action_type: "data_import_abandon_recovery".into(),
            risk_tier: "critical".into(),
            scope_summary:
                "不重新执行或回滚中断的导入；重新观察每个 canonical owner，只保存 digest、分类、时间和 StateStore 投递引用，然后以 abandoned_preserving_current 终止恢复。当前数据保持原样，应用必须重启后才能恢复普通副作用。"
                    .into(),
            data_categories: vec![
                "governed_import_journal_metadata".into(),
                "canonical_owner_digest_evidence".into(),
                "state_projection_delivery_metadata".into(),
            ],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "not_applicable_preserves_current_canonical_data".into(),
            requires_typed_confirmation,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:abandon_governed_data_import_recovery".into(),
                "governed_import_resolution:metadata_only_preserve_current".into(),
            ],
            recovery_operation_id: None,
            recovery_stage: None,
        },
        "mcp_audit_export" => DangerActionPreflightView {
            action_type: "mcp_audit_export".into(),
            risk_tier: "high".into(),
            scope_summary:
                "导出最近 MCP 审计日志到用户选择的本地 JSON 文件，可能包含工具名称、工具输入参数文本、工具执行结果文本、执行状态和审计元数据。"
                    .into(),
            data_categories: vec![
                "mcp_audit_metadata".into(),
                "tool_metadata".into(),
                "tool_input_text".into(),
                "tool_output_text".into(),
            ],
            writes_durable_state: false,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "not_required_read_only".into(),
            requires_typed_confirmation,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:export_mcp_audit_logs".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
            recovery_operation_id: None,
            recovery_stage: None,
        },
        "mcp_audit_cleanup" => DangerActionPreflightView {
            action_type: "mcp_audit_cleanup".into(),
            risk_tier: "high".into(),
            scope_summary: "删除超过保留期限的本地 MCP 审计日志。".into(),
            data_categories: vec!["mcp_audit_metadata".into(), "tool_metadata".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "none".into(),
            requires_typed_confirmation,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:cleanup_mcp_audit_logs".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
            recovery_operation_id: None,
            recovery_stage: None,
        },
        "mcp_audit_key_rotation" => DangerActionPreflightView {
            action_type: "mcp_audit_key_rotation".into(),
            risk_tier: "critical".into(),
            scope_summary:
                "轮换本地 MCP 审计加密 epoch；历史 epoch 会保留以便旧审计日志继续可读。".into(),
            data_categories: vec!["mcp_audit_metadata".into(), "mcp_audit_key_epochs".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "historical_key_epochs_retained".into(),
            requires_typed_confirmation,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:rotate_mcp_audit_key".into(),
                "governance:slice5b_danger_action_preflight".into(),
            ],
            recovery_operation_id: None,
            recovery_stage: None,
        },
        "agent_run_delete" => DangerActionPreflightView {
            action_type: "agent_run_delete".into(),
            risk_tier: "high".into(),
            scope_summary:
                "删除选中的 AgentRun 运行记录；预检只保留数量和 id digest，不展开 transcript、tool input 或模型输出。"
                    .into(),
            data_categories: vec!["agent_run_metadata".into(), "run_trace_metadata".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "soft_delete_trash_view".into(),
            requires_typed_confirmation,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:delete_agent_run".into(),
                "governance:slice5c_danger_zone_consolidation".into(),
            ],
            recovery_operation_id: None,
            recovery_stage: None,
        },
        "agent_run_bulk_delete" => DangerActionPreflightView {
            action_type: "agent_run_bulk_delete".into(),
            risk_tier: "high".into(),
            scope_summary:
                "批量删除选中的 AgentRun 运行记录；预检只保留 bounded 数量和 id digest，不展开 transcript、tool input 或模型输出。"
                    .into(),
            data_categories: vec!["agent_run_metadata".into(), "run_trace_metadata".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "soft_delete_trash_view".into(),
            requires_typed_confirmation,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:delete_agent_run".into(),
                "governance:slice5c_danger_zone_consolidation".into(),
            ],
            recovery_operation_id: None,
            recovery_stage: None,
        },
        "vector_rebuild" => DangerActionPreflightView {
            action_type: "vector_rebuild".into(),
            risk_tier: "high".into(),
            scope_summary:
                "基于现有聊天消息重建本地向量索引；预检只展示消息数量和 scope digest，不展示原始消息或向量内容。"
                    .into(),
            data_categories: vec!["messages_metadata".into(), "vectors".into()],
            writes_durable_state: true,
            privacy_sensitive: true,
            external_transmission: "not_sent_externally".into(),
            dry_run_available: false,
            backup_status: "rollback_previous_vectors_on_failure".into(),
            requires_typed_confirmation,
            confirmation_required,
            confirmation_phrase,
            confirmation_scope_digest: scope_digest.clone(),
            preflight_id: preflight_id.clone(),
            affected_item_count: affected_count,
            affected_item_digest: scope_digest.clone(),
            final_action_enabled: true,
            safe_mode_blocked: false,
            blocking_reasons: vec![],
            source_refs: vec![
                "settings_command:get_danger_action_preflight".into(),
                "final_command:rebuild_memory_index".into(),
                "governance:slice5c_danger_zone_consolidation".into(),
            ],
            recovery_operation_id: None,
            recovery_stage: None,
        },
        _ => {
            return Err(AppError::permission(
                "unsupported danger action preflight action type",
            ));
        }
    };

    if safe_mode && view.writes_durable_state {
        view.final_action_enabled = false;
        view.safe_mode_blocked = true;
        view.blocking_reasons
            .push("safe_mode_blocks_durable_write".into());
        view.source_refs.push("safe_mode:blocked".into());
    }
    view.source_refs
        .push(format!("scope_digest:{}", view.confirmation_scope_digest));

    Ok(view)
}

pub(crate) async fn danger_action_safe_mode_active(state: &Arc<AppState>) -> bool {
    if !state.startup_warnings.is_empty() {
        return true;
    }
    let store = state.vector_store.lock().await;
    store
        .integrity_report()
        .map(|report| report.corrupt_embedding_count > 0)
        .unwrap_or(true)
}

async fn governed_import_recovery_has_no_other_safe_mode_blocker(state: &Arc<AppState>) -> bool {
    // Recovery admission is based on coordinator-owned typed reason codes,
    // never on human-readable bootstrap warning text. The journal-bound
    // capability minted below remains the final authority for the exact
    // operation and owners.
    let persistence = state.persistence_coordinator.snapshot();
    if persistence.global_reason_codes.as_slice() != [GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON]
    {
        return false;
    }
    state
        .vector_store
        .lock()
        .await
        .integrity_report()
        .is_ok_and(|report| report.corrupt_embedding_count == 0)
}

fn required_governed_data_import_journal(
    state: &AppState,
) -> Result<Arc<GovernedDataImportJournal>, AppError> {
    state
        .governed_data_import_journal
        .as_ref()
        .cloned()
        .ok_or_else(|| {
            AppError::db_with_hint(
                "governed data-import journal was unavailable during bootstrap; effects remain fail-closed",
                "data_import_journal_unavailable",
            )
        })
}

async fn governed_import_recovery_preflight_receipt(
    state: &Arc<AppState>,
) -> Result<Option<GovernedDataImportReceipt>, AppError> {
    if !governed_import_recovery_has_no_other_safe_mode_blocker(state).await {
        return Ok(None);
    }
    let journal = required_governed_data_import_journal(state)?;
    let Some(receipt) = journal.recovery_requirement().map_err(|error| {
        AppError::db_with_hint(error.to_string(), "data_import_journal_unavailable")
    })?
    else {
        return Ok(None);
    };
    state
        .persistence_coordinator
        .mint_governed_data_import_recovery_admission(
            &journal,
            &receipt,
            &receipt.operation_id,
            &receipt.payload_digest,
            &receipt.request_digest,
        )
        .map_err(|error| {
            AppError::db_with_hint(error.to_string(), "data_import_recovery_required")
        })?;
    Ok(Some(receipt))
}

pub(crate) async fn require_danger_action_confirmation(
    request: DangerActionConfirmationRequest<'_>,
    window: &tauri::WebviewWindow,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    let scope = DangerActionPreflightScope {
        target_ids: request.target_ids_for_new_challenge.to_vec(),
        affected_count: request.affected_count,
    };
    let expected = danger_action_preflight_for_action_scoped(request.action_type, false, scope)?;
    let recovery_safe_mode_override = matches!(
        request.action_type,
        "data_import_overwrite" | "data_import_abandon_recovery"
    ) && request.governed_data_import_recovery.is_some()
        && governed_import_recovery_has_no_other_safe_mode_blocker(state).await;
    if expected.writes_durable_state
        && danger_action_safe_mode_active(state).await
        && !recovery_safe_mode_override
    {
        return Err(AppError::permission(
            "danger action blocked because Safe Mode is active",
        ));
    }
    if !expected.confirmation_required {
        return Ok(());
    }
    require_native_danger_action_confirmation(
        window,
        NativeDangerActionRequest {
            action_type: request.action_type,
            target_ids_for_new_challenge: request.target_ids_for_new_challenge,
            requested_target: request.requested_target,
            affected_count: expected.affected_item_count,
            arguments: request.arguments,
            arguments_summary: request.arguments_summary,
            scope_summary: &expected.scope_summary,
            challenge_id: request
                .reference
                .map(|reference| reference.preflight_id.as_str()),
        },
    )
    .await
}

#[tauri::command]
pub async fn get_danger_action_preflight(
    action_type: String,
    safe_mode: Option<bool>,
    target_ids: Option<Vec<String>>,
    affected_count: Option<usize>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<DangerActionPreflightView, AppError> {
    let requested_safe_mode = safe_mode.unwrap_or(false);
    let recovery = if matches!(
        action_type.as_str(),
        "data_import_overwrite" | "data_import_abandon_recovery"
    ) {
        governed_import_recovery_preflight_receipt(state.inner()).await?
    } else {
        None
    };
    let mut effective_safe_mode = requested_safe_mode;
    if danger_action_safe_mode_active(state.inner()).await
        && (recovery.is_none() || requested_safe_mode)
    {
        effective_safe_mode = true;
    }
    let target_ids = if action_type == "data_import_abandon_recovery" {
        vec![recovery
            .as_ref()
            .ok_or_else(|| {
                AppError::permission("no governed data-import recovery is pending for abandonment")
            })?
            .operation_id
            .clone()]
    } else {
        target_ids.unwrap_or_default()
    };
    let effective_affected_count = if action_type == "data_import_abandon_recovery" {
        Some(
            recovery
                .as_ref()
                .map(|receipt| receipt.owners.len())
                .unwrap_or_default(),
        )
    } else if action_type == "vector_rebuild" && affected_count.is_none() {
        let store = state.memory_store.lock().await;
        Some(store.export_all_messages().map_err(AppError::from)?.len())
    } else {
        affected_count
    };
    let mut view = danger_action_preflight_for_action_scoped(
        &action_type,
        effective_safe_mode,
        DangerActionPreflightScope {
            target_ids: target_ids.clone(),
            affected_count: effective_affected_count,
        },
    )?;
    if view.confirmation_required && view.final_action_enabled {
        view.preflight_id = issue_danger_action_challenge(
            window.label(),
            &action_type,
            &target_ids,
            view.affected_item_count,
        )?;
        view.source_refs
            .push("native_confirmation:server_challenge_pending".into());
    }
    if let Some(receipt) = recovery {
        view.recovery_operation_id = Some(receipt.operation_id);
        view.recovery_stage = Some(receipt.stage.as_str().into());
        view.source_refs
            .push("governed_data_import:recovery_preflight".into());
    }
    Ok(view)
}

impl GovernedDataImportRequest {
    #[cfg(test)]
    fn manual_restore_all_targets() -> Self {
        Self {
            operation_id: Uuid::new_v4().hyphenated().to_string(),
            purpose: "manual_restore".into(),
            explicit_user_intent: true,
            create_pre_change_snapshot: true,
            import_targets: vec![
                "life_model".into(),
                "messages".into(),
                "vectors".into(),
                "state_store".into(),
            ],
        }
    }

    fn is_valid(&self) -> bool {
        let unique_targets = self
            .import_targets
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        Uuid::parse_str(&self.operation_id).is_ok_and(|operation_id| {
            operation_id.get_version() == Some(Version::Random)
                && operation_id.hyphenated().to_string() == self.operation_id
        }) && self.explicit_user_intent
            && self.create_pre_change_snapshot
            && matches!(self.purpose.as_str(), "manual_restore" | "migration")
            && !self.import_targets.is_empty()
            && unique_targets.len() == self.import_targets.len()
            && self.import_targets.iter().all(|target| {
                matches!(
                    target.as_str(),
                    "life_model" | "messages" | "vectors" | "state_store"
                )
            })
    }
}

fn require_governed_data_import_request(
    import_request: Option<&GovernedDataImportRequest>,
) -> Result<&GovernedDataImportRequest, AppError> {
    if let Some(request) = import_request.filter(|request| request.is_valid()) {
        Ok(request)
    } else {
        Err(AppError::permission(
            "import_all_data requires an explicit governed import request with purpose manual_restore or migration, explicitUserIntent=true, createPreChangeSnapshot=true, and supported importTargets.",
        ))
    }
}

fn hash_json_value(value: &serde_json::Value) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn hash_serializable_value<T: Serialize>(value: &T) -> Result<String, AppError> {
    hash_json_value(&serde_json::to_value(value).map_err(AppError::from)?)
}

fn validate_import_json_budget(payload: &serde_json::Value) -> Result<(), AppError> {
    fn charge(remaining: &mut usize, amount: usize) -> Result<(), AppError> {
        if amount > *remaining {
            return Err(AppError::external(
                "OpenLife backup exceeds the bounded import size limit",
            ));
        }
        *remaining -= amount;
        Ok(())
    }

    fn visit(
        value: &serde_json::Value,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<(), AppError> {
        if depth > MAX_GOVERNED_IMPORT_JSON_DEPTH {
            return Err(AppError::external(
                "OpenLife backup exceeds the bounded JSON nesting limit",
            ));
        }
        charge(remaining, 16)?;
        match value {
            serde_json::Value::String(text) => {
                if text.len() > MAX_GOVERNED_IMPORT_STRING_BYTES {
                    return Err(AppError::external(
                        "OpenLife backup contains an oversized text field",
                    ));
                }
                charge(remaining, text.len())
            }
            serde_json::Value::Array(items) => {
                if items.len() > MAX_GOVERNED_IMPORT_CONTAINER_ITEMS {
                    return Err(AppError::external(
                        "OpenLife backup contains an oversized JSON array",
                    ));
                }
                for item in items {
                    visit(item, depth + 1, remaining)?;
                }
                Ok(())
            }
            serde_json::Value::Object(fields) => {
                if fields.len() > MAX_GOVERNED_IMPORT_CONTAINER_ITEMS {
                    return Err(AppError::external(
                        "OpenLife backup contains an oversized JSON object",
                    ));
                }
                for (key, value) in fields {
                    if key.len() > MAX_GOVERNED_IMPORT_STRING_BYTES {
                        return Err(AppError::external(
                            "OpenLife backup contains an oversized object key",
                        ));
                    }
                    charge(remaining, key.len())?;
                    visit(value, depth + 1, remaining)?;
                }
                Ok(())
            }
            serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
                Ok(())
            }
        }
    }

    let mut remaining = MAX_GOVERNED_IMPORT_JSON_BYTES;
    visit(payload, 0, &mut remaining)
}

fn validate_import_payload_shape(payload: &serde_json::Value) -> Result<(), AppError> {
    let object = payload
        .as_object()
        .ok_or_else(|| AppError::external("导入 payload 必须是 JSON object"))?;
    validate_import_json_budget(payload)?;
    if object
        .get("messages")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| items.len() > MAX_GOVERNED_IMPORT_MESSAGES)
    {
        return Err(AppError::external(
            "OpenLife backup exceeds the bounded message import limit",
        ));
    }
    if object
        .get("vectors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| items.len() > MAX_GOVERNED_IMPORT_VECTORS)
    {
        return Err(AppError::external(
            "OpenLife backup exceeds the bounded vector import limit",
        ));
    }
    if object
        .get("state_store")
        .and_then(|state| state.get("dailyTasks"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| items.len() > MAX_GOVERNED_IMPORT_STATE_TASKS)
    {
        return Err(AppError::external(
            "OpenLife backup exceeds the bounded StateStore daily-task import limit",
        ));
    }
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "version"
                | "app_version"
                | "exported_at"
                | "vector_export_semantics"
                | "life_model"
                | "messages"
                | "vectors"
                | "state_store"
        ) {
            return Err(AppError::permission(format!(
                "import_all_data received unsupported import target: {key}"
            )));
        }
    }
    if !object.contains_key("life_model") {
        return Err(AppError::external("导入 payload 缺少 life_model"));
    }
    match object.get("version").and_then(serde_json::Value::as_str) {
        Some("1.0") if object.contains_key("state_store") => {
            return Err(AppError::permission(
                "OpenLife v1 backup cannot carry a v2 state_store archive",
            ));
        }
        Some("1.0") => {}
        Some("2.0") if !object.contains_key("state_store") => {
            return Err(AppError::external(
                "OpenLife v2 backup is missing the required state_store archive",
            ));
        }
        Some("2.0") => {}
        _ => {
            return Err(AppError::external(
                "OpenLife backup version is missing or unsupported",
            ));
        }
    }
    Ok(())
}

fn validate_import_targets_cover_payload(
    payload: &serde_json::Value,
    request: &GovernedDataImportRequest,
) -> Result<(), AppError> {
    let object = payload
        .as_object()
        .ok_or_else(|| AppError::external("导入 payload 必须是 JSON object"))?;
    for target in ["life_model", "messages", "vectors", "state_store"] {
        if object.contains_key(target) && !request.import_targets.iter().any(|item| item == target)
        {
            return Err(AppError::permission(format!(
                "import_all_data payload contains {target}, but the governed import request did not include that import target."
            )));
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct LastModelError {
    pub message: String,
    pub phase: String,
    pub timestamp: String,
}

#[tauri::command]
pub async fn get_last_model_error(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<LastModelError>, AppError> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| AppError::internal("agent_run_store_unavailable"))?;
    let store = store_arc.lock().await;
    let runs = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state.inner(),
        store.list_runs(10, 0).map_err(|error| error.to_string()),
    )
    .map_err(AppError::internal)?;
    let last_error = runs
        .iter()
        .find(|r| r.error.is_some())
        .and_then(|r| r.error.as_ref())
        .map(|e| LastModelError {
            message: e.message.clone(),
            phase: e.phase.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    Ok(last_error)
}

/// Mask for sensitive API keys sent to the frontend.
const KEY_MASK: &str = "***";

fn resolve_masked_api_key(submitted_key: &str, current_key: &str) -> String {
    if submitted_key.trim().is_empty() || submitted_key == KEY_MASK {
        current_key.to_string()
    } else {
        submitted_key.to_string()
    }
}

fn provider_endpoint_identity(config: &AppConfig) -> Option<String> {
    let provider = config.llm.provider.trim().to_ascii_lowercase();
    if provider.is_empty() {
        return None;
    }
    let base = if config.llm.openai_base.trim().is_empty() {
        default_base_for_provider(&provider).to_string()
    } else {
        config.llm.openai_base.trim().to_string()
    };
    let endpoint = chat_completions_url(&provider, &base);
    let parsed = reqwest::Url::parse(&endpoint).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }
    let host = parsed
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let port = parsed.port_or_known_default()?;
    let path = parsed.path().trim_end_matches('/');
    Some(format!(
        "{provider}|{}://{host}:{port}{path}",
        parsed.scheme()
    ))
}

fn resolve_submitted_provider_api_key(submitted: &AppConfig, current: &AppConfig) -> String {
    let submitted_key = submitted.llm.openai_key.trim();
    if !submitted_key.is_empty() && submitted_key != KEY_MASK {
        return submitted.llm.openai_key.clone();
    }
    let identity_unchanged = provider_endpoint_identity(submitted).is_some_and(|identity| {
        provider_endpoint_identity(current).as_deref() == Some(identity.as_str())
    });
    if identity_unchanged {
        current.llm.openai_key.clone()
    } else {
        String::new()
    }
}

fn resolved_provider_credential_version(submitted: &AppConfig, current: &AppConfig) -> u64 {
    let identity_changed =
        provider_endpoint_identity(submitted) != provider_endpoint_identity(current);
    let submitted_key = submitted.llm.openai_key.trim();
    let explicit_key_changed = !submitted_key.is_empty()
        && submitted_key != KEY_MASK
        && submitted_key != current.llm.openai_key;
    if identity_changed || explicit_key_changed {
        current.llm.credential_version.saturating_add(1)
    } else {
        current.llm.credential_version
    }
}

async fn replace_runtime_provider_config(state: &Arc<AppState>, config: AppConfig) {
    state.replace_provider_runtime_config(config).await;
}

fn credential_initialization_native_request<'a>(
    eligible_purposes: &'a [String],
    confirmation_arguments: &'a serde_json::Value,
) -> NativeDangerActionRequest<'a> {
    NativeDangerActionRequest {
        action_type: "credential_store_initialization",
        target_ids_for_new_challenge: eligible_purposes,
        // The native authority consumes one target as the batch anchor, while
        // the challenge itself and the arguments remain bound to the complete
        // sorted purpose set and matching affected count.
        requested_target: eligible_purposes.first().map(String::as_str),
        affected_count: eligible_purposes.len(),
        arguments: confirmation_arguments,
        arguments_summary:
            "仅初始化后端启动快照明确标记为 initialization_required 的系统凭据；完成后必须重启。",
        scope_summary: "后端快照列出的 OpenLife 内部系统凭据",
        challenge_id: None,
    }
}

/// User-initiated recovery for OS credential ACL changes after an application
/// update or development re-sign. Startup intentionally stays non-interactive
/// and bounded; this command is the only product path that may let the OS show
/// its credential authorization UI. It returns status only and never exposes
/// key material to the webview.
#[tauri::command]
pub async fn recover_required_credential_access(
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<CredentialRecoveryReport, AppError> {
    CREDENTIAL_RECOVERY_ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| AppError::permission("credential recovery is already in progress"))?;
    let _activity_guard = CredentialRecoveryActivityGuard;
    let expected_snapshot = state.credential_bootstrap_snapshot.clone();
    let expected_eligible_purposes = eligible_credential_purposes(&expected_snapshot);
    if expected_eligible_purposes.is_empty() {
        return Err(AppError::permission(
            "LifeStateProjection reports no credential eligible for initialization",
        ));
    }
    let data_dir = app_data_dir();
    let pre_confirmation_data_dir = data_dir.clone();
    let pre_confirmation_snapshot = tauri::async_runtime::spawn_blocking(move || {
        inspect_required_credential_snapshot(&pre_confirmation_data_dir, &KeyringSecretStore)
    })
    .await
    .map_err(|error| {
        AppError::internal(format!(
            "credential initialization preflight worker failed: {error}"
        ))
    })?;
    if pre_confirmation_snapshot != expected_snapshot {
        return Err(AppError::permission(
            "credential bootstrap snapshot changed before native confirmation; restart and retry",
        ));
    }
    let confirmation_arguments = serde_json::json!({
        "operation": "initialize_required_credentials",
        "eligiblePurposeIds": expected_eligible_purposes,
        "affectedCount": expected_eligible_purposes.len(),
        "bootstrapSnapshotVersion": expected_snapshot.version,
        "bootstrapSnapshotDigest": expected_snapshot.digest,
    });
    require_native_danger_action_confirmation(
        &window,
        credential_initialization_native_request(
            &expected_eligible_purposes,
            &confirmation_arguments,
        ),
    )
    .await?;
    let snapshot_for_worker = expected_snapshot.clone();
    tauri::async_runtime::spawn_blocking(move || {
        initialize_required_credentials_with_confirmation_result(
            true,
            &data_dir,
            &KeyringSecretStore,
            &snapshot_for_worker,
        )
    })
    .await
    .map_err(|error| AppError::internal(format!("credential recovery worker failed: {error}")))?
}

#[tauri::command]
pub async fn get_config(state: State<'_, Arc<AppState>>) -> Result<AppConfig, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("ConfigStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let mut cfg = state.config.lock().await.clone();
    // Sanitize API keys before sending to frontend
    if !cfg.llm.openai_key.is_empty() {
        cfg.llm.openai_key = KEY_MASK.to_string();
    }
    if !cfg.system.search_provider_key.is_empty() {
        cfg.system.search_provider_key = KEY_MASK.to_string();
    }
    Ok(cfg)
}

#[tauri::command]
pub async fn save_config(
    mut config: AppConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let _config_write_guard = CONFIG_WRITE_COORDINATOR.lock().await;
    config.normalize_provider_from_base();
    let data_dir = app_data_dir();
    let config_path = data_dir.join("config.yaml");

    // Preserve existing API key if the submitted config has a mask or empty key
    let current_config = {
        let cfg = state.config.lock().await;
        cfg.clone()
    };
    let provider_identity_unchanged = provider_endpoint_identity(&config).is_some_and(|identity| {
        provider_endpoint_identity(&current_config).as_deref() == Some(identity.as_str())
    });
    config.llm.credential_version = resolved_provider_credential_version(&config, &current_config);
    config.llm.openai_key = resolve_submitted_provider_api_key(&config, &current_config);
    config.system.search_provider_key = resolve_masked_api_key(
        &config.system.search_provider_key,
        &current_config.system.search_provider_key,
    );
    if !provider_identity_unchanged {
        // A secret reference is bound to the provider plus canonical endpoint. A masked
        // frontend value cannot carry an old credential to a different destination.
        config.llm.openai_key_ref = None;
    } else if config.llm.openai_key_ref.is_none() {
        config.llm.openai_key_ref = current_config.llm.openai_key_ref;
    }
    if config.system.search_provider_key_ref.is_none() {
        config.system.search_provider_key_ref = current_config.system.search_provider_key_ref;
    }

    let secret_store = KeyringSecretStore;
    let rollback = stage_config_secrets(&mut config, &secret_store).map_err(AppError::from)?;
    if let Err(save_error) = config.save(&config_path) {
        return match rollback.rollback(&secret_store) {
            Ok(()) => Err(AppError::from(save_error)),
            Err(rollback_error) => Err(AppError::internal(format!(
                "config save failed: {save_error}; credential rollback failed: {rollback_error}"
            ))),
        };
    }
    replace_runtime_provider_config(state.inner(), config).await;
    Ok(())
}

#[tauri::command]
pub async fn export_all_data(
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let export = export_all_data_with_state(state.inner()).await?;
    let export_digest = hash_json_value(&export)?;
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "data_export",
            target_ids_for_new_challenge: &[],
            requested_target: None,
            affected_count: None,
            reference: None,
            arguments: &serde_json::json!({
                "export_digest": export_digest,
                "data_categories": ["life_model", "state_store", "messages", "vectors"],
            }),
            arguments_summary:
                "导出当前 LifeModel、StateStore 日任务、聊天和向量数据快照；原始内容不会复制进 confirmation grant。",
            governed_data_import_recovery: None,
        },
        &window,
        state.inner(),
    )
    .await?;
    Ok(export)
}

async fn export_all_data_with_state(state: &Arc<AppState>) -> Result<serde_json::Value, AppError> {
    let exported_at = chrono::Utc::now();
    let mut life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    // `goals.daily` and `state.alerts` are compatibility projections. Their
    // portable owner is StateStore; copying them into the LifeModel payload
    // would create a second backup truth and make OpenLife's own archive fail
    // its field-authority guard on restore.
    life_model.goals.daily.clear();
    life_model.state.alerts.clear();
    let state_store = state.state_store.as_ref().ok_or_else(|| {
        AppError::db_with_hint(
            "StateStore is unavailable; a complete OpenLife v2 backup cannot be produced",
            "canonical_state_unknown",
        )
    })?;
    let state_store_archive = state_store
        .export_portable_daily_tasks(exported_at)
        .map_err(|error| {
            AppError::db_with_hint(
                format!("StateStore portable export failed: {error}"),
                "canonical_state_unknown",
            )
        })?;
    let messages = {
        let store = state.memory_store.lock().await;
        store.export_all_messages().map_err(AppError::from)?
    };
    let vectors = {
        let store = state.vector_store.lock().await;
        store.export_portable_chunks().map_err(AppError::from)?
    };
    Ok(serde_json::json!({
        "version": "2.0",
        "app_version": env!("CARGO_PKG_VERSION"),
        "exported_at": exported_at.to_rfc3339(),
        "vector_export_semantics": "portable_only_canonical_and_chat_projections_derived",
        "life_model": life_model,
        "state_store": state_store_archive,
        "messages": messages,
        "vectors": vectors,
    }))
}

#[tauri::command]
pub async fn import_all_data(
    payload: serde_json::Value,
    import_request: Option<GovernedDataImportRequest>,
    confirmation_evidence: Option<DangerActionConfirmationReference>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let request = require_governed_data_import_request(import_request.as_ref())?.clone();
    validate_import_payload_shape(&payload)?;
    validate_import_targets_cover_payload(&payload, &request)?;
    let payload_digest = hash_json_value(&payload)?;
    let request_digest = hash_serializable_value(&request)?;
    // Normal imports use the ordinary product effect gate. A restarted,
    // non-terminal import can reach only this same command and only after its
    // exact durable journal binding has minted an unforgeable recovery token.
    let recovery_journal = required_governed_data_import_journal(state.inner())?;
    let recovery_receipt = recovery_journal.recovery_requirement().map_err(|error| {
        AppError::db_with_hint(error.to_string(), "data_import_journal_unavailable")
    })?;
    let recovery_admission = if state
        .persistence_coordinator
        .require_effects_allowed()
        .is_ok()
    {
        None
    } else {
        let receipt = recovery_receipt.as_ref().ok_or_else(|| {
            AppError::db_with_hint(
                "persistence effects are blocked and no exact data-import recovery is pending",
                "read_only_degraded",
            )
        })?;
        Some(
            state
                .persistence_coordinator
                .mint_governed_data_import_recovery_admission(
                    &recovery_journal,
                    receipt,
                    &request.operation_id,
                    &payload_digest,
                    &request_digest,
                )
                .map_err(|error| {
                    AppError::db_with_hint(error.to_string(), "data_import_recovery_required")
                })?,
        )
    };
    let confirmation_arguments = serde_json::json!({
        "payload_digest": payload_digest,
        "governed_request": request,
    });
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "data_import_overwrite",
            target_ids_for_new_challenge: &[],
            requested_target: None,
            affected_count: None,
            reference: confirmation_evidence.as_ref(),
            arguments: &confirmation_arguments,
            arguments_summary:
                "覆盖导入已校验的 OpenLife 备份；参数已绑定到 payload digest 和 governed request。",
            governed_data_import_recovery: recovery_admission.as_ref(),
        },
        &window,
        state.inner(),
    )
    .await?;
    import_all_data_governed_operation(payload, state.inner(), &request).await
}

async fn observe_governed_import_owner_resolutions(
    state: &Arc<AppState>,
    receipt: &GovernedDataImportReceipt,
) -> Result<Vec<GovernedDataImportOwnerObservation>, AppError> {
    let mut resolutions = Vec::with_capacity(receipt.owners.len());
    for owner in &receipt.owners {
        let observed_at = chrono::Utc::now();
        let (
            observed_digest,
            state_restore_request_digest,
            state_restore_payload_digest,
            state_restore_before_canonical_digest,
            state_restore_after_canonical_digest,
            state_restore_outbox_event_id,
            state_projection_delivery_state,
        ) = match owner.import_target.as_str() {
            "life_model" => (
                current_lifemodel_owner_digest(state).await?,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            "messages" => (
                state
                    .memory_store
                    .lock()
                    .await
                    .export_canonical_message_archive()
                    .map_err(AppError::from)?
                    .digest,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            "vectors" => (
                state
                    .vector_store
                    .lock()
                    .await
                    .export_portable_archive()
                    .map_err(AppError::from)?
                    .digest,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            "state_store" => {
                let store = state.state_store.as_ref().ok_or_else(|| {
                    AppError::db_with_hint(
                        "StateStore unavailable while resolving governed import",
                        "canonical_state_unknown",
                    )
                })?;
                let observed = store
                    .export_portable_daily_tasks(receipt.created_at)
                    .map_err(|error| {
                        AppError::db_with_hint(error.to_string(), "canonical_state_unknown")
                    })?
                    .canonical_digest;
                let state_receipt = store
                    .portable_daily_task_restore_receipt(&receipt.operation_id, true)
                    .map_err(|error| {
                        AppError::db_with_hint(error.to_string(), "canonical_state_unknown")
                    })?;
                match state_receipt {
                    Some(state_receipt) => {
                        if state_receipt.request_digest != receipt.request_digest
                            || state_receipt.payload_digest != owner.target_digest
                            || state_receipt.before_canonical_digest != owner.before_digest
                            || state_receipt.committed_at != receipt.created_at
                        {
                            return Err(AppError::db_with_hint(
                                    "StateStore restore evidence is not bound to the exact governed import",
                                    "canonical_state_unknown",
                                ));
                        }
                        let delivery_state = store
                            .projection_delivery_state_for_event(&state_receipt.outbox_event_id)
                            .map_err(|error| {
                                AppError::db_with_hint(error.to_string(), "canonical_state_unknown")
                            })?;
                        (
                            observed,
                            Some(state_receipt.request_digest),
                            Some(state_receipt.payload_digest),
                            Some(state_receipt.before_canonical_digest),
                            Some(state_receipt.after_canonical_digest),
                            Some(state_receipt.outbox_event_id),
                            Some(delivery_state),
                        )
                    }
                    // A crash before the StateStore lane legitimately has
                    // no restore event. The before/current digest is still
                    // durable owner evidence; the journal core decides
                    // whether the absence is valid for this resolution.
                    None => (observed, None, None, None, None, None, None),
                }
            }
            unsupported => {
                return Err(AppError::db_with_hint(
                    format!("unsupported governed import owner target: {unsupported}"),
                    "canonical_state_unknown",
                ));
            }
        };
        resolutions.push(GovernedDataImportOwnerObservation {
            owner: owner.owner.clone(),
            observed_digest,
            observed_at,
            state_restore_request_digest,
            state_restore_payload_digest,
            state_restore_before_canonical_digest,
            state_restore_after_canonical_digest,
            state_restore_outbox_event_id,
            state_projection_delivery_state,
        });
    }
    Ok(resolutions)
}

fn governed_import_resolution_confirmation_facts(
    resolutions: &[GovernedDataImportOwnerResolution],
) -> Result<(String, String), AppError> {
    let mut before = 0usize;
    let mut target = 0usize;
    let mut other = 0usize;
    let mut owner_classifications = Vec::with_capacity(resolutions.len());
    let facts = resolutions
        .iter()
        .map(|resolution| {
            match resolution.classification {
                GovernedDataImportResolutionClassification::Before => before += 1,
                GovernedDataImportResolutionClassification::Target => target += 1,
                GovernedDataImportResolutionClassification::Other => other += 1,
            }
            let owner_label = match resolution.owner.as_str() {
                "LifeModelFileStore" => "LifeModel",
                "MemoryStore" => "Memory",
                "VectorStore" => "Vector",
                "StateStore" => "StateStore",
                _ => "UnknownOwner",
            };
            let classification_label = match resolution.classification {
                GovernedDataImportResolutionClassification::Before => "before",
                GovernedDataImportResolutionClassification::Target => "target",
                GovernedDataImportResolutionClassification::Other => "other",
            };
            owner_classifications.push(format!(
                "{owner_label}={classification_label}"
            ));
            serde_json::json!({
                "owner": resolution.owner,
                "observed_digest": resolution.observed_digest,
                "classification": resolution.classification,
                "state_restore_request_digest": resolution.state_restore_request_digest,
                "state_restore_payload_digest": resolution.state_restore_payload_digest,
                "state_restore_before_canonical_digest": resolution.state_restore_before_canonical_digest,
                "state_restore_after_canonical_digest": resolution.state_restore_after_canonical_digest,
                "state_restore_outbox_event_id": resolution.state_restore_outbox_event_id,
                "state_projection_delivery_state": resolution.state_projection_delivery_state,
            })
        })
        .collect::<Vec<_>>();
    Ok((
        hash_json_value(&serde_json::Value::Array(facts))?,
        format!(
            "owner 分类：{}；合计 before={before}, target={target}, other={other}",
            owner_classifications.join(", ")
        ),
    ))
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportResolutionCounts {
    pub before: usize,
    pub target: usize,
    pub other: usize,
}

fn governed_import_resolution_counts(
    receipt: &GovernedDataImportReceipt,
) -> GovernedDataImportResolutionCounts {
    let mut counts = GovernedDataImportResolutionCounts::default();
    for evidence in &receipt.resolution_evidence {
        match evidence.resolution.classification {
            GovernedDataImportResolutionClassification::Before => counts.before += 1,
            GovernedDataImportResolutionClassification::Target => counts.target += 1,
            GovernedDataImportResolutionClassification::Other => counts.other += 1,
        }
    }
    counts
}

fn governed_import_runtime_recovery_isolation_active(state: &Arc<AppState>) -> bool {
    state
        .persistence_coordinator
        .snapshot()
        .global_reason_codes
        .iter()
        .any(|reason| reason == GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON)
}

fn governed_import_abandonment_result(
    receipt: &GovernedDataImportReceipt,
    restart_required: bool,
) -> serde_json::Value {
    let counts = governed_import_resolution_counts(receipt);
    serde_json::json!({
        "success": true,
        "status": if restart_required {
            "abandoned_preserving_current_restart_required"
        } else {
            "abandoned_preserving_current"
        },
        "operation_id": receipt.operation_id,
        "stage": receipt.stage.as_str(),
        "recovery_terminalized": true,
        "original_import_completed": false,
        "rollback_completed": false,
        "preserved_current_canonical_data": true,
        "abandonment_mutated_canonical_owners": false,
        "original_import_effect_state": "preserved_current_observed_per_owner",
        "owner_resolution_counts": counts,
        "resolution_evidence_count": receipt.resolution_evidence.len(),
        "restart_required": restart_required,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportStatusView {
    pub status: String,
    pub operation_id: Option<String>,
    pub stage: Option<String>,
    pub terminal: bool,
    pub terminal_at: Option<String>,
    pub recovery_required: bool,
    pub runtime_recovery_isolation_active: bool,
    pub restart_required: bool,
    pub original_import_completed: bool,
    pub rollback_completed: bool,
    pub preserved_current: bool,
    pub owner_count: usize,
    pub resolution_evidence_count: usize,
    pub owner_resolution_counts: GovernedDataImportResolutionCounts,
    pub observed_at: String,
}

fn governed_data_import_status_view(
    receipt: Option<&GovernedDataImportReceipt>,
    isolation_active: bool,
    observed_at: String,
) -> GovernedDataImportStatusView {
    let Some(receipt) = receipt else {
        return GovernedDataImportStatusView {
            status: "idle".into(),
            operation_id: None,
            stage: None,
            terminal: false,
            terminal_at: None,
            recovery_required: false,
            runtime_recovery_isolation_active: isolation_active,
            restart_required: false,
            original_import_completed: false,
            rollback_completed: false,
            preserved_current: false,
            owner_count: 0,
            resolution_evidence_count: 0,
            owner_resolution_counts: GovernedDataImportResolutionCounts::default(),
            observed_at,
        };
    };
    let terminal = receipt.stage.is_terminal();
    GovernedDataImportStatusView {
        status: receipt.stage.as_str().into(),
        operation_id: Some(receipt.operation_id.clone()),
        stage: Some(receipt.stage.as_str().into()),
        terminal,
        terminal_at: receipt
            .terminal_at
            .as_ref()
            .map(chrono::DateTime::to_rfc3339),
        recovery_required: !terminal,
        runtime_recovery_isolation_active: isolation_active,
        restart_required: terminal && isolation_active,
        original_import_completed: receipt.stage == GovernedDataImportStage::Completed,
        rollback_completed: receipt.stage == GovernedDataImportStage::Compensated,
        preserved_current: receipt.stage == GovernedDataImportStage::AbandonedPreservingCurrent,
        owner_count: receipt.owners.len(),
        resolution_evidence_count: receipt.resolution_evidence.len(),
        owner_resolution_counts: governed_import_resolution_counts(receipt),
        observed_at,
    }
}

/// Bounded, metadata-only durable status. This makes an abandonment visible
/// after an IPC response loss and clean restart without recreating a ledger or
/// exposing import bodies/digests to the webview.
#[tauri::command]
pub async fn get_governed_data_import_status(
    state: State<'_, Arc<AppState>>,
) -> Result<GovernedDataImportStatusView, AppError> {
    let journal = required_governed_data_import_journal(state.inner())?;
    let receipt = journal.latest_receipt().map_err(|error| {
        AppError::db_with_hint(error.to_string(), "data_import_journal_unavailable")
    })?;
    let isolation_active = governed_import_runtime_recovery_isolation_active(state.inner());
    Ok(governed_data_import_status_view(
        receipt.as_ref(),
        isolation_active,
        chrono::Utc::now().to_rfc3339(),
    ))
}

/// Payload-independent recovery exit. It changes no canonical owner and does
/// not claim that the interrupted import completed or rolled back. The only
/// durable write is metadata-only resolution evidence plus the journal's
/// explicit `abandoned_preserving_current` terminal state.
#[tauri::command]
pub async fn abandon_governed_data_import_recovery(
    operation_id: String,
    confirmation_evidence: Option<DangerActionConfirmationReference>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let journal = required_governed_data_import_journal(state.inner())?;
    if let Some(terminal) = journal.terminal_receipt(&operation_id).map_err(|error| {
        AppError::db_with_hint(error.to_string(), "data_import_journal_unavailable")
    })? {
        if terminal.stage == GovernedDataImportStage::AbandonedPreservingCurrent {
            let restart_required = governed_import_runtime_recovery_isolation_active(state.inner());
            return Ok(governed_import_abandonment_result(
                &terminal,
                restart_required,
            ));
        }
        return Err(AppError::permission(
            "governed data-import operation is already terminal and cannot be abandoned",
        ));
    }
    let confirmed_receipt = journal
        .recovery_requirement()
        .map_err(|error| {
            AppError::db_with_hint(error.to_string(), "data_import_journal_unavailable")
        })?
        .ok_or_else(|| {
            AppError::permission("no governed data-import recovery is pending for abandonment")
        })?;
    if confirmed_receipt.operation_id != operation_id {
        return Err(AppError::permission(
            "governed data-import abandonment operation does not match durable recovery truth",
        ));
    }
    let recovery_admission = state
        .persistence_coordinator
        .mint_governed_data_import_recovery_admission(
            &journal,
            &confirmed_receipt,
            &confirmed_receipt.operation_id,
            &confirmed_receipt.payload_digest,
            &confirmed_receipt.request_digest,
        )
        .map_err(|error| {
            AppError::db_with_hint(error.to_string(), "data_import_recovery_required")
        })?;
    let confirmed_observations =
        observe_governed_import_owner_resolutions(state.inner(), &confirmed_receipt).await?;
    let confirmed_resolutions = journal
        .preview_abandonment_resolutions(&confirmed_receipt.operation_id, &confirmed_observations)
        .map_err(|error| {
            AppError::db_with_hint(error.to_string(), "data_import_recovery_required")
        })?;
    let (confirmed_resolution_digest, confirmed_resolution_summary) =
        governed_import_resolution_confirmation_facts(&confirmed_resolutions)?;
    let target_ids = vec![operation_id.clone()];
    let confirmation_arguments = serde_json::json!({
        "operation_id": operation_id.clone(),
        "payload_digest": confirmed_receipt.payload_digest.clone(),
        "request_digest": confirmed_receipt.request_digest.clone(),
        "observed_stage": confirmed_receipt.stage.as_str(),
        "disposition": "abandoned_preserving_current",
        "owner_resolution_facts_digest": confirmed_resolution_digest,
        "owner_resolution_summary": confirmed_resolution_summary,
    });
    let arguments_summary = format!(
        "保留当前 canonical 数据并终止中断的导入；{}；观测证据已绑定到本次系统确认，完成后必须重启。",
        confirmed_resolution_summary
    );
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "data_import_abandon_recovery",
            target_ids_for_new_challenge: &target_ids,
            requested_target: Some(target_ids[0].as_str()),
            affected_count: Some(confirmed_receipt.owners.len()),
            reference: confirmation_evidence.as_ref(),
            arguments: &confirmation_arguments,
            arguments_summary: &arguments_summary,
            governed_data_import_recovery: Some(&recovery_admission),
        },
        &window,
        state.inner(),
    )
    .await?;

    let _import_guard = GOVERNED_DATA_IMPORT_LOCK.lock().await;
    let _resolution_fence = state
        .persistence_coordinator
        .acquire_governed_data_import_resolution_fence(&journal, &confirmed_receipt)
        .await
        .map_err(|error| {
            AppError::db_with_hint(error.to_string(), "data_import_recovery_required")
        })?;
    let current = journal
        .recovery_requirement()
        .map_err(|error| {
            AppError::db_with_hint(error.to_string(), "data_import_journal_unavailable")
        })?
        .ok_or_else(|| {
            AppError::permission(
                "governed data-import recovery changed after confirmation; run a fresh preflight",
            )
        })?;
    if current != confirmed_receipt {
        return Err(AppError::permission(
            "governed data-import recovery changed after confirmation; run a fresh preflight",
        ));
    }
    let observations = observe_governed_import_owner_resolutions(state.inner(), &current).await?;
    let current_resolutions = journal
        .preview_abandonment_resolutions(&current.operation_id, &observations)
        .map_err(|error| {
            AppError::db_with_hint(error.to_string(), "data_import_recovery_required")
        })?;
    let (current_resolution_digest, _) =
        governed_import_resolution_confirmation_facts(&current_resolutions)?;
    if current_resolution_digest != confirmed_resolution_digest {
        return Err(AppError::permission(
            "canonical owner facts changed after confirmation; run a fresh abandonment preflight",
        ));
    }
    let reason_digest = metadata_digest("explicit native-confirmed preserve-current abandonment");
    let terminal = journal
        .abandon_preserving_current(&current.operation_id, &observations, &reason_digest)
        .map_err(|error| {
            mark_governed_import_recovery_required(state.inner());
            AppError::db_with_hint(error.to_string(), "data_import_recovery_required")
        })?;
    // The coordinator is intentionally monotonic for the life of this
    // process. A clean bootstrap must re-read the now-terminal journal before
    // provider, tool, or canonical effects resume.
    mark_governed_import_recovery_required(state.inner());
    Ok(governed_import_abandonment_result(&terminal, true))
}

#[cfg(test)]
async fn import_all_data_with_state(
    payload: serde_json::Value,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    import_all_data_with_state_gated(payload, state, None).await
}

#[cfg(test)]
async fn import_all_data_with_state_for_governed_import(
    payload: serde_json::Value,
    state: &Arc<AppState>,
    import_request: GovernedDataImportRequest,
) -> Result<serde_json::Value, AppError> {
    import_all_data_with_state_gated(payload, state, Some(import_request)).await
}

#[cfg(test)]
async fn import_all_data_with_state_gated(
    payload: serde_json::Value,
    state: &Arc<AppState>,
    import_request: Option<GovernedDataImportRequest>,
) -> Result<serde_json::Value, AppError> {
    let request = require_governed_data_import_request(import_request.as_ref())?;
    import_all_data_governed_operation(payload, state, request).await
}

fn governed_import_lifemodel_owner_digest(model: &LifeModel) -> Result<String, AppError> {
    let mut owner_view = model.clone();
    owner_view.goals.daily.clear();
    owner_view.state.alerts.clear();
    life_model_write_gateway::hash_life_model(&owner_view).map_err(AppError::from)
}

fn governed_import_owner<'a>(
    receipt: &'a GovernedDataImportReceipt,
    import_target: &str,
) -> Result<&'a GovernedDataImportOwnerReceipt, AppError> {
    receipt
        .owners
        .iter()
        .find(|owner| owner.import_target == import_target)
        .ok_or_else(|| {
            AppError::internal(format!(
                "governed data-import journal is missing owner for {import_target}"
            ))
        })
}

fn owner_applied_status(owner: &GovernedDataImportOwnerReceipt) -> GovernedDataImportOwnerStatus {
    if owner.before_digest == owner.target_digest {
        GovernedDataImportOwnerStatus::Skipped
    } else {
        GovernedDataImportOwnerStatus::Applied
    }
}

fn state_restore_owner_applied_status(
    receipt: &openlife_core::state_store::PortableDailyTaskRestoreReceipt,
) -> GovernedDataImportOwnerStatus {
    if receipt.before_canonical_digest == receipt.after_canonical_digest {
        GovernedDataImportOwnerStatus::Skipped
    } else {
        GovernedDataImportOwnerStatus::Applied
    }
}

fn mark_governed_import_recovery_required(state: &Arc<AppState>) {
    state
        .persistence_coordinator
        .degrade_globally(GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON);
}

fn transition_governed_import(
    journal: &GovernedDataImportJournal,
    state: &Arc<AppState>,
    operation_id: &str,
    stage: GovernedDataImportStage,
    updates: &[GovernedDataImportOwnerUpdate],
    evidence: Option<&str>,
) -> Result<GovernedDataImportReceipt, AppError> {
    journal
        .transition(operation_id, stage, updates, evidence)
        .map_err(|error| {
            mark_governed_import_recovery_required(state);
            AppError::db_with_hint(
                format!("governed data-import journal transition failed: {error}"),
                "data_import_recovery_required",
            )
        })
}

async fn current_lifemodel_owner_digest(state: &Arc<AppState>) -> Result<String, AppError> {
    let current = state
        .life_model_manager
        .lock()
        .await
        .load()
        .map_err(AppError::from)?;
    governed_import_lifemodel_owner_digest(&current)
}

async fn apply_imported_lifemodel(
    state: &Arc<AppState>,
    model: &LifeModel,
    expected_physical_hash: &str,
    recovery: Option<(&GovernedDataImportRecoveryAdmission<'_>, &str, &str, &str)>,
) -> Result<(), AppError> {
    let caller = LifeModelMaterializerCallerContext::new(
        "data_import_governed_operation",
        LifeModelMaterializerCallerKind::GovernedRestoreImportOperation,
        LifeModelMaterializerCallerPurpose::GovernedRestoreImportOperation,
    );
    if let Some(recovery) = recovery {
        life_model_write_gateway::restore_life_model_with_gateway_for_import_recovery(
            state,
            model,
            caller,
            Some(expected_physical_hash),
            recovery,
        )
        .await
    } else {
        life_model_write_gateway::restore_life_model_with_gateway(
            state,
            model,
            caller,
            Some(expected_physical_hash),
        )
        .await
    }
}

async fn replace_import_owner_memory(
    state: &Arc<AppState>,
    messages: Option<&[openlife_core::memory::ExportedMessage]>,
    vectors: Option<&[openlife_core::vectors::ExportedVectorChunk]>,
    expected: memory_gateway::ImportedMemoryExpectedDigests,
    recovery: Option<(&GovernedDataImportRecoveryAdmission<'_>, &str, &str, &str)>,
) -> Result<memory_gateway::ImportedMemoryReplaceReport, AppError> {
    if let Some(recovery) = recovery {
        memory_gateway::replace_imported_memory_with_state_guarded_for_import_recovery(
            state, messages, vectors, expected, recovery,
        )
        .await
    } else {
        memory_gateway::replace_imported_memory_with_state_guarded(
            state, messages, vectors, expected,
        )
        .await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GovernedImportFault {
    None,
    #[cfg(test)]
    AfterSnapshotBeforeJournalPrepare,
    #[cfg(test)]
    AfterLifeModelCommitBeforeJournal,
    #[cfg(test)]
    AfterMemoryCommitBeforeJournal,
    #[cfg(test)]
    AfterMemoryCommitWithLateDrift,
    #[cfg(test)]
    AfterVectorCommitBeforeJournal,
    #[cfg(test)]
    AfterStateCommitBeforeJournal,
    #[cfg(test)]
    BeforeTerminalVerificationWithMemoryDrift,
}

async fn import_all_data_governed_operation(
    payload: serde_json::Value,
    state: &Arc<AppState>,
    request: &GovernedDataImportRequest,
) -> Result<serde_json::Value, AppError> {
    import_all_data_governed_operation_with_fault(
        payload,
        state,
        request,
        GovernedImportFault::None,
    )
    .await
}

async fn import_all_data_governed_operation_with_fault(
    payload: serde_json::Value,
    state: &Arc<AppState>,
    request: &GovernedDataImportRequest,
    fault: GovernedImportFault,
) -> Result<serde_json::Value, AppError> {
    // Production always passes `None`; tests use the closed variants below to
    // prove recovery at owner-commit / journal-transition crash boundaries.
    let _ = fault;
    validate_import_payload_shape(&payload)?;
    validate_import_targets_cover_payload(&payload, request)?;
    let _import_guard = GOVERNED_DATA_IMPORT_LOCK.lock().await;
    let import_payload_hash = hash_json_value(&payload)?;
    let request_digest = hash_serializable_value(request)?;
    let backup_version = payload
        .get("version")
        .and_then(serde_json::Value::as_str)
        .expect("validated backup version");
    let mut imported_model: LifeModel = serde_json::from_value(
        payload
            .get("life_model")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|error| AppError::external(format!("解析 life_model 失败: {error}")))?;
    let state_store_archive: Option<openlife_core::state_store::PortableDailyTaskArchiveV1> =
        payload
            .get("state_store")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| {
                AppError::external(format!("解析 state_store portable archive 失败: {error}"))
            })?;
    match backup_version {
        "1.0" if !imported_model.goals.daily.is_empty() => {
            return Err(AppError::external(
                "OpenLife v1 backup contains daily tasks without canonical expiry metadata; the daily-task category is quarantined instead of inventing a TTL. Re-export with OpenLife backup v2 before restoring this category.",
            ));
        }
        "2.0"
            if !imported_model.goals.daily.is_empty()
                || !imported_model.state.alerts.is_empty() =>
        {
            return Err(AppError::permission(
                "OpenLife v2 backup duplicates StateStore-owned derived fields inside life_model",
            ));
        }
        _ => {}
    }
    let messages: Option<Vec<openlife_core::memory::ExportedMessage>> = payload
        .get("messages")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| AppError::external(format!("解析 messages 失败: {error}")))?;
    let vectors: Option<Vec<openlife_core::vectors::ExportedVectorChunk>> = payload
        .get("vectors")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| AppError::external(format!("解析 vectors 失败: {error}")))?;

    let previous_model = state
        .life_model_manager
        .lock()
        .await
        .load()
        .map_err(AppError::from)?;
    imported_model.goals.daily = previous_model.goals.daily.clone();
    imported_model.state.alerts = previous_model.state.alerts.clone();
    let previous_model_physical_hash =
        life_model_write_gateway::hash_life_model(&previous_model).map_err(AppError::from)?;
    let imported_model_physical_hash =
        life_model_write_gateway::hash_life_model(&imported_model).map_err(AppError::from)?;
    let current_model_owner_digest = governed_import_lifemodel_owner_digest(&previous_model)?;
    let target_model_owner_digest = governed_import_lifemodel_owner_digest(&imported_model)?;
    let previous_messages = if messages.is_some() {
        Some(
            state
                .memory_store
                .lock()
                .await
                .export_canonical_message_archive()
                .map_err(AppError::from)?,
        )
    } else {
        None
    };
    let previous_vectors = if vectors.is_some() {
        Some(
            state
                .vector_store
                .lock()
                .await
                .export_portable_archive()
                .map_err(AppError::from)?,
        )
    } else {
        None
    };
    let state_store = if state_store_archive.is_some() {
        Some(state.state_store.as_ref().ok_or_else(|| {
            AppError::db_with_hint(
                "StateStore is unavailable; governed v2 restore cannot continue",
                "canonical_state_unknown",
            )
        })?)
    } else {
        None
    };
    let previous_state_archive = match state_store {
        Some(store) => Some(
            store
                .export_portable_daily_tasks(chrono::Utc::now())
                .map_err(|error| {
                    AppError::db_with_hint(error.to_string(), "canonical_state_unknown")
                })?,
        ),
        None => None,
    };

    let journal = required_governed_data_import_journal(state)?;
    let existing = journal.receipt(&request.operation_id).map_err(|error| {
        AppError::db_with_hint(error.to_string(), "data_import_journal_unavailable")
    })?;
    if existing.is_none() {
        state
            .persistence_coordinator
            .require_effects_allowed()
            .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    }
    // The snapshot must durably precede the saga receipt. Therefore any
    // existing receipt proves that its pre-change snapshot completed before a
    // canonical owner could have been touched. A crash after the idempotent
    // snapshot and before `prepare` leaves no saga and is safe to retry.
    let pre_import_snapshot_version = if existing.is_some() {
        None
    } else {
        Some(
            state
                .version_manager
                .lock()
                .await
                .ensure_projection_snapshot(
                    &previous_model,
                    &format!(
                        "pre-change:import:{import_payload_hash}:{previous_model_physical_hash}"
                    ),
                    "auto:pre-import",
                    "导入覆盖之前自动备份",
                )
                .map_err(AppError::from)?
                .version,
        )
    };
    #[cfg(test)]
    if fault == GovernedImportFault::AfterSnapshotBeforeJournalPrepare {
        return Err(AppError::internal(
            "injected crash after pre-import snapshot before journal prepare",
        ));
    }
    let before_digest = |target: &str, observed: &str| -> Result<String, AppError> {
        match existing.as_ref() {
            Some(receipt) => Ok(governed_import_owner(receipt, target)?
                .before_digest
                .clone()),
            None => Ok(observed.to_string()),
        }
    };
    let mut owners = vec![GovernedDataImportOwnerPlan {
        owner: "LifeModelFileStore".into(),
        import_target: "life_model".into(),
        before_digest: before_digest("life_model", &current_model_owner_digest)?,
        target_digest: target_model_owner_digest.clone(),
        item_count: 1,
    }];
    if let (Some(messages), Some(previous)) = (messages.as_ref(), previous_messages.as_ref()) {
        owners.push(GovernedDataImportOwnerPlan {
            owner: "MemoryStore".into(),
            import_target: "messages".into(),
            before_digest: before_digest("messages", &previous.digest)?,
            target_digest: openlife_core::memory::canonical_message_archive_digest(messages),
            item_count: u64::try_from(messages.len())
                .map_err(|_| AppError::internal("message import count exceeds u64"))?,
        });
    }
    if let (Some(vectors), Some(previous)) = (vectors.as_ref(), previous_vectors.as_ref()) {
        owners.push(GovernedDataImportOwnerPlan {
            owner: "VectorStore".into(),
            import_target: "vectors".into(),
            before_digest: before_digest("vectors", &previous.digest)?,
            target_digest: openlife_core::vectors::portable_vector_archive_digest(vectors),
            item_count: u64::try_from(vectors.len())
                .map_err(|_| AppError::internal("vector import count exceeds u64"))?,
        });
    }
    if let (Some(archive), Some(previous)) = (
        state_store_archive.as_ref(),
        previous_state_archive.as_ref(),
    ) {
        owners.push(GovernedDataImportOwnerPlan {
            owner: "StateStore".into(),
            import_target: "state_store".into(),
            before_digest: before_digest("state_store", &previous.canonical_digest)?,
            // StateStore creates fresh canonical asset ids, so its pre-write
            // target identity is the validated portable payload. Commit truth
            // is proven later by the owner receipt and its after digest.
            target_digest: archive.payload_digest.clone(),
            item_count: u64::try_from(archive.daily_tasks.len())
                .map_err(|_| AppError::internal("StateStore import count exceeds u64"))?,
        });
    }
    let prepared = journal
        .prepare(GovernedDataImportPrepare {
            operation_id: request.operation_id.clone(),
            payload_digest: import_payload_hash.clone(),
            request_digest: request_digest.clone(),
            owners,
        })
        .map_err(|error| {
            AppError::db_with_hint(error.to_string(), "data_import_journal_unavailable")
        })?;
    let mut saga = prepared.receipt;
    if prepared.replayed && saga.stage == GovernedDataImportStage::Compensated {
        return Err(AppError::permission(
            "this governed import operation was compensated; retry requires a new operationId",
        ));
    }
    if prepared.replayed && saga.stage == GovernedDataImportStage::AbandonedPreservingCurrent {
        return Err(AppError::permission(
            "this governed import operation was abandoned while preserving current data; a new import requires a new operationId",
        ));
    }
    let recovery_admission = if state
        .persistence_coordinator
        .require_effects_allowed()
        .is_ok()
    {
        None
    } else {
        Some(
            state
                .persistence_coordinator
                .mint_governed_data_import_recovery_admission(
                    &journal,
                    &saga,
                    &request.operation_id,
                    &import_payload_hash,
                    &request_digest,
                )
                .map_err(|error| {
                    AppError::db_with_hint(error.to_string(), "data_import_recovery_required")
                })?,
        )
    };
    let recovery_binding = recovery_admission.as_ref().map(|token| {
        (
            token,
            request.operation_id.as_str(),
            import_payload_hash.as_str(),
            request_digest.as_str(),
        )
    });

    let state_restore_command = match (state_store, state_store_archive.as_ref()) {
        (Some(store), Some(archive)) => {
            let owner = governed_import_owner(&saga, "state_store")?;
            let command = openlife_core::state_store::RestorePortableDailyTasksCommand {
                operation_id: request.operation_id.clone(),
                request_digest: request_digest.clone(),
                expected_before_digest: owner.before_digest.clone(),
                archive: archive.clone(),
                // This durable timestamp is stable across restart and is also
                // bound by the StateStore owner receipt.
                restored_at: saga.created_at,
            };
            if let Err(error) = store.preflight_portable_daily_task_restore(&command) {
                if !prepared.replayed {
                    let updates = saga
                        .owners
                        .iter()
                        .map(|owner| GovernedDataImportOwnerUpdate {
                            owner: owner.owner.clone(),
                            status: GovernedDataImportOwnerStatus::Skipped,
                        })
                        .collect::<Vec<_>>();
                    let _ = transition_governed_import(
                        &journal,
                        state,
                        &request.operation_id,
                        GovernedDataImportStage::Compensated,
                        &updates,
                        Some(&metadata_digest(
                            "state restore preflight rejected before effects",
                        )),
                    );
                } else {
                    mark_governed_import_recovery_required(state);
                }
                return Err(AppError::external(format!(
                    "StateStore restore payload preflight failed before product write: {error}"
                )));
            }
            Some(command)
        }
        _ => None,
    };

    if saga.stage == GovernedDataImportStage::Completed {
        let durable_lifemodel_write = {
            let owner = governed_import_owner(&saga, "life_model")?;
            owner.before_digest != owner.target_digest
        };
        return governed_import_result(
            state,
            request,
            &import_payload_hash,
            &previous_model_physical_hash,
            &imported_model_physical_hash,
            messages.as_deref(),
            vectors.as_deref(),
            None,
            None,
            state_store,
            true,
            pre_import_snapshot_version,
            recovery_admission.is_some(),
            durable_lifemodel_write,
            None,
        )
        .await;
    }
    if saga.stage == GovernedDataImportStage::CompensationUnknown {
        return abandon_governed_import_after_exact_observation(
            state,
            &journal,
            &saga,
            "resumed import already had an unknown owner state",
        )
        .await;
    }

    // Prepared -> LifeModelApplied. Recovery observes before/target first, so a
    // crash after the owner commit but before the journal transition is a
    // replay, not a duplicate write.
    if saga.stage == GovernedDataImportStage::Prepared {
        let owner = governed_import_owner(&saga, "life_model")?.clone();
        let observed = current_lifemodel_owner_digest(state).await?;
        if observed == owner.before_digest && observed != owner.target_digest {
            let current_physical = current_lifemodel_file_hash(state).await?;
            let apply = apply_imported_lifemodel(
                state,
                &imported_model,
                &current_physical,
                recovery_binding,
            )
            .await;
            let after = current_lifemodel_owner_digest(state).await?;
            if after != owner.target_digest {
                let import_error = apply
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "LifeModel owner digest mismatch after restore".into());
                if after != owner.before_digest {
                    return mark_governed_import_owner_unknown(
                        state,
                        &journal,
                        &saga,
                        &owner.owner,
                        "LifeModel changed after restore before its journal transition",
                    )
                    .await;
                }
                return fail_or_compensate_governed_import(
                    state,
                    &journal,
                    &saga,
                    prepared.replayed,
                    &previous_model,
                    previous_messages.as_ref(),
                    previous_vectors.as_ref(),
                    &imported_model_physical_hash,
                    import_error,
                )
                .await;
            }
        } else if observed != owner.target_digest {
            return mark_governed_import_owner_unknown(
                state,
                &journal,
                &saga,
                &owner.owner,
                "LifeModel owner is neither journal before nor target digest",
            )
            .await;
        }
        #[cfg(test)]
        if fault == GovernedImportFault::AfterLifeModelCommitBeforeJournal {
            return Err(AppError::internal(
                "injected crash after LifeModel commit before journal transition",
            ));
        }
        saga = transition_governed_import(
            &journal,
            state,
            &request.operation_id,
            GovernedDataImportStage::LifeModelApplied,
            &[GovernedDataImportOwnerUpdate {
                owner: owner.owner.clone(),
                status: owner_applied_status(&owner),
            }],
            None,
        )?;
    }

    let mut message_report = None;
    if saga.stage == GovernedDataImportStage::LifeModelApplied {
        let update = if let Some(messages) = messages.as_deref() {
            let owner = governed_import_owner(&saga, "messages")?.clone();
            let observed = state
                .memory_store
                .lock()
                .await
                .export_canonical_message_archive()
                .map_err(AppError::from)?
                .digest;
            if observed == owner.before_digest && observed != owner.target_digest {
                let result = replace_import_owner_memory(
                    state,
                    Some(messages),
                    None,
                    memory_gateway::ImportedMemoryExpectedDigests {
                        messages: Some(owner.before_digest.clone()),
                        vectors: None,
                    },
                    recovery_binding,
                )
                .await;
                #[cfg(test)]
                if fault == GovernedImportFault::AfterMemoryCommitWithLateDrift {
                    tests::inject_governed_import_memory_drift(
                        state,
                        "governed-import-late-drift",
                        "LATE_MEMORY_WRITE_MUST_NOT_BE_COMPENSATED_AWAY",
                    )
                    .await;
                }
                let after = state
                    .memory_store
                    .lock()
                    .await
                    .export_canonical_message_archive()
                    .map_err(AppError::from)?
                    .digest;
                if after != owner.target_digest {
                    let import_error = result
                        .err()
                        .map(|error| error.to_string())
                        .unwrap_or_else(|| "Memory owner digest mismatch after replace".into());
                    if after != owner.before_digest {
                        return mark_governed_import_owner_unknown(
                            state,
                            &journal,
                            &saga,
                            &owner.owner,
                            "Memory changed after replace before its journal transition",
                        )
                        .await;
                    }
                    return fail_or_compensate_governed_import(
                        state,
                        &journal,
                        &saga,
                        prepared.replayed,
                        &previous_model,
                        previous_messages.as_ref(),
                        previous_vectors.as_ref(),
                        &imported_model_physical_hash,
                        import_error,
                    )
                    .await;
                }
                message_report = result.ok();
            } else if observed != owner.target_digest {
                return mark_governed_import_owner_unknown(
                    state,
                    &journal,
                    &saga,
                    &owner.owner,
                    "Memory owner is neither journal before nor target digest",
                )
                .await;
            }
            vec![GovernedDataImportOwnerUpdate {
                owner: owner.owner.clone(),
                status: owner_applied_status(&owner),
            }]
        } else {
            Vec::new()
        };
        #[cfg(test)]
        if fault == GovernedImportFault::AfterMemoryCommitBeforeJournal {
            return Err(AppError::internal(
                "injected crash after Memory commit before journal transition",
            ));
        }
        saga = transition_governed_import(
            &journal,
            state,
            &request.operation_id,
            GovernedDataImportStage::MemoryApplied,
            &update,
            None,
        )?;
    }

    let mut vector_report = None;
    if saga.stage == GovernedDataImportStage::MemoryApplied {
        let update = if let Some(vectors) = vectors.as_deref() {
            let owner = governed_import_owner(&saga, "vectors")?.clone();
            let observed = state
                .vector_store
                .lock()
                .await
                .export_portable_archive()
                .map_err(AppError::from)?
                .digest;
            if observed == owner.before_digest && observed != owner.target_digest {
                let result = replace_import_owner_memory(
                    state,
                    None,
                    Some(vectors),
                    memory_gateway::ImportedMemoryExpectedDigests {
                        messages: None,
                        vectors: Some(owner.before_digest.clone()),
                    },
                    recovery_binding,
                )
                .await;
                let after = state
                    .vector_store
                    .lock()
                    .await
                    .export_portable_archive()
                    .map_err(AppError::from)?
                    .digest;
                if after != owner.target_digest {
                    let import_error = result
                        .err()
                        .map(|error| error.to_string())
                        .unwrap_or_else(|| "Vector owner digest mismatch after replace".into());
                    if after != owner.before_digest {
                        return mark_governed_import_owner_unknown(
                            state,
                            &journal,
                            &saga,
                            &owner.owner,
                            "Vector changed after replace before its journal transition",
                        )
                        .await;
                    }
                    return fail_or_compensate_governed_import(
                        state,
                        &journal,
                        &saga,
                        prepared.replayed,
                        &previous_model,
                        previous_messages.as_ref(),
                        previous_vectors.as_ref(),
                        &imported_model_physical_hash,
                        import_error,
                    )
                    .await;
                }
                vector_report = result.ok();
            } else if observed != owner.target_digest {
                return mark_governed_import_owner_unknown(
                    state,
                    &journal,
                    &saga,
                    &owner.owner,
                    "Vector owner is neither journal before nor target digest",
                )
                .await;
            }
            vec![GovernedDataImportOwnerUpdate {
                owner: owner.owner.clone(),
                status: owner_applied_status(&owner),
            }]
        } else {
            Vec::new()
        };
        #[cfg(test)]
        if fault == GovernedImportFault::AfterVectorCommitBeforeJournal {
            return Err(AppError::internal(
                "injected crash after Vector commit before journal transition",
            ));
        }
        saga = transition_governed_import(
            &journal,
            state,
            &request.operation_id,
            GovernedDataImportStage::VectorApplied,
            &update,
            None,
        )?;
    }

    let mut state_restore_receipt = None;
    if saga.stage == GovernedDataImportStage::VectorApplied {
        let update = if let (Some(store), Some(command)) =
            (state_store, state_restore_command.clone())
        {
            let owner = governed_import_owner(&saga, "state_store")?.clone();
            let state_write_admission = state
                .persistence_coordinator
                .require_normal_or_governed_data_import_write(
                    GovernedDataImportRecoveryOwner::StateStore,
                    recovery_admission.as_ref(),
                    &request.operation_id,
                    &import_payload_hash,
                    &request_digest,
                )
                .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
            let state_commit_permit = state
                .persistence_coordinator
                .acquire_canonical_commit_permit(&state_write_admission)
                .await
                .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
            let result = store.restore_portable_daily_tasks(command.clone());
            drop(state_commit_permit);
            let receipt = match result {
                Ok(receipt) => receipt,
                Err(error) => {
                    if let Some(receipt) = store
                        .portable_daily_task_restore_receipt(&request.operation_id, true)
                        .map_err(AppError::from)?
                    {
                        receipt
                    } else {
                        let current = store
                            .export_portable_daily_tasks(saga.created_at)
                            .map_err(AppError::from)?;
                        if current.canonical_digest == owner.before_digest && !prepared.replayed {
                            return fail_or_compensate_governed_import(
                                state,
                                &journal,
                                &saga,
                                false,
                                &previous_model,
                                previous_messages.as_ref(),
                                previous_vectors.as_ref(),
                                &imported_model_physical_hash,
                                error.to_string(),
                            )
                            .await;
                        }
                        return mark_governed_import_owner_unknown(
                            state,
                            &journal,
                            &saga,
                            &owner.owner,
                            "StateStore commit result is unknown and no exact owner receipt exists",
                        )
                        .await;
                    }
                }
            };
            if receipt.request_digest != request_digest
                || receipt.payload_digest != owner.target_digest
                || receipt.before_canonical_digest != owner.before_digest
                || receipt.committed_at != saga.created_at
            {
                return mark_governed_import_owner_unknown(
                    state,
                    &journal,
                    &saga,
                    &owner.owner,
                    "StateStore restore receipt binding mismatch",
                )
                .await;
            }
            let current = store
                .export_portable_daily_tasks(saga.created_at)
                .map_err(AppError::from)?;
            if current.canonical_digest != receipt.after_canonical_digest {
                return mark_governed_import_owner_unknown(
                    state,
                    &journal,
                    &saga,
                    &owner.owner,
                    "StateStore current digest does not match committed restore receipt",
                )
                .await;
            }
            let owner_status = state_restore_owner_applied_status(&receipt);
            state_restore_receipt = Some(receipt);
            vec![GovernedDataImportOwnerUpdate {
                owner: owner.owner.clone(),
                status: owner_status,
            }]
        } else {
            Vec::new()
        };
        #[cfg(test)]
        if fault == GovernedImportFault::AfterStateCommitBeforeJournal {
            return Err(AppError::internal(
                "injected crash after StateStore commit before journal transition",
            ));
        }
        saga = transition_governed_import(
            &journal,
            state,
            &request.operation_id,
            GovernedDataImportStage::StateCommitted,
            &update,
            None,
        )?;
    }

    if let (Some(state_store), None) = (state_store.as_ref(), state_restore_receipt.as_ref()) {
        state_restore_receipt = state_store
            .portable_daily_task_restore_receipt(&request.operation_id, prepared.replayed)
            .map_err(AppError::from)?;
    }
    let mut projection_evidence = None;
    if matches!(
        saga.stage,
        GovernedDataImportStage::StateCommitted | GovernedDataImportStage::ProjectionDegraded
    ) {
        if let Some(receipt) = state_restore_receipt.as_ref() {
            let projection = if let Some(binding) = recovery_binding {
                crate::state_projection::reconcile_state_store_lifemodel_projection_for_import_recovery_event(
                    state,
                    &receipt.outbox_event_id,
                    binding,
                )
                .await
            } else {
                crate::state_projection::reconcile_state_store_lifemodel_projection_for_event(
                    state,
                    &receipt.outbox_event_id,
                )
                .await
            };
            match projection {
                Ok(report)
                    if report.status
                        == openlife_core::state_store::StateProjectionStatus::Applied
                        && report.required_event.as_ref().is_some_and(|proof| {
                            proof.event_id == receipt.outbox_event_id
                                && proof.delivery_state == ProjectionDeliveryState::Applied
                        }) =>
                {
                    projection_evidence = Some(metadata_digest(&format!(
                        "state_projection_applied:{}:{}:{}",
                        receipt.receipt_id, receipt.outbox_event_id, report.delivery_count
                    )));
                }
                Err(error) if error.is_deferred() => {
                    // The exact StateStore delivery remains pending. Preserve
                    // that fact and require an exact retry; terminalization
                    // contention is not a durable projection failure.
                    mark_governed_import_recovery_required(state);
                    return Err(AppError::db_with_hint(
                        format!(
                            "canonical owners committed but StateStore projection finalization was deferred; operation={} stage={}; {error}",
                            saga.operation_id,
                            saga.stage.as_str()
                        ),
                        "data_import_recovery_required",
                    ));
                }
                other => {
                    let reason = metadata_digest(&format!(
                        "state_projection_recovery_required:{}",
                        match other {
                            Ok(report) => format!("{:?}", report.status),
                            Err(error) => error.to_string(),
                        }
                    ));
                    if saga.stage == GovernedDataImportStage::StateCommitted {
                        saga = transition_governed_import(
                            &journal,
                            state,
                            &request.operation_id,
                            GovernedDataImportStage::ProjectionDegraded,
                            &[],
                            Some(&reason),
                        )?;
                    }
                    mark_governed_import_recovery_required(state);
                    return Err(AppError::db_with_hint(
                        format!(
                            "canonical owners committed but projection recovery is required; operation={} stage={}",
                            saga.operation_id,
                            saga.stage.as_str()
                        ),
                        "data_import_recovery_required",
                    ));
                }
            }
        }
    }

    #[cfg(test)]
    if fault == GovernedImportFault::BeforeTerminalVerificationWithMemoryDrift {
        tests::inject_governed_import_memory_drift(
            state,
            "governed-import-terminal-drift",
            "LATE_TERMINAL_MEMORY_WRITE_MUST_REMAIN_CANONICAL",
        )
        .await;
    }
    let completion_fence = state
        .persistence_coordinator
        .acquire_governed_data_import_completion_fence(&journal, &saga)
        .await
        .map_err(|error| {
            mark_governed_import_recovery_required(state);
            AppError::db_with_hint(
                format!(
                    "governed data-import completion could not establish an exclusive owner observation point: {error}"
                ),
                "data_import_recovery_required",
            )
        })?;
    if let Err(drift) =
        verify_governed_import_terminal_facts(state, &saga, state_restore_receipt.as_ref()).await
    {
        // Block every new admission before releasing the completion fence.
        // The unknown/abandonment path obtains the same barrier exclusively;
        // attempting that recursively here would deadlock under Tokio writer
        // preference.
        mark_governed_import_recovery_required(state);
        drop(completion_fence);
        return mark_governed_import_owner_unknown(
            state,
            &journal,
            &saga,
            &drift.owner,
            &drift.reason,
        )
        .await;
    }
    let terminal_model_physical_hash = current_lifemodel_file_hash(state).await.map_err(|error| {
        mark_governed_import_recovery_required(state);
        AppError::db_with_hint(
            format!(
                "governed data-import terminal LifeModel physical hash could not be observed: {error}"
            ),
            "data_import_recovery_required",
        )
    })?;
    saga = transition_governed_import(
        &journal,
        state,
        &request.operation_id,
        GovernedDataImportStage::Completed,
        &[],
        projection_evidence.as_deref(),
    )?;
    debug_assert_eq!(saga.stage, GovernedDataImportStage::Completed);
    drop(completion_fence);

    let durable_lifemodel_write = {
        let owner = governed_import_owner(&saga, "life_model")?;
        owner.before_digest != owner.target_digest
    };
    governed_import_result(
        state,
        request,
        &import_payload_hash,
        &previous_model_physical_hash,
        &imported_model_physical_hash,
        messages.as_deref(),
        vectors.as_deref(),
        message_report.as_ref(),
        vector_report.as_ref(),
        state_store,
        prepared.replayed,
        pre_import_snapshot_version,
        recovery_admission.is_some(),
        durable_lifemodel_write,
        Some(terminal_model_physical_hash),
    )
    .await
}

async fn current_lifemodel_file_hash(state: &Arc<AppState>) -> Result<String, AppError> {
    let current = state
        .life_model_manager
        .lock()
        .await
        .load()
        .map_err(AppError::from)?;
    life_model_write_gateway::hash_life_model(&current).map_err(AppError::from)
}

async fn abandon_governed_import_after_exact_observation(
    state: &Arc<AppState>,
    journal: &GovernedDataImportJournal,
    saga: &GovernedDataImportReceipt,
    reason: &str,
) -> Result<serde_json::Value, AppError> {
    let _resolution_fence = state
        .persistence_coordinator
        .acquire_governed_data_import_resolution_fence(journal, saga)
        .await
        .map_err(|error| {
            mark_governed_import_recovery_required(state);
            AppError::db_with_hint(
                format!(
                    "governed data-import remains unresolved because the exclusive resolution fence could not be acquired: {reason}; {error}"
                ),
                "data_import_recovery_required",
            )
        })?;
    let observations = match observe_governed_import_owner_resolutions(state, saga).await {
        Ok(observations) => observations,
        Err(error) => {
            return Err(AppError::db_with_hint(
                format!(
                    "governed data-import remains unresolved because current owner facts could not be observed exactly: {reason}; {error}"
                ),
                "data_import_recovery_required",
            ));
        }
    };
    let reason_digest = metadata_digest(&format!(
        "automatic preserve-current terminalization after exact observation: {reason}"
    ));
    match journal.abandon_preserving_current(&saga.operation_id, &observations, &reason_digest) {
        Ok(terminal) => Err(AppError::db_with_hint(
            format!(
                "governed data-import did not complete or roll back; exact current owner facts were preserved and operation {} was terminalized as {} with {} metadata-only evidence records; restart required",
                terminal.operation_id,
                terminal.stage.as_str(),
                terminal.resolution_evidence.len()
            ),
            "data_import_abandoned_restart_required",
        )),
        Err(error) => Err(AppError::db_with_hint(
            format!(
                "governed data-import remains unresolved because preserve-current terminalization failed after exact observation: {reason}; {error}"
            ),
            "data_import_recovery_required",
        )),
    }
}

async fn mark_governed_import_owner_unknown(
    state: &Arc<AppState>,
    journal: &GovernedDataImportJournal,
    saga: &GovernedDataImportReceipt,
    owner: &str,
    reason: &str,
) -> Result<serde_json::Value, AppError> {
    let reason_digest = metadata_digest(reason);
    let transition = journal.transition(
        &saga.operation_id,
        GovernedDataImportStage::CompensationUnknown,
        &[GovernedDataImportOwnerUpdate {
            owner: owner.to_string(),
            status: GovernedDataImportOwnerStatus::Unknown,
        }],
        Some(&reason_digest),
    );
    match transition {
        Ok(unknown) => {
            abandon_governed_import_after_exact_observation(state, journal, &unknown, reason).await
        }
        Err(error) => {
            mark_governed_import_recovery_required(state);
            Err(AppError::db_with_hint(
                format!(
                    "governed data-import owner state is unknown and journal transition failed: {reason}; {error}"
                ),
                "data_import_recovery_required",
            ))
        }
    }
}

// One saga-compensation boundary must receive every already-committed owner and
// receipt explicitly; a partial options bag would make rollback omissions easy.
#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
async fn fail_or_compensate_governed_import(
    state: &Arc<AppState>,
    journal: &GovernedDataImportJournal,
    saga: &GovernedDataImportReceipt,
    replayed: bool,
    previous_model: &LifeModel,
    previous_messages: Option<&openlife_core::memory::CanonicalMessageArchiveSnapshot>,
    previous_vectors: Option<&openlife_core::vectors::PortableVectorArchiveSnapshot>,
    imported_model_physical_hash: &str,
    import_error: String,
) -> Result<serde_json::Value, AppError> {
    if replayed {
        mark_governed_import_recovery_required(state);
        return Err(AppError::db_with_hint(
            format!("governed data-import recovery did not converge: {import_error}"),
            "data_import_recovery_required",
        ));
    }

    let mut failures = Vec::new();
    if let (Ok(owner), Some(previous)) = (governed_import_owner(saga, "vectors"), previous_vectors)
    {
        if owner.status == GovernedDataImportOwnerStatus::Applied {
            let current = state
                .vector_store
                .lock()
                .await
                .export_portable_archive()
                .map_err(AppError::from)?;
            if current.digest != owner.target_digest {
                failures.push("vector_compensation_refused_after_late_write".into());
            } else if let Err(error) = replace_import_owner_memory(
                state,
                None,
                Some(&previous.chunks),
                memory_gateway::ImportedMemoryExpectedDigests {
                    messages: None,
                    vectors: Some(owner.target_digest.clone()),
                },
                None,
            )
            .await
            {
                failures.push(format!("vector_compensation_failed:{error}"));
            }
        }
    }
    if let (Ok(owner), Some(previous)) =
        (governed_import_owner(saga, "messages"), previous_messages)
    {
        if owner.status == GovernedDataImportOwnerStatus::Applied {
            let current = state
                .memory_store
                .lock()
                .await
                .export_canonical_message_archive()
                .map_err(AppError::from)?;
            if current.digest != owner.target_digest {
                failures.push("message_compensation_refused_after_late_write".into());
            } else if let Err(error) = replace_import_owner_memory(
                state,
                Some(&previous.messages),
                None,
                memory_gateway::ImportedMemoryExpectedDigests {
                    messages: Some(owner.target_digest.clone()),
                    vectors: None,
                },
                None,
            )
            .await
            {
                failures.push(format!("message_compensation_failed:{error}"));
            }
        }
    }
    if let Ok(owner) = governed_import_owner(saga, "life_model") {
        if owner.status == GovernedDataImportOwnerStatus::Applied {
            let current_owner = current_lifemodel_owner_digest(state).await?;
            let current_physical = current_lifemodel_file_hash(state).await?;
            if current_owner != owner.target_digest
                || current_physical != imported_model_physical_hash
            {
                failures.push("lifemodel_compensation_refused_after_late_write".into());
            } else if let Err(error) = life_model_write_gateway::restore_life_model_with_gateway(
                state,
                previous_model,
                LifeModelMaterializerCallerContext::new(
                    "data_import_exact_compensation",
                    LifeModelMaterializerCallerKind::GovernedRestoreImportOperation,
                    LifeModelMaterializerCallerPurpose::GovernedRestoreImportOperation,
                ),
                Some(&current_physical),
            )
            .await
            {
                failures.push(format!("lifemodel_compensation_failed:{error}"));
            }
        }
    }

    if failures.is_empty() {
        let updates = saga
            .owners
            .iter()
            .filter_map(|owner| match owner.status {
                GovernedDataImportOwnerStatus::Applied => Some(GovernedDataImportOwnerUpdate {
                    owner: owner.owner.clone(),
                    status: GovernedDataImportOwnerStatus::Compensated,
                }),
                GovernedDataImportOwnerStatus::Pending => Some(GovernedDataImportOwnerUpdate {
                    owner: owner.owner.clone(),
                    status: GovernedDataImportOwnerStatus::Skipped,
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        transition_governed_import(
            journal,
            state,
            &saga.operation_id,
            GovernedDataImportStage::Compensated,
            &updates,
            Some(&metadata_digest("owner CAS compensation verified")),
        )?;
        return Err(AppError::internal(format!(
            "导入失败；所有已应用 canonical owner 已恢复到导入前状态，并通过 digest/CAS 复核: {import_error}"
        )));
    }

    let updates = saga
        .owners
        .iter()
        .filter(|owner| {
            matches!(
                owner.status,
                GovernedDataImportOwnerStatus::Applied
                    | GovernedDataImportOwnerStatus::Pending
                    | GovernedDataImportOwnerStatus::Skipped
            )
        })
        .map(|owner| GovernedDataImportOwnerUpdate {
            owner: owner.owner.clone(),
            status: GovernedDataImportOwnerStatus::Unknown,
        })
        .collect::<Vec<_>>();
    let reason = metadata_digest(&failures.join(";"));
    let unknown = journal.transition(
        &saga.operation_id,
        GovernedDataImportStage::CompensationUnknown,
        &updates,
        Some(&reason),
    );
    match unknown {
        Ok(unknown) => {
            abandon_governed_import_after_exact_observation(
                state,
                journal,
                &unknown,
                &format!(
                    "import failed and compensation was refused; import={import_error}; compensation={}",
                    failures.join(";")
                ),
            )
            .await
        }
        Err(error) => {
            mark_governed_import_recovery_required(state);
            Err(AppError::db_with_hint(
                format!(
                    "导入失败且补偿被晚到写或 owner 错误拒绝；未覆盖任何晚到事实，且 unknown journal transition 失败。import={import_error}; compensation={}; journal={error}",
                    failures.join(";")
                ),
                "data_import_recovery_required",
            ))
        }
    }
}

struct GovernedImportTerminalDrift {
    owner: String,
    reason: String,
}

impl GovernedImportTerminalDrift {
    fn new(owner: &str, reason: impl Into<String>) -> Self {
        Self {
            owner: owner.to_string(),
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
async fn wait_at_governed_import_terminal_observation_barrier(operation_id: &str) {
    let barrier = GOVERNED_IMPORT_TERMINAL_OBSERVATION_BARRIERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(operation_id);
    if let Some(barrier) = barrier {
        let _ = barrier.observed_first_owner.send(());
        let _ = barrier.release.await;
    }
}

#[cfg(test)]
fn install_governed_import_terminal_observation_barrier(
    operation_id: &str,
) -> (
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (observed_tx, observed_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let replaced = GOVERNED_IMPORT_TERMINAL_OBSERVATION_BARRIERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(
            operation_id.to_string(),
            GovernedImportTerminalObservationBarrier {
                observed_first_owner: observed_tx,
                release: release_rx,
            },
        );
    assert!(
        replaced.is_none(),
        "governed import terminal observation barrier already installed"
    );
    (observed_rx, release_tx)
}

async fn verify_governed_import_terminal_facts(
    state: &Arc<AppState>,
    saga: &GovernedDataImportReceipt,
    state_receipt: Option<&openlife_core::state_store::PortableDailyTaskRestoreReceipt>,
) -> Result<(), GovernedImportTerminalDrift> {
    let life_model = governed_import_owner(saga, "life_model").map_err(|_| {
        GovernedImportTerminalDrift::new(
            "LifeModelFileStore",
            "LifeModel journal owner missing before import terminalization",
        )
    })?;
    let current_lifemodel = current_lifemodel_owner_digest(state).await.map_err(|_| {
        GovernedImportTerminalDrift::new(
            &life_model.owner,
            "LifeModel owner could not be observed before import terminalization",
        )
    })?;
    if current_lifemodel != life_model.target_digest {
        return Err(GovernedImportTerminalDrift::new(
            &life_model.owner,
            "LifeModel owner changed before import terminalization",
        ));
    }
    #[cfg(test)]
    wait_at_governed_import_terminal_observation_barrier(&saga.operation_id).await;
    if let Ok(owner) = governed_import_owner(saga, "messages") {
        let digest = state
            .memory_store
            .lock()
            .await
            .export_canonical_message_archive()
            .map_err(|_| {
                GovernedImportTerminalDrift::new(
                    &owner.owner,
                    "Memory owner could not be observed before import terminalization",
                )
            })?
            .digest;
        if digest != owner.target_digest {
            return Err(GovernedImportTerminalDrift::new(
                &owner.owner,
                "Memory owner changed before import terminalization",
            ));
        }
    }
    if let Ok(owner) = governed_import_owner(saga, "vectors") {
        let digest = state
            .vector_store
            .lock()
            .await
            .export_portable_archive()
            .map_err(|_| {
                GovernedImportTerminalDrift::new(
                    &owner.owner,
                    "Vector owner could not be observed before import terminalization",
                )
            })?
            .digest;
        if digest != owner.target_digest {
            return Err(GovernedImportTerminalDrift::new(
                &owner.owner,
                "Vector owner changed before import terminalization",
            ));
        }
    }
    if let Ok(owner) = governed_import_owner(saga, "state_store") {
        let receipt = state_receipt.ok_or_else(|| {
            GovernedImportTerminalDrift::new(
                &owner.owner,
                "StateStore owner receipt missing before import terminalization",
            )
        })?;
        if receipt.request_digest != saga.request_digest
            || receipt.payload_digest != owner.target_digest
            || receipt.before_canonical_digest != owner.before_digest
            || receipt.committed_at != saga.created_at
        {
            return Err(GovernedImportTerminalDrift::new(
                &owner.owner,
                "StateStore owner receipt drift before import terminalization",
            ));
        }
        let current = state
            .state_store
            .as_ref()
            .ok_or_else(|| {
                GovernedImportTerminalDrift::new(
                    &owner.owner,
                    "StateStore unavailable before import terminalization",
                )
            })?
            .export_portable_daily_tasks(saga.created_at)
            .map_err(|_| {
                GovernedImportTerminalDrift::new(
                    &owner.owner,
                    "StateStore owner could not be observed before import terminalization",
                )
            })?;
        if current.canonical_digest != receipt.after_canonical_digest {
            return Err(GovernedImportTerminalDrift::new(
                &owner.owner,
                "StateStore owner changed before import terminalization",
            ));
        }
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
async fn governed_import_result(
    state: &Arc<AppState>,
    request: &GovernedDataImportRequest,
    import_payload_hash: &str,
    previous_model_hash: &str,
    requested_model_hash: &str,
    messages: Option<&[openlife_core::memory::ExportedMessage]>,
    vectors: Option<&[openlife_core::vectors::ExportedVectorChunk]>,
    message_report: Option<&memory_gateway::ImportedMemoryReplaceReport>,
    vector_report: Option<&memory_gateway::ImportedMemoryReplaceReport>,
    state_store: Option<&Arc<openlife_core::state_store::StateStore>>,
    replayed: bool,
    pre_import_snapshot_version: Option<String>,
    recovery_completed: bool,
    durable_lifemodel_write: bool,
    terminal_model_physical_hash: Option<String>,
) -> Result<serde_json::Value, AppError> {
    let supplied_message_count =
        messages.map_or(0, <[openlife_core::memory::ExportedMessage]>::len);
    let imported_message_count = message_report
        .map(|report| report.applied_message_count)
        .unwrap_or(supplied_message_count);
    // A completed replay is historical truth. Re-running VectorStore
    // admission against later tombstones or policy state could turn a durable
    // success into a false failure, so execution-only counts remain unknown
    // unless this invocation actually performed/validated the vector lane.
    let vector_preflight = if !replayed {
        if let Some(vectors) = vectors {
            Some(
                state
                    .vector_store
                    .lock()
                    .await
                    .validate_portable_replacement(vectors)
                    .map_err(AppError::from)?,
            )
        } else {
            None
        }
    } else {
        None
    };
    let vector_stats = vector_report
        .map(|report| report.vectors)
        .or(vector_preflight);
    let supplied_vector_count =
        vectors.map_or(0, <[openlife_core::vectors::ExportedVectorChunk]>::len);
    let imported_vector_count = if vectors.is_none() {
        Some(0)
    } else {
        vector_stats.map(|stats| stats.applied)
    };
    let skipped_vector_count = if vectors.is_none() {
        Some(0)
    } else {
        vector_stats.map(|stats| stats.skipped())
    };
    let skipped_canonical_vector_count = if vectors.is_none() {
        Some(0)
    } else {
        vector_stats.map(|stats| stats.skipped_canonical_projection)
    };
    let skipped_legacy_chat_vector_count = if vectors.is_none() {
        Some(0)
    } else {
        vector_stats.map(|stats| stats.skipped_legacy_chat_projection)
    };
    let state_receipt = match state_store {
        Some(store) => store
            .portable_daily_task_restore_receipt(&request.operation_id, replayed)
            .map_err(AppError::from)?,
        None => None,
    };
    let state_store_projection_status = match (state_store, state_receipt.as_ref()) {
        (Some(store), Some(receipt)) => store
            .projection_delivery_state_for_event(&receipt.outbox_event_id)
            .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?
            .as_str(),
        (Some(_), None) => {
            return Err(AppError::db_with_hint(
                "completed governed import is missing its exact StateStore restore receipt",
                "canonical_state_unknown",
            ));
        }
        (None, None) => "not_requested",
        (None, Some(_)) => {
            return Err(AppError::internal(
                "StateStore restore receipt exists without a targeted StateStore owner",
            ));
        }
    };
    let status = if recovery_completed {
        "recovery_completed_restart_required"
    } else if replayed {
        "replayed"
    } else {
        "completed"
    };
    let pre_import_lifemodel_snapshot_created = pre_import_snapshot_version.is_some() || replayed;
    let physical_hashes_observed_for_this_execution = !replayed;
    let previous_model_hash_value =
        physical_hashes_observed_for_this_execution.then(|| previous_model_hash.to_string());
    let requested_model_hash_value =
        physical_hashes_observed_for_this_execution.then(|| requested_model_hash.to_string());
    let final_model_hash_status = if terminal_model_physical_hash.is_some() {
        if replayed {
            "observed_at_recovery_terminalization"
        } else {
            "observed_at_terminalization"
        }
    } else {
        "not_persisted_for_completed_replay"
    };
    let audit = serde_json::json!({
        "source_kind": "data_import",
        "operation_purpose": request.purpose,
        "import_targets": request.import_targets,
        "import_payload_hash": import_payload_hash,
        "previous_model_hash": previous_model_hash_value,
        "requested_model_hash": requested_model_hash_value,
        "physical_hash_status": if physical_hashes_observed_for_this_execution { "observed" } else { "not_reconstructed_on_replay" },
        "final_model_hash": terminal_model_physical_hash.clone(),
        "final_model_hash_status": final_model_hash_status,
        "messages_targeted": messages.is_some(),
        "vectors_targeted": vectors.is_some(),
        "supplied_message_count": supplied_message_count,
        "imported_message_count": imported_message_count,
        "supplied_vector_count": supplied_vector_count,
        "imported_vector_count": imported_vector_count,
        "skipped_vector_count": skipped_vector_count,
        "state_store_targeted": state_store.is_some(),
        "state_store_restored_count": state_receipt.as_ref().map_or(0, |receipt| receipt.restored_count),
        "state_store_skipped_expired_count": state_receipt.as_ref().map_or(0, |receipt| receipt.skipped_expired_count),
        "state_store_projection_status": state_store_projection_status,
        "pre_change_snapshot_version": pre_import_snapshot_version,
        "metadata_safe": true,
        "contains_raw_content": false,
    });
    Ok(serde_json::json!({
        "success": true,
        "status": status,
        "restart_required": recovery_completed,
        "legacy": false,
        "governed_operation": true,
        "operation_kind": "data_import",
        "operation_purpose": request.purpose,
        "warning": if recovery_completed {
            "data import recovery is terminal; restart OpenLife before ordinary effects resume."
        } else {
            "data import ran as an explicit governed restore/import operation."
        },
        "vector_import_semantics": "portable_only_canonical_and_chat_projections_skipped",
        "metadata_safe": true,
        "contains_raw_content": false,
        "durable_lifemodel_write": durable_lifemodel_write,
        "messages_targeted": messages.is_some(),
        "vectors_targeted": vectors.is_some(),
        "supplied_message_count": supplied_message_count,
        "imported_message_count": imported_message_count,
        "supplied_vector_count": supplied_vector_count,
        "imported_vector_count": imported_vector_count,
        "skipped_vector_count": skipped_vector_count,
        "skipped_canonical_vector_count": skipped_canonical_vector_count,
        "skipped_legacy_chat_vector_count": skipped_legacy_chat_vector_count,
        "import_payload_hash": import_payload_hash,
        "previous_model_hash": previous_model_hash_value,
        "requested_model_hash": requested_model_hash_value,
        "imported_model_hash": requested_model_hash_value,
        "physical_hash_status": if physical_hashes_observed_for_this_execution { "observed" } else { "not_reconstructed_on_replay" },
        "final_model_hash": terminal_model_physical_hash,
        "final_model_hash_status": final_model_hash_status,
        "state_store_targeted": state_store.is_some(),
        "state_store_replayed": state_receipt.as_ref().is_some_and(|receipt| receipt.replayed),
        "state_store_restored_count": state_receipt.as_ref().map_or(0, |receipt| receipt.restored_count),
        "state_store_skipped_expired_count": state_receipt.as_ref().map_or(0, |receipt| receipt.skipped_expired_count),
        "state_store_projection_status": state_store_projection_status,
        "pre_import_snapshot_created": pre_import_lifemodel_snapshot_created,
        "pre_import_snapshot_scope": "life_model_only",
        "other_owner_recovery": "owner_cas_and_forward_recovery_no_full_rollback_archive",
        "pre_import_snapshot_version": pre_import_snapshot_version,
        "audit": audit,
    }))
}

#[derive(serde::Serialize)]
pub struct LlmConnectionTestResult {
    pub ok: bool,
    pub provider: String,
    pub message: String,
    pub validation_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_policy_decision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_network_policy_decision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_proposal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_id: Option<String>,
    /// Exact metadata-only terminal from the scheduler's provider adapter seam.
    /// Provider request/response bodies and credentials are never included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_invocation_receipt: Option<ProviderInvocationReceipt>,
}

#[tauri::command]
pub async fn test_llm_connection(
    config: AppConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<LlmConnectionTestResult, AppError> {
    test_llm_connection_with_state_and_validation_path(
        config,
        state.inner(),
        &crate::provider_validation::provider_validation_path(),
    )
    .await
}

pub(crate) async fn test_llm_connection_with_state_and_validation_path(
    mut config: AppConfig,
    state: &Arc<AppState>,
    validation_path: &std::path::Path,
) -> Result<LlmConnectionTestResult, AppError> {
    config.normalize_provider_from_base();
    let provider = config.llm.provider.clone();
    let label = provider_label(&provider);

    let current_runtime = state.provider_runtime_snapshot().await;
    let current_runtime_coherent = current_runtime.coherent;
    let current_config = current_runtime.config;
    config.llm.credential_version = resolved_provider_credential_version(&config, &current_config);
    config.llm.openai_key = resolve_submitted_provider_api_key(&config, &current_config);

    if !current_runtime_coherent {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "provider_runtime_generation_incoherent",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            validation_path,
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "Provider 配置与执行适配器不属于同一运行代；连接测试已在网络请求前失败关闭。"
                .into(),
            validation_status: "runtime_generation_incoherent".into(),
            network_policy_decision_id: None,
            effective_network_policy_decision_id: None,
            consent_status: Some("blocked".into()),
            review_proposal_id: None,
            permission_id: None,
            provider_invocation_receipt: None,
        });
    }

    let api_key =
        effective_api_key_for_endpoint(&provider, &config.llm.openai_base, &config.llm.openai_key);
    if api_key.trim().is_empty() {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "missing_api_key",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            validation_path,
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "未检测到 API Key，请填写后再测试。".to_string(),
            validation_status: "failed".into(),
            network_policy_decision_id: None,
            effective_network_policy_decision_id: None,
            consent_status: None,
            review_proposal_id: None,
            permission_id: None,
            provider_invocation_receipt: None,
        });
    }

    let backend_network_policy = current_config.system.network_policy.clone();
    // The submitted Settings payload cannot choose the network authority used
    // for either dispatch or durable validation identity.
    config.system.network_policy = backend_network_policy.clone();
    if !backend_network_policy.enabled {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "network_policy_disabled",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            validation_path,
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "连接测试被当前网络策略阻止。请先启用网络访问后再验证 provider。".to_string(),
            validation_status: "failed".into(),
            network_policy_decision_id: None,
            effective_network_policy_decision_id: None,
            consent_status: Some("blocked".into()),
            review_proposal_id: None,
            permission_id: None,
            provider_invocation_receipt: None,
        });
    }

    let base = if config.llm.openai_base.trim().is_empty() {
        default_base_for_provider(&provider).to_string()
    } else {
        config.llm.openai_base.trim_end_matches('/').to_string()
    };
    let model = config.llm.chat_model.trim().to_string();
    if model.is_empty() {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "missing_model",
            chrono::Utc::now(),
        );
        crate::provider_validation::save_provider_validation_record_to_path(
            validation_path,
            &record,
        )?;
        return Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: "未配置要验证的模型；连接测试没有发送 provider 请求。".into(),
            validation_status: "failed".into(),
            network_policy_decision_id: None,
            effective_network_policy_decision_id: None,
            consent_status: None,
            review_proposal_id: None,
            permission_id: None,
            provider_invocation_receipt: None,
        });
    }
    config.llm.openai_base = base.clone();
    config.llm.chat_model = model.clone();
    config.llm.openai_key = api_key.clone();
    let probe_scheduler = InferenceScheduler::new(
        config.local_model.clone(),
        false,
        provider.clone(),
        base.clone(),
        api_key,
        model.clone(),
        config.llm.embedding_model.clone(),
        false,
    )
    .with_provider_credential_version(config.llm.credential_version);
    let probe_scheduler = {
        let permission_store = state.tool_permission_store.lock().await;
        permission_store.bind_explicit_provider_probe_scheduler(probe_scheduler)
    };
    let url = chat_completions_url(&provider, &base);
    let network_capability = format!("provider.{provider}");
    let network_policy_decision =
        resolve_network_policy_decision(&backend_network_policy, &url, &network_capability)
            .map_err(|_| AppError::external("provider network policy decision failed"))?;
    let original_network_policy_decision_id = network_policy_decision.decision_id.clone();
    let (probe_grant, effective_network_policy_decision_id, permission_id) =
        match authorize_explicit_provider_probe(
            state,
            &probe_scheduler,
            &backend_network_policy,
            &network_policy_decision,
            &url,
            &network_capability,
            &provider,
        )
        .await?
        {
            ExplicitProviderProbeAuthorization::Authorized {
                grant,
                effective_network_policy_decision_id,
                permission_id,
            } => (*grant, effective_network_policy_decision_id, permission_id),
            ExplicitProviderProbeAuthorization::ConsentRequired { proposal_id } => {
                return Ok(LlmConnectionTestResult {
                    ok: false,
                    provider: label,
                    message: "需要在 Review Center 明确批准一次 provider 网络连接；批准前不会发送请求，批准后请重试连接测试。".into(),
                    validation_status: "consent_required".into(),
                    network_policy_decision_id: Some(original_network_policy_decision_id),
                    effective_network_policy_decision_id: None,
                    consent_status: Some("pending_review".into()),
                    review_proposal_id: Some(proposal_id),
                    permission_id: None,
                    provider_invocation_receipt: None,
                });
            }
            ExplicitProviderProbeAuthorization::Denied { reason_code } => {
                return Ok(LlmConnectionTestResult {
                    ok: false,
                    provider: label,
                    message: format!("连接测试被当前网络策略阻止（{reason_code}）。"),
                    validation_status: "blocked".into(),
                    network_policy_decision_id: Some(original_network_policy_decision_id),
                    effective_network_policy_decision_id: None,
                    consent_status: Some("blocked".into()),
                    review_proposal_id: None,
                    permission_id: None,
                    provider_invocation_receipt: None,
                });
            }
        };
    let prepared = match probe_scheduler.prepare_explicit_provider_probe(probe_grant) {
        Ok(prepared) => prepared,
        Err(_) => {
            let record = crate::provider_validation::failed_provider_validation_record(
                &config,
                "settings_manual_test",
                "provider_probe_pre_dispatch_rejected",
                chrono::Utc::now(),
            );
            crate::provider_validation::save_provider_validation_record_to_path(
                validation_path,
                &record,
            )?;
            return Ok(LlmConnectionTestResult {
                ok: false,
                provider: label,
                message: "连接测试在 provider 请求发出前被拒绝，未建立可用性证据。".into(),
                validation_status: "failed".into(),
                network_policy_decision_id: Some(original_network_policy_decision_id),
                effective_network_policy_decision_id: Some(
                    effective_network_policy_decision_id.clone(),
                ),
                consent_status: Some(if permission_id.is_some() {
                    "allow_once_consumed".into()
                } else {
                    "not_required".into()
                }),
                review_proposal_id: None,
                permission_id,
                provider_invocation_receipt: None,
            });
        }
    };
    // These are the exact prepared-generation facts later sealed into the
    // adapter terminal proof. They are captured before ownership moves into
    // execution; the submitted Settings payload is not proof authority.
    let prepared_provider_config_generation = prepared.provider_config_generation.clone();
    let prepared_network_policy = prepared.network_policy.clone();
    let prepared_network_policy_decision = prepared.network_policy_decision.clone();
    let outcome = probe_scheduler.execute_prepared(prepared).await;
    let result_has_content = outcome
        .result
        .as_ref()
        .is_ok_and(|content| !content.trim().is_empty());
    let observed_receipt = outcome.receipt;
    let terminal_proof = outcome.terminal_proof;
    let write_observed_at = chrono::Utc::now();
    let mut receipt = None;
    let mut terminal_status = None;
    let mut completed = false;
    let record = match (observed_receipt.as_ref(), terminal_proof) {
        (Some(observed), Some(proof)) if proof.receipt() == observed => {
            let candidate_status = proof.receipt().status;
            let candidate_receipt = proof.receipt().clone();
            let candidate_completed =
                candidate_status == ProviderInvocationStatus::Completed && result_has_content;
            let safe_error = match candidate_status {
                ProviderInvocationStatus::RemoteUnknown => "provider_remote_state_unknown",
                ProviderInvocationStatus::Failed => "provider_confirmed_failure",
                ProviderInvocationStatus::Completed if !candidate_completed => {
                    "provider_completion_inconsistent"
                }
                ProviderInvocationStatus::Completed => "validation_failed",
            };
            match crate::provider_validation::provider_validation_record_with_terminal_proof(
                &config,
                "settings_manual_test",
                proof,
                &prepared_provider_config_generation,
                &prepared_network_policy,
                &prepared_network_policy_decision,
                candidate_completed,
                (!candidate_completed).then_some(safe_error),
                write_observed_at,
            ) {
                Ok(record) => {
                    // A receipt reaches product/durable projection only after
                    // the opaque proof passes every exact runtime binding.
                    receipt = Some(candidate_receipt);
                    terminal_status = Some(candidate_status);
                    completed = candidate_completed;
                    record
                }
                Err(_) => crate::provider_validation::failed_provider_validation_record(
                    &config,
                    "settings_manual_test",
                    "provider_terminal_proof_invalid",
                    write_observed_at,
                ),
            }
        }
        (Some(_), None) => crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "provider_terminal_proof_missing",
            write_observed_at,
        ),
        (None, None) => crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_manual_test",
            "provider_not_attempted",
            write_observed_at,
        ),
        (None, Some(_)) | (Some(_), Some(_)) => {
            crate::provider_validation::failed_provider_validation_record(
                &config,
                "settings_manual_test",
                "provider_terminal_proof_mismatch",
                write_observed_at,
            )
        }
    };
    crate::provider_validation::save_provider_validation_record_to_path(validation_path, &record)?;

    if completed {
        let model_note = if model.to_lowercase().contains("reasoner") {
            " 当前选择的是推理模型，首次可见输出可能更慢；试用聊天建议优先使用 deepseek-chat 这类通用聊天模型。"
        } else {
            ""
        };
        Ok(LlmConnectionTestResult {
            ok: true,
            provider: label,
            message: format!("连接成功，当前供应商模型可用。{}", model_note),
            validation_status: "validated".into(),
            network_policy_decision_id: Some(original_network_policy_decision_id),
            effective_network_policy_decision_id: Some(
                effective_network_policy_decision_id.clone(),
            ),
            consent_status: Some(if permission_id.is_some() {
                "allow_once_consumed".into()
            } else {
                "not_required".into()
            }),
            review_proposal_id: None,
            permission_id,
            provider_invocation_receipt: receipt,
        })
    } else {
        let remote_unknown = terminal_status == Some(ProviderInvocationStatus::RemoteUnknown);
        Ok(LlmConnectionTestResult {
            ok: false,
            provider: label,
            message: if remote_unknown {
                "连接请求已开始，但没有观察到可信的远端终态；当前状态为 unknown，不能标记为可用。"
                    .into()
            } else if terminal_status == Some(ProviderInvocationStatus::Failed) {
                "Provider 已返回明确失败，连接不能标记为可用。请检查 provider、模型和 API Key。"
                    .into()
            } else {
                "没有获得完整且可信的 provider 响应，连接不能标记为可用。".into()
            },
            validation_status: if remote_unknown {
                "remote_unknown".into()
            } else {
                "failed".into()
            },
            network_policy_decision_id: Some(original_network_policy_decision_id),
            effective_network_policy_decision_id: Some(
                effective_network_policy_decision_id.clone(),
            ),
            consent_status: Some(if permission_id.is_some() {
                "allow_once_consumed".into()
            } else {
                "not_required".into()
            }),
            review_proposal_id: None,
            permission_id,
            provider_invocation_receipt: receipt,
        })
    }
}

#[tauri::command]
pub async fn export_mcp_audit_logs(
    days: i64,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<AuditExport, AppError> {
    let export = {
        let store = state.mcp_audit_store.lock().await;
        store.export_logs(days).map_err(AppError::from)?
    };
    let export_value = serde_json::to_value(&export)?;
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "mcp_audit_export",
            target_ids_for_new_challenge: &[],
            requested_target: None,
            affected_count: None,
            reference: None,
            arguments: &serde_json::json!({
                "days": days,
                "export_digest": hash_json_value(&export_value)?,
            }),
            arguments_summary: &format!(
                "导出最近 {days} 天的 MCP 审计快照；原始日志不会复制进 confirmation grant。"
            ),
            governed_data_import_recovery: None,
        },
        &window,
        state.inner(),
    )
    .await?;
    Ok(export)
}

#[tauri::command]
pub async fn cleanup_mcp_audit_logs(
    retention_days: i64,
    confirmation_evidence: Option<DangerActionConfirmationReference>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let confirmation_arguments = serde_json::json!({ "retention_days": retention_days });
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "mcp_audit_cleanup",
            target_ids_for_new_challenge: &[],
            requested_target: None,
            affected_count: None,
            reference: confirmation_evidence.as_ref(),
            arguments: &confirmation_arguments,
            arguments_summary: &format!("删除超过 {retention_days} 天保留期的 MCP 审计记录。"),
            governed_data_import_recovery: None,
        },
        &window,
        state.inner(),
    )
    .await?;
    let store = state.mcp_audit_store.lock().await;
    store.cleanup(retention_days).map_err(AppError::from)
}

#[tauri::command]
pub async fn rotate_mcp_audit_key(
    confirmation_evidence: Option<DangerActionConfirmationReference>,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    require_danger_action_confirmation(
        DangerActionConfirmationRequest {
            action_type: "mcp_audit_key_rotation",
            target_ids_for_new_challenge: &[],
            requested_target: None,
            affected_count: None,
            reference: confirmation_evidence.as_ref(),
            arguments: &serde_json::json!({ "operation": "rotate_mcp_audit_key_epoch" }),
            arguments_summary: "轮换 MCP 审计加密 epoch，并保留历史 epoch 供旧记录解密。",
            governed_data_import_recovery: None,
        },
        &window,
        state.inner(),
    )
    .await?;
    let mut store = state.mcp_audit_store.lock().await;
    let timestamp_epoch = chrono::Utc::now().timestamp().max(0) as u64;
    let epoch = timestamp_epoch.max(store.key_config().epoch.saturating_add(1));
    let secret_store = KeyringSecretStore;
    let material = create_mcp_audit_key_material(epoch, &secret_store).map_err(AppError::from)?;
    let secret_ref = material.config.key_ref.clone().unwrap_or_default();
    let snapshot = store.clone();
    if let Err(error) = store.rotate_key_material(material) {
        let _ = secret_store.delete(&secret_ref);
        return Err(AppError::from(error));
    }
    if let Err(error) =
        save_mcp_audit_keyring_to_path(&mcp_audit_keyring_path(), store.key_configs())
    {
        *store = snapshot;
        let _ = secret_store.delete(&secret_ref);
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_privacy_policy(
    state: State<'_, Arc<AppState>>,
) -> Result<PrivacyPolicy, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("PrivacyPolicyStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let engine = state.privacy_engine.lock().await;
    Ok(engine.policy().clone())
}

#[tauri::command]
pub async fn set_privacy_policy(
    policy: PrivacyPolicy,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    save_privacy_policy_to_path(&privacy_policy_path(), &policy)?;
    let mut engine = state.privacy_engine.lock().await;
    engine.set_policy(policy);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_network_consent::{
        authorize_provider_network_dispatch, NetworkConsentSubmissionScope,
        ProviderNetworkAuthorization,
    };
    use openlife_core::llm::{provider_endpoint_is_official, ChatMessage};
    use std::collections::HashMap;
    use std::sync::Mutex;

    const W84_IMPORT_CURRENT_NAME_SECRET: &str = "W84_IMPORT_CURRENT_LIFEMODEL_SECRET";
    const W84_IMPORT_PAYLOAD_NAME_SECRET: &str = "W84_IMPORT_PAYLOAD_LIFEMODEL_SECRET";
    const W84_IMPORT_CURRENT_MESSAGE_SECRET: &str = "W84_IMPORT_CURRENT_MESSAGE_SECRET";
    const W84_IMPORT_PAYLOAD_MESSAGE_SECRET: &str = "W84_IMPORT_PAYLOAD_MESSAGE_SECRET";
    const W84_IMPORT_CURRENT_VECTOR_SECRET: &str = "W84_IMPORT_CURRENT_VECTOR_SECRET";
    const W84_IMPORT_PAYLOAD_VECTOR_SECRET: &str = "W84_IMPORT_PAYLOAD_VECTOR_SECRET";

    fn install_release_like_persistence_coordinator(state: &mut Arc<AppState>) {
        let coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        coordinator.seal();
        Arc::get_mut(state)
            .expect("test state must be uniquely owned before work starts")
            .persistence_coordinator = coordinator;
    }

    fn release_like_test_app_state() -> Arc<AppState> {
        let mut state = crate::test_utils::test_app_state();
        install_release_like_persistence_coordinator(&mut state);
        state
    }

    pub(super) async fn inject_governed_import_memory_drift(
        state: &Arc<AppState>,
        session_id: &str,
        content: &str,
    ) {
        state
            .memory_store
            .lock()
            .await
            .save_message(
                session_id,
                &ChatMessage {
                    role: "user".into(),
                    content: content.into(),
                },
            )
            .expect("inject governed-import Memory owner drift");
    }

    #[derive(Default)]
    struct RecoverySecretStore {
        values: Mutex<HashMap<String, String>>,
        writes: Mutex<usize>,
        deletes: Mutex<usize>,
        fail_set_at: Mutex<Option<usize>>,
        fail_after_set_at: Mutex<Option<usize>>,
    }

    impl SecretStore for RecoverySecretStore {
        fn get(&self, secret_ref: &str) -> anyhow::Result<Option<String>> {
            Ok(self.values.lock().unwrap().get(secret_ref).cloned())
        }

        fn set(&self, secret_ref: &str, value: &str) -> anyhow::Result<()> {
            let mut writes = self.writes.lock().unwrap();
            *writes += 1;
            if *self.fail_set_at.lock().unwrap() == Some(*writes) {
                anyhow::bail!("injected credential write failure");
            }
            self.values
                .lock()
                .unwrap()
                .insert(secret_ref.into(), value.into());
            if *self.fail_after_set_at.lock().unwrap() == Some(*writes) {
                anyhow::bail!("injected credential post-write failure");
            }
            Ok(())
        }

        fn delete(&self, secret_ref: &str) -> anyhow::Result<()> {
            *self.deletes.lock().unwrap() += 1;
            self.values.lock().unwrap().remove(secret_ref);
            Ok(())
        }
    }

    #[test]
    fn nkr_s2_credential_initialization_creates_exactly_five_empty_slots() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        let report = initialize_required_credentials_with_confirmation_result(
            true,
            directory.path(),
            &store,
            &snapshot,
        )
        .unwrap();

        assert!(report.initialization_completed_for_restart);
        assert!(report.restart_required);
        assert_eq!(report.cleanup_status, "not_required");
        assert_eq!(
            report
                .items
                .iter()
                .map(|item| item.status.as_str())
                .collect::<Vec<_>>(),
            vec!["created", "created", "created", "created", "created"]
        );
        assert_eq!(*store.writes.lock().unwrap(), 5);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("keychain://"));
        for value in store.values.lock().unwrap().values() {
            assert!(!serialized.contains(value));
        }
    }

    #[test]
    fn nkr_s2_rejected_confirmation_result_performs_zero_sets_and_zero_deletes() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        let error = initialize_required_credentials_with_confirmation_result(
            false,
            directory.path(),
            &store,
            &snapshot,
        )
        .unwrap_err();

        assert!(error.to_string().contains("native system dialog"));
        assert_eq!(*store.writes.lock().unwrap(), 0);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        assert!(store.values.lock().unwrap().is_empty());
        assert!(!directory.path().join("mcp_audit_keys.json").exists());
    }

    #[test]
    fn nkr_s2_command_source_keeps_native_confirmation_before_the_first_write_owner() {
        fn confirmation_precedes_write_owner(source: &str) -> bool {
            let Some(confirmation) = source.find("require_native_danger_action_confirmation(")
            else {
                return false;
            };
            let Some(write_owner) =
                source.find("initialize_required_credentials_with_confirmation_result(")
            else {
                return false;
            };
            confirmation < write_owner
                && !source[..write_owner].contains("SecretStore::set")
                && !source[..write_owner].contains("SecretStore::delete")
        }

        let source = include_str!("settings.rs");
        let command = source
            .split("pub async fn recover_required_credential_access(")
            .nth(1)
            .unwrap()
            .split("#[tauri::command]")
            .next()
            .unwrap();
        assert!(confirmation_precedes_write_owner(command));
        assert!(!confirmation_precedes_write_owner(
            "initialize_required_credentials_with_confirmation_result(); require_native_danger_action_confirmation();"
        ));
    }

    #[test]
    fn nkr_s2_native_request_binds_exact_sorted_purpose_scope_and_batch_anchor() {
        let purposes = vec![
            "action_queue".to_string(),
            "agent_run_receipts".to_string(),
            "main_chat_events".to_string(),
        ];
        let arguments = serde_json::json!({
            "eligiblePurposeIds": purposes,
            "affectedCount": purposes.len(),
            "bootstrapSnapshotVersion": "credential_bootstrap_v1",
            "bootstrapSnapshotDigest": "a".repeat(64),
        });

        let request = credential_initialization_native_request(&purposes, &arguments);

        assert_eq!(request.target_ids_for_new_challenge, purposes);
        assert_eq!(request.requested_target, Some("action_queue"));
        assert!(request
            .target_ids_for_new_challenge
            .iter()
            .any(|purpose| Some(purpose.as_str()) == request.requested_target));
        assert_eq!(request.affected_count, purposes.len());
        assert_eq!(
            request.arguments["eligiblePurposeIds"],
            arguments["eligiblePurposeIds"]
        );
        assert_eq!(request.arguments["affectedCount"], purposes.len());
        assert_eq!(request.arguments["bootstrapSnapshotDigest"], "a".repeat(64));
    }

    #[test]
    fn nkr_s2_existing_canonical_data_never_becomes_initialization_eligible() {
        let directory = tempfile::tempdir().unwrap();
        for file_name in [
            "agent_runs.db",
            "main_chat_agent_events.db",
            "main_chat_action_queue.db",
            "tasks.db",
        ] {
            std::fs::write(directory.path().join(file_name), b"canonical-data-sentinel").unwrap();
        }
        std::fs::write(
            directory.path().join("mcp_audit.db"),
            b"uninspectable-canonical-data",
        )
        .unwrap();
        let store = RecoverySecretStore::default();

        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        assert!(eligible_credential_purposes(&snapshot).is_empty());
        assert!(snapshot.purposes[..4]
            .iter()
            .all(|item| item.status == CredentialBootstrapStatus::MissingExistingData));
        assert_eq!(
            snapshot.purposes[4].status,
            CredentialBootstrapStatus::Unknown
        );
        assert_eq!(*store.writes.lock().unwrap(), 0);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        assert!(store.values.lock().unwrap().is_empty());
    }

    #[test]
    fn nkr_s2_invalid_existing_key_material_is_never_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        for secret_ref in [
            AGENT_RUN_RECEIPT_KEY_REF,
            MAIN_CHAT_EVENT_INTEGRITY_KEY_REF,
            ACTION_QUEUE_AUTHORITY_KEY_REF,
            TASK_STORE_AUTHORITY_KEY_REF,
        ] {
            store.set(secret_ref, "not-base64").unwrap();
        }
        *store.writes.lock().unwrap() = 0;

        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        assert!(snapshot.purposes[..4]
            .iter()
            .all(|item| item.status == CredentialBootstrapStatus::Invalid));
        assert_eq!(*store.writes.lock().unwrap(), 0);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        assert!(store
            .values
            .lock()
            .unwrap()
            .values()
            .all(|value| value == "not-base64"));
    }

    #[test]
    fn nkr_s2_replay_cannot_create_a_second_mcp_epoch() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);
        initialize_required_credentials_after_confirmation(directory.path(), &store, &snapshot)
            .unwrap();
        let writes_after_first = *store.writes.lock().unwrap();
        let keyring_after_first =
            std::fs::read(directory.path().join("mcp_audit_keys.json")).unwrap();

        let replay =
            initialize_required_credentials_after_confirmation(directory.path(), &store, &snapshot);

        assert!(replay.is_err());
        assert_eq!(*store.writes.lock().unwrap(), writes_after_first);
        assert_eq!(
            std::fs::read(directory.path().join("mcp_audit_keys.json")).unwrap(),
            keyring_after_first
        );
    }

    #[test]
    fn nkr_s2_fixed_credential_failure_compensates_every_prior_write() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        *store.fail_set_at.lock().unwrap() = Some(3);
        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        let report =
            initialize_required_credentials_after_confirmation(directory.path(), &store, &snapshot)
                .unwrap();

        assert!(!report.initialization_completed_for_restart);
        assert_eq!(report.cleanup_status, "compensated");
        assert_eq!(*store.writes.lock().unwrap(), 3);
        assert_eq!(*store.deletes.lock().unwrap(), 2);
        assert!(store.values.lock().unwrap().is_empty());
    }

    #[test]
    fn nkr_s2_fixed_post_write_error_retains_ambiguous_secret_and_never_deletes_it() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        *store.fail_after_set_at.lock().unwrap() = Some(3);
        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        let report =
            initialize_required_credentials_after_confirmation(directory.path(), &store, &snapshot)
                .unwrap();

        assert_eq!(report.cleanup_status, "unknown");
        assert_eq!(*store.writes.lock().unwrap(), 3);
        assert_eq!(*store.deletes.lock().unwrap(), 2);
        assert_eq!(store.values.lock().unwrap().len(), 1);
        assert_eq!(
            report
                .items
                .iter()
                .find(|item| item.purpose == "action_queue")
                .unwrap()
                .status,
            "cleanup_unknown"
        );
    }

    #[test]
    fn nkr_s2_process_lock_rejects_a_parallel_initialization_owner() {
        let directory = tempfile::tempdir().unwrap();
        let first = CredentialRecoveryProcessLock::acquire(directory.path()).unwrap();
        let second = CredentialRecoveryProcessLock::acquire(directory.path());

        assert!(second.is_err());
        drop(first);
        assert!(CredentialRecoveryProcessLock::acquire(directory.path()).is_ok());
    }

    #[test]
    fn nkr_s2_mcp_pre_write_save_failure_restores_prior_absence() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);
        crate::storage::fail_next_mcp_audit_keyring_save_for_test(
            directory.path().join("mcp_audit_keys.json"),
        );

        let report =
            initialize_required_credentials_after_confirmation(directory.path(), &store, &snapshot)
                .unwrap();

        assert_eq!(report.cleanup_status, "compensated");
        assert_eq!(*store.writes.lock().unwrap(), 5);
        assert_eq!(*store.deletes.lock().unwrap(), 5);
        assert!(store.values.lock().unwrap().is_empty());
        assert!(!directory.path().join("mcp_audit_keys.json").exists());
    }

    #[test]
    fn nkr_s2_mcp_pre_write_save_failure_preserves_exact_prior_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit_keys.json");
        let prior = vec![openlife_core::mcp_audit::AuditKeyConfig {
            mode: openlife_core::mcp_audit::KeyMode::Derived,
            epoch: 7,
            created_at: "2026-07-29T00:00:00Z".into(),
            ..Default::default()
        }];
        crate::storage::save_mcp_audit_keyring_to_path(&path, &prior).unwrap();
        let exact_prior_bytes = std::fs::read(&path).unwrap();
        crate::storage::fail_next_mcp_audit_keyring_save_for_test(&path);
        let store = RecoverySecretStore::default();

        let result = initialize_mcp_audit_credential(directory.path(), &store);

        assert!(matches!(result, Err((_, false))));
        assert_eq!(std::fs::read(&path).unwrap(), exact_prior_bytes);
        assert_eq!(*store.writes.lock().unwrap(), 1);
        assert_eq!(*store.deletes.lock().unwrap(), 1);
        assert!(store.values.lock().unwrap().is_empty());
    }

    #[test]
    fn nkr_s2_mcp_unreadable_final_state_is_cleanup_unknown_and_retains_secret() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit_keys.json");
        let store = RecoverySecretStore::default();
        crate::storage::fail_next_mcp_audit_keyring_save_with_unreadable_result_for_test(&path);

        let result = initialize_mcp_audit_credential(directory.path(), &store);

        assert!(matches!(result, Err((_, true))));
        assert_eq!(*store.writes.lock().unwrap(), 1);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        assert_eq!(store.values.lock().unwrap().len(), 1);
        assert!(path.exists());
    }

    #[test]
    fn nkr_s2_mcp_unreadable_prior_state_never_writes_or_deletes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit_keys.json");
        crate::storage::fail_next_mcp_audit_keyring_read_for_test(&path);
        let store = RecoverySecretStore::default();

        let result = initialize_mcp_audit_credential(directory.path(), &store);

        assert!(matches!(result, Err((_, false))));
        assert_eq!(*store.writes.lock().unwrap(), 0);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
    }

    #[test]
    fn nkr_s2_mcp_post_write_set_error_retains_ambiguous_secret() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        *store.fail_after_set_at.lock().unwrap() = Some(1);

        let result = initialize_mcp_audit_credential(directory.path(), &store);

        assert!(matches!(result, Err((_, true))));
        assert_eq!(*store.writes.lock().unwrap(), 1);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        assert_eq!(store.values.lock().unwrap().len(), 1);
        assert!(!directory.path().join("mcp_audit_keys.json").exists());
    }

    #[test]
    fn nkr_s2_mcp_ambiguous_post_write_failure_retains_observably_referenced_secret() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);
        crate::storage::fail_next_mcp_audit_keyring_save_after_write_for_test(
            directory.path().join("mcp_audit_keys.json"),
        );

        let report =
            initialize_required_credentials_after_confirmation(directory.path(), &store, &snapshot)
                .unwrap();

        assert_eq!(report.cleanup_status, "unknown");
        assert_eq!(
            report
                .items
                .iter()
                .find(|item| item.purpose == "mcp_audit")
                .unwrap()
                .status,
            "cleanup_unknown"
        );
        assert_eq!(*store.writes.lock().unwrap(), 5);
        assert_eq!(*store.deletes.lock().unwrap(), 4);
        assert_eq!(store.values.lock().unwrap().len(), 1);
        assert!(directory.path().join("mcp_audit_keys.json").exists());
    }

    #[test]
    fn resolve_masked_api_key_uses_current_key_for_mask_or_empty() {
        assert_eq!(resolve_masked_api_key(KEY_MASK, "sk-current"), "sk-current");
        assert_eq!(resolve_masked_api_key("", "sk-current"), "sk-current");
        assert_eq!(resolve_masked_api_key("   ", "sk-current"), "sk-current");
        assert_eq!(resolve_masked_api_key(KEY_MASK, ""), "");
    }

    #[test]
    fn resolve_masked_api_key_uses_submitted_new_key() {
        assert_eq!(resolve_masked_api_key("sk-new", "sk-current"), "sk-new");
    }

    #[test]
    fn masked_provider_key_is_bound_to_the_same_provider_endpoint_identity() {
        let mut current = AppConfig::default();
        current.llm.provider = "openai".into();
        current.llm.openai_base = "https://api.openai.com/v1".into();
        current.llm.openai_key = "sk-current-openai".into();

        let mut same = current.clone();
        same.llm.openai_key = KEY_MASK.into();
        assert_eq!(
            resolve_submitted_provider_api_key(&same, &current),
            "sk-current-openai"
        );

        let mut changed_provider = same.clone();
        changed_provider.llm.provider = "deepseek".into();
        changed_provider.llm.openai_base = "https://api.deepseek.com".into();
        assert!(resolve_submitted_provider_api_key(&changed_provider, &current).is_empty());

        let mut changed_endpoint = same;
        changed_endpoint.llm.openai_base = "https://capture.example/v1".into();
        assert!(resolve_submitted_provider_api_key(&changed_endpoint, &current).is_empty());
    }

    #[test]
    fn only_canonical_provider_endpoint_can_implicitly_use_environment_credentials() {
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = "https://api.openai.com/v1/".into();
        assert!(provider_endpoint_is_official(
            &config.llm.provider,
            &config.llm.openai_base,
        ));

        config.llm.openai_base = "https://proxy.example/v1".into();
        assert!(!provider_endpoint_is_official(
            &config.llm.provider,
            &config.llm.openai_base,
        ));
    }

    #[test]
    fn provider_credential_version_changes_only_with_secret_identity() {
        let mut current = AppConfig::default();
        current.llm.provider = "openai".into();
        current.llm.openai_base = "https://api.openai.com/v1".into();
        current.llm.openai_key = "sk-current".into();
        current.llm.credential_version = 7;

        let mut masked_same = current.clone();
        masked_same.llm.openai_key = KEY_MASK.into();
        assert_eq!(
            resolved_provider_credential_version(&masked_same, &current),
            7
        );

        let mut replaced = masked_same.clone();
        replaced.llm.openai_key = "sk-replaced".into();
        assert_eq!(resolved_provider_credential_version(&replaced, &current), 8);

        let mut moved = masked_same;
        moved.llm.openai_base = "https://custom.example/v1".into();
        assert_eq!(resolved_provider_credential_version(&moved, &current), 8);
    }

    #[tokio::test]
    async fn explicit_provider_probe_uses_scheduler_receipt_and_keeps_loopback_capability() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let captured = Arc::new(std::sync::Mutex::new(String::new()));
        let captured_server = Arc::clone(&captured);
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 16 * 1024];
            let read = socket.read(&mut request).await.unwrap();
            *captured_server.lock().unwrap() =
                String::from_utf8_lossy(&request[..read]).to_string();
            let body = r#"{"choices":[{"message":{"content":"pong"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut runtime_config = state.config.lock().await.clone();
        runtime_config.system.network_policy = openlife_core::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        state.replace_provider_runtime_config(runtime_config).await;
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = base;
        config.llm.openai_key = "sk-test".into();
        config.llm.chat_model = "gpt-test".into();
        let dir = tempfile::tempdir().unwrap();
        let validation_path = dir.path().join("provider-validation.json");

        let result =
            test_llm_connection_with_state_and_validation_path(config, &state, &validation_path)
                .await
                .unwrap();
        server.await.unwrap();
        assert!(result.ok);
        assert_eq!(result.validation_status, "validated");
        assert!(result.message.contains("当前供应商模型可用"));
        assert!(!result.message.contains("云端模型可用"));
        let receipt = result.provider_invocation_receipt.unwrap();
        assert_eq!(receipt.status, ProviderInvocationStatus::Completed);
        assert_eq!(receipt.provider, "openai");
        assert_eq!(receipt.model, "gpt-test");
        assert!(!receipt.simulated);
        let request = captured.lock().unwrap().clone();
        assert!(request.contains(r#""content":"ping""#));
        let persisted =
            crate::provider_validation::load_provider_validation_record_from_path(&validation_path)
                .as_record()
                .expect("completed probe must persist a valid validation record")
                .clone();
        assert_eq!(
            persisted
                .invocation_receipt
                .as_ref()
                .map(|receipt| receipt.request_id.as_str()),
            Some(receipt.request_id.as_str())
        );
        let raw = std::fs::read_to_string(validation_path).unwrap();
        assert!(!raw.contains("ping"));
        assert!(!raw.contains("pong"));
        assert!(!raw.contains("sk-test"));
    }

    #[tokio::test]
    async fn explicit_provider_probe_remote_unknown_is_persisted_and_never_reports_success() {
        use tokio::io::AsyncReadExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            // Drop after the adapter start boundary without a terminal response.
        });
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut runtime_config = state.config.lock().await.clone();
        runtime_config.system.network_policy = openlife_core::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        state.replace_provider_runtime_config(runtime_config).await;
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = base;
        config.llm.openai_key = "sk-test".into();
        config.llm.chat_model = "gpt-test".into();
        config.system.network_policy = openlife_core::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        // Summaries are bound to the effective credential generation and the
        // backend-owned network policy used by the probe, not to the stale
        // pre-resolution Settings payload.
        let current_runtime = state.provider_runtime_snapshot().await;
        let mut validation_config = config.clone();
        validation_config.llm.credential_version =
            resolved_provider_credential_version(&validation_config, &current_runtime.config);
        validation_config.llm.openai_key =
            resolve_submitted_provider_api_key(&validation_config, &current_runtime.config);
        validation_config.system.network_policy = current_runtime.config.system.network_policy;
        let dir = tempfile::tempdir().unwrap();
        let validation_path = dir.path().join("provider-validation.json");

        let result =
            test_llm_connection_with_state_and_validation_path(config, &state, &validation_path)
                .await
                .unwrap();
        server.await.unwrap();
        assert!(!result.ok);
        assert_eq!(result.validation_status, "remote_unknown");
        assert!(result.message.contains("unknown"));
        assert_eq!(
            result
                .provider_invocation_receipt
                .as_ref()
                .map(|receipt| receipt.status),
            Some(ProviderInvocationStatus::RemoteUnknown)
        );
        let persisted =
            crate::provider_validation::load_provider_validation_record_from_path(&validation_path)
                .as_record()
                .expect("remote-unknown probe must persist a valid validation record")
                .clone();
        assert_eq!(
            crate::provider_validation::summarize_provider_validation(
                &validation_config,
                Some(&persisted),
                chrono::Utc::now(),
            )
            .status,
            "remote_unknown"
        );
    }

    #[tokio::test]
    async fn explicit_provider_probe_ask_stages_review_and_performs_zero_dispatch() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut runtime_config = state.config.lock().await.clone();
        runtime_config.system.network_policy = openlife_core::config::NetworkPolicy::default();
        state.replace_provider_runtime_config(runtime_config).await;
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = base;
        config.llm.openai_key = "sk-test".into();
        config.llm.chat_model = "gpt-test".into();
        let dir = tempfile::tempdir().unwrap();
        let validation_path = dir.path().join("provider-validation.json");

        let result =
            test_llm_connection_with_state_and_validation_path(config, &state, &validation_path)
                .await
                .unwrap();
        assert!(!result.ok);
        assert_eq!(result.validation_status, "consent_required");
        assert!(result.review_proposal_id.is_some());
        assert!(result.provider_invocation_receipt.is_none());
        assert!(!validation_path.exists());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "an Ask decision must stage review before any provider dispatch"
        );
    }

    #[tokio::test]
    async fn provider_network_ask_reuses_review_workflow_and_allow_once_is_recoverable() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let policy = openlife_core::config::NetworkPolicy::default();
        let capability = "provider.openai";
        let url = "https://api.openai.com/v1/chat/completions";
        let ask = resolve_network_policy_decision(&policy, url, capability).unwrap();

        let proposal_id = match authorize_provider_network_dispatch(
            &state,
            &policy,
            &ask,
            url,
            capability,
            "openai",
            None,
            NetworkConsentSubmissionScope::ExplicitCommand,
        )
        .await
        .unwrap()
        {
            ProviderNetworkAuthorization::ConsentRequired { proposal_id } => proposal_id,
            _ => panic!("Ask must stage consent without dispatch authorization"),
        };
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            proposal.proposal_type,
            openlife_core::agent::ProposalType::ToolPermission
        );
        assert_eq!(
            proposal.status,
            openlife_core::agent::ProposalStatus::Pending
        );
        assert_eq!(
            proposal.source,
            openlife_core::agent::ProposalSource::NetworkConsent,
            "an explicit Settings probe must not claim Main Chat proposal authority"
        );

        crate::commands::proposal::accept_proposal_with_state(proposal_id, &state)
            .await
            .unwrap();
        let authorized = authorize_provider_network_dispatch(
            &state,
            &policy,
            &ask,
            url,
            capability,
            "openai",
            None,
            NetworkConsentSubmissionScope::ExplicitCommand,
        )
        .await
        .unwrap();
        match authorized {
            ProviderNetworkAuthorization::Authorized {
                network_policy,
                network_policy_decision,
                permission_id,
                ..
            } => {
                assert_eq!(
                    network_policy_decision.disposition,
                    openlife_core::network_client::NetworkPolicyDisposition::Allow
                );
                assert_eq!(
                    network_policy
                        .tool_overrides
                        .get(capability)
                        .map(String::as_str),
                    Some("allow")
                );
                assert!(permission_id.is_some());
            }
            _ => panic!("accepted AllowOnce must authorize exactly one retry"),
        }

        assert!(matches!(
            authorize_provider_network_dispatch(
                &state,
                &policy,
                &ask,
                url,
                capability,
                "openai",
                None,
                NetworkConsentSubmissionScope::ExplicitCommand,
            )
            .await
            .unwrap(),
            ProviderNetworkAuthorization::ConsentRequired { .. }
        ));
    }

    #[tokio::test]
    async fn replacing_provider_runtime_config_invalidates_cached_provider_truth() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        *state.provider_health_cache.lock().await = Some(crate::state::ProviderHealthCache {
            providers: Vec::new(),
            checked_at: chrono::Utc::now().to_rfc3339(),
            identity_digest: "stale-provider-identity".into(),
        });
        let mut replacement = AppConfig::default();
        replacement.llm.provider = "openai".into();
        replacement.llm.openai_base = "https://api.openai.com/v1/changed-path".into();
        replacement.llm.chat_model = "changed-model".into();
        replacement.llm.openai_key = String::new();

        replace_runtime_provider_config(&state, replacement).await;

        assert!(state.provider_health_cache.lock().await.is_none());
        let scheduler = state.scheduler.lock().await;
        assert_eq!(
            scheduler.openai_base,
            "https://api.openai.com/v1/changed-path"
        );
        assert_eq!(scheduler.chat_model, "changed-model");
        assert!(scheduler.openai_key.is_empty());
    }

    #[tokio::test]
    async fn concurrent_provider_replacement_never_exposes_a_mixed_status_generation() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let configured = |suffix: &str, credential_version: u64| {
            let mut config = AppConfig {
                local_model: format!("local-{suffix}"),
                prefer_local_model: false,
                ..Default::default()
            };
            config.llm.provider = "openai".into();
            config.llm.openai_base = format!("https://api.example.test/{suffix}");
            config.llm.openai_key = format!("sk-{suffix}");
            config.llm.chat_model = format!("model-{suffix}");
            config.llm.credential_version = credential_version;
            config
        };
        let first = configured("generation-a", 41);
        let second = configured("generation-b", 42);
        replace_runtime_provider_config(&state, first.clone()).await;

        let writer = async {
            for index in 0..64 {
                let next = if index % 2 == 0 {
                    second.clone()
                } else {
                    first.clone()
                };
                replace_runtime_provider_config(&state, next).await;
                tokio::task::yield_now().await;
            }
        };
        let reader = async {
            for _ in 0..128 {
                let snapshot = state.provider_runtime_snapshot().await;
                assert!(
                    snapshot.coherent,
                    "a status snapshot must never combine config and adapter generations"
                );
                let observed = (
                    snapshot.config.llm.openai_base.as_str(),
                    snapshot.scheduler.openai_base.as_str(),
                    snapshot.config.llm.chat_model.as_str(),
                    snapshot.scheduler.chat_model.as_str(),
                    snapshot.config.llm.credential_version,
                    snapshot.scheduler.provider_credential_version(),
                );
                assert!(matches!(
                    observed,
                    (
                        "https://api.example.test/generation-a",
                        "https://api.example.test/generation-a",
                        "model-generation-a",
                        "model-generation-a",
                        41,
                        41
                    ) | (
                        "https://api.example.test/generation-b",
                        "https://api.example.test/generation-b",
                        "model-generation-b",
                        "model-generation-b",
                        42,
                        42
                    )
                ));
                assert!(!snapshot
                    .scheduler
                    .provider_config_generation()
                    .trim()
                    .is_empty());
                tokio::task::yield_now().await;
            }
        };

        tokio::join!(writer, reader);
    }

    #[test]
    fn danger_action_preflight_returns_safe_data_export_scope() {
        let view = danger_action_preflight_for_action("data_export", false).unwrap();

        assert_eq!(view.action_type, "data_export");
        assert_eq!(view.risk_tier, "high");
        assert_eq!(
            view.data_categories,
            vec!["life_model", "state_store", "messages", "vectors"]
        );
        assert!(!view.writes_durable_state);
        assert!(view.privacy_sensitive);
        assert_eq!(view.external_transmission, "not_sent_externally");
        assert_eq!(view.backup_status, "not_required_read_only");
        assert!(view.confirmation_required);
        assert!(!view.requires_typed_confirmation);
        assert!(view.confirmation_phrase.is_none());
        assert!(view.final_action_enabled);
        assert!(!view.safe_mode_blocked);
        assert!(view
            .source_refs
            .iter()
            .any(|source| source == "final_command:export_all_data"));
    }

    #[test]
    fn danger_action_preflight_marks_import_overwrite_as_critical_without_claiming_existing_snapshot(
    ) {
        let view = danger_action_preflight_for_action("data_import_overwrite", false).unwrap();

        assert_eq!(view.action_type, "data_import_overwrite");
        assert_eq!(view.risk_tier, "critical");
        assert!(view.writes_durable_state);
        assert!(view.privacy_sensitive);
        assert_eq!(view.external_transmission, "not_sent_externally");
        assert_eq!(
            view.backup_status,
            "lifemodel_snapshot_only_other_owners_forward_recovery"
        );
        assert!(view.final_action_enabled);
        assert!(view
            .source_refs
            .iter()
            .any(|source| source
                == "governed_request:create_lifemodel_pre_change_snapshot_on_execute"));

        let serialized = serde_json::to_string(&view).unwrap();
        for forbidden in [
            "snapshot_available",
            "snapshot_exists",
            "existing_snapshot",
            "already_created",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "import preflight must not claim existing snapshot via {forbidden}"
            );
        }
    }

    #[test]
    fn danger_action_preflight_marks_audit_export_as_sensitive_read_only() {
        let view = danger_action_preflight_for_action("mcp_audit_export", false).unwrap();

        assert_eq!(view.action_type, "mcp_audit_export");
        assert_eq!(view.risk_tier, "high");
        assert_eq!(
            view.data_categories,
            vec![
                "mcp_audit_metadata",
                "tool_metadata",
                "tool_input_text",
                "tool_output_text"
            ]
        );
        assert!(view.scope_summary.contains("工具输入参数文本"));
        assert!(view.scope_summary.contains("工具执行结果文本"));
        assert!(!view.writes_durable_state);
        assert!(view.privacy_sensitive);
        assert_eq!(view.external_transmission, "not_sent_externally");
        assert_eq!(view.backup_status, "not_required_read_only");
        assert!(view.confirmation_required);
        assert!(!view.requires_typed_confirmation);
        assert!(view.confirmation_phrase.is_none());
        assert!(view.final_action_enabled);
    }

    #[test]
    fn danger_action_preflight_marks_cleanup_and_key_rotation_as_mutating() {
        for action_type in ["mcp_audit_cleanup", "mcp_audit_key_rotation"] {
            let view = danger_action_preflight_for_action(action_type, false).unwrap();
            assert_eq!(view.action_type, action_type);
            assert!(view.writes_durable_state);
            assert!(view.privacy_sensitive);
            assert_eq!(view.external_transmission, "not_sent_externally");
            assert!(view.final_action_enabled);
            assert!(!view.safe_mode_blocked);
            assert!(
                view.backup_status == "none"
                    || view.backup_status == "historical_key_epochs_retained"
            );
        }
    }

    #[test]
    fn danger_action_preflight_covers_run_delete_and_vector_rebuild_without_raw_scope_leaks() {
        let view = danger_action_preflight_for_action_scoped(
            "agent_run_bulk_delete",
            false,
            DangerActionPreflightScope {
                target_ids: vec!["run-private-1".into(), "run-private-2".into()],
                affected_count: Some(2),
            },
        )
        .unwrap();

        assert_eq!(view.action_type, "agent_run_bulk_delete");
        assert!(view.writes_durable_state);
        assert!(view.confirmation_required);
        assert!(!view.requires_typed_confirmation);
        assert!(view.confirmation_phrase.is_none());
        assert_eq!(view.affected_item_count, 2);
        assert!(view.affected_item_digest.starts_with("bytes:"));
        assert!(view
            .source_refs
            .iter()
            .any(|source| source == "final_command:delete_agent_run"));
        let serialized = serde_json::to_string(&view).unwrap();
        assert!(!serialized.contains("run-private-1"));
        assert!(!serialized.contains("run-private-2"));

        let vector = danger_action_preflight_for_action_scoped(
            "vector_rebuild",
            false,
            DangerActionPreflightScope {
                target_ids: vec![],
                affected_count: Some(12),
            },
        )
        .unwrap();
        assert_eq!(vector.action_type, "vector_rebuild");
        assert!(vector.confirmation_required);
        assert!(!vector.requires_typed_confirmation);
        assert!(vector.confirmation_phrase.is_none());
        assert_eq!(vector.affected_item_count, 12);
        assert!(vector
            .source_refs
            .iter()
            .any(|source| source == "final_command:rebuild_memory_index"));
    }

    #[test]
    fn deterministic_preflight_view_cannot_mint_confirmation_authority() {
        let view = danger_action_preflight_for_action_scoped(
            "agent_run_delete",
            false,
            DangerActionPreflightScope {
                target_ids: vec!["run-confirm-1".to_string()],
                affected_count: Some(1),
            },
        )
        .unwrap();
        assert!(view.confirmation_required);
        assert!(!view.requires_typed_confirmation);
        assert!(view.confirmation_phrase.is_none());
        assert!(view.preflight_id.is_empty());
        assert!(!view
            .source_refs
            .iter()
            .any(|source| source == "native_confirmation:server_challenge_pending"));
    }

    #[test]
    fn danger_action_preflight_safe_mode_blocks_destructive_actions() {
        for action_type in [
            "data_import_overwrite",
            "mcp_audit_cleanup",
            "mcp_audit_key_rotation",
            "agent_run_delete",
            "agent_run_bulk_delete",
            "vector_rebuild",
        ] {
            let view = danger_action_preflight_for_action(action_type, true).unwrap();
            assert!(view.writes_durable_state);
            assert!(view.safe_mode_blocked);
            assert!(!view.final_action_enabled);
            assert_eq!(
                view.blocking_reasons,
                vec!["safe_mode_blocks_durable_write"]
            );
        }

        for action_type in ["data_export", "mcp_audit_export"] {
            let view = danger_action_preflight_for_action(action_type, true).unwrap();
            assert!(!view.writes_durable_state);
            assert!(!view.safe_mode_blocked);
            assert!(view.final_action_enabled);
            assert!(view.blocking_reasons.is_empty());
        }
    }

    #[tokio::test]
    async fn governed_import_recovery_preflight_uses_typed_same_process_health_not_warning_text() {
        let mut state = crate::test_utils::test_app_state();
        let coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        coordinator.seal();
        coordinator.degrade_globally(GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON);
        Arc::get_mut(&mut state)
            .expect("isolated test state must be uniquely owned")
            .persistence_coordinator = coordinator;
        assert!(
            state.startup_warnings.is_empty(),
            "same-process drift does not create a bootstrap warning"
        );

        let journal = required_governed_data_import_journal(&state).unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        journal
            .prepare(GovernedDataImportPrepare {
                operation_id: operation_id.clone(),
                payload_digest: metadata_digest("same-process recovery payload"),
                request_digest: metadata_digest("same-process recovery request"),
                owners: vec![GovernedDataImportOwnerPlan {
                    owner: "LifeModelFileStore".into(),
                    import_target: "life_model".into(),
                    before_digest: metadata_digest("same-process owner before"),
                    target_digest: metadata_digest("same-process owner target"),
                    item_count: 1,
                }],
            })
            .unwrap();

        let recovery = governed_import_recovery_preflight_receipt(&state)
            .await
            .unwrap()
            .expect("typed coordinator reason and durable receipt must enable recovery preflight");
        assert_eq!(recovery.operation_id, operation_id);
        assert_eq!(
            state.persistence_coordinator.snapshot().global_reason_codes,
            vec![GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON.to_string()]
        );
    }

    #[test]
    fn governed_import_product_paths_reuse_the_app_state_journal_owner() {
        let state = crate::test_utils::test_app_state();
        let journal_path = state
            .life_model_manager
            .try_lock()
            .expect("isolated LifeModel manager must be uncontended")
            .mutation_journal_path();
        let schema_version = || {
            let connection = rusqlite::Connection::open_with_flags(
                &journal_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            connection
                .pragma_query_value(None, "schema_version", |row| row.get::<_, i64>(0))
                .unwrap()
        };
        let schema_before_reads = schema_version();
        let bootstrap_owned = state
            .governed_data_import_journal
            .as_ref()
            .expect("isolated state must install one governed import journal");
        let first = required_governed_data_import_journal(&state).unwrap();
        let second = required_governed_data_import_journal(&state).unwrap();
        first.latest_receipt().unwrap();
        second.latest_receipt().unwrap();

        assert!(Arc::ptr_eq(bootstrap_owned, &first));
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            schema_version(),
            schema_before_reads,
            "repeated status reads through the shared journal must not run migration or DDL"
        );
    }

    #[test]
    fn governed_import_product_paths_fail_closed_when_bootstrap_journal_is_unavailable() {
        let mut state = crate::test_utils::test_app_state();
        Arc::get_mut(&mut state)
            .expect("isolated test state must be uniquely owned")
            .governed_data_import_journal = None;

        let error = match required_governed_data_import_journal(&state) {
            Ok(_) => panic!("missing bootstrap journal must fail closed"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            AppError::Database { hint: Some(ref hint), .. }
                if hint == "data_import_journal_unavailable"
        ));
    }

    #[test]
    fn governed_import_abandonment_result_separates_runtime_restart_from_original_effect_truth() {
        let directory = tempfile::tempdir().unwrap();
        let journal =
            GovernedDataImportJournal::new(directory.path().join("abandonment-result.db")).unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let target_digest = metadata_digest("observed imported target");
        let prepared = journal
            .prepare(GovernedDataImportPrepare {
                operation_id: operation_id.clone(),
                payload_digest: metadata_digest("abandonment result payload"),
                request_digest: metadata_digest("abandonment result request"),
                owners: vec![GovernedDataImportOwnerPlan {
                    owner: "LifeModelFileStore".into(),
                    import_target: "life_model".into(),
                    before_digest: metadata_digest("owner before"),
                    target_digest: target_digest.clone(),
                    item_count: 1,
                }],
            })
            .unwrap()
            .receipt;
        let terminal = journal
            .abandon_preserving_current(
                &operation_id,
                &[GovernedDataImportOwnerObservation {
                    owner: "LifeModelFileStore".into(),
                    observed_digest: target_digest.clone(),
                    observed_at: chrono::Utc::now(),
                    state_restore_request_digest: None,
                    state_restore_payload_digest: None,
                    state_restore_before_canonical_digest: None,
                    state_restore_after_canonical_digest: None,
                    state_restore_outbox_event_id: None,
                    state_projection_delivery_state: None,
                }],
                &metadata_digest("explicit preserve current"),
            )
            .unwrap();
        assert_eq!(prepared.operation_id, terminal.operation_id);

        let same_process = governed_import_abandonment_result(&terminal, true);
        assert_eq!(
            same_process["status"],
            "abandoned_preserving_current_restart_required"
        );
        assert_eq!(same_process["restart_required"], true);
        assert_eq!(same_process["success"], true);
        assert_eq!(same_process["recovery_terminalized"], true);
        assert_eq!(same_process["original_import_completed"], false);
        assert_eq!(same_process["rollback_completed"], false);
        assert_eq!(same_process["abandonment_mutated_canonical_owners"], false);
        assert_eq!(same_process["owner_resolution_counts"]["target"], 1);
        assert!(same_process.get("canonical_effect_committed").is_none());

        let clean_restart_replay = governed_import_abandonment_result(&terminal, false);
        assert_eq!(
            clean_restart_replay["status"],
            "abandoned_preserving_current"
        );
        assert_eq!(clean_restart_replay["restart_required"], false);

        let durable_status =
            governed_data_import_status_view(Some(&terminal), false, "2026-07-17T00:00:00Z".into());
        assert_eq!(durable_status.status, "abandoned_preserving_current");
        assert!(durable_status.terminal);
        assert!(durable_status.preserved_current);
        assert!(!durable_status.restart_required);
        assert_eq!(durable_status.owner_resolution_counts.target, 1);
        let serialized = serde_json::to_string(&durable_status).unwrap();
        assert!(!serialized.contains(&target_digest));
        assert!(!serialized.contains("observedDigest"));
        assert!(!serialized.contains("payloadDigest"));
    }

    #[test]
    fn danger_action_preflight_rejects_unknown_action_type() {
        let err =
            danger_action_preflight_for_action("/tmp/sk-secret-unknown-action", false).unwrap_err();

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert_eq!(
            err.message(),
            "unsupported danger action preflight action type"
        );
        assert!(!err.message().contains("/tmp"));
        assert!(!err.message().contains("sk-secret"));
    }

    #[test]
    fn danger_action_preflight_never_serializes_payload_paths_or_key_material() {
        let views = [
            "data_export",
            "data_import_overwrite",
            "mcp_audit_export",
            "mcp_audit_cleanup",
            "mcp_audit_key_rotation",
            "agent_run_delete",
            "agent_run_bulk_delete",
            "vector_rebuild",
        ]
        .into_iter()
        .map(|action_type| danger_action_preflight_for_action(action_type, true).unwrap())
        .collect::<Vec<_>>();
        let serialized = serde_json::to_string(&views).unwrap();

        for forbidden in [
            "/tmp/",
            "/Users/",
            "C:\\",
            "sk-secret",
            "Bearer ",
            "api_key",
            "openai_key",
            "keyring",
            "payload",
            "arguments",
            "results",
            "raw_import",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "danger preflight leaked forbidden marker {forbidden}: {serialized}"
            );
        }
    }

    async fn seed_current_data(state: &Arc<AppState>) {
        let current_model = {
            let manager = state.life_model_manager.lock().await;
            let mut model = manager.load().unwrap();
            model.identity.name = W84_IMPORT_CURRENT_NAME_SECRET.into();
            manager.save(&model).unwrap();
            model
        };
        crate::state_projection::reconcile_and_import_legacy_yaml_daily_tasks(
            state.state_store.as_ref().expect("test StateStore"),
            &current_model,
            chrono::Utc::now(),
        )
        .expect("test StateStore daily-task owner receipt");
        {
            let store = state.memory_store.lock().await;
            store
                .save_message(
                    "w84-current-session",
                    &ChatMessage {
                        role: "user".into(),
                        content: W84_IMPORT_CURRENT_MESSAGE_SECRET.into(),
                    },
                )
                .unwrap();
        }
        {
            let store = state.vector_store.lock().await;
            let profile = openlife_core::embedding::EmbeddingProfile::new(
                openlife_core::embedding::EmbeddingRouteKind::DeterministicHash,
                "openlife-test",
                "settings-import-test-v1",
                "builtin:test",
                "settings-import-test-artifact-v1",
                4,
            )
            .unwrap();
            store
                .insert(
                    "w84-current-session",
                    W84_IMPORT_CURRENT_VECTOR_SECRET,
                    &[0.1, 0.2, 0.3, 0.4],
                    &profile,
                    "w84-current",
                )
                .unwrap();
        }
    }

    fn import_payload() -> serde_json::Value {
        let mut model = LifeModel::default_model();
        model.identity.name = W84_IMPORT_PAYLOAD_NAME_SECRET.into();
        serde_json::json!({
            "version": "1.0",
            "life_model": model,
            "messages": [{
                "session_id": "w84-import-session",
                "role": "assistant",
                "content": W84_IMPORT_PAYLOAD_MESSAGE_SECRET,
                "created_at": "2026-06-03T00:00:00Z"
            }],
            "vectors": [{
                "session_id": "w84-import-session",
                "content": W84_IMPORT_PAYLOAD_VECTOR_SECRET,
                "embedding": [0.4, 0.3, 0.2, 0.1],
                "source": "w84-import",
                "created_at": "2026-06-03T00:00:00Z",
                "tier": 2,
                "access_count": 0,
                "last_accessed_at": "",
                "importance_score": 0.5,
                "archived": false,
                "archived_at": null,
                "summary": null
            }]
        })
    }

    async fn exported_message_contents(state: &Arc<AppState>) -> Vec<String> {
        state
            .memory_store
            .lock()
            .await
            .export_all_messages()
            .unwrap()
            .into_iter()
            .map(|message| message.content)
            .collect()
    }

    async fn exported_vector_contents(state: &Arc<AppState>) -> Vec<String> {
        state
            .vector_store
            .lock()
            .await
            .export_all_chunks()
            .unwrap()
            .into_iter()
            .map(|chunk| chunk.content)
            .collect()
    }

    async fn current_model_name(state: &Arc<AppState>) -> String {
        state
            .life_model_manager
            .lock()
            .await
            .load()
            .unwrap()
            .identity
            .name
    }

    fn create_test_daily_task(state: &Arc<AppState>, title: &str) {
        let now = chrono::Utc::now();
        state
            .state_store
            .as_ref()
            .expect("test StateStore")
            .create_daily_task(openlife_core::state_store::CreateDailyTaskCommand {
                operation_id: Uuid::new_v4().hyphenated().to_string(),
                request_digest: None,
                source_message_ref: format!("settings-test:{}", Uuid::new_v4()),
                title: title.into(),
                due_at: Some(now + chrono::Duration::hours(6)),
                created_at: now,
                expires_at: now + chrono::Duration::days(2),
                risk: openlife_core::state_store::StateRisk::Low,
                sensitivity: openlife_core::state_store::StateSensitivity::Internal,
                source_kind:
                    openlife_core::state_store::StateSourceKind::CurrentAuthenticatedUserMessage,
                confidence: 1.0,
                privacy_class: openlife_core::state_store::StatePrivacyClass::Private,
            })
            .expect("create canonical test daily task");
    }

    #[tokio::test]
    async fn w93_import_all_data_without_governed_request_fails_closed() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;

        let err = import_all_data_with_state(import_payload(), &state)
            .await
            .expect_err("data import must fail closed by default");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("import_all_data"));
        assert!(err.message().contains("governed import request"));
        assert!(err.message().contains("explicitUserIntent=true"));
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_CURRENT_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_CURRENT_VECTOR_SECRET.to_string()]
        );
    }

    #[tokio::test]
    async fn w93_import_all_data_governed_request_allows_metadata_safe_import() {
        let state = release_like_test_app_state();
        seed_current_data(&state).await;

        let result = import_all_data_with_state_for_governed_import(
            import_payload(),
            &state,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["legacy"], false);
        assert_eq!(result["governed_operation"], true);
        assert_eq!(result["operation_kind"], "data_import");
        assert_eq!(result["operation_purpose"], "manual_restore");
        assert_eq!(result["metadata_safe"], true);
        assert_eq!(result["contains_raw_content"], false);
        assert_eq!(result["durable_lifemodel_write"], true);
        assert_eq!(result["imported_message_count"], 1);
        assert_eq!(result["imported_vector_count"], 1);
        assert!(result["import_payload_hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert!(result["previous_model_hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert!(result["imported_model_hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert_eq!(result["pre_import_snapshot_created"], true);
        assert!(result["pre_import_snapshot_version"].is_string());
        assert_eq!(result["audit"]["metadata_safe"], true);
        assert_eq!(result["audit"]["contains_raw_content"], false);
        assert!(result.get("life_model").is_none());
        assert!(result.get("messages").is_none());
        assert!(result.get("vectors").is_none());
        assert!(result.get("payload").is_none());
        assert!(result.get("import_payload").is_none());

        let response_dump = result.to_string();
        for forbidden in [
            W84_IMPORT_CURRENT_NAME_SECRET,
            W84_IMPORT_PAYLOAD_NAME_SECRET,
            W84_IMPORT_CURRENT_MESSAGE_SECRET,
            W84_IMPORT_PAYLOAD_MESSAGE_SECRET,
            W84_IMPORT_CURRENT_VECTOR_SECRET,
            W84_IMPORT_PAYLOAD_VECTOR_SECRET,
        ] {
            assert!(
                !response_dump.contains(forbidden),
                "data import response leaked raw marker {forbidden}"
            );
        }

        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_PAYLOAD_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_PAYLOAD_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_PAYLOAD_VECTOR_SECRET.to_string()]
        );
    }

    #[tokio::test]
    async fn governed_import_with_daily_tasks_fails_closed_without_replacing_other_stores() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;
        let before_model_hash = current_lifemodel_file_hash(&state).await.unwrap();
        let mut payload = import_payload();
        payload["life_model"]["goals"]["daily"] = serde_json::json!([
            {
                "name": "must not become YAML truth",
                "done": false,
                "time_block": null
            }
        ]);

        let error = import_all_data_with_state_for_governed_import(
            payload,
            &state,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .expect_err("legacy daily tasks without TTL must be quarantined before any write");

        assert!(error.message().contains("v1 backup contains daily tasks"));
        assert!(error.message().contains("quarantined"));
        assert_eq!(
            current_lifemodel_file_hash(&state).await.unwrap(),
            before_model_hash,
            "preflight rejection must not bump metadata or rewrite canonical truth"
        );
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_CURRENT_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_CURRENT_VECTOR_SECRET.to_string()]
        );
    }

    #[tokio::test]
    async fn governed_import_missing_memory_targets_preserves_existing_memory() {
        let state = release_like_test_app_state();
        seed_current_data(&state).await;
        let mut payload = import_payload();
        payload.as_object_mut().unwrap().remove("messages");
        payload.as_object_mut().unwrap().remove("vectors");

        let result = import_all_data_with_state_for_governed_import(
            payload,
            &state,
            GovernedDataImportRequest {
                operation_id: Uuid::new_v4().hyphenated().to_string(),
                purpose: "manual_restore".into(),
                explicit_user_intent: true,
                create_pre_change_snapshot: true,
                import_targets: vec!["life_model".into()],
            },
        )
        .await
        .unwrap();

        assert_eq!(result["messages_targeted"], false);
        assert_eq!(result["vectors_targeted"], false);
        assert_eq!(result["imported_message_count"], 0);
        assert_eq!(result["imported_vector_count"], 0);
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_CURRENT_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_CURRENT_VECTOR_SECRET.to_string()]
        );
    }

    #[tokio::test]
    async fn governed_import_skips_derived_vectors_and_reports_only_applied_rows() {
        let state = release_like_test_app_state();
        seed_current_data(&state).await;
        let profile = openlife_core::embedding::EmbeddingProfile::new(
            openlife_core::embedding::EmbeddingRouteKind::DeterministicHash,
            "openlife-test",
            "settings-import-canonical-v1",
            "builtin:test",
            "settings-import-canonical-artifact-v1",
            4,
        )
        .unwrap();
        let owner = openlife_core::vectors::CanonicalVectorOwnerRef::new(
            "knowledge_note",
            "settings-import-owner",
        )
        .unwrap();
        state
            .vector_store
            .lock()
            .await
            .project_memory_embedding(
                "outbox:settings-import-owner",
                &owner,
                "canonical-settings-session",
                "CANONICAL_DESTINATION_VECTOR",
                &[0.1, 0.3, 0.2, 0.4],
                &profile,
            )
            .unwrap();

        let mut payload = import_payload();
        let portable = payload["vectors"][0].clone();
        let mut canonical = portable.clone();
        canonical["source"] = serde_json::Value::String(owner.source());
        canonical["content"] = serde_json::Value::String("SPOOFED_CANONICAL_VECTOR".into());
        let mut legacy_chat = portable;
        legacy_chat["source"] = serde_json::Value::String("user_message".into());
        legacy_chat["content"] = serde_json::Value::String("LEGACY_CHAT_VECTOR".into());
        payload["vectors"]
            .as_array_mut()
            .unwrap()
            .extend([canonical, legacy_chat]);

        let result = import_all_data_with_state_for_governed_import(
            payload,
            &state,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .unwrap();

        assert_eq!(result["supplied_vector_count"], 3);
        assert_eq!(result["imported_vector_count"], 1);
        assert_eq!(result["skipped_vector_count"], 2);
        assert_eq!(result["skipped_canonical_vector_count"], 1);
        assert_eq!(result["skipped_legacy_chat_vector_count"], 1);
        let vectors = state.vector_store.lock().await.export_all_chunks().unwrap();
        assert!(vectors
            .iter()
            .any(|chunk| chunk.content == "CANONICAL_DESTINATION_VECTOR"));
        assert!(vectors
            .iter()
            .any(|chunk| chunk.content == W84_IMPORT_PAYLOAD_VECTOR_SECRET));
        assert!(!vectors
            .iter()
            .any(|chunk| chunk.content == "SPOOFED_CANONICAL_VECTOR"));
        assert!(!vectors
            .iter()
            .any(|chunk| chunk.content == "LEGACY_CHAT_VECTOR"));

        let exported = export_all_data_with_state(&state).await.unwrap();
        assert_eq!(
            exported["vector_export_semantics"],
            "portable_only_canonical_and_chat_projections_derived"
        );
        assert_eq!(exported["vectors"].as_array().unwrap().len(), 1);
        assert_eq!(
            exported["vectors"][0]["content"],
            W84_IMPORT_PAYLOAD_VECTOR_SECRET
        );
    }

    #[tokio::test]
    async fn governed_import_vector_tombstone_failure_restores_all_preimport_truth() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;
        let before_model_hash = current_lifemodel_file_hash(&state).await.unwrap();
        state
            .vector_store
            .lock()
            .await
            .project_conversation_tombstone("settings-import-tombstone", "blocked-import-session")
            .unwrap();
        let mut payload = import_payload();
        payload["messages"][0]["session_id"] =
            serde_json::Value::String("blocked-import-session".into());
        payload["vectors"][0]["session_id"] =
            serde_json::Value::String("blocked-import-session".into());

        let error = import_all_data_with_state_for_governed_import(
            payload,
            &state,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .expect_err("a projected conversation tombstone must reject archive resurrection");
        assert!(
            error.message().contains("已恢复到导入前状态"),
            "unexpected import failure: {}",
            error.message()
        );
        assert_eq!(
            current_lifemodel_file_hash(&state).await.unwrap(),
            before_model_hash,
            "compensation must restore the exact pre-import physical model hash"
        );
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_CURRENT_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_CURRENT_VECTOR_SECRET.to_string()]
        );
    }

    #[tokio::test]
    async fn w93_import_all_data_invalid_governed_request_fails_closed() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;

        let err = import_all_data_with_state_gated(
            import_payload(),
            &state,
            Some(GovernedDataImportRequest {
                operation_id: Uuid::new_v4().hyphenated().to_string(),
                purpose: "normal_product".into(),
                explicit_user_intent: true,
                create_pre_change_snapshot: true,
                import_targets: vec!["life_model".into()],
            }),
        )
        .await
        .expect_err("invalid governed import purpose must fail closed");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("manual_restore"));
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
    }

    #[test]
    fn governed_import_contract_rejects_invalid_operation_id_and_duplicate_targets() {
        let invalid_operation = GovernedDataImportRequest {
            operation_id: "not-a-uuid".into(),
            purpose: "manual_restore".into(),
            explicit_user_intent: true,
            create_pre_change_snapshot: true,
            import_targets: vec!["life_model".into()],
        };
        assert!(!invalid_operation.is_valid());

        let duplicate_target = GovernedDataImportRequest {
            operation_id: Uuid::new_v4().hyphenated().to_string(),
            purpose: "manual_restore".into(),
            explicit_user_intent: true,
            create_pre_change_snapshot: true,
            import_targets: vec!["life_model".into(), "life_model".into()],
        };
        assert!(!duplicate_target.is_valid());
    }

    #[test]
    fn governed_import_payload_version_and_state_store_contract_fail_closed() {
        let mut v2_missing_state_store = import_payload();
        v2_missing_state_store["version"] = serde_json::json!("2.0");
        let missing_error = validate_import_payload_shape(&v2_missing_state_store)
            .expect_err("v2 must not silently omit canonical StateStore data");
        assert!(missing_error.message().contains("state_store"));

        let mut v1_with_state_store = import_payload();
        v1_with_state_store["state_store"] = serde_json::json!({
            "schema": "openlife.state-store-daily-tasks-portable.v1",
            "exportedAt": "2026-06-03T00:00:00Z",
            "dailyTasks": []
        });
        let mixed_error = validate_import_payload_shape(&v1_with_state_store)
            .expect_err("v1 must not smuggle a v2 owner payload");
        assert!(matches!(mixed_error, AppError::PermissionDenied { .. }));
        assert!(mixed_error.message().contains("v1"));
    }

    #[test]
    fn governed_import_payload_resource_limits_fail_before_owner_parsing() {
        let mut too_many_messages = import_payload();
        too_many_messages["messages"] = serde_json::Value::Array(vec![
            serde_json::Value::Null;
            MAX_GOVERNED_IMPORT_MESSAGES
                + 1
        ]);
        let message_error = validate_import_payload_shape(&too_many_messages).unwrap_err();
        assert!(message_error.message().contains("message import limit"));

        let mut oversized_text = import_payload();
        oversized_text["life_model"]["identity"]["name"] =
            serde_json::Value::String("x".repeat(MAX_GOVERNED_IMPORT_STRING_BYTES + 1));
        let text_error = validate_import_payload_shape(&oversized_text).unwrap_err();
        assert!(text_error.message().contains("oversized text field"));

        let mut too_many_tasks = import_payload();
        too_many_tasks["version"] = serde_json::json!("2.0");
        too_many_tasks["state_store"] = serde_json::json!({
            "dailyTasks": vec![serde_json::Value::Null; MAX_GOVERNED_IMPORT_STATE_TASKS + 1]
        });
        let task_error = validate_import_payload_shape(&too_many_tasks).unwrap_err();
        assert!(task_error.message().contains("daily-task import limit"));

        let mut nested = serde_json::Value::Null;
        for _ in 0..=MAX_GOVERNED_IMPORT_JSON_DEPTH {
            nested = serde_json::json!({"nested": nested});
        }
        let mut excessive_depth = import_payload();
        excessive_depth["life_model"] = nested;
        let depth_error = validate_import_payload_shape(&excessive_depth).unwrap_err();
        assert!(depth_error.message().contains("JSON nesting limit"));
    }

    #[tokio::test]
    async fn w93_import_all_data_payload_targets_must_match_governed_request() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;

        let err = import_all_data_with_state_for_governed_import(
            import_payload(),
            &state,
            GovernedDataImportRequest {
                operation_id: Uuid::new_v4().hyphenated().to_string(),
                purpose: "manual_restore".into(),
                explicit_user_intent: true,
                create_pre_change_snapshot: true,
                import_targets: vec!["life_model".into()],
            },
        )
        .await
        .expect_err("payload targets outside the governed request must fail closed");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("messages"));
        assert!(err.message().contains("import target"));
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_CURRENT_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_CURRENT_VECTOR_SECRET.to_string()]
        );
    }

    #[tokio::test]
    async fn w93_import_all_data_unsupported_payload_target_fails_closed() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;
        let mut payload = import_payload();
        payload["unsupported_target"] =
            serde_json::json!({"secret": W84_IMPORT_PAYLOAD_NAME_SECRET});

        let err = import_all_data_with_state_for_governed_import(
            payload,
            &state,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .expect_err("unsupported import target must fail closed");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("unsupported import target"));
        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
    }

    #[tokio::test]
    async fn w84_export_all_data_remains_read_only_and_ungated() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;

        let exported = export_all_data_with_state(&state).await.unwrap();

        assert_eq!(
            current_model_name(&state).await,
            W84_IMPORT_CURRENT_NAME_SECRET
        );
        assert_eq!(
            exported_message_contents(&state).await,
            vec![W84_IMPORT_CURRENT_MESSAGE_SECRET.to_string()]
        );
        assert_eq!(
            exported_vector_contents(&state).await,
            vec![W84_IMPORT_CURRENT_VECTOR_SECRET.to_string()]
        );
        assert!(exported
            .to_string()
            .contains(W84_IMPORT_CURRENT_NAME_SECRET));
        assert!(exported
            .to_string()
            .contains(W84_IMPORT_CURRENT_MESSAGE_SECRET));
        assert!(exported
            .to_string()
            .contains(W84_IMPORT_CURRENT_VECTOR_SECRET));
        assert_eq!(exported["version"], "2.0");
        assert_eq!(
            exported["state_store"]["schema"],
            "openlife.state-store-daily-tasks-portable.v1"
        );
        assert_eq!(
            exported["life_model"]["goals"]["daily"],
            serde_json::json!([]),
            "StateStore-owned daily tasks must not be copied into LifeModel backup truth"
        );
        assert_eq!(
            exported["life_model"]["state"]["alerts"],
            serde_json::json!([]),
            "derived alerts must not be copied into LifeModel backup truth"
        );
    }

    #[tokio::test]
    async fn openlife_v2_export_import_round_trips_canonical_daily_tasks_without_title_leak() {
        let source = crate::test_utils::test_app_state();
        seed_current_data(&source).await;
        create_test_daily_task(&source, "PORTABLE_DAILY_TASK_SECRET");
        let exported = export_all_data_with_state(&source).await.unwrap();

        let target = release_like_test_app_state();
        seed_current_data(&target).await;
        create_test_daily_task(&target, "REPLACED_TARGET_TASK_SECRET");
        let result = import_all_data_with_state_for_governed_import(
            exported,
            &target,
            GovernedDataImportRequest::manual_restore_all_targets(),
        )
        .await
        .unwrap();

        let tasks = target
            .state_store
            .as_ref()
            .unwrap()
            .get_product_daily_tasks()
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "PORTABLE_DAILY_TASK_SECRET");
        assert_eq!(
            tasks[0].source_kind,
            openlife_core::state_store::StateSourceKind::GovernedDataRestore
        );
        assert_eq!(result["state_store_targeted"], true);
        assert_eq!(result["state_store_restored_count"], 1);
        assert_eq!(result["state_store_projection_status"], "applied");
        let response = result.to_string();
        assert!(!response.contains("PORTABLE_DAILY_TASK_SECRET"));
        assert!(!response.contains("REPLACED_TARGET_TASK_SECRET"));
    }

    #[tokio::test]
    async fn governed_import_state_owner_marks_semantic_noop_restore_as_skipped() {
        let state = release_like_test_app_state();
        seed_current_data(&state).await;
        let archive = state
            .state_store
            .as_ref()
            .unwrap()
            .export_portable_daily_tasks(chrono::Utc::now())
            .unwrap();
        assert!(archive.daily_tasks.is_empty());
        let mut payload = import_payload();
        payload["version"] = serde_json::json!("2.0");
        payload["state_store"] = serde_json::to_value(archive).unwrap();
        let request = GovernedDataImportRequest::manual_restore_all_targets();

        import_all_data_governed_operation(payload, &state, &request)
            .await
            .unwrap();

        let journal = required_governed_data_import_journal(&state).unwrap();
        let terminal = journal.receipt(&request.operation_id).unwrap().unwrap();
        assert_eq!(terminal.stage, GovernedDataImportStage::Completed);
        assert_eq!(
            governed_import_owner(&terminal, "state_store")
                .unwrap()
                .status,
            GovernedDataImportOwnerStatus::Skipped
        );
        let restore = state
            .state_store
            .as_ref()
            .unwrap()
            .portable_daily_task_restore_receipt(&request.operation_id, false)
            .unwrap()
            .unwrap();
        assert_eq!(
            restore.before_canonical_digest, restore.after_canonical_digest,
            "StateStore owner status must use canonical before/after domains"
        );
    }

    #[tokio::test]
    async fn governed_import_replay_result_preserves_exact_degraded_projection_truth() {
        let state = crate::test_utils::test_app_state();
        seed_current_data(&state).await;
        let store = state.state_store.as_ref().unwrap();
        let restored_at = chrono::Utc::now();
        let archive = store.export_portable_daily_tasks(restored_at).unwrap();
        let request = GovernedDataImportRequest {
            operation_id: Uuid::new_v4().hyphenated().to_string(),
            purpose: "manual_restore".into(),
            explicit_user_intent: true,
            create_pre_change_snapshot: true,
            import_targets: vec!["state_store".into()],
        };
        let request_digest = hash_serializable_value(&request).unwrap();
        let receipt = store
            .restore_portable_daily_tasks(
                openlife_core::state_store::RestorePortableDailyTasksCommand {
                    operation_id: request.operation_id.clone(),
                    request_digest,
                    expected_before_digest: archive.canonical_digest.clone(),
                    archive,
                    restored_at,
                },
            )
            .unwrap();
        store
            .mark_projection_degraded(&receipt.outbox_event_id, "injected_projection_failure")
            .unwrap();

        let result = governed_import_result(
            &state,
            &request,
            &metadata_digest("replay payload"),
            &metadata_digest("previous model"),
            &metadata_digest("requested model"),
            None,
            None,
            None,
            None,
            Some(store),
            true,
            None,
            false,
            false,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result["state_store_projection_status"], "degraded");
        assert_eq!(result["audit"]["state_store_projection_status"], "degraded");
        assert_ne!(result["state_store_projection_status"], "applied");
    }

    #[tokio::test]
    async fn governed_import_replays_every_owner_commit_before_journal_crash_boundary() {
        let archive_source = crate::test_utils::test_app_state();
        seed_current_data(&archive_source).await;
        create_test_daily_task(&archive_source, "PORTABLE_CRASH_RECOVERY_TASK_SECRET");
        let state_archive = archive_source
            .state_store
            .as_ref()
            .unwrap()
            .export_portable_daily_tasks(chrono::Utc::now())
            .unwrap();
        let mut payload = import_payload();
        payload["version"] = serde_json::json!("2.0");
        payload["state_store"] = serde_json::to_value(state_archive).unwrap();

        for (fault, expected_stage) in [
            (
                GovernedImportFault::AfterLifeModelCommitBeforeJournal,
                GovernedDataImportStage::Prepared,
            ),
            (
                GovernedImportFault::AfterMemoryCommitBeforeJournal,
                GovernedDataImportStage::LifeModelApplied,
            ),
            (
                GovernedImportFault::AfterVectorCommitBeforeJournal,
                GovernedDataImportStage::MemoryApplied,
            ),
            (
                GovernedImportFault::AfterStateCommitBeforeJournal,
                GovernedDataImportStage::VectorApplied,
            ),
        ] {
            let state = release_like_test_app_state();
            seed_current_data(&state).await;
            create_test_daily_task(&state, "REPLACED_CRASH_RECOVERY_TASK_SECRET");
            let request = GovernedDataImportRequest::manual_restore_all_targets();

            let crash = import_all_data_governed_operation_with_fault(
                payload.clone(),
                &state,
                &request,
                fault,
            )
            .await
            .expect_err("fault injection must stop after owner commit and before journal advance");
            assert!(crash.message().contains("injected crash"));

            let journal = required_governed_data_import_journal(&state).unwrap();
            let interrupted = journal
                .receipt(&request.operation_id)
                .unwrap()
                .expect("interrupted import must leave a durable nonterminal receipt");
            assert_eq!(interrupted.stage, expected_stage);
            assert_eq!(
                journal
                    .recovery_requirement()
                    .unwrap()
                    .expect("interrupted import must be discoverable for recovery")
                    .operation_id,
                request.operation_id
            );

            let recovered = import_all_data_governed_operation(payload.clone(), &state, &request)
                .await
                .expect("exact replay must converge without a duplicate owner write");
            assert_eq!(recovered["success"], true);
            assert_eq!(recovered["status"], "replayed");
            assert_eq!(recovered["restart_required"], false);

            let terminal = journal
                .receipt(&request.operation_id)
                .unwrap()
                .expect("recovered import receipt");
            assert_eq!(terminal.stage, GovernedDataImportStage::Completed);
            assert!(terminal.terminal_at.is_some());
            assert!(journal.recovery_requirement().unwrap().is_none());

            let state_receipt = state
                .state_store
                .as_ref()
                .unwrap()
                .portable_daily_task_restore_receipt(&request.operation_id, true)
                .unwrap()
                .expect("StateStore must retain the exact durable restore receipt");
            assert_eq!(state_receipt.committed_at, terminal.created_at);
            assert_eq!(state_receipt.operation_id, request.operation_id);

            assert_eq!(
                current_model_name(&state).await,
                W84_IMPORT_PAYLOAD_NAME_SECRET
            );
            assert_eq!(
                exported_message_contents(&state).await,
                vec![W84_IMPORT_PAYLOAD_MESSAGE_SECRET.to_string()]
            );
            assert_eq!(
                exported_vector_contents(&state).await,
                vec![W84_IMPORT_PAYLOAD_VECTOR_SECRET.to_string()]
            );
            let tasks = state
                .state_store
                .as_ref()
                .unwrap()
                .get_product_daily_tasks()
                .unwrap();
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].title, "PORTABLE_CRASH_RECOVERY_TASK_SECRET");

            let response = recovered.to_string();
            assert!(!response.contains("PORTABLE_CRASH_RECOVERY_TASK_SECRET"));
            assert!(!response.contains("REPLACED_CRASH_RECOVERY_TASK_SECRET"));
        }
    }

    #[tokio::test]
    async fn governed_import_snapshot_is_durable_before_journal_prepare() {
        let state = release_like_test_app_state();
        seed_current_data(&state).await;
        let request = GovernedDataImportRequest::manual_restore_all_targets();
        let payload = import_payload();

        let crash = import_all_data_governed_operation_with_fault(
            payload.clone(),
            &state,
            &request,
            GovernedImportFault::AfterSnapshotBeforeJournalPrepare,
        )
        .await
        .expect_err("fault injection must stop after snapshot and before saga prepare");
        assert!(crash.message().contains("injected crash"));

        let journal = required_governed_data_import_journal(&state).unwrap();
        assert!(journal.receipt(&request.operation_id).unwrap().is_none());
        let snapshots_before_retry = state
            .version_manager
            .lock()
            .await
            .list_versions()
            .unwrap()
            .into_iter()
            .filter(|snapshot| snapshot.tag == "auto:pre-import")
            .collect::<Vec<_>>();
        assert_eq!(snapshots_before_retry.len(), 1);
        assert!(snapshots_before_retry[0]
            .yaml_content
            .contains(W84_IMPORT_CURRENT_NAME_SECRET));
        assert!(!snapshots_before_retry[0]
            .yaml_content
            .contains(W84_IMPORT_PAYLOAD_NAME_SECRET));

        let completed = import_all_data_governed_operation(payload, &state, &request)
            .await
            .expect("retry must reuse the exact pre-change snapshot before preparing the saga");
        assert_eq!(completed["status"], "completed");
        let snapshots_after_retry = state
            .version_manager
            .lock()
            .await
            .list_versions()
            .unwrap()
            .into_iter()
            .filter(|snapshot| snapshot.tag == "auto:pre-import")
            .count();
        assert_eq!(snapshots_after_retry, 1);
        assert_eq!(
            journal
                .receipt(&request.operation_id)
                .unwrap()
                .unwrap()
                .stage,
            GovernedDataImportStage::Completed
        );
    }

    #[tokio::test]
    async fn governed_import_pending_owner_late_drift_is_preserved_and_terminalized() {
        let mut state = crate::test_utils::test_app_state();
        install_release_like_persistence_coordinator(&mut state);
        seed_current_data(&state).await;
        let request = GovernedDataImportRequest::manual_restore_all_targets();

        let error = import_all_data_governed_operation_with_fault(
            import_payload(),
            &state,
            &request,
            GovernedImportFault::AfterMemoryCommitWithLateDrift,
        )
        .await
        .expect_err("late owner drift must fail closed");
        assert!(matches!(
            &error,
            AppError::Database { hint: Some(hint), .. }
                if hint == "data_import_abandoned_restart_required"
        ));

        let journal = required_governed_data_import_journal(&state).unwrap();
        let receipt = journal.receipt(&request.operation_id).unwrap().unwrap();
        assert_eq!(
            receipt.stage,
            GovernedDataImportStage::AbandonedPreservingCurrent
        );
        assert_eq!(
            governed_import_owner(&receipt, "messages").unwrap().status,
            GovernedDataImportOwnerStatus::Unknown
        );
        assert_eq!(receipt.resolution_evidence.len(), receipt.owners.len());
        assert!(journal.recovery_requirement().unwrap().is_none());
        let messages = exported_message_contents(&state).await;
        assert!(messages
            .iter()
            .any(|message| message == W84_IMPORT_PAYLOAD_MESSAGE_SECRET));
        assert!(messages
            .iter()
            .any(|message| message == "LATE_MEMORY_WRITE_MUST_NOT_BE_COMPENSATED_AWAY"));
        assert!(!error.message().contains("已恢复到导入前状态"));
        assert!(error.message().contains("did not complete or roll back"));
        assert!(
            !state
                .persistence_coordinator
                .snapshot()
                .canonical_writes_allowed
        );
    }

    #[tokio::test]
    async fn governed_import_terminal_drift_is_preserved_and_terminalized() {
        let mut state = crate::test_utils::test_app_state();
        install_release_like_persistence_coordinator(&mut state);
        seed_current_data(&state).await;
        let request = GovernedDataImportRequest::manual_restore_all_targets();

        let error = import_all_data_governed_operation_with_fault(
            import_payload(),
            &state,
            &request,
            GovernedImportFault::BeforeTerminalVerificationWithMemoryDrift,
        )
        .await
        .expect_err("terminal fact drift must fail closed");
        assert!(matches!(
            &error,
            AppError::Database { hint: Some(hint), .. }
                if hint == "data_import_abandoned_restart_required"
        ));

        let journal = required_governed_data_import_journal(&state).unwrap();
        let receipt = journal.receipt(&request.operation_id).unwrap().unwrap();
        assert_eq!(
            receipt.stage,
            GovernedDataImportStage::AbandonedPreservingCurrent
        );
        assert_eq!(
            governed_import_owner(&receipt, "messages").unwrap().status,
            GovernedDataImportOwnerStatus::Unknown
        );
        assert_eq!(receipt.resolution_evidence.len(), receipt.owners.len());
        assert!(journal.recovery_requirement().unwrap().is_none());
        assert!(exported_message_contents(&state)
            .await
            .iter()
            .any(|message| message == "LATE_TERMINAL_MEMORY_WRITE_MUST_REMAIN_CANONICAL"));
        assert!(!error.message().contains("已恢复到导入前状态"));
        assert!(error.message().contains("did not complete or roll back"));
        assert!(
            !state
                .persistence_coordinator
                .snapshot()
                .canonical_writes_allowed
        );
    }

    #[tokio::test]
    async fn governed_import_completion_fence_linearizes_owner_checks_and_terminal_journal() {
        let mut state = crate::test_utils::test_app_state();
        install_release_like_persistence_coordinator(&mut state);
        seed_current_data(&state).await;
        let request = GovernedDataImportRequest::manual_restore_all_targets();
        let old_admission = state
            .persistence_coordinator
            .admit_normal_or_governed_data_import_writes(
                &[GovernedDataImportRecoveryOwner::MemoryStore],
                None,
                "",
                "",
                "",
            )
            .unwrap();
        let (observed_first_owner, release_terminal_verification) =
            install_governed_import_terminal_observation_barrier(&request.operation_id);

        let import_state = Arc::clone(&state);
        let import_request = request.clone();
        let import = tokio::spawn(async move {
            import_all_data_governed_operation(import_payload(), &import_state, &import_request)
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), observed_first_owner)
            .await
            .expect("import reaches terminal owner observation")
            .expect("terminal observation barrier remains installed");

        let stale_coordinator = Arc::clone(&state.persistence_coordinator);
        let (stale_queued_tx, stale_queued_rx) = tokio::sync::oneshot::channel();
        let stale_writer = tokio::spawn(async move {
            let mut permit =
                Box::pin(stale_coordinator.acquire_canonical_commit_permit(&old_admission));
            // Poll the read-lock first so the writer is deterministically
            // queued behind the live completion fence before verification is
            // allowed to continue.
            tokio::select! {
                biased;
                result = &mut permit => {
                    return result.map(drop);
                }
                _ = async { let _ = stale_queued_tx.send(()); } => {}
            }
            permit.await.map(drop)
        });
        stale_queued_rx.await.unwrap();
        assert!(
            !stale_writer.is_finished(),
            "an owner admission minted before terminal observation must not bypass the fence"
        );

        release_terminal_verification.send(()).unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), import)
            .await
            .expect("terminal verification and durable Completed transition finish")
            .unwrap()
            .unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["status"], "completed");
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), stale_writer)
                .await
                .expect("stale writer is released after terminal journal commit")
                .unwrap()
                .is_err(),
            "the completion fence must invalidate admissions minted before owner observation"
        );

        let journal = required_governed_data_import_journal(&state).unwrap();
        assert_eq!(
            journal
                .receipt(&request.operation_id)
                .unwrap()
                .unwrap()
                .stage,
            GovernedDataImportStage::Completed
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite,
            "successful completion must not masquerade as recovery degradation"
        );

        memory_gateway::save_conversation_message_idempotent_with_state(
            "after-governed-import-completion",
            &ChatMessage {
                role: "user".into(),
                content: "POST_COMPLETION_WRITE_IS_ORDERED_AFTER_TERMINAL_FACT".into(),
            },
            &Uuid::new_v4().hyphenated().to_string(),
            &state,
        )
        .await
        .expect("fresh product writes remain available after the completion fence");
    }

    #[tokio::test]
    async fn completed_import_replay_does_not_relabel_later_lifemodel_hash_as_terminal_fact() {
        let mut state = crate::test_utils::test_app_state();
        install_release_like_persistence_coordinator(&mut state);
        seed_current_data(&state).await;
        let request = GovernedDataImportRequest::manual_restore_all_targets();
        let payload = import_payload();

        let completed = import_all_data_governed_operation(payload.clone(), &state, &request)
            .await
            .unwrap();
        assert_eq!(completed["status"], "completed");
        assert!(completed["final_model_hash"].is_string());
        assert_eq!(
            completed["final_model_hash_status"],
            "observed_at_terminalization"
        );

        let (mut later_model, expected_hash) = {
            let manager = state.life_model_manager.lock().await;
            let model = manager.load().unwrap();
            let hash = life_model_write_gateway::hash_life_model(&model).unwrap();
            (model, hash)
        };
        later_model.identity.name = "LATER_LEGITIMATE_LIFEMODEL_WRITE".into();
        life_model_write_gateway::restore_life_model_with_gateway(
            &state,
            &later_model,
            LifeModelMaterializerCallerContext::new(
                "completed_import_replay_counterfactual",
                LifeModelMaterializerCallerKind::GovernedRestoreImportOperation,
                LifeModelMaterializerCallerPurpose::GovernedRestoreImportOperation,
            ),
            Some(&expected_hash),
        )
        .await
        .unwrap();

        let replayed = import_all_data_governed_operation(payload, &state, &request)
            .await
            .unwrap();
        assert_eq!(replayed["status"], "replayed");
        assert!(replayed["final_model_hash"].is_null());
        assert!(replayed["audit"]["final_model_hash"].is_null());
        assert_eq!(
            replayed["final_model_hash_status"],
            "not_persisted_for_completed_replay"
        );
        assert_eq!(
            replayed["audit"]["final_model_hash_status"],
            "not_persisted_for_completed_replay"
        );
    }
}
