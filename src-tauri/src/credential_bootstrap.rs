//! First-run initialization for OpenLife-owned integrity credentials.
//!
//! This module distinguishes an empty profile from credential recovery. It
//! may create missing internal keys only while every protected owner for that
//! key is absent. Existing, invalid, or unavailable authority is never
//! rotated here and remains a Settings recovery concern.

use crate::secret_store::{
    create_mcp_audit_key_material, hydrate_or_create_canonical_store_integrity_key,
    hydrate_or_create_integrity_key, inspect_and_hydrate_integrity_key,
    inspect_existing_mcp_audit_keys, IntegrityKeyHydration, McpAuditKeyHydrationInspection,
    SecretReader, SecretStore, ACTION_QUEUE_AUTHORITY_KEY_REF, AGENT_RUN_RECEIPT_KEY_REF,
    MAIN_CHAT_EVENT_INTEGRITY_KEY_REF, MCP_AUDIT_KEY_REF_PREFIX, TASK_STORE_AUTHORITY_KEY_REF,
};
use crate::storage::{
    load_mcp_audit_keyring_from_path, save_mcp_audit_keyring_to_path, McpAuditKeyringLoad,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use std::fs::File;
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::Path;

struct CreatedCredential {
    secret_ref: String,
    encoded_value: String,
}

struct FreshInitializationLock(File);

impl FreshInitializationLock {
    fn acquire(data_dir: &Path) -> Result<Self> {
        let file = File::open(data_dir).context("open fresh credential initialization owner")?;
        #[cfg(unix)]
        {
            // SAFETY: `file` owns this descriptor for the guard lifetime. The
            // data-directory lock serializes reinspection and all exact writes
            // without introducing another durable lock authority.
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result != 0 {
                return Err(std::io::Error::last_os_error())
                    .context("fresh credential initialization is active in another process");
            }
            Ok(Self(file))
        }
        #[cfg(not(unix))]
        {
            let _ = file;
            anyhow::bail!("fresh credential initialization locking is unavailable")
        }
    }
}

impl Drop for FreshInitializationLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // SAFETY: this guard still owns the live descriptor and releases
            // it immediately before the descriptor itself is dropped.
            unsafe {
                libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }
}

fn protected_paths_are_absent(data_dir: &Path, relative_paths: &[&str]) -> Result<bool> {
    for relative_path in relative_paths {
        match std::fs::symlink_metadata(data_dir.join(relative_path)) {
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("inspect protected credential owner"),
        }
    }
    Ok(true)
}

fn delete_exact_created_credential(
    store: &dyn SecretStore,
    credential: &CreatedCredential,
) -> bool {
    match store.get(&credential.secret_ref) {
        Ok(Some(current)) if current == credential.encoded_value => {
            store.delete(&credential.secret_ref).is_ok()
        }
        _ => false,
    }
}

fn rollback_created_credentials(store: &dyn SecretStore, created: &[CreatedCredential]) -> bool {
    let mut complete = true;
    for credential in created.iter().rev() {
        if !delete_exact_created_credential(store, credential) {
            complete = false;
        }
    }
    complete
}

fn fixed_key_needs_initialization<R: SecretReader + ?Sized>(
    data_dir: &Path,
    reader: &R,
    secret_ref: &'static str,
    protected_paths: &[&str],
) -> Result<bool> {
    match inspect_and_hydrate_integrity_key(secret_ref, reader) {
        IntegrityKeyHydration::Available(_) => Ok(false),
        IntegrityKeyHydration::Missing
            if protected_paths_are_absent(data_dir, protected_paths)? =>
        {
            Ok(true)
        }
        IntegrityKeyHydration::Missing => {
            anyhow::bail!("existing protected data has no matching internal credential")
        }
        IntegrityKeyHydration::Invalid => anyhow::bail!("an internal credential is invalid"),
        IntegrityKeyHydration::Unavailable => {
            anyhow::bail!("an internal credential is unavailable")
        }
    }
}

fn mcp_key_needs_initialization<R: SecretReader + ?Sized>(
    data_dir: &Path,
    reader: &R,
) -> Result<bool> {
    let keyring_path = data_dir.join("mcp_audit_keys.json");
    match load_mcp_audit_keyring_from_path(&keyring_path) {
        McpAuditKeyringLoad::Absent => {
            if protected_paths_are_absent(data_dir, &["mcp_audit.db"])? {
                Ok(true)
            } else {
                anyhow::bail!("existing MCP audit data has no matching key reference")
            }
        }
        McpAuditKeyringLoad::Present(configs) => {
            match inspect_existing_mcp_audit_keys(configs, reader) {
                McpAuditKeyHydrationInspection::Available(_) => Ok(false),
                McpAuditKeyHydrationInspection::MissingExistingData => {
                    anyhow::bail!("an MCP audit key reference has no credential")
                }
                McpAuditKeyHydrationInspection::Invalid => {
                    anyhow::bail!("MCP audit credential authority is invalid")
                }
                McpAuditKeyHydrationInspection::Unavailable => {
                    anyhow::bail!("MCP audit credential authority is unavailable")
                }
            }
        }
        McpAuditKeyringLoad::PresentInvalid { .. } => {
            anyhow::bail!("MCP audit key references are invalid")
        }
        McpAuditKeyringLoad::Unreadable { .. } => {
            anyhow::bail!("MCP audit key references are unreadable")
        }
    }
}

