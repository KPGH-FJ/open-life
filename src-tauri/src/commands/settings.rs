use crate::errors::AppError;
use base64::{engine::general_purpose, Engine as _};
use once_cell::sync::Lazy as LazyLock;
use openlife_core::config::{AppConfig, SystemConfig};
use openlife_core::conversation::{ProviderConnectionRecord, ProviderModelProfileRecord};
use openlife_core::llm::{
    chat_completions_url, default_base_for_provider, effective_api_key_for_endpoint,
    provider_label, ProviderInvocationReceipt, ProviderInvocationStatus,
};
use openlife_core::mcp_audit::McpAuditStore;
use openlife_core::network_client::resolve_network_policy_decision;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use std::fs::File;
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Runtime, State};
use tauri_plugin_dialog::DialogExt;

use crate::credential_bootstrap::{
    inspect_provider_connections_credentials, inspect_search_credential_status,
    ProviderConnectionsCredentialInspection,
};
use crate::danger_action_confirmation::{
    require_native_danger_action_confirmation, NativeDangerActionRequest,
};
use crate::provider_network_consent::{
    authorize_explicit_provider_probe, ExplicitProviderProbeAuthorization,
};
use crate::secret_store::{
    create_mcp_audit_key_material, hydrate_bound_provider_secret, hydrate_or_create_integrity_key,
    inspect_existing_mcp_audit_keys, inspect_integrity_key_access, provider_connection_secret_ref,
    stage_search_config_secret, IntegrityKeyInspection, McpAuditKeyHydrationInspection,
    ProfileSecretStore, SecretStore, CANONICAL_TASK_RECEIPT_KEY_REF, MCP_AUDIT_KEY_REF_PREFIX,
    PROVIDER_CONNECTION_KEY_REF_PREFIX,
};
use crate::state::{CredentialBootstrapSnapshot, CredentialBootstrapStatus};
use crate::storage::{
    app_data_dir, load_mcp_audit_keyring_from_path, mcp_audit_keyring_bytes,
    save_mcp_audit_keyring_to_path, McpAuditKeyringBytes, McpAuditKeyringLoad,
};
use crate::AppState;
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
            CANONICAL_TASK_RECEIPT_KEY_REF,
            &["task_runtime.db"],
        ),
        inspect_mcp_audit_credential_status(data_dir, store),
    ])
}

fn inspect_current_credential_snapshot(
    data_dir: &Path,
    config: &AppConfig,
    provider_connections: Option<&[ProviderConnectionRecord]>,
    selected_provider_connection: Option<&ProviderConnectionRecord>,
    store: &dyn SecretStore,
) -> CredentialBootstrapSnapshot {
    let provider_inspection = provider_connections.map_or(
        ProviderConnectionsCredentialInspection {
            status: CredentialBootstrapStatus::Unknown,
            scope_digest: "provider_connection_store_unavailable".into(),
        },
        |items| inspect_provider_connections_credentials(items, store),
    );
    inspect_required_credential_snapshot(data_dir, store)
        .with_provider_connections_status(
            provider_inspection.status,
            Some(provider_inspection.scope_digest),
        )
        .with_search_provider_status(inspect_search_credential_status(
            config,
            selected_provider_connection,
            store,
        ))
}

fn selected_provider_connection_from_records<'a>(
    connections: &'a [ProviderConnectionRecord],
    profiles: &[ProviderModelProfileRecord],
    selected_profile_id: Option<&str>,
) -> Option<&'a ProviderConnectionRecord> {
    let connection_id = profiles
        .iter()
        .find(|profile| Some(profile.profile_id.as_str()) == selected_profile_id)?
        .connection_id
        .as_str();
    connections
        .iter()
        .find(|connection| connection.id == connection_id)
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

