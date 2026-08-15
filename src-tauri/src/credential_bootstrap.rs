//! First-run initialization for OpenLife-owned integrity credentials.
//!
//! This module distinguishes an empty profile from credential recovery. It
//! may create missing internal keys only while every protected owner for that
//! key is absent. Existing, invalid, or unavailable authority is never
//! rotated here and remains a Settings recovery concern.

use crate::secret_store::{
    hydrate_or_create_integrity_key, inspect_and_hydrate_integrity_key, IntegrityKeyHydration,
    SecretReader, SecretStore, CANONICAL_TASK_RECEIPT_KEY_REF,
};
use anyhow::{Context, Result};
use std::fs::File;
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::Path;

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

/// Initialize the one credential required by the canonical Work owner when
/// its protected store is provably absent. Retired execution and optional
/// capability credentials are explicit Settings concerns and cannot block a
/// new Chat/Work profile.
pub(crate) fn initialize_fresh_profile_credentials<R: SecretReader + ?Sized>(
    data_dir: &Path,
    reader: &R,
    store: &dyn SecretStore,
) -> Result<bool> {
    std::fs::create_dir_all(data_dir).context("create application data directory")?;
    let _lock = FreshInitializationLock::acquire(data_dir)?;

    let needs_canonical_task = fixed_key_needs_initialization(
        data_dir,
        reader,
        CANONICAL_TASK_RECEIPT_KEY_REF,
        &["task_runtime.db"],
    )?;
    if !needs_canonical_task {
        return Ok(false);
    }
    hydrate_or_create_integrity_key(CANONICAL_TASK_RECEIPT_KEY_REF, store)
        .context("initialize canonical Task receipt credential")?;
    Ok(true)
}
