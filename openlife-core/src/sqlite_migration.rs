use anyhow::{Context, Result};
use fs2::FileExt;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DatabaseFileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    hard_link_count: u64,
    #[cfg(windows)]
    volume_serial_number: u64,
    #[cfg(windows)]
    file_index: u64,
    #[cfg(windows)]
    hard_link_count: u64,
}

impl DatabaseFileIdentity {
    fn binding_material(&self) -> String {
        #[cfg(unix)]
        {
            format!(
                "unix:{}:{}:{}",
                self.device, self.inode, self.hard_link_count
            )
        }
        #[cfg(windows)]
        {
            format!(
                "windows:{}:{}:{}",
                self.volume_serial_number, self.file_index, self.hard_link_count
            )
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            "unsupported-platform-no-stable-file-identity".to_string()
        }
    }

    fn validate_metadata(metadata: &std::fs::Metadata, path: &Path, component: &str) -> Result<()> {
        if !metadata.is_file() {
            anyhow::bail!(
                "{component} canonical SQLite database slot is not a regular file: {}",
                path.display()
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let hard_link_count = metadata.nlink();
            if hard_link_count != 1 {
                anyhow::bail!(
                    "{component}_database_link_count_invalid:{hard_link_count}:{}",
                    path.display()
                );
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                anyhow::bail!("{component}_reparse_point_rejected:{}", path.display());
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn from_metadata(metadata: &std::fs::Metadata, path: &Path, component: &str) -> Result<Self> {
        Self::validate_metadata(metadata, path, component)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                hard_link_count: metadata.nlink(),
            })
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            let _ = metadata;
            anyhow::bail!(
                "{component}_stable_file_identity_unsupported:{}",
                path.display()
            )
        }
    }

    fn from_path(path: &Path, component: &str) -> Result<Self> {
        Self::from_path_no_follow(path, component)
    }

    fn from_path_no_follow(path: &Path, component: &str) -> Result<Self> {
        let metadata = std::fs::symlink_metadata(path).with_context(|| {
            format!(
                "read {component} no-follow SQLite file identity at {}",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!("{component}_symlink_rejected:{}", path.display());
        }
        Self::validate_metadata(&metadata, path, component)?;
        #[cfg(windows)]
        {
            let mut options = OpenOptions::new();
            options.read(true);
            configure_no_follow_open_options(&mut options);
            let file = options.open(path).with_context(|| {
                format!(
                    "open {component} canonical SQLite identity handle at {}",
                    path.display()
                )
            })?;
            Self::from_file(&file, path, component)
        }
        #[cfg(not(windows))]
        {
            Self::from_metadata(&metadata, path, component)
        }
    }

    fn from_file(file: &File, path: &Path, component: &str) -> Result<Self> {
        let metadata = file.metadata().with_context(|| {
            format!(
                "read {component} opened SQLite file identity at {}",
                path.display()
            )
        })?;
        Self::validate_metadata(&metadata, path, component)?;
        #[cfg(windows)]
        {
            let information = winapi_util::file::information(file).with_context(|| {
                format!(
                    "read {component} stable Windows file identity at {}",
                    path.display()
                )
            })?;
            let hard_link_count = information.number_of_links();
            if hard_link_count != 1 {
                anyhow::bail!(
                    "{component}_database_link_count_invalid:{hard_link_count}:{}",
                    path.display()
                );
            }
            Ok(Self {
                volume_serial_number: information.volume_serial_number(),
                file_index: information.file_index(),
                hard_link_count,
            })
        }
        #[cfg(not(windows))]
        {
            Self::from_metadata(&metadata, path, component)
        }
    }

    #[cfg(windows)]
    fn from_windows_handle(
        handle: windows_sys::Win32::Foundation::HANDLE,
        path: &Path,
        component: &str,
    ) -> Result<Self> {
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
            FILE_ATTRIBUTE_REPARSE_POINT,
        };

        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            anyhow::bail!(
                "{component}_sqlite_windows_handle_invalid:{}",
                path.display()
            );
        }
        let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: SQLite retains ownership of `handle`; this call only reads
        // BY_HANDLE_FILE_INFORMATION while the Connection (and therefore the
        // sqlite3_file) is alive. No owned Rust handle is constructed.
        if unsafe { GetFileInformationByHandle(handle, &mut information) } == 0 {
            return Err(std::io::Error::last_os_error()).with_context(|| {
                format!(
                    "read {component} SQLite Win32 handle identity at {}",
                    path.display()
                )
            });
        }
        if information.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)
            != 0
        {
            anyhow::bail!(
                "{component}_sqlite_windows_handle_type_invalid:{}",
                path.display()
            );
        }
        let hard_link_count = information.nNumberOfLinks as u64;
        if hard_link_count != 1 {
            anyhow::bail!(
                "{component}_database_link_count_invalid:{hard_link_count}:{}",
                path.display()
            );
        }
        Ok(Self {
            volume_serial_number: information.dwVolumeSerialNumber as u64,
            file_index: ((information.nFileIndexHigh as u64) << 32)
                | information.nFileIndexLow as u64,
            hard_link_count,
        })
    }
}

fn configure_no_follow_open_options(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
}

/// Read-only binding between a store handle and the exact database file that
/// occupied its canonical pathname when SQLite opened. A later pathname
/// replacement never retargets the existing owner to the replacement file.
#[derive(Debug, Clone)]
pub struct SqliteDatabaseIdentityGuard {
    canonical_slot: PathBuf,
    identity: DatabaseFileIdentity,
    component: String,
    poisoned: Arc<Mutex<Option<String>>>,
}

impl SqliteDatabaseIdentityGuard {
    pub fn capture(canonical_slot: &Path, component: &str) -> Result<Self> {
        Ok(Self {
            canonical_slot: canonical_slot.to_path_buf(),
            identity: DatabaseFileIdentity::from_path(canonical_slot, component)?,
            component: component.to_string(),
            poisoned: Arc::new(Mutex::new(None)),
        })
    }

    pub fn validate(&self) -> Result<()> {
        if let Some(reason) = self
            .poisoned
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_identity_poison_mutex:{error}"))?
            .clone()
        {
            anyhow::bail!("{}_database_identity_poisoned:{reason}", self.component);
        }
        let validation = (|| {
            let current = DatabaseFileIdentity::from_path(&self.canonical_slot, &self.component)
                .map_err(|error| {
                    anyhow::anyhow!("{}_database_identity_unavailable:{}", self.component, error)
                })?;
            if current != self.identity {
                anyhow::bail!(
                    "{}_database_identity_changed:{}",
                    self.component,
                    self.canonical_slot.display()
                );
            }
            Ok(())
        })();
        if let Err(error) = validation {
            let reason = error.to_string();
            *self
                .poisoned
                .lock()
                .map_err(|poison| anyhow::anyhow!("sqlite_identity_poison_mutex:{poison}"))? =
                Some(reason.clone());
            anyhow::bail!("{}_database_identity_poisoned:{reason}", self.component);
        }
        Ok(())
    }

    fn retained_identity(&self) -> Result<DatabaseFileIdentity> {
        self.validate()?;
        Ok(self.identity.clone())
    }
}

#[derive(Debug)]
struct SqliteSlotOwnerLeaseInner {
    canonical_slot: PathBuf,
    component: String,
    owner_generation_id: uuid::Uuid,
    lock_path: PathBuf,
    lock_file: File,
    lock_file_identity: DatabaseFileIdentity,
    lock_file_io: Mutex<()>,
    database_existed_at_reservation: bool,
    database_binding: Mutex<SqliteSlotDatabaseBinding>,
    poisoned: Mutex<Option<String>>,
}

#[derive(Debug)]
struct SqliteSlotDatabaseBinding {
    file: Option<File>,
    identity: Option<DatabaseFileIdentity>,
    active: bool,
}