fn create_fixed_credential(
    data_dir: &Path,
    store: &dyn SecretStore,
    secret_ref: &'static str,
) -> Result<CreatedCredential> {
    let key = if secret_ref == TASK_STORE_AUTHORITY_KEY_REF {
        hydrate_or_create_canonical_store_integrity_key(
            secret_ref,
            &data_dir.join("tasks.db"),
            store,
        )?
    } else {
        hydrate_or_create_integrity_key(secret_ref, store)?
    };
    Ok(CreatedCredential {
        secret_ref: secret_ref.to_string(),
        encoded_value: general_purpose::STANDARD.encode(key),
    })
}

fn create_mcp_credential(data_dir: &Path, store: &dyn SecretStore) -> Result<CreatedCredential> {
    let mut epoch = chrono::Utc::now().timestamp().max(1) as u64;
    let material = loop {
        let secret_ref = format!("{MCP_AUDIT_KEY_REF_PREFIX}{epoch}");
        match store.get(&secret_ref) {
            Ok(None) => break create_mcp_audit_key_material(epoch, store)?,
            Ok(Some(_)) => {
                epoch = epoch
                    .checked_add(1)
                    .context("MCP audit key epoch is exhausted")?;
            }
            Err(error) => return Err(error).context("inspect MCP audit credential slot"),
        }
    };
    let secret_ref = material
        .config
        .key_ref
        .clone()
        .context("new MCP audit key has no credential reference")?;
    let credential = CreatedCredential {
        secret_ref,
        encoded_value: general_purpose::STANDARD.encode(material.key),
    };
    if let Err(error) = save_mcp_audit_keyring_to_path(
        &data_dir.join("mcp_audit_keys.json"),
        std::slice::from_ref(&material.config),
    ) {
        let cleanup_complete = delete_exact_created_credential(store, &credential);
        if cleanup_complete {
            return Err(error).context("persist first MCP audit key reference");
        }
        anyhow::bail!(
            "persist first MCP audit key reference failed and exact credential cleanup is unknown: {error}"
        );
    }
    Ok(credential)
}

/// Initialize only OpenLife-owned credentials whose protected stores are
/// provably absent. Returns `true` when this call created at least one key.
pub(crate) fn initialize_fresh_profile_credentials<R: SecretReader + ?Sized>(
    data_dir: &Path,
    reader: &R,
    store: &dyn SecretStore,
) -> Result<bool> {
    std::fs::create_dir_all(data_dir).context("create application data directory")?;
    let _lock = FreshInitializationLock::acquire(data_dir)?;

    let fixed = [
        (
            AGENT_RUN_RECEIPT_KEY_REF,
            &[
                "agent_runs.db",
                "life_events.db",
                "main_chat_agent_sessions.db",
            ][..],
        ),
        (
            MAIN_CHAT_EVENT_INTEGRITY_KEY_REF,
            &["main_chat_agent_events.db"][..],
        ),
        (
            ACTION_QUEUE_AUTHORITY_KEY_REF,
            &["main_chat_action_queue.db"][..],
        ),
        (TASK_STORE_AUTHORITY_KEY_REF, &["tasks.db"][..]),
    ];
    let needs_fixed = fixed
        .iter()
        .map(|(secret_ref, paths)| {
            fixed_key_needs_initialization(data_dir, reader, secret_ref, paths)
        })
        .collect::<Result<Vec<_>>>()?;
    let needs_mcp = mcp_key_needs_initialization(data_dir, reader)?;

    if !needs_fixed.iter().any(|needs| *needs) && !needs_mcp {
        return Ok(false);
    }

    let mut created = Vec::new();
    for ((secret_ref, _), needs_initialization) in fixed.iter().zip(needs_fixed) {
        if !needs_initialization {
            continue;
        }
        match create_fixed_credential(data_dir, store, secret_ref) {
            Ok(credential) => created.push(credential),
            Err(error) => {
                if rollback_created_credentials(store, &created) {
                    return Err(error).context("initialize fresh internal credential");
                }
                anyhow::bail!(
                    "fresh internal credential initialization failed and rollback is unknown: {error}"
                );
            }
        }
    }

    if needs_mcp {
        match create_mcp_credential(data_dir, store) {
            Ok(credential) => created.push(credential),
            Err(error) => {
                if rollback_created_credentials(store, &created) {
                    return Err(error).context("initialize fresh MCP audit credential");
                }
                anyhow::bail!(
                    "fresh MCP audit credential initialization failed and rollback is unknown: {error}"
                );
            }
        }
    }

    Ok(!created.is_empty())
}