fn recoverable_credential_access_purposes(snapshot: &CredentialBootstrapSnapshot) -> Vec<String> {
    let mut purposes = snapshot
        .purposes
        .iter()
        .filter(|item| item.status == CredentialBootstrapStatus::Unavailable)
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
    let expected_provider_connections_status = expected_snapshot
        .purposes
        .iter()
        .find(|item| item.purpose == "provider_connections")
        .map(|item| item.status)
        .unwrap_or(CredentialBootstrapStatus::Unknown);
    let expected_provider_connections_scope = expected_snapshot
        .purposes
        .iter()
        .find(|item| item.purpose == "provider_connections")
        .and_then(|item| item.scope_digest.clone());
    let expected_search_status = expected_snapshot
        .purposes
        .iter()
        .find(|item| item.purpose == "search_provider_api_key")
        .map(|item| item.status)
        .unwrap_or(CredentialBootstrapStatus::Unknown);
    let current_snapshot = inspect_required_credential_snapshot(data_dir, store)
        .with_provider_connections_status(
            expected_provider_connections_status,
            expected_provider_connections_scope,
        )
        .with_search_provider_status(expected_search_status);
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
    for (purpose, secret_ref) in [("canonical_task_receipts", CANONICAL_TASK_RECEIPT_KEY_REF)] {
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
        let result = hydrate_or_create_integrity_key(secret_ref, store);
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

fn recover_unavailable_credential_access_after_confirmation(
    native_confirmed: bool,
    data_dir: &Path,
    config: &AppConfig,
    provider_connections: Option<&[ProviderConnectionRecord]>,
    selected_provider_connection: Option<&ProviderConnectionRecord>,
    store: &dyn SecretStore,
    expected_snapshot: &CredentialBootstrapSnapshot,
) -> Result<CredentialRecoveryReport, AppError> {
    if !native_confirmed {
        return Err(AppError::permission(
            "credential access recovery was not confirmed in the native system dialog",
        ));
    }
    let _process_lock = CredentialRecoveryProcessLock::acquire(data_dir)?;
    let recoverable = recoverable_credential_access_purposes(expected_snapshot);
    if recoverable.is_empty() {
        return Err(AppError::permission(
            "no required credential is explicitly eligible for access recovery",
        ));
    }

    // This is the deliberately interactive read. It runs only after the
    // product-native confirmation and may let the OS present its own Keychain
    // authorization UI. Secret bytes stay inside the Rust process.
    let observed = inspect_current_credential_snapshot(
        data_dir,
        config,
        provider_connections,
        selected_provider_connection,
        store,
    );
    let mut report = recovery_report_from_snapshot(expected_snapshot);
    let mut unresolved = Vec::new();
    for purpose in &recoverable {
        let expected_item = expected_snapshot
            .purposes
            .iter()
            .find(|item| item.purpose == *purpose)
            .expect("recoverable purpose came from the expected snapshot");
        let observed_item = observed
            .purposes
            .iter()
            .find(|item| item.purpose == *purpose);
        if expected_item.scope_digest.is_some()
            && observed_item.and_then(|item| item.scope_digest.as_ref())
                != expected_item.scope_digest.as_ref()
        {
            set_recovery_item_status(&mut report, purpose, "unknown");
            unresolved.push(format!("{purpose}=scope_changed"));
            continue;
        }
        let status = observed_item
            .map(|item| item.status)
            .unwrap_or(CredentialBootstrapStatus::Unknown);
        if status == CredentialBootstrapStatus::Available {
            set_recovery_item_status(&mut report, purpose, "pending_restart_verification");
        } else {
            set_recovery_item_status(&mut report, purpose, status.as_str());
            unresolved.push(format!("{purpose}={}", status.as_str()));
        }
    }

    if unresolved.is_empty() {
        // Keep the legacy field true for the existing frontend contract: the
        // recovery operation is complete for this process, but ordinary
        // effects remain closed until a clean restart rehydrates every owner.
        report.initialization_completed_for_restart = false;
        report.restart_required = true;
    } else {
        report.blocked_reason = Some(format!(
            "credential access recovery remains unresolved: {}",
            unresolved.join(", ")
        ));
    }
    Ok(report)
}

/// Mask for sensitive API keys sent to the frontend.
const KEY_MASK: &str = "***";

fn provider_endpoint_identity(provider_id: &str, provider_endpoint: &str) -> Option<String> {
    let provider = provider_id.trim().to_ascii_lowercase();
    if provider.is_empty() {
        return None;
    }
    let base = if provider_endpoint.trim().is_empty() {
        default_base_for_provider(&provider).to_string()
    } else {
        provider_endpoint.trim().to_string()
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

fn search_provider_identity(config: &AppConfig) -> Option<String> {
    let provider = config.system.search_provider.trim().to_ascii_lowercase();
    match provider.as_str() {
        "auto" | "duckduckgo" | "brave" | "deepseek" | "openrouter" => Some(provider),
        "searxng" => {
            let parsed = reqwest::Url::parse(config.system.searxng_url.trim()).ok()?;
            if !matches!(parsed.scheme(), "http" | "https")
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return None;
            }
            Some(format!("searxng|{}", parsed.as_str().trim_end_matches('/')))
        }
        _ => None,
    }
}

fn resolve_submitted_search_provider_api_key(submitted: &AppConfig, current: &AppConfig) -> String {
    let submitted_key = submitted.system.search_provider_key.trim();
    if !submitted_key.is_empty() && submitted_key != KEY_MASK {
        return submitted.system.search_provider_key.clone();
    }
    let identity_unchanged = search_provider_identity(submitted).is_some_and(|identity| {
        search_provider_identity(current).as_deref() == Some(identity.as_str())
    });
    if identity_unchanged {
        current.system.search_provider_key.clone()
    } else {
        String::new()
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

fn credential_access_recovery_native_request<'a>(
    eligible_purposes: &'a [String],
    confirmation_arguments: &'a serde_json::Value,
) -> NativeDangerActionRequest<'a> {
    NativeDangerActionRequest {
        action_type: "credential_access_recovery",
        target_ids_for_new_challenge: eligible_purposes,
        requested_target: eligible_purposes.first().map(String::as_str),
        affected_count: eligible_purposes.len(),
        arguments: confirmation_arguments,
        arguments_summary:
            "仅重新请求后端启动快照明确标记为 unavailable 的既有凭据访问；不创建、不覆盖、不返回密钥。",
        scope_summary: "后端快照列出的 OpenLife 既有系统与 Provider 凭据",
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
    let expected_access_recovery_purposes =
        recoverable_credential_access_purposes(&expected_snapshot);
    if expected_eligible_purposes.is_empty() && expected_access_recovery_purposes.is_empty() {
        return Err(AppError::permission(
            "LifeStateProjection reports no credential eligible for initialization or access recovery",
        ));
    }
    let data_dir = app_data_dir();
    let (provider_connections, selected_provider_connection) =
        match state.conversation_store.as_ref() {
            Some(store) => {
                let store = store.lock().await;
                let connections = store.list_provider_connections().map_err(AppError::from)?;
                let profiles = store
                    .list_provider_model_profiles()
                    .map_err(AppError::from)?;
                let selected_profile_id = store
                    .selected_provider_profile_id(None)
                    .map_err(AppError::from)?;
                let selected = selected_provider_connection_from_records(
                    &connections,
                    &profiles,
                    selected_profile_id.as_deref(),
                )
                .cloned();
                (Some(connections), selected)
            }
            None => (None, None),
        };

    if !expected_access_recovery_purposes.is_empty() {
        let confirmation_arguments = serde_json::json!({
            "operation": "recover_unavailable_credential_access",
            "eligiblePurposeIds": expected_access_recovery_purposes.clone(),
            "affectedCount": expected_access_recovery_purposes.len(),
            "bootstrapSnapshotVersion": expected_snapshot.version.clone(),
            "bootstrapSnapshotDigest": expected_snapshot.digest.clone(),
            "createsCredentials": false,
            "returnsSecretMaterial": false,
        });
        require_native_danger_action_confirmation(
            &window,
            credential_access_recovery_native_request(
                &expected_access_recovery_purposes,
                &confirmation_arguments,
            ),
        )
        .await?;
        let config = state.config.lock().await.clone();
        let connections_for_worker = provider_connections.clone();
        let selected_connection_for_worker = selected_provider_connection.clone();
        let snapshot_for_worker = expected_snapshot.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            recover_unavailable_credential_access_after_confirmation(
                true,
                &data_dir,
                &config,
                connections_for_worker.as_deref(),
                selected_connection_for_worker.as_ref(),
                &ProfileSecretStore,
                &snapshot_for_worker,
            )
        })
        .await
        .map_err(|error| {
            AppError::internal(format!("credential access recovery worker failed: {error}"))
        })?;
    }

    let pre_confirmation_data_dir = data_dir.clone();
    let pre_confirmation_config = state.config.lock().await.clone();
    let pre_confirmation_connections = provider_connections;
    let pre_confirmation_selected_connection = selected_provider_connection;
    let pre_confirmation_snapshot = tauri::async_runtime::spawn_blocking(move || {
        inspect_current_credential_snapshot(
            &pre_confirmation_data_dir,
            &pre_confirmation_config,
            pre_confirmation_connections.as_deref(),
            pre_confirmation_selected_connection.as_ref(),
            &ProfileSecretStore,
        )
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
            &ProfileSecretStore,
            &snapshot_for_worker,
        )
    })
    .await
    .map_err(|error| AppError::internal(format!("credential recovery worker failed: {error}")))?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditableSettingsConfig {
    pub prefer_local_model: bool,
    pub local_model: String,
    #[serde(default)]
    pub system: SystemConfig,
}

impl From<&AppConfig> for EditableSettingsConfig {
    fn from(config: &AppConfig) -> Self {
        Self {
            prefer_local_model: config.prefer_local_model,
            local_model: config.local_model.clone(),
            system: config.system.clone(),
        }
    }
}

#[tauri::command]
pub async fn get_config(
    state: State<'_, Arc<AppState>>,
) -> Result<EditableSettingsConfig, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("ConfigStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let cfg = state.config.lock().await;
    let mut editable = EditableSettingsConfig::from(&*cfg);
    if !editable.system.search_provider_key.is_empty() {
        editable.system.search_provider_key = KEY_MASK.to_string();
    }
    Ok(editable)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactOutputDirectorySelection {
    pub cancelled: bool,
    pub selected_path: Option<String>,
}

fn validate_artifact_output_directory(path: &Path) -> Result<PathBuf, AppError> {
    let metadata = path.symlink_metadata().map_err(|error| {
        AppError::permission(format!(
            "artifact output directory cannot be inspected: {error}"
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::permission(
            "artifact output selection must be a real directory",
        ));
    }
    let canonical = path.canonicalize().map_err(|error| {
        AppError::permission(format!(
            "artifact output directory cannot be canonicalized: {error}"
        ))
    })?;
    if canonical.parent().is_none() {
        return Err(AppError::permission(
            "filesystem root cannot be used as an artifact output directory",
        ));
    }
    Ok(canonical)
}

fn reference_only_config_for_first_persist(mut config: AppConfig) -> AppConfig {
    config.llm.openai_key.clear();
    config.system.search_provider_key.clear();
    config
}

fn require_config_write_admission(state: &Arc<AppState>) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConfigStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))
}

fn preserve_backend_owned_filesystem_scopes(submitted: &mut AppConfig, current: &AppConfig) {
    submitted.system.artifact_output_directory = current.system.artifact_output_directory.clone();
    submitted.system.additional_read_roots = current.system.additional_read_roots.clone();
}

async fn persist_artifact_output_directory(
    state: &Arc<AppState>,
    selected_path: &Path,
) -> Result<PathBuf, AppError> {
    require_config_write_admission(state)?;
    let canonical = validate_artifact_output_directory(selected_path)?;
    let _config_write_guard = CONFIG_WRITE_COORDINATOR.lock().await;
    let config_path = app_data_dir().join("config.yaml");
    let mut persisted = if config_path.exists() {
        AppConfig::load(&config_path).map_err(|error| {
            AppError::db_with_hint(
                format!("artifact output directory config load failed: {error}"),
                "canonical_state_unknown",
            )
        })?
    } else {
        // The runtime configuration may contain hydrated credentials. A first-run
        // path selection may create config.yaml, but it must never serialize them.
        reference_only_config_for_first_persist(state.config.lock().await.clone())
    };
    persisted.system.artifact_output_directory = Some(canonical.to_string_lossy().into_owned());
    persisted.save(&config_path).map_err(AppError::from)?;

    let mut runtime_config = state.config.lock().await;
    runtime_config.system.artifact_output_directory =
        persisted.system.artifact_output_directory.clone();
    Ok(canonical)
}

pub async fn select_artifact_output_directory<R: Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: &Arc<AppState>,
) -> Result<ArtifactOutputDirectorySelection, AppError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app_handle
        .dialog()
        .file()
        .set_title("选择 OpenLife artifact 输出文件夹")
        .pick_folder(move |path| {
            let _ = sender.send(path);
        });
    let selected = receiver.await.map_err(|_| {
        AppError::internal("artifact output directory picker closed without a result")
    })?;
    let Some(selected) = selected else {
        return Ok(ArtifactOutputDirectorySelection {
            cancelled: true,
            selected_path: None,
        });
    };
    let selected = selected.into_path().map_err(|_| {
        AppError::permission("artifact output directory picker returned an invalid path")
    })?;
    let canonical = persist_artifact_output_directory(state, &selected).await?;
    Ok(ArtifactOutputDirectorySelection {
        cancelled: false,
        selected_path: Some(canonical.to_string_lossy().into_owned()),
    })
}