/// OS-backed no-create reservation for one canonical SQLite writable-owner
/// slot. It owns the exact sidecar lock and, when present, a read-only handle
/// for the existing database, but it cannot create or activate SQLite until
/// the caller has durably completed component-specific prerequisites.
#[derive(Debug)]
pub struct SqliteSlotOwnerReservation {
    inner: Arc<SqliteSlotOwnerLeaseInner>,
}

#[cfg(test)]
std::thread_local! {
    static ACTIVATION_POST_CREATE_FAILURE_PATH: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn activation_post_create_failure_injected(path: &Path) -> bool {
    ACTIVATION_POST_CREATE_FAILURE_PATH.with(|slot| {
        slot.borrow()
            .as_deref()
            .is_some_and(|injected| injected == path)
    })
}

#[cfg(not(test))]
fn activation_post_create_failure_injected(_path: &Path) -> bool {
    false
}

impl SqliteSlotOwnerLeaseInner {
    fn current_poison(&self) -> Result<Option<String>> {
        self.poisoned
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_slot_owner_poison_mutex:{error}"))
            .map(|reason| reason.clone())
    }

    fn poison(&self, reason: String) -> Result<()> {
        *self
            .poisoned
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_slot_owner_poison_mutex:{error}"))? =
            Some(reason);
        Ok(())
    }

    fn validate_lock_identity_unpoisoned(&self) -> Result<()> {
        let lock_component = format!("{}_owner_lock", self.component);
        let opened_lock =
            DatabaseFileIdentity::from_file(&self.lock_file, &self.lock_path, &lock_component)?;
        let current_lock =
            DatabaseFileIdentity::from_path_no_follow(&self.lock_path, &lock_component)?;
        if opened_lock != self.lock_file_identity || current_lock != self.lock_file_identity {
            anyhow::bail!(
                "{}_owner_lock_identity_changed:{}",
                self.component,
                self.lock_path.display()
            );
        }
        Ok(())
    }

    fn validate_database_binding(&self, require_active: bool) -> Result<()> {
        if let Some(reason) = self.current_poison()? {
            anyhow::bail!("{}_sqlite_slot_owner_poisoned:{reason}", self.component);
        }
        let validation = (|| {
            self.validate_lock_identity_unpoisoned()?;
            let binding = self.database_binding.lock().map_err(|error| {
                anyhow::anyhow!("sqlite_slot_owner_binding_mutex_poisoned:{error}")
            })?;
            if binding.active != require_active {
                let state = if binding.active { "active" } else { "reserved" };
                anyhow::bail!(
                    "{}_sqlite_slot_owner_state_mismatch:expected_active={require_active}:actual={state}",
                    self.component
                );
            }
            match (binding.file.as_ref(), binding.identity.as_ref()) {
                (Some(file), Some(expected)) => {
                    let opened_database = DatabaseFileIdentity::from_file(
                        file,
                        &self.canonical_slot,
                        &self.component,
                    )?;
                    let current_database = DatabaseFileIdentity::from_path_no_follow(
                        &self.canonical_slot,
                        &self.component,
                    )
                    .map_err(|error| {
                        anyhow::anyhow!(
                            "{}_database_identity_unavailable:{}",
                            self.component,
                            error
                        )
                    })?;
                    if &opened_database != expected || &current_database != expected {
                        anyhow::bail!(
                            "{}_database_identity_changed:{}",
                            self.component,
                            self.canonical_slot.display()
                        );
                    }
                }
                (None, None) if !require_active => {
                    match std::fs::symlink_metadata(&self.canonical_slot) {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Ok(_) => anyhow::bail!(
                            "{}_database_appeared_after_reservation:{}",
                            self.component,
                            self.canonical_slot.display()
                        ),
                        Err(error) => return Err(error.into()),
                    }
                }
                _ => anyhow::bail!(
                    "{}_sqlite_slot_owner_database_binding_incomplete",
                    self.component
                ),
            }
            self.validate_lock_identity_unpoisoned()?;
            Ok(())
        })();
        if let Err(error) = validation {
            let reason = error.to_string();
            self.poison(reason.clone())?;
            anyhow::bail!("{}_sqlite_slot_owner_poisoned:{reason}", self.component);
        }
        Ok(())
    }
}

/// OS-backed exclusive lease for one canonical SQLite writable-owner slot.
/// Store clones share the already-held Arc; a second independent Store::new
/// must acquire again and therefore fails while any original clone remains.
/// The final Arc drop releases the OS lock; process crash releases it through
/// the operating system.
#[derive(Debug, Clone)]
pub struct SqliteSlotOwnerLease {
    inner: Arc<SqliteSlotOwnerLeaseInner>,
}

/// Connection mutex that validates its retained database identity before every
/// operation. Writable stores supply the OS-backed owner lease; read-only
/// recovery stores supply an immutable identity guard.
pub struct IdentityBoundSqliteConnection {
    conn: Mutex<Connection>,
    owner_lease: Option<SqliteSlotOwnerLease>,
    identity_guard: Option<SqliteDatabaseIdentityGuard>,
    component: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqliteCheckedOperationFailurePhase {
    Preflight,
    Operation,
    Postflight,
}

#[derive(Debug)]
pub struct SqliteCheckedOperationError {
    phase: SqliteCheckedOperationFailurePhase,
    source: anyhow::Error,
}

impl SqliteCheckedOperationError {
    pub fn phase(&self) -> SqliteCheckedOperationFailurePhase {
        self.phase
    }

    pub fn invalidates_identity(&self) -> bool {
        self.phase != SqliteCheckedOperationFailurePhase::Operation
    }
}

impl std::fmt::Display for SqliteCheckedOperationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "sqlite_checked_operation_{:?}:{}",
            self.phase, self.source
        )
    }
}

impl std::error::Error for SqliteCheckedOperationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[cfg(test)]
std::thread_local! {
    static CHECKED_OPERATION_POST_USE_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
    static CHECKED_OPERATION_OPEN_IDENTITY_VALIDATION_COUNT: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) struct CheckedOperationPostUseHookGuard;

#[cfg(test)]
impl Drop for CheckedOperationPostUseHookGuard {
    fn drop(&mut self) {
        CHECKED_OPERATION_POST_USE_HOOK.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

#[cfg(test)]
pub(crate) fn inject_checked_operation_post_use_hook(
    hook: impl FnOnce() + 'static,
) -> CheckedOperationPostUseHookGuard {
    CHECKED_OPERATION_POST_USE_HOOK.with(|slot| {
        assert!(
            slot.borrow_mut().replace(Box::new(hook)).is_none(),
            "only one checked-operation hook may be active on a test thread"
        );
    });
    CheckedOperationPostUseHookGuard
}

fn run_checked_operation_post_use_hook() {
    #[cfg(test)]
    CHECKED_OPERATION_POST_USE_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook();
        }
    });
}

#[cfg(test)]
fn reset_checked_operation_open_identity_validation_count() {
    CHECKED_OPERATION_OPEN_IDENTITY_VALIDATION_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
fn checked_operation_open_identity_validation_count() -> usize {
    CHECKED_OPERATION_OPEN_IDENTITY_VALIDATION_COUNT.with(std::cell::Cell::get)
}

#[cfg(unix)]
fn validate_sqlite_connection_identity(
    conn: &Connection,
    component: &str,
    _expected: &DatabaseFileIdentity,
) -> Result<()> {
    let mut moved: std::os::raw::c_int = 0;
    // SAFETY: `conn.handle()` is valid for the lifetime of `conn`; the static
    // NUL-terminated database name and the aligned `c_int` output pointer live
    // across the call. No ownership crosses the FFI boundary. A VFS that does
    // not implement SQLITE_FCNTL_HAS_MOVED is rejected rather than treated as
    // proof, because path + retained-handle checks alone cannot exclude a
    // swap-during-open-and-restore race.
    let result = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            conn.handle(),
            b"main\0".as_ptr().cast(),
            rusqlite::ffi::SQLITE_FCNTL_HAS_MOVED,
            (&mut moved as *mut std::os::raw::c_int).cast(),
        )
    };
    if result != rusqlite::ffi::SQLITE_OK {
        anyhow::bail!("{component}_sqlite_has_moved_check_unsupported:{result}");
    }
    if moved != 0 {
        anyhow::bail!("{component}_sqlite_connection_file_moved");
    }
    Ok(())
}

