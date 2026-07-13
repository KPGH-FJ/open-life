use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

/// Test-only runtime evidence for SQLite work performed during a canonical
/// store preflight. Counters come from SQLite itself, not from SQL source
/// matching or an inferred query plan.
#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteRuntimeObservation {
    pub database_path: PathBuf,
    pub database_identity: String,
    pub component: String,
    pub operation: String,
    pub fullscan_steps: i64,
    pub vm_steps: i64,
    pub quick_check_result: Option<String>,
}

#[cfg(any(test, feature = "test-utils"))]
std::thread_local! {
    static SQLITE_RUNTIME_OBSERVATIONS: std::cell::RefCell<Option<Vec<SqliteRuntimeObservation>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(any(test, feature = "test-utils"))]
pub struct SqliteRuntimeObserverGuard {
    previous: Option<Vec<SqliteRuntimeObservation>>,
}

#[cfg(any(test, feature = "test-utils"))]
impl SqliteRuntimeObserverGuard {
    pub fn snapshot(&self) -> Vec<SqliteRuntimeObservation> {
        SQLITE_RUNTIME_OBSERVATIONS.with(|slot| slot.borrow().clone().unwrap_or_default())
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Drop for SqliteRuntimeObserverGuard {
    fn drop(&mut self) {
        SQLITE_RUNTIME_OBSERVATIONS.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

#[cfg(any(test, feature = "test-utils"))]
pub fn begin_sqlite_runtime_observation_for_test() -> SqliteRuntimeObserverGuard {
    let previous = SQLITE_RUNTIME_OBSERVATIONS.with(|slot| slot.replace(Some(Vec::new())));
    SqliteRuntimeObserverGuard { previous }
}

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
    #[cfg(not(unix))]
    created: Option<std::time::SystemTime>,
    #[cfg(not(unix))]
    length: u64,
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
        #[cfg(not(unix))]
        {
            format!("portable:{:?}:{}", self.created, self.length)
        }
    }

    fn from_metadata(metadata: &std::fs::Metadata, path: &Path, component: &str) -> Result<Self> {
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
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                hard_link_count,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                created: metadata.created().ok(),
                length: metadata.len(),
            })
        }
    }

    fn from_path(path: &Path, component: &str) -> Result<Self> {
        let metadata = std::fs::metadata(path).with_context(|| {
            format!(
                "read {component} canonical SQLite database identity at {}",
                path.display()
            )
        })?;
        Self::from_metadata(&metadata, path, component)
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
        Self::from_metadata(&metadata, path, component)
    }

    fn from_file(file: &File, path: &Path, component: &str) -> Result<Self> {
        let metadata = file.metadata().with_context(|| {
            format!(
                "read {component} opened SQLite file identity at {}",
                path.display()
            )
        })?;
        Self::from_metadata(&metadata, path, component)
    }
}

#[cfg(any(test, feature = "test-utils"))]
fn sqlite_runtime_observation_identity(path: &Path, component: &str) -> String {
    DatabaseFileIdentity::from_path(path, component)
        .map(|identity| identity.binding_material())
        .unwrap_or_else(|error| format!("identity_unavailable:{error}"))
}

#[cfg(any(test, feature = "test-utils"))]
fn push_sqlite_runtime_observation(
    path: &Path,
    component: &str,
    operation: &str,
    fullscan_steps: i64,
    vm_steps: i64,
    quick_check_result: Option<String>,
) {
    SQLITE_RUNTIME_OBSERVATIONS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(observations) = slot.as_mut() else {
            return;
        };
        observations.push(SqliteRuntimeObservation {
            database_path: path.to_path_buf(),
            database_identity: sqlite_runtime_observation_identity(path, component),
            component: component.to_string(),
            operation: operation.to_string(),
            fullscan_steps,
            vm_steps,
            quick_check_result,
        });
    });
}

#[cfg(any(test, feature = "test-utils"))]
pub(crate) fn observe_sqlite_runtime_event_for_test(path: &Path, component: &str, operation: &str) {
    push_sqlite_runtime_observation(path, component, operation, 0, 0, None);
}

#[cfg(any(test, feature = "test-utils"))]
pub(crate) fn observe_sqlite_statement_for_test(
    path: &Path,
    component: &str,
    operation: &str,
    statement: &rusqlite::Statement<'_>,
    quick_check_result: Option<&str>,
) {
    push_sqlite_runtime_observation(
        path,
        component,
        operation,
        i64::from(statement.get_status(rusqlite::StatementStatus::FullscanStep)),
        i64::from(statement.get_status(rusqlite::StatementStatus::VmStep)),
        quick_check_result.map(str::to_string),
    );
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
    database_file: File,
    database_was_created: bool,
    database_identity: Mutex<Option<DatabaseFileIdentity>>,
    poisoned: Mutex<Option<String>>,
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
}

