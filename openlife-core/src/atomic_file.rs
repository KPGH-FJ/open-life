use anyhow::{Context, Result};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicWriteCommitState {
    NotCommitted,
    VisibleDurabilityUnknown,
}

#[derive(Debug)]
pub struct AtomicWriteError {
    commit_state: AtomicWriteCommitState,
    source: anyhow::Error,
}

impl AtomicWriteError {
    pub fn commit_state(&self) -> AtomicWriteCommitState {
        self.commit_state
    }
}

impl std::fmt::Display for AtomicWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "atomic_write_{:?}:{}",
            self.commit_state, self.source
        )
    }
}

impl std::error::Error for AtomicWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

fn atomic_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(any(test, feature = "test-utils"))]
std::thread_local! {
    static POST_RENAME_SYNC_FAILURE_PATH: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(any(test, feature = "test-utils"))]
pub struct AtomicPostRenameSyncFailureGuard {
    previous: Option<std::path::PathBuf>,
}

#[cfg(any(test, feature = "test-utils"))]
impl Drop for AtomicPostRenameSyncFailureGuard {
    fn drop(&mut self) {
        POST_RENAME_SYNC_FAILURE_PATH.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

#[cfg(any(test, feature = "test-utils"))]
pub fn inject_post_rename_sync_failure_for_test(
    path: std::path::PathBuf,
) -> AtomicPostRenameSyncFailureGuard {
    let previous = POST_RENAME_SYNC_FAILURE_PATH.with(|slot| slot.replace(Some(path)));
    AtomicPostRenameSyncFailureGuard { previous }
}

fn post_rename_sync_failure_injected(path: &Path) -> bool {
    #[cfg(any(test, feature = "test-utils"))]
    {
        return POST_RENAME_SYNC_FAILURE_PATH.with(|slot| {
            slot.borrow()
                .as_deref()
                .is_some_and(|injected| injected == path)
        });
    }
    #[cfg(not(any(test, feature = "test-utils")))]
    {
        let _ = path;
        false
    }
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomic_commit_aware(path, bytes).map_err(anyhow::Error::new)
}

#[cfg(windows)]
fn replace_atomic_temp(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let from = temp_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let to = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let moved = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_atomic_temp(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    std::fs::rename(temp_path, path)
}

/// Windows no-replace publication. MOVEFILE_WRITE_THROUGH does not return
/// until the move has been flushed to disk, and omitting
/// MOVEFILE_REPLACE_EXISTING preserves create-new semantics when the
/// destination already exists.
#[cfg(windows)]
pub(crate) fn rename_no_replace(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};

    let from = temp_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let to = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let moved = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if moved == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) fn rename_no_replace(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let from = CString::new(temp_path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let to = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: both C strings are NUL-terminated and remain alive for the call.
    // RENAME_EXCL is the Apple no-replace atomic rename contract.
    let renamed = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    if renamed == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn rename_no_replace(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let from = CString::new(temp_path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let to = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: both C strings are NUL-terminated and remain alive for the call.
    // RENAME_NOREPLACE is the Linux/Android no-replace atomic rename contract.
    let renamed = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if renamed == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    ))
))]
pub(crate) fn rename_no_replace(_temp_path: &Path, _path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace rename is unsupported on this Unix platform",
    ))
}

#[cfg(all(not(unix), not(windows)))]
pub(crate) fn rename_no_replace(_temp_path: &Path, _path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace rename is unsupported on this platform",
    ))
}