#[cfg(windows)]
fn validate_sqlite_connection_identity(
    conn: &Connection,
    component: &str,
    expected: &DatabaseFileIdentity,
) -> Result<()> {
    use windows_sys::Win32::Foundation::HANDLE;

    let mut handle: HANDLE = std::ptr::null_mut();
    // SAFETY: `conn.handle()` and the static database name are valid for this
    // call. The Win VFS writes a borrowed HANDLE value into `handle`; SQLite
    // retains ownership, and we never construct an owning File/Handle wrapper.
    let result = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            conn.handle(),
            b"main\0".as_ptr().cast(),
            rusqlite::ffi::SQLITE_FCNTL_WIN32_GET_HANDLE,
            (&mut handle as *mut HANDLE).cast(),
        )
    };
    if result != rusqlite::ffi::SQLITE_OK {
        anyhow::bail!("{component}_sqlite_win32_handle_check_unsupported:{result}");
    }
    let observed = DatabaseFileIdentity::from_windows_handle(
        handle,
        Path::new("<sqlite-main-handle>"),
        component,
    )?;
    if &observed != expected {
        anyhow::bail!("{component}_sqlite_connection_file_identity_changed");
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn validate_sqlite_connection_identity(
    _conn: &Connection,
    component: &str,
    _expected: &DatabaseFileIdentity,
) -> Result<()> {
    anyhow::bail!("{component}_sqlite_connection_identity_unsupported_platform")
}

impl IdentityBoundSqliteConnection {
    pub fn writable(conn: Connection, owner_lease: SqliteSlotOwnerLease) -> Result<Self> {
        let component = owner_lease.inner.component.clone();
        owner_lease.validate_database_identity()?;
        let expected = owner_lease.retained_database_identity()?;
        validate_sqlite_connection_identity(&conn, &component, &expected)?;
        owner_lease.validate_database_identity()?;
        Ok(Self {
            conn: Mutex::new(conn),
            owner_lease: Some(owner_lease),
            identity_guard: None,
            component: Some(component),
        })
    }

    pub fn read_only(
        conn: Connection,
        identity_guard: SqliteDatabaseIdentityGuard,
    ) -> Result<Self> {
        let component = identity_guard.component.clone();
        identity_guard.validate()?;
        let expected = identity_guard.retained_identity()?;
        validate_sqlite_connection_identity(&conn, &component, &expected)?;
        identity_guard.validate()?;
        Ok(Self {
            conn: Mutex::new(conn),
            owner_lease: None,
            identity_guard: Some(identity_guard),
            component: Some(component),
        })
    }

    pub fn in_memory(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
            owner_lease: None,
            identity_guard: None,
            component: None,
        }
    }

    fn validate_external_identity(&self) -> Result<()> {
        if let Some(owner_lease) = self.owner_lease.as_ref() {
            owner_lease.validate_database_identity()?;
        }
        if let Some(identity_guard) = self.identity_guard.as_ref() {
            identity_guard.validate()?;
        }
        Ok(())
    }

    fn validate_open_connection(&self, conn: &Connection) -> Result<()> {
        #[cfg(test)]
        CHECKED_OPERATION_OPEN_IDENTITY_VALIDATION_COUNT.with(|count| count.set(count.get() + 1));
        if let (Some(component), Some(owner_lease)) =
            (self.component.as_deref(), self.owner_lease.as_ref())
        {
            let expected = owner_lease.retained_database_identity()?;
            validate_sqlite_connection_identity(conn, component, &expected)?;
        } else if let (Some(component), Some(identity_guard)) =
            (self.component.as_deref(), self.identity_guard.as_ref())
        {
            let expected = identity_guard.retained_identity()?;
            validate_sqlite_connection_identity(conn, component, &expected)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn validate_identity(&self) -> Result<()> {
        self.validate_external_identity()?;
        let guard = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_connection_mutex_poisoned:{error}"))?;
        self.validate_open_connection(&guard)?;
        self.validate_external_identity()
    }

    /// Acquire the identity-checked raw connection guard for legacy callers.
    /// Callers that need to revalidate after a pre-effect seam must use
    /// `with_checked_operation_after_preflight`; calling `validate_identity`
    /// while this guard is held would attempt to re-lock the same mutex.
    pub(crate) fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.validate_external_identity()?;
        let guard = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_connection_mutex_poisoned:{error}"))?;
        self.validate_open_connection(&guard)?;
        self.validate_external_identity()?;
        Ok(guard)
    }

    /// Execute one complete SQLite operation inside a pre/post identity
    /// boundary. Returning a raw `MutexGuard` cannot report a post-use identity
    /// failure to the current caller; this closure seam can. If both the SQL
    /// operation and postflight fail, postflight wins because the operation's
    /// externally observable result is no longer trustworthy.
    pub fn with_checked_operation<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> std::result::Result<T, SqliteCheckedOperationError> {
        self.with_checked_operation_internal(None::<fn()>, operation)
    }

    /// Execute a hook after the first identity preflight while retaining the
    /// single connection mutex guard, then revalidate the same borrowed SQLite
    /// handle and its external owner immediately before the operation. This is
    /// the seam for open/configure races: identity drift caused by the hook is
    /// reported as `Preflight` and the operation is never invoked.
    pub(crate) fn with_checked_operation_after_preflight<T>(
        &self,
        after_preflight: impl FnOnce(),
        operation: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> std::result::Result<T, SqliteCheckedOperationError> {
        self.with_checked_operation_internal(Some(after_preflight), operation)
    }

    fn with_checked_operation_internal<T>(
        &self,
        after_preflight: Option<impl FnOnce()>,
        operation: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> std::result::Result<T, SqliteCheckedOperationError> {
        self.validate_external_identity()
            .map_err(|source| SqliteCheckedOperationError {
                phase: SqliteCheckedOperationFailurePhase::Preflight,
                source,
            })?;
        let mut guard = self
            .conn
            .lock()
            .map_err(|error| SqliteCheckedOperationError {
                phase: SqliteCheckedOperationFailurePhase::Preflight,
                source: anyhow::anyhow!("sqlite_connection_mutex_poisoned:{error}"),
            })?;
        self.validate_open_connection(&guard)
            .and_then(|()| self.validate_external_identity())
            .map_err(|source| SqliteCheckedOperationError {
                phase: SqliteCheckedOperationFailurePhase::Preflight,
                source,
            })?;

        if let Some(after_preflight) = after_preflight {
            after_preflight();
            self.validate_open_connection(&guard)
                .and_then(|()| self.validate_external_identity())
                .map_err(|source| SqliteCheckedOperationError {
                    phase: SqliteCheckedOperationFailurePhase::Preflight,
                    source,
                })?;
        }

        let operation_result = operation(&mut guard);
        run_checked_operation_post_use_hook();
        let postflight = self
            .validate_open_connection(&guard)
            .and_then(|()| self.validate_external_identity());
        match (operation_result, postflight) {
            (_, Err(source)) => Err(SqliteCheckedOperationError {
                phase: SqliteCheckedOperationFailurePhase::Postflight,
                source,
            }),
            (Err(source), Ok(())) => Err(SqliteCheckedOperationError {
                phase: SqliteCheckedOperationFailurePhase::Operation,
                source,
            }),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}

fn sqlite_slot_owner_leases() -> &'static Mutex<HashMap<PathBuf, Weak<SqliteSlotOwnerLeaseInner>>> {
    static LEASES: OnceLock<Mutex<HashMap<PathBuf, Weak<SqliteSlotOwnerLeaseInner>>>> =
        OnceLock::new();
    LEASES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn canonical_sqlite_slot(path: &Path, component: &str) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("{component}_database_file_name_missing"))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_parent = std::fs::canonicalize(parent).with_context(|| {
        format!(
            "canonicalize {component} SQLite database parent before owner reservation: {}",
            parent.display()
        )
    })?;
    Ok(canonical_parent.join(file_name))
}

/// Stable, non-reversible binding for a canonical SQLite pathname. The digest
/// is intentionally path-bound (the database verifier separately binds the
/// retained file identity) so copying a reference document to another profile
/// cannot authorize that profile's database.
pub fn canonical_sqlite_slot_digest(path: &Path, component: &str) -> Result<String> {
    let canonical_slot = canonical_sqlite_slot(path, component)?;
    let mut material = b"openlife-sqlite-canonical-slot-v1\0".to_vec();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        material.extend_from_slice(canonical_slot.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        for unit in canonical_slot.as_os_str().encode_wide() {
            material.extend_from_slice(&unit.to_le_bytes());
        }
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        anyhow::bail!(
            "{component}_canonical_slot_digest_unsupported:{}",
            canonical_slot.display()
        );
    }
    let digest = ring::digest::digest(&ring::digest::SHA256, &material);
    let mut encoded = String::with_capacity("sha256:".len() + digest.as_ref().len() * 2);
    encoded.push_str("sha256:");
    for byte in digest.as_ref() {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(encoded)
}

fn sqlite_slot_owner_lease_path(canonical_slot: &Path) -> Result<PathBuf> {
    let file_name = canonical_slot
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("sqlite_slot_owner_lease_file_name_missing"))?;
    let mut lease_name = file_name.to_os_string();
    lease_name.push(".openlife-owner.lock");
    Ok(canonical_slot.with_file_name(lease_name))
}

impl SqliteSlotOwnerReservation {
    pub fn canonical_slot(&self) -> &Path {
        &self.inner.canonical_slot
    }

    pub fn canonical_slot_digest(&self) -> Result<String> {
        canonical_sqlite_slot_digest(&self.inner.canonical_slot, &self.inner.component)
    }

    pub fn existing_database_len(&self) -> Result<Option<u64>> {
        self.inner.validate_database_binding(false)?;
        let length = self
            .inner
            .database_binding
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_slot_owner_binding_mutex_poisoned:{error}"))?
            .file
            .as_ref()
            .map(|file| file.metadata().map(|metadata| metadata.len()))
            .transpose()?;
        self.inner.validate_database_binding(false)?;
        Ok(length)
    }

    /// Activate the exact database retained by this reservation. For a
    /// genuinely absent slot this is the first operation allowed to create the
    /// database file.
    pub fn activate_exact_database(self) -> Result<SqliteSlotOwnerLease> {
        let mut created_database = false;
        let activation = (|| -> Result<()> {
            self.inner.validate_database_binding(false)?;
            let mut binding = self.inner.database_binding.lock().map_err(|error| {
                anyhow::anyhow!("sqlite_slot_owner_binding_mutex_poisoned:{error}")
            })?;
            if binding.active {
                anyhow::bail!(
                    "{}_sqlite_slot_owner_reservation_already_activated",
                    self.inner.component
                );
            }
            match (binding.file.as_ref(), binding.identity.as_ref()) {
                (Some(file), Some(expected)) => {
                    let opened = DatabaseFileIdentity::from_file(
                        file,
                        &self.inner.canonical_slot,
                        &self.inner.component,
                    )?;
                    let current = DatabaseFileIdentity::from_path_no_follow(
                        &self.inner.canonical_slot,
                        &self.inner.component,
                    )?;
                    if &opened != expected || &current != expected {
                        anyhow::bail!(
                            "{}_database_identity_changed_before_activation:{}",
                            self.inner.component,
                            self.inner.canonical_slot.display()
                        );
                    }
                }
                (None, None) => {
                    match std::fs::symlink_metadata(&self.inner.canonical_slot) {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Ok(_) => anyhow::bail!(
                            "{}_database_appeared_before_activation:{}",
                            self.inner.component,
                            self.inner.canonical_slot.display()
                        ),
                        Err(error) => return Err(error.into()),
                    }
                    let mut database_options = OpenOptions::new();
                    database_options.create_new(true).read(true).write(true);
                    configure_no_follow_open_options(&mut database_options);
                    let database_file = database_options
                        .open(&self.inner.canonical_slot)
                        .with_context(|| {
                            format!(
                                "activate {} canonical SQLite database at {}",
                                self.inner.component,
                                self.inner.canonical_slot.display()
                            )
                        })?;
                    created_database = true;
                    binding.file = Some(database_file);
                    if activation_post_create_failure_injected(&self.inner.canonical_slot) {
                        anyhow::bail!("injected_sqlite_activation_post_create_failure");
                    }
                    let database_identity = DatabaseFileIdentity::from_file(
                        binding
                            .file
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("sqlite_activation_handle_missing"))?,
                        &self.inner.canonical_slot,
                        &self.inner.component,
                    )?;
                    let current = DatabaseFileIdentity::from_path_no_follow(
                        &self.inner.canonical_slot,
                        &self.inner.component,
                    )?;
                    if current != database_identity {
                        anyhow::bail!(
                            "{}_database_identity_changed_during_activation:{}",
                            self.inner.component,
                            self.inner.canonical_slot.display()
                        );
                    }
                    binding.identity = Some(database_identity);
                }
                _ => anyhow::bail!(
                    "{}_sqlite_slot_owner_database_binding_incomplete",
                    self.inner.component
                ),
            }
            binding.active = true;
            Ok(())
        })();
        if let Err(error) = activation {
            let reason = error.to_string();
            let cleanup = if created_database {
                cleanup_failed_database_activation(&self.inner).err()
            } else {
                None
            };
            let poison = self.inner.poison(reason.clone()).err();
            let detail = format!(
                "{}; cleanup={}; poison={}",
                reason,
                cleanup
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "ok_or_not_required".into()),
                poison
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "ok".into())
            );
            // An activation ambiguity must not release the process-local slot
            // for a later retry that could reinterpret a replaced pathname as
            // a clean database. The OS releases the leaked lock on process
            // exit; current-process effects remain fail-closed.
            std::mem::forget(self);
            anyhow::bail!("sqlite_exact_database_activation_failed:{detail}");
        }
        let lease = SqliteSlotOwnerLease { inner: self.inner };
        if let Err(error) = lease.validate_database_identity() {
            let reason = error.to_string();
            let cleanup = if created_database {
                cleanup_failed_database_activation(&lease.inner).err()
            } else {
                None
            };
            let _ = lease.inner.poison(reason.clone());
            let detail = format!(
                "{}; cleanup={}",
                reason,
                cleanup
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "ok_or_not_required".into())
            );
            std::mem::forget(lease);
            anyhow::bail!("sqlite_exact_database_activation_failed:{detail}");
        }
        Ok(lease)
    }
}