impl IdentityBoundSqliteConnection {
    pub fn writable(conn: Connection, owner_lease: SqliteSlotOwnerLease) -> Self {
        Self {
            conn: Mutex::new(conn),
            owner_lease: Some(owner_lease),
            identity_guard: None,
        }
    }

    pub fn read_only(conn: Connection, identity_guard: SqliteDatabaseIdentityGuard) -> Self {
        Self {
            conn: Mutex::new(conn),
            owner_lease: None,
            identity_guard: Some(identity_guard),
        }
    }

    pub fn in_memory(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
            owner_lease: None,
            identity_guard: None,
        }
    }

    pub fn validate_identity(&self) -> Result<()> {
        if let Some(owner_lease) = self.owner_lease.as_ref() {
            owner_lease.validate_database_identity()?;
        }
        if let Some(identity_guard) = self.identity_guard.as_ref() {
            identity_guard.validate()?;
        }
        Ok(())
    }

    pub fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.validate_identity()?;
        self.conn
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_connection_mutex_poisoned:{error}"))
    }
}

fn sqlite_slot_owner_leases() -> &'static Mutex<HashMap<PathBuf, Weak<SqliteSlotOwnerLeaseInner>>> {
    static LEASES: OnceLock<Mutex<HashMap<PathBuf, Weak<SqliteSlotOwnerLeaseInner>>>> =
        OnceLock::new();
    LEASES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sqlite_slot_owner_lease_path(canonical_slot: &Path) -> Result<PathBuf> {
    let file_name = canonical_slot
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("sqlite_slot_owner_lease_file_name_missing"))?;
    let mut lease_name = file_name.to_os_string();
    lease_name.push(".openlife-owner.lock");
    Ok(canonical_slot.with_file_name(lease_name))
}