#[tauri::command]
pub async fn save_config(
    submitted: EditableSettingsConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let _config_write_guard = CONFIG_WRITE_COORDINATOR.lock().await;
    let data_dir = app_data_dir();
    let config_path = data_dir.join("config.yaml");

    let current_config = {
        let cfg = state.config.lock().await;
        cfg.clone()
    };
    let mut config = current_config.clone();
    config.prefer_local_model = submitted.prefer_local_model;
    config.local_model = submitted.local_model;
    config.system = submitted.system;
    // Filesystem authority is never accepted from the editable Settings JSON.
    // The native picker owns Artifact destination validation; Project commands
    // and future dedicated read-root commands own their separate scopes.
    preserve_backend_owned_filesystem_scopes(&mut config, &current_config);
    let search_provider_identity_unchanged =
        search_provider_identity(&config).is_some_and(|identity| {
            search_provider_identity(&current_config).as_deref() == Some(identity.as_str())
        });
    config.system.search_provider_key =
        resolve_submitted_search_provider_api_key(&config, &current_config);
    if !search_provider_identity_unchanged {
        config.system.search_provider_key_ref = None;
    } else if config.system.search_provider_key_ref.is_none() {
        config.system.search_provider_key_ref = current_config.system.search_provider_key_ref;
    }

    let secret_store = ProfileSecretStore;
    let rollback =
        stage_search_config_secret(&mut config, &secret_store).map_err(AppError::from)?;
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionModelViewModel {
    pub profile_id: String,
    pub model_id: String,
    pub display_name: String,
    pub selected: bool,
    pub validation_state: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionViewModel {
    pub id: String,
    pub provider_id: String,
    pub display_name: String,
    pub endpoint: String,
    pub credential_state: String,
    pub validation_state: String,
    pub models: Vec<ProviderConnectionModelViewModel>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionsViewModel {
    pub connections: Vec<ProviderConnectionViewModel>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderConnectionInput {
    pub id: Option<String>,
    pub provider_id: String,
    pub display_name: String,
    pub endpoint: String,
    pub model_id: String,
    pub credential: Option<String>,
}

pub(crate) fn provider_connection_config(
    base: &AppConfig,
    connection: &ProviderConnectionRecord,
    model_id: &str,
    credential: String,
) -> AppConfig {
    let mut config = base.clone();
    config.prefer_local_model = false;
    config.llm.provider = connection.provider_id.clone();
    config.llm.openai_base = connection.endpoint.clone();
    config.llm.chat_model = model_id.to_string();
    config.llm.openai_key = credential;
    config.llm.openai_key_ref = connection.credential_reference.clone();
    config.llm.credential_version = connection.credential_version;
    config
}

fn stable_connection_model_profile_id(connection_id: &str, model_id: &str) -> String {
    use sha2::{Digest, Sha256};

    let material = format!("openlife_provider_profile_v2\0{connection_id}\0{model_id}");
    let digest = format!("{:x}", Sha256::digest(material.as_bytes()));
    format!("provider-profile:{}", &digest[..24])
}

pub(crate) fn provider_connection_validation_path(connection_id: &str) -> PathBuf {
    use sha2::{Digest, Sha256};

    let digest = format!("{:x}", Sha256::digest(connection_id.as_bytes()));
    app_data_dir()
        .join("provider-validations")
        .join(format!("{digest}.json"))
}

async fn provider_connections_view_model(
    state: &Arc<AppState>,
) -> Result<ProviderConnectionsViewModel, AppError> {
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::db("conversation_store_unavailable"))?;
    let (connections, profiles, selected_profile_id) = {
        let store = store.lock().await;
        (
            store.list_provider_connections().map_err(AppError::from)?,
            store
                .list_provider_model_profiles()
                .map_err(AppError::from)?,
            store
                .selected_provider_profile_id(None)
                .map_err(AppError::from)?,
        )
    };
    let secret_store = ProfileSecretStore;
    let mut view_connections = Vec::with_capacity(connections.len());
    for connection in connections
        .into_iter()
        .filter(|connection| connection.endpoint_class == "cloud")
    {
        let models = profiles
            .iter()
            .filter(|profile| profile.connection_id == connection.id)
            .map(|profile| ProviderConnectionModelViewModel {
                profile_id: profile.profile_id.clone(),
                model_id: profile.model_id.clone(),
                display_name: profile.display_name.clone(),
                selected: selected_profile_id.as_deref() == Some(profile.profile_id.as_str()),
                validation_state: profile.validation_state.clone(),
            })
            .collect::<Vec<_>>();
        let credential_state = match connection.credential_reference.as_deref() {
            None if connection.endpoint_class == "local" => "not_required",
            None => "missing",
            Some(reference) => match secret_store.get(reference) {
                Ok(Some(encoded)) => {
                    if hydrate_bound_provider_secret(
                        &connection.provider_id,
                        &connection.endpoint,
                        connection.credential_version,
                        &encoded,
                    )
                    .is_ok()
                    {
                        "stored"
                    } else {
                        "invalid"
                    }
                }
                Ok(None) => "missing",
                Err(_) => "unavailable",
            },
        }
        .to_string();
        view_connections.push(ProviderConnectionViewModel {
            id: connection.id,
            provider_id: connection.provider_id,
            display_name: connection.display_name,
            endpoint: connection.endpoint,
            credential_state,
            validation_state: connection.validation_state,
            models,
        });
    }
    Ok(ProviderConnectionsViewModel {
        connections: view_connections,
    })
}

#[tauri::command]
pub async fn get_provider_connections(
    state: State<'_, Arc<AppState>>,
) -> Result<ProviderConnectionsViewModel, AppError> {
    provider_connections_view_model(state.inner()).await
}

#[tauri::command]
pub async fn save_provider_connection(
    input: SaveProviderConnectionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ProviderConnectionsViewModel, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let provider_id = input.provider_id.trim().to_ascii_lowercase();
    if !matches!(
        provider_id.as_str(),
        "deepseek"
            | "openai"
            | "openrouter"
            | "gemini"
            | "siliconflow"
            | "moonshot"
            | "dashscope"
            | "zhipu"
            | "custom"
    ) {
        return Err(AppError::external("provider_connection_provider_invalid"));
    }
    let endpoint = input.endpoint.trim().trim_end_matches('/').to_string();
    let model_id = input.model_id.trim().to_string();
    if model_id.is_empty() || model_id.len() > 512 {
        return Err(AppError::external("provider_connection_model_invalid"));
    }
    if provider_endpoint_identity(&provider_id, &endpoint).is_none() {
        return Err(AppError::external("provider_connection_endpoint_invalid"));
    }
    let connection_id = input.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::db("conversation_store_unavailable"))?;
    let existing = store
        .lock()
        .await
        .get_provider_connection(&connection_id)
        .map_err(AppError::from)?;
    let identity_changed = existing.as_ref().is_some_and(|connection| {
        connection.provider_id != provider_id || connection.endpoint != endpoint
    });
    let submitted_credential = input
        .credential
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != KEY_MASK)
        .map(str::to_string);
    if (existing.is_none() || identity_changed) && submitted_credential.is_none() {
        return Err(AppError::external(
            "provider_connection_credential_required",
        ));
    }
    let existing_credential_reference = existing
        .as_ref()
        .and_then(|connection| connection.credential_reference.clone());
    let credential_reference = existing_credential_reference
        .as_deref()
        .filter(|reference| reference.starts_with(PROVIDER_CONNECTION_KEY_REF_PREFIX))
        .map(str::to_string)
        .unwrap_or_else(|| provider_connection_secret_ref(&connection_id));
    let credential_version = match (&existing, &submitted_credential) {
        (Some(connection), Some(_)) => connection.credential_version.saturating_add(1),
        (Some(connection), None) => connection.credential_version,
        (None, Some(_)) => 1,
        (None, None) => 0,
    };
    let existing_profiles = store
        .lock()
        .await
        .list_provider_model_profiles()
        .map_err(AppError::from)?;
    let existing_profile = existing_profiles
        .iter()
        .find(|profile| profile.connection_id == connection_id && profile.model_id == model_id);
    let now = chrono::Utc::now();
    let submitted_display_name = input.display_name.trim();
    let connection = ProviderConnectionRecord {
        id: connection_id.clone(),
        provider_id: provider_id.clone(),
        display_name: if submitted_display_name.is_empty() {
            provider_label(&provider_id)
        } else {
            submitted_display_name.chars().take(256).collect::<String>()
        },
        endpoint,
        endpoint_class: "cloud".into(),
        credential_reference: Some(credential_reference.clone()),
        credential_version,
        protocol: "openai_compatible_chat_completions".into(),
        privacy_boundary: "provider_hosted".into(),
        validation_state: if existing.is_some()
            && !identity_changed
            && submitted_credential.is_none()
            && existing_profile.is_some()
        {
            existing
                .as_ref()
                .map(|value| value.validation_state.clone())
                .unwrap_or_else(|| "unverified".into())
        } else {
            "unverified".into()
        },
        created_at: existing
            .as_ref()
            .map(|value| value.created_at)
            .unwrap_or(now),
        updated_at: now,
    };
    let profile_id = existing_profile
        .map(|profile| profile.profile_id.clone())
        .unwrap_or_else(|| stable_connection_model_profile_id(&connection_id, &model_id));
    let profile = ProviderModelProfileRecord {
        profile_id,
        connection_id,
        model_id: model_id.clone(),
        display_name: model_id,
        capability_snapshot_json: "{}".into(),
        capability_source: "settings_user_configured".into(),
        validation_state: if existing.is_some()
            && !identity_changed
            && submitted_credential.is_none()
        {
            existing_profile
                .map(|profile| profile.validation_state.clone())
                .unwrap_or_else(|| "unverified".into())
        } else {
            "unverified".into()
        },
        created_at: now,
        updated_at: now,
    };
    let secret_store = ProfileSecretStore;
    let encoded_secret_to_write = if let Some(credential) = submitted_credential {
        Some(
            crate::secret_store::encode_provider_secret(
                &connection.provider_id,
                &connection.endpoint,
                connection.credential_version,
                &credential,
            )
            .map_err(AppError::from)?,
        )
    } else if existing_credential_reference.as_deref() != Some(credential_reference.as_str()) {
        let source_reference = existing_credential_reference
            .as_deref()
            .ok_or_else(|| AppError::external("provider_connection_credential_required"))?;
        Some(
            secret_store
                .get(source_reference)
                .map_err(AppError::from)?
                .ok_or_else(|| AppError::external("provider_connection_credential_missing"))?,
        )
    } else {
        None
    };
    let previous_secret = if encoded_secret_to_write.is_some() {
        Some(
            secret_store
                .get(&credential_reference)
                .map_err(AppError::from)?,
        )
    } else {
        None
    };
    if let Some(encoded) = encoded_secret_to_write {
        secret_store
            .set(&credential_reference, &encoded)
            .map_err(AppError::from)?;
    }
    let persist_result = store
        .lock()
        .await
        .upsert_provider_model_profile(&connection, &profile);
    if let Err(error) = persist_result {
        let rollback = match previous_secret {
            Some(Some(value)) => secret_store.set(&credential_reference, &value),
            Some(None) => secret_store.delete(&credential_reference),
            None => Ok(()),
        };
        return match rollback {
            Ok(()) => Err(AppError::from(error)),
            Err(rollback_error) => Err(AppError::internal(format!(
                "provider connection save failed: {error}; credential rollback failed: {rollback_error}"
            ))),
        };
    }
    provider_connections_view_model(state.inner()).await
}

#[tauri::command]
pub async fn delete_provider_connection(
    connection_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ProviderConnectionsViewModel, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::db("conversation_store_unavailable"))?;
    let connection = store
        .lock()
        .await
        .get_provider_connection(&connection_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::external("provider_connection_not_found"))?;
    let secret_store = ProfileSecretStore;
    let previous_secret = match connection.credential_reference.as_deref() {
        Some(reference) if reference.starts_with(PROVIDER_CONNECTION_KEY_REF_PREFIX) => {
            let previous = secret_store.get(reference).map_err(AppError::from)?;
            secret_store.delete(reference).map_err(AppError::from)?;
            previous.map(|value| (reference.to_string(), value))
        }
        Some(_) | None => None,
    };
    if let Err(error) = store
        .lock()
        .await
        .delete_provider_connection(&connection_id)
    {
        if let Some((reference, value)) = previous_secret {
            secret_store.set(&reference, &value).map_err(|rollback_error| {
                AppError::internal(format!(
                    "provider connection delete failed: {error}; credential rollback failed: {rollback_error}"
                ))
            })?;
        }
        return Err(AppError::from(error));
    }
    provider_connections_view_model(state.inner()).await
}