fn cleanup_failed_database_activation(inner: &SqliteSlotOwnerLeaseInner) -> Result<()> {
    let mut binding = inner
        .database_binding
        .lock()
        .map_err(|error| anyhow::anyhow!("sqlite_slot_owner_binding_mutex_poisoned:{error}"))?;
    let file = binding
        .file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("sqlite_activation_cleanup_handle_missing"))?;
    let opened = DatabaseFileIdentity::from_file(file, &inner.canonical_slot, &inner.component)?;
    let current =
        DatabaseFileIdentity::from_path_no_follow(&inner.canonical_slot, &inner.component)?;
    if opened != current {
        anyhow::bail!(
            "{}_activation_cleanup_identity_changed:{}",
            inner.component,
            inner.canonical_slot.display()
        );
    }
    let file = binding
        .file
        .take()
        .ok_or_else(|| anyhow::anyhow!("sqlite_activation_cleanup_handle_missing"))?;
    binding.identity = None;
    drop(file);
    std::fs::remove_file(&inner.canonical_slot).with_context(|| {
        format!(
            "remove failed {} database activation at {}",
            inner.component,
            inner.canonical_slot.display()
        )
    })?;
    #[cfg(unix)]
    if let Some(parent) = inner.canonical_slot.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

impl SqliteSlotOwnerLease {
    /// Reserve the exact canonical owner slot without creating or activating
    /// the database. The owner sidecar lock is the only object this operation
    /// may create.
    pub fn reserve_no_create(
        canonical_slot: &Path,
        component: &str,
    ) -> Result<SqliteSlotOwnerReservation> {
        let lease_path = sqlite_slot_owner_lease_path(canonical_slot)?;
        let mut leases = sqlite_slot_owner_leases()
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_slot_owner_registry_poisoned:{error}"))?;
        if leases.get(&lease_path).and_then(Weak::upgrade).is_some() {
            anyhow::bail!(
                "{component}_sqlite_slot_owner_lease_unavailable:{}",
                canonical_slot.display()
            );
        }
        leases.remove(&lease_path);

        let lock_component = format!("{component}_owner_lock");
        match std::fs::symlink_metadata(&lease_path) {
            Ok(metadata) => {
                DatabaseFileIdentity::validate_metadata(&metadata, &lease_path, &lock_component)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "inspect {component} canonical SQLite owner lease at {}",
                        lease_path.display()
                    )
                });
            }
        }
        let mut lock_options = OpenOptions::new();
        lock_options.create(true).read(true).write(true);
        configure_no_follow_open_options(&mut lock_options);
        let lock_file = lock_options.open(&lease_path).with_context(|| {
            format!(
                "open {component} canonical SQLite owner lease at {}",
                lease_path.display()
            )
        })?;
        lock_file.try_lock_exclusive().with_context(|| {
            format!(
                "{component}_sqlite_slot_owner_lease_unavailable:{}",
                canonical_slot.display()
            )
        })?;
        let lock_file_identity =
            DatabaseFileIdentity::from_file(&lock_file, &lease_path, &lock_component)?;
        let lock_path_identity =
            DatabaseFileIdentity::from_path_no_follow(&lease_path, &lock_component)?;
        if lock_file_identity != lock_path_identity {
            anyhow::bail!(
                "{component}_owner_lock_identity_changed_during_open:{}",
                lease_path.display()
            );
        }

        let (database_existed_at_reservation, database_file, database_identity) =
            match std::fs::symlink_metadata(canonical_slot) {
                Ok(metadata) => {
                    DatabaseFileIdentity::validate_metadata(&metadata, canonical_slot, component)?;
                    let mut database_options = OpenOptions::new();
                    database_options.read(true);
                    configure_no_follow_open_options(&mut database_options);
                    let database_file =
                        database_options.open(canonical_slot).with_context(|| {
                            format!(
                                "retain existing {component} SQLite identity at {}",
                                canonical_slot.display()
                            )
                        })?;
                    let database_identity =
                        DatabaseFileIdentity::from_file(&database_file, canonical_slot, component)?;
                    let database_path_identity =
                        DatabaseFileIdentity::from_path_no_follow(canonical_slot, component)?;
                    if database_identity != database_path_identity {
                        anyhow::bail!(
                            "{component}_database_identity_changed_during_reservation:{}",
                            canonical_slot.display()
                        );
                    }
                    (true, Some(database_file), Some(database_identity))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (false, None, None),
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "inspect {component} canonical SQLite database at {}",
                            canonical_slot.display()
                        )
                    });
                }
            };

        let inner = Arc::new(SqliteSlotOwnerLeaseInner {
            canonical_slot: canonical_slot.to_path_buf(),
            component: component.to_string(),
            owner_generation_id: uuid::Uuid::new_v4(),
            lock_path: lease_path.clone(),
            lock_file,
            lock_file_identity,
            lock_file_io: Mutex::new(()),
            database_existed_at_reservation,
            database_binding: Mutex::new(SqliteSlotDatabaseBinding {
                file: database_file,
                identity: database_identity,
                active: false,
            }),
            poisoned: Mutex::new(None),
        });
        leases.insert(lease_path, Arc::downgrade(&inner));
        Ok(SqliteSlotOwnerReservation { inner })
    }

    pub fn acquire(canonical_slot: &Path, component: &str) -> Result<Self> {
        Self::reserve_no_create(canonical_slot, component)?.activate_exact_database()
    }

    /// One generation for one successfully acquired writable owner lease.
    /// Clones retain the same generation; a later owner receives a new one.
    pub fn owner_generation_id(&self) -> uuid::Uuid {
        self.inner.owner_generation_id
    }

    pub fn owner_lock_identity_material(&self) -> String {
        self.inner.lock_file_identity.binding_material()
    }

    /// Exact retained database-file identity used by a component-specific
    /// pre-authentication envelope. This is derived from the already-open
    /// no-follow descriptor, not from a pathname that can be retargeted.
    pub fn database_identity_material(&self) -> Result<String> {
        self.validate_database_identity()?;
        let binding =
            self.inner.database_binding.lock().map_err(|error| {
                anyhow::anyhow!("sqlite_slot_owner_binding_mutex_poisoned:{error}")
            })?;
        let identity = binding.identity.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "{}_sqlite_slot_owner_lease_initializing:{}",
                self.inner.component,
                self.inner.canonical_slot.display()
            )
        })?;
        Ok(identity.binding_material())
    }

    /// Size of the exact retained database inode. Callers use this instead of
    /// a pre-lease pathname stat so a same-inode write or path race cannot make
    /// a non-empty legacy store look like a fresh empty slot.
    pub fn database_len(&self) -> Result<u64> {
        self.validate_database_identity()?;
        let length = self
            .inner
            .database_binding
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_slot_owner_binding_mutex_poisoned:{error}"))?
            .file
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("sqlite_slot_owner_database_handle_missing"))?
            .metadata()?
            .len();
        self.validate_database_identity()?;
        Ok(length)
    }

    pub fn database_was_created(&self) -> bool {
        !self.inner.database_existed_at_reservation
    }

    /// Read a small component-owned authentication envelope from the retained
    /// owner-lock inode. The OS lock is already held, all accesses are bounded,
    /// and identities are revalidated on both sides of the read.
    pub fn read_owner_lock_envelope(&self, max_bytes: usize) -> Result<Vec<u8>> {
        self.validate_database_identity()?;
        let _io = self
            .inner
            .lock_file_io
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_owner_lock_io_mutex_poisoned:{error}"))?;
        let metadata = self.inner.lock_file.metadata().with_context(|| {
            format!(
                "read {} owner-lock envelope metadata at {}",
                self.inner.component,
                self.inner.lock_path.display()
            )
        })?;
        if metadata.len() > max_bytes as u64 {
            anyhow::bail!(
                "{}_owner_lock_envelope_too_large:{}",
                self.inner.component,
                metadata.len()
            );
        }
        let mut reader = self.inner.lock_file.try_clone()?;
        reader.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        reader.take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > max_bytes {
            anyhow::bail!("{}_owner_lock_envelope_too_large", self.inner.component);
        }
        self.validate_database_identity()?;
        Ok(bytes)
    }

    /// Persist a bounded envelope on the exact locked inode. A partial write
    /// after a crash is intentionally not recoverable as valid authority: the
    /// caller's signed envelope parser must reject it on the next open. fsync
    /// covers both file contents and first-name directory durability.
    pub fn write_owner_lock_envelope(&self, bytes: &[u8], max_bytes: usize) -> Result<()> {
        if bytes.is_empty() || bytes.len() > max_bytes {
            anyhow::bail!(
                "{}_owner_lock_envelope_size_invalid:{}",
                self.inner.component,
                bytes.len()
            );
        }
        self.validate_database_identity()?;
        let _io = self
            .inner
            .lock_file_io
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_owner_lock_io_mutex_poisoned:{error}"))?;
        let mut writer = self.inner.lock_file.try_clone()?;
        writer.seek(SeekFrom::Start(0))?;
        writer.set_len(0)?;
        writer.write_all(bytes)?;
        writer.sync_all()?;
        #[cfg(unix)]
        if let Some(parent) = self.inner.lock_path.parent() {
            File::open(parent)?.sync_all()?;
        }
        self.validate_database_identity()?;
        Ok(())
    }

    /// Bind the lease to the exact file SQLite opened. This catches a same-path
    /// inode replacement between lease acquisition and `Connection::open`.
    pub fn bind_opened_database_identity(&self, conn: &Connection) -> Result<()> {
        self.validate_database_identity()?;
        let expected = self.retained_database_identity()?;
        validate_sqlite_connection_identity(conn, &self.inner.component, &expected)?;
        self.validate_database_identity()
    }

    pub fn validate_database_identity(&self) -> Result<()> {
        self.inner.validate_database_binding(true)
    }

    fn retained_database_identity(&self) -> Result<DatabaseFileIdentity> {
        self.validate_database_identity()?;
        let identity = self
            .inner
            .database_binding
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_slot_owner_binding_mutex_poisoned:{error}"))?
            .identity
            .clone()
            .ok_or_else(|| anyhow::anyhow!("{}_database_identity_missing", self.inner.component))?;
        self.validate_database_identity()?;
        Ok(identity)
    }

    #[cfg(test)]
    fn lease_path(&self) -> Result<PathBuf> {
        sqlite_slot_owner_lease_path(&self.inner.canonical_slot)
    }
}