impl SqliteSlotOwnerLease {
    pub fn acquire(canonical_slot: &Path, component: &str) -> Result<Self> {
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

        // Existing stores are retained through a read-only, no-follow file
        // descriptor. Opening that descriptor must not itself grant write
        // authority before the component-specific pre-authentication envelope
        // has been verified. Only a genuinely absent slot is created writable;
        // it is still empty and has not been handed to SQLite at this point.
        let database_exists = match std::fs::symlink_metadata(canonical_slot) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    anyhow::bail!(
                        "{component}_database_symlink_rejected:{}",
                        canonical_slot.display()
                    );
                }
                DatabaseFileIdentity::from_metadata(&metadata, canonical_slot, component)?;
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "inspect {component} canonical SQLite database at {}",
                        canonical_slot.display()
                    )
                });
            }
        };
        let mut database_options = OpenOptions::new();
        database_options.read(true);
        if database_exists {
            // Deliberately no write flag for an existing database.
        } else {
            database_options.create_new(true).write(true);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            database_options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let database_file = database_options.open(canonical_slot).with_context(|| {
            format!(
                "open {component} canonical SQLite database identity at {}",
                canonical_slot.display()
            )
        })?;
        let database_file_identity =
            DatabaseFileIdentity::from_file(&database_file, canonical_slot, component)?;
        let database_path_identity = DatabaseFileIdentity::from_path(canonical_slot, component)?;
        if database_file_identity != database_path_identity {
            anyhow::bail!(
                "{component}_database_identity_changed_during_owner_open:{}",
                canonical_slot.display()
            );
        }
        let lock_component = format!("{component}_owner_lock");
        match std::fs::symlink_metadata(&lease_path) {
            Ok(_) => {
                DatabaseFileIdentity::from_path_no_follow(&lease_path, &lock_component)?;
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Explicit CLOEXEC is part of the lease contract. Scheduler tests
            // and the product may launch child processes while a store is
            // open; an inherited lock descriptor must never extend the
            // writable-owner lifetime past the owning TaskStore.
            lock_options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let lock_file = lock_options.open(&lease_path).with_context(|| {
            format!(
                "open {component} canonical SQLite owner lease at {}",
                lease_path.display()
            )
        })?;
        lock_file.try_lock().with_context(|| {
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
        let inner = Arc::new(SqliteSlotOwnerLeaseInner {
            canonical_slot: canonical_slot.to_path_buf(),
            component: component.to_string(),
            owner_generation_id: uuid::Uuid::new_v4(),
            lock_path: lease_path.clone(),
            lock_file,
            lock_file_identity,
            lock_file_io: Mutex::new(()),
            database_file,
            database_was_created: !database_exists,
            database_identity: Mutex::new(Some(database_file_identity)),
            poisoned: Mutex::new(None),
        });
        leases.insert(lease_path, Arc::downgrade(&inner));
        Ok(Self { inner })
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
        let identity = self
            .inner
            .database_identity
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_slot_owner_identity_mutex_poisoned:{error}"))?
            .clone()
            .ok_or_else(|| {
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
        let length = self.inner.database_file.metadata()?.len();
        self.validate_database_identity()?;
        Ok(length)
    }

    pub fn database_was_created(&self) -> bool {
        self.inner.database_was_created
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
    pub fn bind_opened_database_identity(&self) -> Result<()> {
        self.validate_database_identity()
    }

    pub fn validate_database_identity(&self) -> Result<()> {
        if let Some(reason) = self
            .inner
            .poisoned
            .lock()
            .map_err(|error| anyhow::anyhow!("sqlite_slot_owner_poison_mutex:{error}"))?
            .clone()
        {
            anyhow::bail!(
                "{}_sqlite_slot_owner_poisoned:{reason}",
                self.inner.component
            );
        }
        let validation: Result<()> = (|| {
            let expected = self
                .inner
                .database_identity
                .lock()
                .map_err(|error| {
                    anyhow::anyhow!("sqlite_slot_owner_identity_mutex_poisoned:{error}")
                })?
                .clone()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "{}_sqlite_slot_owner_lease_initializing:{}",
                        self.inner.component,
                        self.inner.canonical_slot.display()
                    )
                })?;
            let opened_database = DatabaseFileIdentity::from_file(
                &self.inner.database_file,
                &self.inner.canonical_slot,
                &self.inner.component,
            )?;
            let current_database =
                DatabaseFileIdentity::from_path(&self.inner.canonical_slot, &self.inner.component)
                    .map_err(|error| {
                        anyhow::anyhow!(
                            "{}_database_identity_unavailable:{}",
                            self.inner.component,
                            error
                        )
                    })?;
            if opened_database != expected || current_database != expected {
                anyhow::bail!(
                    "{}_database_identity_changed:{}",
                    self.inner.component,
                    self.inner.canonical_slot.display()
                );
            }

            let lock_component = format!("{}_owner_lock", self.inner.component);
            let opened_lock = DatabaseFileIdentity::from_file(
                &self.inner.lock_file,
                &self.inner.lock_path,
                &lock_component,
            )?;
            let current_lock =
                DatabaseFileIdentity::from_path_no_follow(&self.inner.lock_path, &lock_component)?;
            if opened_lock != self.inner.lock_file_identity
                || current_lock != self.inner.lock_file_identity
            {
                anyhow::bail!(
                    "{}_owner_lock_identity_changed:{}",
                    self.inner.component,
                    self.inner.lock_path.display()
                );
            }
            Ok(())
        })();
        if let Err(error) = validation {
            let reason = error.to_string();
            *self
                .inner
                .poisoned
                .lock()
                .map_err(|poison| anyhow::anyhow!("sqlite_slot_owner_poison_mutex:{poison}"))? =
                Some(reason.clone());
            anyhow::bail!(
                "{}_sqlite_slot_owner_poisoned:{reason}",
                self.inner.component
            );
        }
        Ok(())
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

fn sqlite_quick_check(
    conn: &Connection,
    path: &Path,
    component: &str,
    operation: &str,
) -> Result<String> {
    #[cfg(not(any(test, feature = "test-utils")))]
    let _ = (path, component, operation);
    let mut statement = conn.prepare("PRAGMA quick_check(1)")?;
    let integrity = statement.query_row([], |row| row.get::<_, String>(0))?;
    #[cfg(any(test, feature = "test-utils"))]
    observe_sqlite_statement_for_test(path, component, operation, &statement, Some(&integrity));
    Ok(integrity)
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

    #[cfg(any(test, feature = "test-utils"))]
    observe_sqlite_runtime_event_for_test(path, component, "sqlite_read_only_reader_open_start");

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

    let integrity = sqlite_quick_check(&conn, path, component, "sqlite_quick_check_read_only")?;
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
    let integrity = sqlite_quick_check(&conn, path, component, "sqlite_quick_check_immutable")?;
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
        first.bind_opened_database_identity().unwrap();
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
            competing_lock.try_lock().is_err(),
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
        replacement_owner.bind_opened_database_identity().unwrap();
        replacement_owner.validate_database_identity().unwrap();
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

        assert!(
            lease.inner.database_file.write_at(b"X", 0).is_err(),
            "an existing database identity fd must not carry write authority"
        );
        assert_eq!(std::fs::read(&canonical_slot).unwrap(), before);
        for fd in [
            lease.inner.database_file.as_raw_fd(),
            lease.inner.lock_file.as_raw_fd(),
        ] {
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