#[tauri::command]
pub async fn test_provider_connection(
    connection_id: String,
    profile_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<LlmConnectionTestResult, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::db("conversation_store_unavailable"))?;
    let (connection, profile) = {
        let store = store.lock().await;
        let connection = store
            .get_provider_connection(&connection_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::external("provider_connection_not_found"))?;
        let profile = store
            .list_provider_model_profiles()
            .map_err(AppError::from)?
            .into_iter()
            .find(|profile| {
                profile.profile_id == profile_id && profile.connection_id == connection_id
            })
            .ok_or_else(|| AppError::external("provider_connection_model_not_found"))?;
        (connection, profile)
    };
    let reference = connection
        .credential_reference
        .as_deref()
        .ok_or_else(|| AppError::external("provider_connection_credential_missing"))?;
    let encoded = ProfileSecretStore
        .get(reference)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::external("provider_connection_credential_missing"))?;
    let base_config = state.config.lock().await.clone();
    let credential = hydrate_bound_provider_secret(
        &connection.provider_id,
        &connection.endpoint,
        connection.credential_version,
        &encoded,
    )
    .map_err(|_| AppError::external("provider_connection_credential_invalid"))?;
    let config =
        provider_connection_config(&base_config, &connection, &profile.model_id, credential);
    let validation_path = provider_connection_validation_path(&connection_id);
    if let Some(parent) = validation_path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::from)?;
    }
    let result = test_provider_connection_config_with_state_and_validation_path(
        config,
        state.inner(),
        &validation_path,
    )
    .await?;
    let validation_state = if result.ok {
        "ready"
    } else {
        result.validation_status.as_str()
    };
    let store = store.lock().await;
    store
        .update_provider_connection_validation_state(&connection_id, validation_state)
        .map_err(AppError::from)?;
    store
        .update_provider_model_profile_validation_state(&profile_id, validation_state)
        .map_err(AppError::from)?;
    if result.ok
        && store
            .selected_provider_profile_id(None)
            .map_err(AppError::from)?
            .is_none()
    {
        store
            .set_selected_provider_profile(None, &profile_id)
            .map_err(AppError::from)?;
    }
    Ok(result)
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