/// Resolve the filesystem slot owned by the already-open SQLite connection.
/// This must be preferred over re-reading a caller path after `Connection::open`:
/// a symlink can be swapped and restored while SQLite keeps the originally
/// opened file descriptor. Empty paths are valid only for in-memory databases,
/// which persistent authority callers must reject themselves.
pub fn canonical_opened_main_database_path(
    conn: &Connection,
    component: &str,
) -> Result<Option<PathBuf>> {
    let database_path: String = conn
        .query_row(
            "SELECT file FROM pragma_database_list WHERE name = 'main'",
            [],
            |row| row.get(0),
        )
        .with_context(|| format!("read {component} SQLite connection-owned database path"))?;
    if database_path.is_empty() {
        return Ok(None);
    }
    let canonical = std::fs::canonicalize(&database_path).with_context(|| {
        format!("canonicalize {component} SQLite connection-owned database path: {database_path}")
    })?;
    Ok(Some(canonical))
}

pub fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    if !valid_identifier(table) || !valid_identifier(column) {
        anyhow::bail!("invalid SQLite migration identifier");
    }
    if definition.trim().is_empty() || definition.contains(';') {
        anyhow::bail!("invalid SQLite column definition");
    }
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

pub fn record_schema_version(conn: &Connection, component: &str, version: i64) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS openlife_schema_versions (
            component TEXT PRIMARY KEY,
            version INTEGER NOT NULL,
            applied_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "INSERT INTO openlife_schema_versions (component, version, applied_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(component) DO UPDATE SET
             version = excluded.version,
             applied_at = excluded.applied_at",
        rusqlite::params![component, version, chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

/// Opens an existing SQLite canonical store without acquiring write authority.
///
/// This is deliberately not a migration path: callers may use it only to keep
/// trustworthy, already-materialized reads available while the product is in
/// a global degraded/read-only state. Missing/corrupt stores and stores whose
/// required schema is absent fail closed instead of being replaced by an empty
/// database.
pub fn open_existing_read_only(
    path: &Path,
    component: &str,
    required_tables: &[&str],
) -> Result<Connection> {
    if !path.is_file() {
        anyhow::bail!(
            "{component} read-only recovery requires an existing SQLite file at {}",
            path.display()
        );
    }
    for table in required_tables {
        if !valid_identifier(table) {
            anyhow::bail!("invalid required SQLite table identifier for {component}");
        }
    }

    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| {
        format!(
            "failed to open {component} canonical store read-only at {}",
            path.display()
        )
    })?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "query_only", true)?;

    let integrity: String = conn.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
    if integrity != "ok" {
        anyhow::bail!("{component} read-only integrity check failed: {integrity}");
    }
    for table in required_tables {
        let exists = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
                [*table],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if exists.is_none() {
            anyhow::bail!("{component} read-only schema is missing required table {table}");
        }
    }
    Ok(conn)
}