pub fn write_atomic_commit_aware(
    path: &Path,
    bytes: &[u8],
) -> std::result::Result<(), AtomicWriteError> {
    let parent = atomic_parent(path);
    std::fs::create_dir_all(parent).map_err(|error| AtomicWriteError {
        commit_state: AtomicWriteCommitState::NotCommitted,
        source: anyhow::Error::from(error)
            .context(format!("create atomic write parent {}", parent.display())),
    })?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("openlife-data");
    let temp_path = path.with_file_name(format!(".{file_name}.tmp-{}", uuid::Uuid::new_v4()));
    let mut renamed = false;
    let write_result = (|| -> Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&temp_path)
            .with_context(|| format!("create atomic temp file {}", temp_path.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("write atomic temp file {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("fsync atomic temp file {}", temp_path.display()))?;
        replace_atomic_temp(&temp_path, path).with_context(|| {
            format!(
                "rename atomic temp file {} to {}",
                temp_path.display(),
                path.display()
            )
        })?;
        renamed = true;
        if post_rename_sync_failure_injected(path) {
            anyhow::bail!("injected_post_rename_parent_sync_failure");
        }
        #[cfg(unix)]
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("fsync atomic write parent {}", parent.display()))?;
        #[cfg(all(not(unix), not(windows)))]
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("fsync atomic write parent {}", parent.display()))?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result.map_err(|source| AtomicWriteError {
        commit_state: if renamed {
            AtomicWriteCommitState::VisibleDurabilityUnknown
        } else {
            AtomicWriteCommitState::NotCommitted
        },
        source,
    })
}

/// Publish fully-written bytes only if the destination is still absent. Apple
/// uses renamex_np(RENAME_EXCL), Linux/Android uses
/// renameat2(RENAME_NOREPLACE), and Windows uses MoveFileExW without
/// REPLACE_EXISTING and with WRITE_THROUGH. Unsupported platforms fail closed;
/// there is no hard-link fallback because it creates a two-name crash window.
/// Any failure after the destination becomes visible is reported as unknown.
pub fn write_atomic_create_new_commit_aware(
    path: &Path,
    bytes: &[u8],
) -> std::result::Result<(), AtomicWriteError> {
    let parent = atomic_parent(path);
    std::fs::create_dir_all(parent).map_err(|error| AtomicWriteError {
        commit_state: AtomicWriteCommitState::NotCommitted,
        source: anyhow::Error::from(error)
            .context(format!("create atomic write parent {}", parent.display())),
    })?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("openlife-data");
    let temp_path = path.with_file_name(format!(
        ".{file_name}.create-new-tmp-{}",
        uuid::Uuid::new_v4()
    ));
    let mut published = false;
    let write_result = (|| -> Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&temp_path)
            .with_context(|| format!("create atomic temp file {}", temp_path.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("write atomic temp file {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("fsync atomic temp file {}", temp_path.display()))?;
        rename_no_replace(&temp_path, path).with_context(|| {
            format!(
                "publish durable create-new atomic file {} to {}",
                temp_path.display(),
                path.display()
            )
        })?;
        published = true;
        if post_rename_sync_failure_injected(path) {
            anyhow::bail!("injected_post_create_new_parent_sync_failure");
        }
        #[cfg(unix)]
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("fsync atomic create-new parent {}", parent.display()))?;
        #[cfg(all(not(unix), not(windows)))]
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("fsync atomic create-new parent {}", parent.display()))?;
        Ok(())
    })();
    if !published {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result.map_err(|source| AtomicWriteError {
        commit_state: if published {
            AtomicWriteCommitState::VisibleDurabilityUnknown
        } else {
            AtomicWriteCommitState::NotCommitted
        },
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_content_without_leaving_temp_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.yaml");
        write_atomic(&path, b"version: 1\n").unwrap();
        write_atomic(&path, b"version: 2\n").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"version: 2\n");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn post_rename_sync_failure_reports_visible_durability_unknown() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.yaml");
        let _fault = inject_post_rename_sync_failure_for_test(path.clone());

        let error = write_atomic_commit_aware(&path, b"version: 2\n").unwrap_err();

        assert_eq!(
            error.commit_state(),
            AtomicWriteCommitState::VisibleDurabilityUnknown
        );
        assert!(std::error::Error::source(&error).is_some());
        assert_eq!(std::fs::read(&path).unwrap(), b"version: 2\n");
    }

    #[test]
    fn relative_file_name_uses_current_directory_as_parent() {
        let name = format!("atomic-relative-{}.json", uuid::Uuid::new_v4());
        let path = Path::new(&name);
        write_atomic(path, b"{\"version\":1}").unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"{\"version\":1}");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn atomic_create_new_publishes_once_without_leaving_temp_names() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("first-generation.json");

        write_atomic_create_new_commit_aware(&path, b"generation-a").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"generation-a");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let metadata = std::fs::metadata(&path).unwrap();
            assert_eq!(metadata.nlink(), 1);
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        }
    }

    #[cfg(unix)]
    #[test]
    fn pre_publish_crash_orphan_cannot_create_a_two_link_canonical_generation() {
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("first-generation.json");
        let orphan = directory.path().join(format!(
            ".first-generation.json.create-new-tmp-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&orphan, b"unpublished-generation").unwrap();

        write_atomic_create_new_commit_aware(&path, b"committed-generation").unwrap();

        let canonical_metadata = std::fs::metadata(&path).unwrap();
        let orphan_metadata = std::fs::metadata(&orphan).unwrap();
        assert_eq!(canonical_metadata.nlink(), 1);
        assert_eq!(orphan_metadata.nlink(), 1);
        assert_ne!(canonical_metadata.ino(), orphan_metadata.ino());
        assert_eq!(std::fs::read(&path).unwrap(), b"committed-generation");
    }

    #[test]
    fn atomic_create_new_never_replaces_an_existing_generation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("first-generation.json");
        write_atomic_create_new_commit_aware(&path, b"generation-a").unwrap();

        let error = write_atomic_create_new_commit_aware(&path, b"generation-b").unwrap_err();

        assert_eq!(error.commit_state(), AtomicWriteCommitState::NotCommitted);
        assert_eq!(std::fs::read(&path).unwrap(), b"generation-a");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn atomic_create_new_reports_unknown_after_the_destination_becomes_visible() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("first-generation.json");
        let _fault = inject_post_rename_sync_failure_for_test(path.clone());

        let error = write_atomic_create_new_commit_aware(&path, b"generation-a").unwrap_err();

        assert_eq!(
            error.commit_state(),
            AtomicWriteCommitState::VisibleDurabilityUnknown
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"generation-a");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_write_through_create_new_is_strictly_no_replace() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("windows-first-generation.json");
        write_atomic_create_new_commit_aware(&path, b"generation-a").unwrap();

        let error = write_atomic_create_new_commit_aware(&path, b"generation-b").unwrap_err();

        assert_eq!(error.commit_state(), AtomicWriteCommitState::NotCommitted);
        assert_eq!(std::fs::read(&path).unwrap(), b"generation-a");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