pub(crate) async fn test_provider_connection_config_with_state_and_validation_path(
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
    if !current_runtime_coherent {
        let record = crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_provider_connection_test",
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
            "settings_provider_connection_test",
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
            "settings_provider_connection_test",
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
            "settings_provider_connection_test",
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
                "settings_provider_connection_test",
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
    let response_content = outcome.result.as_ref().ok().cloned();
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
    let mut record = match (observed_receipt.as_ref(), terminal_proof) {
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
                "settings_provider_connection_test",
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
                    "settings_provider_connection_test",
                    "provider_terminal_proof_invalid",
                    write_observed_at,
                ),
            }
        }
        (Some(_), None) => crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_provider_connection_test",
            "provider_terminal_proof_missing",
            write_observed_at,
        ),
        (None, None) => crate::provider_validation::failed_provider_validation_record(
            &config,
            "settings_provider_connection_test",
            "provider_not_attempted",
            write_observed_at,
        ),
        (None, Some(_)) | (Some(_), Some(_)) => {
            crate::provider_validation::failed_provider_validation_record(
                &config,
                "settings_provider_connection_test",
                "provider_terminal_proof_mismatch",
                write_observed_at,
            )
        }
    };
    if completed {
        let response_content = response_content
            .as_deref()
            .ok_or_else(|| AppError::external("provider Work compatibility response missing"))?;
        crate::provider_validation::attach_work_compatibility_evaluation(
            &config,
            &mut record,
            response_content,
        )?;
    }
    crate::provider_validation::save_provider_validation_record_to_path(validation_path, &record)?;

    if completed {
        let work_compatibility_validated = record
            .work_compatibility
            .as_ref()
            .is_some_and(|evidence| evidence.status == "validated");
        let model_note = if model.to_lowercase().contains("reasoner") {
            " 当前选择的是推理模型，首次可见输出可能更慢；试用聊天建议优先使用 deepseek-chat 这类通用聊天模型。"
        } else {
            ""
        };
        Ok(LlmConnectionTestResult {
            ok: true,
            provider: label,
            message: format!(
                "连接成功。{}{}",
                if work_compatibility_validated {
                    "Work 结构化协议已通过版本化评测。"
                } else {
                    "Work 结构化协议评测未通过；Chat 可用不代表 Work 可用。"
                },
                model_note
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_network_consent::{
        authorize_provider_network_dispatch, NetworkConsentSubmissionScope,
        ProviderNetworkAuthorization,
    };
    use crate::secret_store::{PROVIDER_KEY_REF, SEARCH_KEY_REF};
    use openlife_core::llm::provider_endpoint_is_official;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[test]
    fn artifact_output_directory_validation_accepts_a_real_non_root_directory() {
        let directory = tempfile::tempdir().unwrap();

        assert_eq!(
            validate_artifact_output_directory(directory.path()).unwrap(),
            directory.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn artifact_output_directory_validation_rejects_files_and_filesystem_root() {
        let file = tempfile::NamedTempFile::new().unwrap();

        assert!(validate_artifact_output_directory(file.path()).is_err());
        assert!(validate_artifact_output_directory(Path::new("/")).is_err());
    }

    #[test]
    fn editable_settings_payload_cannot_expand_filesystem_scopes() {
        let mut current = AppConfig::default();
        current.system.artifact_output_directory = Some("/authorized/output".into());
        current.system.additional_read_roots = vec!["/authorized/read".into()];
        let mut submitted = current.clone();
        submitted.system.artifact_output_directory = Some("/injected/output".into());
        submitted.system.additional_read_roots = vec!["/injected/read".into()];

        preserve_backend_owned_filesystem_scopes(&mut submitted, &current);

        assert_eq!(
            submitted.system.artifact_output_directory.as_deref(),
            Some("/authorized/output")
        );
        assert_eq!(
            submitted.system.additional_read_roots,
            vec!["/authorized/read"]
        );
    }

    #[test]
    fn editable_settings_projection_excludes_provider_profile_authority() {
        let mut current = AppConfig::default();
        current.llm.provider = "openrouter".into();
        current.llm.openai_base = "https://openrouter.ai/api/v1".into();
        current.llm.chat_model = "stealth/ox-alpha".into();
        current.llm.openai_key = "runtime-only-secret".into();
        current.llm.openai_key_ref = Some(PROVIDER_KEY_REF.into());
        current.llm.credential_version = 7;

        let editable = EditableSettingsConfig::from(&current);
        let serialized = serde_json::to_value(editable).unwrap();

        assert!(serialized.get("llm").is_none());
        assert!(!serialized.to_string().contains("openrouter"));
        assert!(!serialized.to_string().contains("runtime-only-secret"));
        assert_eq!(serialized["local_model"], current.local_model);
    }

    #[test]
    fn config_only_directory_selection_fails_when_config_store_is_unavailable() {
        let mut state = crate::test_utils::test_app_state();
        let coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            if *store == "ConfigStore" {
                coordinator.register_unavailable(
                    *store,
                    "config_store_unavailable",
                    "config store unavailable",
                );
            } else {
                coordinator.register_read_write(*store);
            }
        }
        coordinator.seal();
        Arc::get_mut(&mut state)
            .expect("isolated test state")
            .persistence_coordinator = coordinator;

        assert!(require_config_write_admission(&state).is_err());
    }

    #[test]
    fn first_artifact_path_persist_never_serializes_runtime_credentials() {
        let mut runtime = AppConfig::default();
        runtime.llm.openai_key = "provider-secret".into();
        runtime.llm.openai_key_ref = Some(PROVIDER_KEY_REF.into());
        runtime.system.search_provider_key = "search-secret".into();
        runtime.system.search_provider_key_ref =
            Some("keychain://com.openlife.desktop/search-provider-api-key".into());

        let persisted = reference_only_config_for_first_persist(runtime);

        assert!(persisted.llm.openai_key.is_empty());
        assert!(persisted.system.search_provider_key.is_empty());
        assert_eq!(
            persisted.llm.openai_key_ref.as_deref(),
            Some(PROVIDER_KEY_REF)
        );
        assert!(persisted.system.search_provider_key_ref.is_some());
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
    fn credential_initialization_creates_only_current_internal_slots() {
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
            vec![
                "created",
                "created",
                "missing_existing_data",
                "missing_existing_data",
            ]
        );
        assert_eq!(*store.writes.lock().unwrap(), 2);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("keychain://"));
        for value in store.values.lock().unwrap().values() {
            assert!(!serialized.contains(value));
        }
    }

    #[test]
    fn credential_access_recovery_restores_existing_keys_without_writes_or_secret_output() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        store
            .set(
                CANONICAL_TASK_RECEIPT_KEY_REF,
                &general_purpose::STANDARD.encode([1_u8; 32]),
            )
            .unwrap();
        let mut config = AppConfig::default();
        let connection_secret_ref = provider_connection_secret_ref("recovery-connection");
        let now = chrono::Utc::now();
        let connection = ProviderConnectionRecord {
            id: "recovery-connection".into(),
            provider_id: "deepseek".into(),
            display_name: "DeepSeek".into(),
            endpoint: "https://api.deepseek.com".into(),
            endpoint_class: "cloud".into(),
            credential_reference: Some(connection_secret_ref.clone()),
            credential_version: 0,
            protocol: "openai_compatible_chat_completions".into(),
            privacy_boundary: "provider_hosted".into(),
            validation_state: "unverified".into(),
            created_at: now,
            updated_at: now,
        };
        store
            .set(
                &connection_secret_ref,
                &crate::secret_store::encode_provider_secret(
                    &connection.provider_id,
                    &connection.endpoint,
                    connection.credential_version,
                    "sk-recovery-test",
                )
                .unwrap(),
            )
            .unwrap();
        config.system.search_provider_key_ref = Some(SEARCH_KEY_REF.into());
        store
            .set(SEARCH_KEY_REF, "sk-search-recovery-test")
            .unwrap();
        *store.writes.lock().unwrap() = 0;
        let provider_scope =
            inspect_provider_connections_credentials(std::slice::from_ref(&connection), &store)
                .scope_digest;

        let expected = CredentialBootstrapSnapshot::from_statuses([
            CredentialBootstrapStatus::Unavailable,
            CredentialBootstrapStatus::InitializationRequired,
        ])
        .with_provider_connections_status(
            CredentialBootstrapStatus::Unavailable,
            Some(provider_scope),
        )
        .with_search_provider_status(CredentialBootstrapStatus::Unavailable);
        let report = recover_unavailable_credential_access_after_confirmation(
            true,
            directory.path(),
            &config,
            Some(std::slice::from_ref(&connection)),
            None,
            &store,
            &expected,
        )
        .unwrap();

        assert!(report.restart_required);
        assert!(!report.initialization_completed_for_restart);
        for purpose in [
            "canonical_task_receipts",
            "provider_connections",
            "search_provider_api_key",
        ] {
            assert_eq!(
                report
                    .items
                    .iter()
                    .find(|item| item.purpose == purpose)
                    .unwrap()
                    .status,
                "pending_restart_verification"
            );
        }
        assert_eq!(
            report
                .items
                .iter()
                .find(|item| item.purpose == "mcp_audit")
                .unwrap()
                .status,
            "initialization_required"
        );
        assert_eq!(*store.writes.lock().unwrap(), 0);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("sk-recovery-test"));
        assert!(!serialized.contains("sk-search-recovery-test"));
        assert!(!serialized.contains("keychain://"));
    }

    #[test]
    fn credential_access_recovery_rejects_changed_provider_connection_scope() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        let config = AppConfig::default();
        let secret_ref = provider_connection_secret_ref("original-connection");
        let now = chrono::Utc::now();
        let connection = ProviderConnectionRecord {
            id: "original-connection".into(),
            provider_id: "deepseek".into(),
            display_name: "DeepSeek".into(),
            endpoint: "https://api.deepseek.com".into(),
            endpoint_class: "cloud".into(),
            credential_reference: Some(secret_ref.clone()),
            credential_version: 0,
            protocol: "openai_compatible_chat_completions".into(),
            privacy_boundary: "provider_hosted".into(),
            validation_state: "unverified".into(),
            created_at: now,
            updated_at: now,
        };
        store
            .set(
                &secret_ref,
                &crate::secret_store::encode_provider_secret(
                    &connection.provider_id,
                    &connection.endpoint,
                    connection.credential_version,
                    "sk-scope",
                )
                .unwrap(),
            )
            .unwrap();
        *store.writes.lock().unwrap() = 0;
        let expected_scope =
            inspect_provider_connections_credentials(std::slice::from_ref(&connection), &store)
                .scope_digest;
        let expected = CredentialBootstrapSnapshot::from_statuses([
            CredentialBootstrapStatus::InitializationRequired,
            CredentialBootstrapStatus::InitializationRequired,
        ])
        .with_provider_connections_status(
            CredentialBootstrapStatus::Unavailable,
            Some(expected_scope),
        );
        let mut changed_connection = connection;
        changed_connection.id = "replacement-connection".into();

        let report = recover_unavailable_credential_access_after_confirmation(
            true,
            directory.path(),
            &config,
            Some(std::slice::from_ref(&changed_connection)),
            None,
            &store,
            &expected,
        )
        .unwrap();

        assert!(!report.restart_required);
        assert_eq!(
            report
                .items
                .iter()
                .find(|item| item.purpose == "provider_connections")
                .unwrap()
                .status,
            "unknown"
        );
        assert!(report
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("provider_connections=scope_changed")));
        assert_eq!(*store.writes.lock().unwrap(), 0);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
    }

    #[test]
    fn official_deepseek_search_does_not_duplicate_provider_connection_credential_status() {
        let store = RecoverySecretStore::default();
        let mut config = AppConfig::default();
        config.system.search_provider = "deepseek".into();
        let now = chrono::Utc::now();
        let selected_connection = ProviderConnectionRecord {
            id: "selected-deepseek".into(),
            provider_id: "deepseek".into(),
            display_name: "DeepSeek".into(),
            endpoint: "https://api.deepseek.com".into(),
            endpoint_class: "cloud".into(),
            credential_reference: Some("profile-secret://selected-deepseek".into()),
            credential_version: 1,
            protocol: "openai_compatible_chat_completions".into(),
            privacy_boundary: "provider_hosted".into(),
            validation_state: "ready".into(),
            created_at: now,
            updated_at: now,
        };

        assert_eq!(
            inspect_search_credential_status(&config, Some(&selected_connection), &store),
            CredentialBootstrapStatus::Available
        );
        assert!(store.get(SEARCH_KEY_REF).unwrap().is_none());
    }

    #[test]
    fn credential_initialization_rejected_confirmation_result_performs_zero_sets_and_zero_deletes()
    {
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
    fn credential_initialization_command_source_keeps_native_confirmation_before_the_first_write_owner(
    ) {
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
    fn credential_initialization_native_request_binds_exact_sorted_purpose_scope_and_batch_anchor()
    {
        let purposes = vec![
            "canonical_task_receipts".to_string(),
            "mcp_audit".to_string(),
        ];
        let arguments = serde_json::json!({
            "eligiblePurposeIds": purposes,
            "affectedCount": purposes.len(),
            "bootstrapSnapshotVersion": "credential_bootstrap_v2",
            "bootstrapSnapshotDigest": "a".repeat(64),
        });

        let request = credential_initialization_native_request(&purposes, &arguments);

        assert_eq!(request.target_ids_for_new_challenge, purposes);
        assert_eq!(request.requested_target, Some("canonical_task_receipts"));
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
    fn credential_initialization_existing_canonical_data_never_becomes_initialization_eligible() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("task_runtime.db"),
            b"canonical-data-sentinel",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("mcp_audit.db"),
            b"uninspectable-canonical-data",
        )
        .unwrap();
        let store = RecoverySecretStore::default();

        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        assert!(eligible_credential_purposes(&snapshot).is_empty());
        assert_eq!(
            snapshot.purposes[0].status,
            CredentialBootstrapStatus::MissingExistingData
        );
        assert_eq!(
            snapshot.purposes[1].status,
            CredentialBootstrapStatus::Unknown
        );
        assert_eq!(*store.writes.lock().unwrap(), 0);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        assert!(store.values.lock().unwrap().is_empty());
    }

    #[test]
    fn credential_initialization_invalid_existing_key_material_is_never_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        store
            .set(CANONICAL_TASK_RECEIPT_KEY_REF, "not-base64")
            .unwrap();
        *store.writes.lock().unwrap() = 0;

        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        assert_eq!(
            snapshot.purposes[0].status,
            CredentialBootstrapStatus::Invalid
        );
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
    fn credential_initialization_replay_cannot_create_a_second_mcp_epoch() {
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
    fn credential_initialization_fixed_credential_failure_compensates_every_prior_write() {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        *store.fail_set_at.lock().unwrap() = Some(1);
        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        let report =
            initialize_required_credentials_after_confirmation(directory.path(), &store, &snapshot)
                .unwrap();

        assert!(!report.initialization_completed_for_restart);
        assert_eq!(report.cleanup_status, "compensated");
        assert_eq!(*store.writes.lock().unwrap(), 1);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        assert!(store.values.lock().unwrap().is_empty());
    }

    #[test]
    fn credential_initialization_fixed_post_write_error_retains_ambiguous_secret_and_never_deletes_it(
    ) {
        let directory = tempfile::tempdir().unwrap();
        let store = RecoverySecretStore::default();
        *store.fail_after_set_at.lock().unwrap() = Some(1);
        let snapshot = inspect_required_credential_snapshot(directory.path(), &store);

        let report =
            initialize_required_credentials_after_confirmation(directory.path(), &store, &snapshot)
                .unwrap();

        assert_eq!(report.cleanup_status, "unknown");
        assert_eq!(*store.writes.lock().unwrap(), 1);
        assert_eq!(*store.deletes.lock().unwrap(), 0);
        assert_eq!(store.values.lock().unwrap().len(), 1);
        assert_eq!(
            report
                .items
                .iter()
                .find(|item| item.purpose == "canonical_task_receipts")
                .unwrap()
                .status,
            "cleanup_unknown"
        );
    }

    #[test]
    fn credential_initialization_process_lock_rejects_a_parallel_initialization_owner() {
        let directory = tempfile::tempdir().unwrap();
        let first = CredentialRecoveryProcessLock::acquire(directory.path()).unwrap();
        let second = CredentialRecoveryProcessLock::acquire(directory.path());

        assert!(second.is_err());
        drop(first);
        assert!(CredentialRecoveryProcessLock::acquire(directory.path()).is_ok());
    }

    #[test]
    fn credential_initialization_mcp_pre_write_save_failure_restores_prior_absence() {
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
        assert_eq!(*store.writes.lock().unwrap(), 2);
        assert_eq!(*store.deletes.lock().unwrap(), 2);
        assert!(store.values.lock().unwrap().is_empty());
        assert!(!directory.path().join("mcp_audit_keys.json").exists());
    }

    #[test]
    fn credential_initialization_mcp_pre_write_save_failure_preserves_exact_prior_bytes() {
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
    fn credential_initialization_mcp_unreadable_final_state_is_cleanup_unknown_and_retains_secret()
    {
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
    fn credential_initialization_mcp_unreadable_prior_state_never_writes_or_deletes() {
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
    fn credential_initialization_mcp_post_write_set_error_retains_ambiguous_secret() {
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
    fn credential_initialization_mcp_ambiguous_post_write_failure_retains_observably_referenced_secret(
    ) {
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
        assert_eq!(*store.writes.lock().unwrap(), 2);
        assert_eq!(*store.deletes.lock().unwrap(), 1);
        assert_eq!(store.values.lock().unwrap().len(), 1);
        assert!(directory.path().join("mcp_audit_keys.json").exists());
    }

    #[test]
    fn masked_search_key_is_bound_to_the_same_search_provider_identity() {
        let mut current = AppConfig::default();
        current.system.search_provider = "deepseek".into();
        current.system.search_provider_key = "sk-current-search".into();

        let mut same = current.clone();
        same.system.search_provider_key = KEY_MASK.into();
        assert_eq!(
            resolve_submitted_search_provider_api_key(&same, &current),
            "sk-current-search"
        );

        let mut changed_provider = same.clone();
        changed_provider.system.search_provider = "brave".into();
        assert!(resolve_submitted_search_provider_api_key(&changed_provider, &current).is_empty());

        let mut changed_endpoint = current.clone();
        changed_endpoint.system.search_provider = "searxng".into();
        changed_endpoint.system.searxng_url = "https://search.example/".into();
        changed_endpoint.system.search_provider_key = KEY_MASK.into();
        current.system.search_provider = "searxng".into();
        current.system.searxng_url = "https://old-search.example".into();
        assert!(resolve_submitted_search_provider_api_key(&changed_endpoint, &current).is_empty());
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
            let body = serde_json::json!({
                "choices": [{
                    "message": {
                        "content": r#"{"schemaVersion":"openlife.agent-step.v1","step":{"kind":"final_answer","payload":{"content":"ready","evidenceRefs":[],"artifactRefs":[],"sourceBlocks":[]}}}"#
                    }
                }]
            })
            .to_string();
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

        let result = test_provider_connection_config_with_state_and_validation_path(
            config,
            &state,
            &validation_path,
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert!(result.ok);
        assert_eq!(result.validation_status, "validated");
        assert!(result.message.contains("Work 结构化协议已通过版本化评测"));
        assert!(!result.message.contains("云端模型可用"));
        let receipt = result.provider_invocation_receipt.unwrap();
        assert_eq!(receipt.status, ProviderInvocationStatus::Completed);
        assert_eq!(receipt.provider, "openai");
        assert_eq!(receipt.model, "gpt-test");
        assert!(!receipt.simulated);
        let request = captured.lock().unwrap().clone();
        assert!(request.contains("OpenLife Work compatibility evaluation v1"));
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
        assert_eq!(
            persisted
                .work_compatibility
                .as_ref()
                .map(|evidence| evidence.status.as_str()),
            Some("validated")
        );
        let raw = std::fs::read_to_string(validation_path).unwrap();
        assert!(!raw.contains("OpenLife Work compatibility evaluation v1"));
        assert!(!raw.contains(r#""content":"ready""#));
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
        // Summaries are bound to the exact Connection/Profile credential
        // generation and backend-owned network policy used by the probe.
        let current_runtime = state.provider_runtime_snapshot().await;
        let mut validation_config = config.clone();
        validation_config.system.network_policy = current_runtime.config.system.network_policy;
        let dir = tempfile::tempdir().unwrap();
        let validation_path = dir.path().join("provider-validation.json");

        let result = test_provider_connection_config_with_state_and_validation_path(
            config,
            &state,
            &validation_path,
        )
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
        runtime_config.system.network_policy = openlife_core::config::NetworkPolicy {
            default_decision: "ask".into(),
            ..openlife_core::config::NetworkPolicy::default()
        };
        state.replace_provider_runtime_config(runtime_config).await;
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = base;
        config.llm.openai_key = "sk-test".into();
        config.llm.chat_model = "gpt-test".into();
        let dir = tempfile::tempdir().unwrap();
        let validation_path = dir.path().join("provider-validation.json");

        let result = test_provider_connection_config_with_state_and_validation_path(
            config,
            &state,
            &validation_path,
        )
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
        let policy = openlife_core::config::NetworkPolicy {
            default_decision: "ask".into(),
            ..openlife_core::config::NetworkPolicy::default()
        };
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
                NetworkConsentSubmissionScope::ExplicitCommand,
            )
            .await
            .unwrap(),
            ProviderNetworkAuthorization::ConsentRequired { .. }
        ));
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
}