fn sqlite_immutable_uri(path: &Path) -> Result<String> {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(not(unix))]
    let bytes = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("immutable SQLite path is not Unicode"))?
        .as_bytes()
        .to_vec();
    let mut encoded = String::with_capacity(bytes.len() + 32);
    for byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(format!("file:{encoded}?mode=ro&immutable=1"))
}

fn sqlite_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut sidecar = path.as_os_str().to_os_string();
    sidecar.push(suffix);
    PathBuf::from(sidecar)
}

/// Zero-write authentication reader for an existing checkpointed SQLite main
/// database. `immutable=1` prevents SQLite from creating WAL/SHM files even
/// though the filesystem is writable. A present WAL/SHM is rejected because
/// immutable mode deliberately ignores it and therefore could authenticate a
/// stale main database instead of the latest committed truth.
pub fn open_existing_immutable_read_only(
    path: &Path,
    component: &str,
    required_tables: &[&str],
) -> Result<Connection> {
    if !path.is_file() {
        anyhow::bail!(
            "{component} immutable preflight requires an existing SQLite file at {}",
            path.display()
        );
    }
    for suffix in ["-wal", "-shm"] {
        let sidecar = sqlite_sidecar_path(path, suffix);
        if std::fs::symlink_metadata(&sidecar).is_ok() {
            anyhow::bail!(
                "{component}_immutable_preflight_sidecar_present:{}",
                sidecar.display()
            );
        }
    }
    for table in required_tables {
        if !valid_identifier(table) {
            anyhow::bail!("invalid required SQLite table identifier for {component}");
        }
    }
    let uri = sqlite_immutable_uri(path)?;
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| {
        format!(
            "failed to open {component} immutable SQLite preflight at {}",
            path.display()
        )
    })?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "query_only", true)?;
    let integrity: String = conn.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
    if integrity != "ok" {
        anyhow::bail!("{component} immutable integrity check failed: {integrity}");
    }
    for table in required_tables {
        let exists = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
                [*table],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if exists.is_none() {
            anyhow::bail!("{component} immutable schema is missing required table {table}");
        }
    }
    Ok(conn)
}

/// Structural sentinel for an unavailable canonical store. It is intentionally
/// schema-less and query-only: every domain read/write fails instead of
/// fabricating an empty canonical database, and it can never accept a write.
pub fn unavailable_read_only_sentinel(component: &str) -> Result<Connection> {
    let conn = Connection::open_in_memory()
        .with_context(|| format!("failed to allocate {component} unavailable sentinel"))?;
    conn.pragma_update(None, "query_only", true)?;
    let query_only: i64 = conn.query_row("PRAGMA query_only", [], |row| row.get(0))?;
    if query_only != 1 {
        anyhow::bail!("{component} unavailable sentinel is not query-only");
    }
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_checked_operation_does_not_pay_for_race_hook_revalidation() {
        let connection =
            IdentityBoundSqliteConnection::in_memory(Connection::open_in_memory().unwrap());

        reset_checked_operation_open_identity_validation_count();
        connection
            .with_checked_operation(|conn| {
                conn.execute("CREATE TABLE ordinary_check(id INTEGER PRIMARY KEY)", [])?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            checked_operation_open_identity_validation_count(),
            2,
            "ordinary operations need one pre-operation handle validation and one postflight; the race-hook-only revalidation must not spread to the hot path"
        );

        reset_checked_operation_open_identity_validation_count();
        let mut hook_ran = false;
        connection
            .with_checked_operation_after_preflight(
                || hook_ran = true,
                |conn| {
                    conn.query_row("SELECT COUNT(*) FROM ordinary_check", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                    Ok(())
                },
            )
            .unwrap();
        assert!(hook_ran);
        assert_eq!(
            checked_operation_open_identity_validation_count(),
            3,
            "the explicit initialization race seam must revalidate once after its hook, in addition to normal pre-operation and postflight checks"
        );
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "subprocess worker for hard-link identity fault injection"]
    fn sqlite_hardlink_child_worker() {
        if std::env::var("OPENLIFE_SQLITE_HARDLINK_CHILD").as_deref() != Ok("1") {
            return;
        }
        let canonical =
            std::path::PathBuf::from(std::env::var("OPENLIFE_SQLITE_HARDLINK_CANONICAL").unwrap());
        let alias =
            std::path::PathBuf::from(std::env::var("OPENLIFE_SQLITE_HARDLINK_ALIAS").unwrap());
        let result = std::fs::hard_link(canonical, alias)
            .map(|()| "ok".to_string())
            .unwrap_or_else(|error| format!("error:{error}"));
        std::fs::write(
            std::env::var("OPENLIFE_SQLITE_HARDLINK_RESULT").unwrap(),
            result,
        )
        .unwrap();
    }

    #[test]
    fn ensure_column_is_idempotent_and_version_is_inspectable() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        tx.execute("CREATE TABLE sample (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        ensure_column(&tx, "sample", "status", "TEXT NOT NULL DEFAULT 'pending'").unwrap();
        ensure_column(&tx, "sample", "status", "TEXT NOT NULL DEFAULT 'pending'").unwrap();
        record_schema_version(&tx, "sample", 2).unwrap();
        tx.commit().unwrap();

        let version: i64 = conn
            .query_row(
                "SELECT version FROM openlife_schema_versions WHERE component = 'sample'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 2);
    }

    #[test]
    fn read_only_recovery_requires_existing_integral_schema_and_rejects_writes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("canonical.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute("CREATE TABLE canonical_facts (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        drop(conn);

        let conn = open_existing_read_only(&path, "test_store", &["canonical_facts"]).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM canonical_facts", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(conn
            .execute("INSERT INTO canonical_facts DEFAULT VALUES", [])
            .is_err());
        assert!(open_existing_read_only(&path, "test_store", &["missing_table"]).is_err());
        assert!(open_existing_read_only(
            &directory.path().join("absent.db"),
            "test_store",
            &["canonical_facts"]
        )
        .is_err());
    }

    #[test]
    fn unavailable_sentinel_is_schema_less_query_only_and_never_looks_empty() {
        let conn = unavailable_read_only_sentinel("test_store").unwrap();
        assert!(conn
            .query_row("SELECT COUNT(*) FROM canonical_facts", [], |row| row
                .get::<_, i64>(0))
            .is_err());
        assert!(conn
            .execute("CREATE TABLE canonical_facts (id INTEGER)", [])
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn preexisting_database_hardlink_is_rejected_before_owner_activation() {
        let directory = tempfile::tempdir().unwrap();
        let slot = directory.path().join("canonical.sqlite");
        let alias = directory.path().join("canonical-alias.sqlite");
        Connection::open(&slot)
            .unwrap()
            .execute("CREATE TABLE canonical_fact(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        std::fs::hard_link(&slot, &alias).unwrap();

        let error = SqliteSlotOwnerLease::reserve_no_create(&slot, "hardlink_preflight")
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("hardlink_preflight_database_link_count_invalid:2"),
            "{error}"
        );
        let conn = Connection::open(&slot).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM canonical_fact", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        std::fs::remove_file(alias).unwrap();
        let reservation =
            SqliteSlotOwnerLease::reserve_no_create(&slot, "hardlink_preflight").unwrap();
        reservation.activate_exact_database().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn cross_process_hardlink_poisons_retained_owner_until_all_clones_drop() {
        let directory = tempfile::tempdir().unwrap();
        let slot = directory.path().join("canonical.sqlite");
        let alias = directory.path().join("canonical-alias.sqlite");
        let result = directory.path().join("child-result.txt");
        Connection::open(&slot)
            .unwrap()
            .execute("CREATE TABLE canonical_fact(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        let owner = SqliteSlotOwnerLease::acquire(&slot, "hardlink_live_owner").unwrap();
        let retained_clone = owner.clone();

        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("sqlite_migration::tests::sqlite_hardlink_child_worker")
            .arg("--ignored")
            .arg("--nocapture")
            .env("OPENLIFE_SQLITE_HARDLINK_CHILD", "1")
            .env("OPENLIFE_SQLITE_HARDLINK_CANONICAL", &slot)
            .env("OPENLIFE_SQLITE_HARDLINK_ALIAS", &alias)
            .env("OPENLIFE_SQLITE_HARDLINK_RESULT", &result)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(&result).unwrap(), "ok");

        let first = owner.validate_database_identity().unwrap_err().to_string();
        assert!(
            first.contains("hardlink_live_owner_database_link_count_invalid:2"),
            "{first}"
        );
        std::fs::remove_file(&alias).unwrap();
        let still_poisoned = retained_clone
            .validate_database_identity()
            .unwrap_err()
            .to_string();
        assert!(
            still_poisoned.contains("hardlink_live_owner_sqlite_slot_owner_poisoned"),
            "{still_poisoned}"
        );
        drop(owner);
        assert!(
            SqliteSlotOwnerLease::reserve_no_create(&slot, "hardlink_live_owner").is_err(),
            "a retained clone must keep the poisoned generation and OS lease alive"
        );
        drop(retained_clone);
        SqliteSlotOwnerLease::reserve_no_create(&slot, "hardlink_live_owner")
            .unwrap()
            .activate_exact_database()
            .unwrap();
    }

    #[test]
    fn no_create_owner_reservation_precedes_database_activation_and_survives_growth() {
        let directory = tempfile::tempdir().unwrap();
        let slot = directory.path().join("reserved.sqlite");
        let canonical_slot = canonical_sqlite_slot(&slot, "reservation_test").unwrap();
        let reservation =
            SqliteSlotOwnerLease::reserve_no_create(&canonical_slot, "reservation_test").unwrap();

        assert!(!slot.exists(), "reservation must not create the database");
        assert_eq!(reservation.existing_database_len().unwrap(), None);
        assert!(
            SqliteSlotOwnerLease::acquire(&canonical_slot, "reservation_test")
                .unwrap_err()
                .to_string()
                .contains("reservation_test_sqlite_slot_owner_lease_unavailable")
        );

        let lease = reservation.activate_exact_database().unwrap();
        assert!(slot.exists());
        let conn = Connection::open_with_flags(
            &canonical_slot,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .unwrap();
        let connection = IdentityBoundSqliteConnection::writable(conn, lease.clone()).unwrap();
        {
            let conn = connection.lock().unwrap();
            conn.execute(
                "CREATE TABLE canonical_growth(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
                [],
            )
            .unwrap();
            for value in 0..256 {
                conn.execute(
                    "INSERT INTO canonical_growth(value) VALUES (?1)",
                    [format!("row-{value:04}-{}", "x".repeat(64))],
                )
                .unwrap();
            }
        }
        connection.validate_identity().unwrap();
        lease.validate_database_identity().unwrap();
        assert!(lease.database_len().unwrap() > 0);
    }

    #[test]
    fn failed_post_create_activation_removes_exact_database_and_poison_holds_slot() {
        let directory = tempfile::tempdir().unwrap();
        let slot = directory.path().join("failed-activation.sqlite");
        let reservation =
            SqliteSlotOwnerLease::reserve_no_create(&slot, "activation_failure_test").unwrap();
        ACTIVATION_POST_CREATE_FAILURE_PATH.with(|injected| {
            injected.replace(Some(slot.clone()));
        });

        let error = reservation.activate_exact_database().unwrap_err();
        ACTIVATION_POST_CREATE_FAILURE_PATH.with(|injected| {
            injected.replace(None);
        });

        assert!(error
            .to_string()
            .contains("injected_sqlite_activation_post_create_failure"));
        assert!(
            !slot.exists(),
            "an exact file created by the failed activation must be removed"
        );
        assert!(
            SqliteSlotOwnerLease::reserve_no_create(&slot, "activation_failure_test")
                .unwrap_err()
                .to_string()
                .contains("activation_failure_test_sqlite_slot_owner_lease_unavailable")
        );
    }

    #[test]
    fn existing_database_retained_identity_stays_bound_across_sqlite_commits() {
        let directory = tempfile::tempdir().unwrap();
        let slot = directory.path().join("existing-bound.sqlite");
        Connection::open(&slot)
            .unwrap()
            .execute(
                "CREATE TABLE canonical_commit(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
                [],
            )
            .unwrap();
        let canonical_slot = canonical_sqlite_slot(&slot, "existing_binding_test").unwrap();
        let reservation =
            SqliteSlotOwnerLease::reserve_no_create(&canonical_slot, "existing_binding_test")
                .unwrap();
        assert!(reservation.existing_database_len().unwrap().unwrap() > 0);
        let lease = reservation.activate_exact_database().unwrap();
        let conn = Connection::open_with_flags(
            &canonical_slot,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .unwrap();
        let connection = IdentityBoundSqliteConnection::writable(conn, lease.clone()).unwrap();

        for value in 0..4 {
            {
                let mut conn = connection.lock().unwrap();
                let transaction = conn.transaction().unwrap();
                transaction
                    .execute(
                        "INSERT INTO canonical_commit(value) VALUES (?1)",
                        [format!("commit-{value}")],
                    )
                    .unwrap();
                transaction.commit().unwrap();
            }
            connection.validate_identity().unwrap();
            lease.validate_database_identity().unwrap();
        }
    }

    #[test]
    fn canonical_slot_owner_lease_rejects_same_path_replacement_and_releases_on_drop() {
        let directory = tempfile::tempdir().unwrap();
        let slot = directory.path().join("canonical.sqlite");
        let replacement = directory.path().join("replacement.sqlite");
        let displaced = directory.path().join("displaced.sqlite");
        Connection::open(&slot)
            .unwrap()
            .execute("CREATE TABLE original_owner(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        Connection::open(&replacement)
            .unwrap()
            .execute("CREATE TABLE replacement_owner(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        let canonical_slot = std::fs::canonicalize(&slot).unwrap();

        let first = SqliteSlotOwnerLease::acquire(&canonical_slot, "lease_test").unwrap();
        let first_connection = Connection::open(&canonical_slot).unwrap();
        first
            .bind_opened_database_identity(&first_connection)
            .unwrap();
        drop(first_connection);
        let final_store_clone = first.clone();
        assert!(SqliteSlotOwnerLease::acquire(&canonical_slot, "lease_test")
            .unwrap_err()
            .to_string()
            .contains("lease_test_sqlite_slot_owner_lease_unavailable"));
        let lease_path = first.lease_path().unwrap();
        let competing_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lease_path)
            .unwrap();
        assert!(
            competing_lock.try_lock_exclusive().is_err(),
            "the retained OS lease must reject an independent lock handle"
        );
        std::fs::rename(&slot, &displaced).unwrap();
        std::fs::rename(&replacement, &slot).unwrap();
        assert!(first
            .validate_database_identity()
            .unwrap_err()
            .to_string()
            .contains("lease_test_database_identity_changed"));
        assert!(SqliteSlotOwnerLease::acquire(&canonical_slot, "lease_test")
            .unwrap_err()
            .to_string()
            .contains("lease_test_sqlite_slot_owner_lease_unavailable"));

        drop(first);
        assert!(SqliteSlotOwnerLease::acquire(&canonical_slot, "lease_test")
            .unwrap_err()
            .to_string()
            .contains("lease_test_sqlite_slot_owner_lease_unavailable"));
        drop(final_store_clone);
        let replacement_owner =
            SqliteSlotOwnerLease::acquire(&canonical_slot, "lease_test").unwrap();
        let replacement_connection = Connection::open(&canonical_slot).unwrap();
        replacement_owner
            .bind_opened_database_identity(&replacement_connection)
            .unwrap();
        replacement_owner.validate_database_identity().unwrap();
    }

    #[test]
    fn sqlite_connection_swap_during_open_then_restore_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let slot = directory.path().join("canonical.sqlite");
        let displaced_original = directory.path().join("original.displaced.sqlite");
        let swap_candidate = directory.path().join("swap.sqlite");
        let displaced_swap = directory.path().join("swap.displaced.sqlite");
        Connection::open(&slot)
            .unwrap()
            .execute("CREATE TABLE original(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        std::fs::copy(&slot, &swap_candidate).unwrap();
        let canonical_slot = std::fs::canonicalize(&slot).unwrap();
        let owner = SqliteSlotOwnerLease::acquire(&canonical_slot, "swap_open_test").unwrap();

        // Deterministic barrier sequence for the race: the lease retains the
        // original inode, SQLite opens a byte-identical clone at the canonical
        // path, then the original path is restored before validation.
        std::fs::rename(&slot, &displaced_original).unwrap();
        std::fs::rename(&swap_candidate, &slot).unwrap();
        let connection = Connection::open_with_flags(
            &canonical_slot,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .unwrap();
        std::fs::rename(&slot, &displaced_swap).unwrap();
        std::fs::rename(&displaced_original, &slot).unwrap();

        owner.validate_database_identity().unwrap();
        let error = owner
            .bind_opened_database_identity(&connection)
            .unwrap_err()
            .to_string();
        #[cfg(unix)]
        assert!(
            error.contains("swap_open_test_sqlite_connection_file_moved"),
            "{error}"
        );
        #[cfg(windows)]
        assert!(
            error.contains("swap_open_test_sqlite_connection_file_identity_changed"),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn existing_owner_lease_database_fd_is_read_only_and_descriptors_are_close_on_exec() {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::FileExt;

        let directory = tempfile::tempdir().unwrap();
        let slot = directory.path().join("existing.sqlite");
        Connection::open(&slot)
            .unwrap()
            .execute("CREATE TABLE canonical_fact(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        let canonical_slot = std::fs::canonicalize(&slot).unwrap();
        let before = std::fs::read(&canonical_slot).unwrap();
        let lease = SqliteSlotOwnerLease::acquire(&canonical_slot, "lease_fd_test").unwrap();
        let binding = lease.inner.database_binding.lock().unwrap();
        let database_file = binding.file.as_ref().unwrap();

        assert!(
            database_file.write_at(b"X", 0).is_err(),
            "an existing database identity fd must not carry write authority"
        );
        assert_eq!(std::fs::read(&canonical_slot).unwrap(), before);
        for fd in [database_file.as_raw_fd(), lease.inner.lock_file.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            assert!(flags >= 0);
            assert_ne!(
                flags & libc::FD_CLOEXEC,
                0,
                "owner descriptors must not leak into unrelated child processes"
            );
        }
    }
}
