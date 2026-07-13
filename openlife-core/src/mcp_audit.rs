use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use ring::digest::{Context as DigestContext, SHA256};

const MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION: i64 = 1;

fn audit_payload_receipt(kind: &str, value_type: &str, bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&SHA256, bytes);
    serde_json::json!({
        "kind": kind,
        "payloadStored": false,
        "valueType": value_type,
        "bytes": bytes.len(),
        "digest": format!(
            "sha256:{}",
            general_purpose::STANDARD_NO_PAD.encode(digest.as_ref())
        ),
    })
    .to_string()
}

fn audit_arguments_receipt(arguments: &Value) -> Result<String> {
    let encoded = serde_json::to_vec(arguments).context("serialize MCP argument receipt input")?;
    let value_type = match arguments {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    Ok(audit_payload_receipt("arguments", value_type, &encoded))
}

fn audit_result_receipt(result: &str) -> String {
    audit_payload_receipt("result", "string", result.as_bytes())
}

/// Key management mode for MCP audit encryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KeyMode {
    /// Derive key from a fixed app secret (default, backward-compatible)
    #[default]
    Derived,
    /// User-provided passphrase (more secure, user-controlled)
    Passphrase,
    /// Environment variable sourced key
    Env,
    /// Random key material held by the operating-system credential store.
    Keychain,
}

/// Key management configuration for MCP audit logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditKeyConfig {
    pub mode: KeyMode,
    /// For Passphrase mode: base64-encoded salt (16 bytes)
    pub salt_b64: Option<String>,
    /// For Env mode: environment variable name
    pub env_var: Option<String>,
    /// Opaque credential-store reference. The secret itself is never serialized here.
    #[serde(default)]
    pub key_ref: Option<String>,
    /// Key rotation epoch (monotonically increasing)
    pub epoch: u64,
    /// When this key config was created
    pub created_at: String,
}

impl Default for AuditKeyConfig {
    fn default() -> Self {
        Self {
            mode: KeyMode::Derived,
            salt_b64: None,
            env_var: None,
            key_ref: None,
            epoch: 0,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Export payload for audit logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditExport {
    pub exported_at: String,
    pub entry_count: usize,
    pub days: i64,
    pub entries: Vec<ExportedAuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedAuditEntry {
    pub id: i64,
    pub tool_name: String,
    pub arguments: String,
    pub result: String,
    pub success: bool,
    pub pii_found: bool,
    pub created_at: String,
}

/// One encrypted MCP audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLogEntry {
    pub id: i64,
    pub tool_name: String,
    pub arguments: String,
    pub result: String,
    pub success: bool,
    pub pii_found: bool,
    pub created_at: String,
}

#[derive(Clone)]
pub struct AuditKeyMaterial {
    pub config: AuditKeyConfig,
    pub key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct McpAuditFileIdentity {
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

impl McpAuditFileIdentity {
    fn from_metadata(metadata: &std::fs::Metadata, path: &Path, component: &str) -> Result<Self> {
        if !metadata.is_file() {
            anyhow::bail!("{component}_not_regular_file:{}", path.display());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let hard_link_count = metadata.nlink();
            if hard_link_count != 1 {
                anyhow::bail!(
                    "{component}_link_count_invalid:{hard_link_count}:{}",
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

    fn from_file(file: &File, path: &Path, component: &str) -> Result<Self> {
        let metadata = file
            .metadata()
            .with_context(|| format!("read {component} opened identity at {}", path.display()))?;
        Self::from_metadata(&metadata, path, component)
    }

    fn from_path_no_follow(path: &Path, component: &str) -> Result<Self> {
        let metadata = std::fs::symlink_metadata(path)
            .with_context(|| format!("read {component} path identity at {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!("{component}_symlink_rejected:{}", path.display());
        }
        Self::from_metadata(&metadata, path, component)
    }
}

#[derive(Debug)]
struct McpAuditWriterReservationInner {
    canonical_slot: PathBuf,
    lock_path: PathBuf,
    lock_file: File,
    lock_identity: McpAuditFileIdentity,
    existing_database_file: Option<File>,
    existing_database_identity: Option<McpAuditFileIdentity>,
    poisoned: Mutex<Option<String>>,
}

/// No-create, OS-backed reservation for one canonical MCP audit writer.
///
/// The reservation locks the sidecar inode and, when the database already
/// exists, retains a read-only no-follow descriptor for its exact inode. It
/// deliberately does not create or open SQLite. Bootstrap can therefore prove
/// key-reference durability before activating the database writer.
#[derive(Debug)]
pub struct McpAuditWriterReservation {
    inner: Arc<McpAuditWriterReservationInner>,
}

fn mcp_audit_writer_registry(
) -> &'static Mutex<HashMap<PathBuf, Weak<McpAuditWriterReservationInner>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Weak<McpAuditWriterReservationInner>>>> =
        OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn canonical_mcp_audit_slot(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("mcp_audit_store_database_file_name_missing"))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_parent = std::fs::canonicalize(parent).with_context(|| {
        format!(
            "canonicalize MCP audit database parent before owner reservation: {}",
            parent.display()
        )
    })?;
    Ok(canonical_parent.join(file_name))
}

fn mcp_audit_owner_lock_path(canonical_slot: &Path) -> Result<PathBuf> {
    let file_name = canonical_slot
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("mcp_audit_store_owner_lock_file_name_missing"))?;
    let mut lock_name = file_name.to_os_string();
    lock_name.push(".openlife-owner.lock");
    Ok(canonical_slot.with_file_name(lock_name))
}

impl McpAuditWriterReservation {
    fn reserve(db_path: &Path) -> Result<Self> {
        let canonical_slot = canonical_mcp_audit_slot(db_path)?;
        let lock_path = mcp_audit_owner_lock_path(&canonical_slot)?;
        let mut registry = mcp_audit_writer_registry()
            .lock()
            .map_err(|error| anyhow::anyhow!("mcp_audit_store_owner_registry_poisoned:{error}"))?;
        if registry.get(&lock_path).and_then(Weak::upgrade).is_some() {
            anyhow::bail!(
                "mcp_audit_store_sqlite_slot_owner_lease_unavailable:{}",
                canonical_slot.display()
            );
        }
        registry.remove(&lock_path);

        let lock_component = "mcp_audit_store_owner_lock";
        match std::fs::symlink_metadata(&lock_path) {
            Ok(_) => {
                McpAuditFileIdentity::from_path_no_follow(&lock_path, lock_component)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("inspect MCP audit owner lock"),
        }
        let mut lock_options = OpenOptions::new();
        lock_options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            lock_options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let lock_file = lock_options
            .open(&lock_path)
            .with_context(|| format!("open MCP audit owner lock at {}", lock_path.display()))?;
        lock_file.try_lock().with_context(|| {
            format!(
                "mcp_audit_store_sqlite_slot_owner_lease_unavailable:{}",
                canonical_slot.display()
            )
        })?;
        let lock_identity =
            McpAuditFileIdentity::from_file(&lock_file, &lock_path, lock_component)?;
        if McpAuditFileIdentity::from_path_no_follow(&lock_path, lock_component)? != lock_identity {
            anyhow::bail!(
                "mcp_audit_store_owner_lock_identity_changed_during_open:{}",
                lock_path.display()
            );
        }

        let (existing_database_file, existing_database_identity) =
            match std::fs::symlink_metadata(&canonical_slot) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        anyhow::bail!(
                            "mcp_audit_store_database_symlink_rejected:{}",
                            canonical_slot.display()
                        );
                    }
                    McpAuditFileIdentity::from_metadata(
                        &metadata,
                        &canonical_slot,
                        "mcp_audit_store_database",
                    )?;
                    let mut database_options = OpenOptions::new();
                    database_options.read(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        database_options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
                    }
                    let database_file =
                        database_options.open(&canonical_slot).with_context(|| {
                            format!(
                                "retain existing MCP audit database identity at {}",
                                canonical_slot.display()
                            )
                        })?;
                    let identity = McpAuditFileIdentity::from_file(
                        &database_file,
                        &canonical_slot,
                        "mcp_audit_store_database",
                    )?;
                    if McpAuditFileIdentity::from_path_no_follow(
                        &canonical_slot,
                        "mcp_audit_store_database",
                    )? != identity
                    {
                        anyhow::bail!(
                            "mcp_audit_store_database_identity_changed_during_reservation:{}",
                            canonical_slot.display()
                        );
                    }
                    (Some(database_file), Some(identity))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (None, None),
                Err(error) => return Err(error).context("inspect MCP audit database slot"),
            };

        let inner = Arc::new(McpAuditWriterReservationInner {
            canonical_slot,
            lock_path: lock_path.clone(),
            lock_file,
            lock_identity,
            existing_database_file,
            existing_database_identity,
            poisoned: Mutex::new(None),
        });
        registry.insert(lock_path, Arc::downgrade(&inner));
        Ok(Self { inner })
    }

    pub fn existing_database_len(&self) -> Result<Option<u64>> {
        self.validate_reserved_identities()?;
        self.inner
            .existing_database_file
            .as_ref()
            .map(|file| {
                file.metadata()
                    .map(|metadata| metadata.len())
                    .map_err(anyhow::Error::from)
            })
            .transpose()
    }

    fn current_poison(&self) -> Result<Option<String>> {
        self.inner
            .poisoned
            .lock()
            .map_err(|error| anyhow::anyhow!("mcp_audit_store_owner_poison_mutex:{error}"))
            .map(|reason| reason.clone())
    }

    fn poison(&self, reason: String) -> Result<()> {
        *self
            .inner
            .poisoned
            .lock()
            .map_err(|error| anyhow::anyhow!("mcp_audit_store_owner_poison_mutex:{error}"))? =
            Some(reason);
        Ok(())
    }

    fn validate_owner_lock_identity(&self) -> Result<()> {
        if let Some(reason) = self.current_poison()? {
            anyhow::bail!("mcp_audit_store_sqlite_slot_owner_poisoned:{reason}");
        }
        let validation = (|| {
            let opened_lock = McpAuditFileIdentity::from_file(
                &self.inner.lock_file,
                &self.inner.lock_path,
                "mcp_audit_store_owner_lock",
            )?;
            let current_lock = McpAuditFileIdentity::from_path_no_follow(
                &self.inner.lock_path,
                "mcp_audit_store_owner_lock",
            )?;
            if opened_lock != self.inner.lock_identity || current_lock != self.inner.lock_identity {
                anyhow::bail!(
                    "mcp_audit_store_owner_lock_identity_changed:{}",
                    self.inner.lock_path.display()
                );
            }
            Ok(())
        })();
        if let Err(error) = validation {
            let reason = error.to_string();
            self.poison(reason.clone())?;
            anyhow::bail!("mcp_audit_store_sqlite_slot_owner_poisoned:{reason}");
        }
        Ok(())
    }

    fn validate_reserved_identities(&self) -> Result<()> {
        self.validate_owner_lock_identity()?;
        let validation = (|| {
            match (
                self.inner.existing_database_file.as_ref(),
                self.inner.existing_database_identity.as_ref(),
            ) {
                (Some(file), Some(expected)) => {
                    let opened = McpAuditFileIdentity::from_file(
                        file,
                        &self.inner.canonical_slot,
                        "mcp_audit_store_database",
                    )?;
                    let current = McpAuditFileIdentity::from_path_no_follow(
                        &self.inner.canonical_slot,
                        "mcp_audit_store_database",
                    )?;
                    if &opened != expected || &current != expected {
                        anyhow::bail!(
                            "mcp_audit_store_database_identity_changed:{}",
                            self.inner.canonical_slot.display()
                        );
                    }
                }
                (None, None) => match std::fs::symlink_metadata(&self.inner.canonical_slot) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Ok(_) => anyhow::bail!(
                        "mcp_audit_store_database_appeared_after_reservation:{}",
                        self.inner.canonical_slot.display()
                    ),
                    Err(error) => return Err(error.into()),
                },
                _ => anyhow::bail!("mcp_audit_store_reservation_identity_incomplete"),
            }
            Ok(())
        })();
        if let Err(error) = validation {
            let reason = error.to_string();
            self.poison(reason.clone())?;
            anyhow::bail!("mcp_audit_store_sqlite_slot_owner_poisoned:{reason}");
        }
        Ok(())
    }
}

#[derive(Debug)]
struct McpAuditWritableBinding {
    reservation: McpAuditWriterReservation,
    database_file: File,
    database_identity: McpAuditFileIdentity,
}

impl McpAuditWritableBinding {
    fn activate(reservation: McpAuditWriterReservation) -> Result<(Self, Connection)> {
        reservation.validate_reserved_identities()?;
        let canonical_slot = &reservation.inner.canonical_slot;
        let (database_file, database_identity) = if let (Some(existing), Some(identity)) = (
            reservation.inner.existing_database_file.as_ref(),
            reservation.inner.existing_database_identity.as_ref(),
        ) {
            (existing.try_clone()?, identity.clone())
        } else {
            let mut database_options = OpenOptions::new();
            database_options.create_new(true).read(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                database_options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
            }
            let file = database_options.open(canonical_slot).with_context(|| {
                format!(
                    "create MCP audit database only after key-reference durability: {}",
                    canonical_slot.display()
                )
            })?;
            let identity =
                McpAuditFileIdentity::from_file(&file, canonical_slot, "mcp_audit_store_database")?;
            (file, identity)
        };
        if McpAuditFileIdentity::from_path_no_follow(canonical_slot, "mcp_audit_store_database")?
            != database_identity
        {
            anyhow::bail!(
                "mcp_audit_store_database_identity_changed_before_sqlite_open:{}",
                canonical_slot.display()
            );
        }
        let conn = Connection::open_with_flags(
            canonical_slot,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .with_context(|| {
            format!(
                "open owned MCP audit database at {}",
                canonical_slot.display()
            )
        })?;
        let observed =
            crate::sqlite_migration::canonical_opened_main_database_path(&conn, "mcp_audit_store")?
                .ok_or_else(|| {
                    anyhow::anyhow!("mcp_audit_store_persistent_database_path_missing")
                })?;
        if observed != *canonical_slot {
            anyhow::bail!(
                "mcp_audit_store_database_slot_changed_during_open:{}!={}",
                canonical_slot.display(),
                observed.display()
            );
        }
        let binding = Self {
            reservation,
            database_file,
            database_identity,
        };
        binding.validate()?;
        Ok((binding, conn))
    }

    fn validate(&self) -> Result<()> {
        self.reservation.validate_owner_lock_identity()?;
        let canonical_slot = &self.reservation.inner.canonical_slot;
        let opened = McpAuditFileIdentity::from_file(
            &self.database_file,
            canonical_slot,
            "mcp_audit_store_database",
        )?;
        let current =
            McpAuditFileIdentity::from_path_no_follow(canonical_slot, "mcp_audit_store_database")?;
        if opened != self.database_identity || current != self.database_identity {
            anyhow::bail!(
                "mcp_audit_store_database_identity_changed:{}",
                canonical_slot.display()
            );
        }
        Ok(())
    }
}

struct McpAuditConnectionOwner {
    conn: Mutex<Connection>,
    canonical_slot: PathBuf,
    writable: Option<McpAuditWritableBinding>,
    read_only_identity: Option<crate::sqlite_migration::SqliteDatabaseIdentityGuard>,
}

impl McpAuditConnectionOwner {
    fn writable(reservation: McpAuditWriterReservation) -> Result<Self> {
        let (binding, conn) = McpAuditWritableBinding::activate(reservation)?;
        let canonical_slot = binding.reservation.inner.canonical_slot.clone();
        Ok(Self {
            conn: Mutex::new(conn),
            canonical_slot,
            writable: Some(binding),
            read_only_identity: None,
        })
    }

    fn read_only(db_path: &Path) -> Result<(PathBuf, Self)> {
        let canonical_slot = canonical_mcp_audit_slot(db_path)?;
        let conn = crate::sqlite_migration::open_existing_read_only(
            &canonical_slot,
            "mcp_audit_store",
            &["mcp_log"],
        )?;
        let observed =
            crate::sqlite_migration::canonical_opened_main_database_path(&conn, "mcp_audit_store")?
                .ok_or_else(|| {
                    anyhow::anyhow!("mcp_audit_store_persistent_database_path_missing")
                })?;
        if observed != canonical_slot {
            anyhow::bail!(
                "mcp_audit_store_read_only_database_slot_changed:{}!={}",
                canonical_slot.display(),
                observed.display()
            );
        }
        let identity = crate::sqlite_migration::SqliteDatabaseIdentityGuard::capture(
            &canonical_slot,
            "mcp_audit_store",
        )?;
        identity.validate()?;
        Ok((
            canonical_slot,
            Self {
                conn: Mutex::new(conn),
                canonical_slot: observed,
                writable: None,
                read_only_identity: Some(identity),
            },
        ))
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        if let Some(binding) = self.writable.as_ref() {
            binding.validate()?;
        }
        if let Some(identity) = self.read_only_identity.as_ref() {
            identity.validate()?;
        }
        let guard = self.conn.lock().map_err(|error| {
            anyhow::anyhow!("mcp_audit_store_connection_mutex_poisoned:{error}")
        })?;
        let observed = crate::sqlite_migration::canonical_opened_main_database_path(
            &guard,
            "mcp_audit_store",
        )?
        .ok_or_else(|| anyhow::anyhow!("mcp_audit_store_persistent_database_path_missing"))?;
        if observed != self.canonical_slot {
            anyhow::bail!(
                "mcp_audit_store_connection_identity_changed:{}!={}",
                self.canonical_slot.display(),
                observed.display()
            );
        }
        if let Some(binding) = self.writable.as_ref() {
            binding.validate()?;
        }
        if let Some(identity) = self.read_only_identity.as_ref() {
            identity.validate()?;
        }
        Ok(guard)
    }
}

/// Encrypted SQLite-backed store for MCP call logs with configurable key management.
#[derive(Clone)]
pub struct McpAuditStore {
    db_path: PathBuf,
    connection: Option<Arc<McpAuditConnectionOwner>>,
    read_only: bool,
    unavailable_reason: Option<String>,
    key: [u8; 32],
    key_config: AuditKeyConfig,
    keyring: HashMap<u64, [u8; 32]>,
    key_configs: Vec<AuditKeyConfig>,
}

impl McpAuditStore {
    /// Test/fixture-only constructor for the historical deterministic key.
    /// Product code must hydrate a random keychain epoch and call
    /// `with_key_materials`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let config = AuditKeyConfig::default();
        Self::with_config(db_path, config)
    }

    #[cfg(not(any(test, feature = "test-utils")))]
    pub(crate) fn new(db_path: impl Into<PathBuf>) -> Self {
        let config = AuditKeyConfig::default();
        Self::with_config(db_path, config)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_config(db_path: impl Into<PathBuf>, config: AuditKeyConfig) -> Self {
        Self::with_keyring(db_path, vec![config])
    }

    #[cfg(not(any(test, feature = "test-utils")))]
    pub(crate) fn with_config(db_path: impl Into<PathBuf>, config: AuditKeyConfig) -> Self {
        Self::with_keyring(db_path, vec![config])
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_keyring(db_path: impl Into<PathBuf>, configs: Vec<AuditKeyConfig>) -> Self {
        Self::with_legacy_keyring_unchecked(db_path, configs)
    }

    #[cfg(not(any(test, feature = "test-utils")))]
    pub(crate) fn with_keyring(db_path: impl Into<PathBuf>, configs: Vec<AuditKeyConfig>) -> Self {
        Self::with_legacy_keyring_unchecked(db_path, configs)
    }

    fn with_legacy_keyring_unchecked(
        db_path: impl Into<PathBuf>,
        configs: Vec<AuditKeyConfig>,
    ) -> Self {
        let path = db_path.into();
        let mut configs = if configs.is_empty() {
            vec![AuditKeyConfig::default()]
        } else {
            configs
        };
        configs.sort_by_key(|config| config.epoch);
        configs.dedup_by_key(|config| config.epoch);
        let config = configs.last().cloned().unwrap_or_default();
        let key = Self::derive_key(&config);
        let keyring: HashMap<u64, [u8; 32]> = configs
            .iter()
            .map(|config| (config.epoch, Self::derive_key(config)))
            .collect();
        let reservation = Self::reserve_writable_owner(&path);
        match reservation.and_then(|reservation| {
            Self::build_writable(
                &path,
                key,
                config.clone(),
                keyring.clone(),
                configs.clone(),
                reservation,
            )
        }) {
            Ok(store) => store,
            Err(writable_error) => Self::build_read_only(&path, key, config, keyring, configs)
                .unwrap_or_else(|read_only_error| {
                    Self::unavailable_sentinel(format!(
                        "legacy fixture writable={writable_error}; read_only={read_only_error}"
                    ))
                }),
        }
    }

    pub fn with_key_materials(
        db_path: impl Into<PathBuf>,
        materials: Vec<AuditKeyMaterial>,
    ) -> Result<Self> {
        let db_path = db_path.into();
        let reservation = Self::reserve_writable_owner(&db_path)?;
        Self::with_key_materials_and_reservation(db_path, materials, reservation)
    }

    /// Reserve the canonical writer slot without creating or opening SQLite.
    /// Bootstrap must call this before creating key material or persisting key
    /// references.
    pub fn reserve_writable_owner(db_path: impl AsRef<Path>) -> Result<McpAuditWriterReservation> {
        McpAuditWriterReservation::reserve(db_path.as_ref())
    }

    /// Activate the database writer from an already-held no-create reservation.
    /// This is the only product constructor that can preserve the ordering
    /// `owner reservation -> key/reference durability -> SQLite writer`.
    pub fn with_key_materials_and_reservation(
        db_path: impl Into<PathBuf>,
        mut materials: Vec<AuditKeyMaterial>,
        reservation: McpAuditWriterReservation,
    ) -> Result<Self> {
        if materials.is_empty() {
            anyhow::bail!("MCP audit key material is empty");
        }
        materials.sort_by_key(|material| material.config.epoch);
        for pair in materials.windows(2) {
            if pair[0].config.epoch == pair[1].config.epoch {
                anyhow::bail!("duplicate MCP audit key epoch");
            }
        }
        let active = materials.last().cloned().expect("non-empty key materials");
        if active.config.mode != KeyMode::Keychain || active.config.key_ref.is_none() {
            anyhow::bail!(
                "active MCP audit key must be random keychain material; legacy modes are read-only migration keys"
            );
        }
        let keyring = materials
            .iter()
            .map(|material| (material.config.epoch, material.key))
            .collect::<HashMap<_, _>>();
        let key_configs = materials
            .iter()
            .map(|material| material.config.clone())
            .collect::<Vec<_>>();
        Self::build_writable(
            &db_path.into(),
            active.key,
            active.config,
            keyring,
            key_configs,
            reservation,
        )
    }

    fn build_writable(
        db_path: &Path,
        key: [u8; 32],
        key_config: AuditKeyConfig,
        keyring: HashMap<u64, [u8; 32]>,
        key_configs: Vec<AuditKeyConfig>,
        reservation: McpAuditWriterReservation,
    ) -> Result<Self> {
        let expected_slot = canonical_mcp_audit_slot(db_path)?;
        if reservation.inner.canonical_slot != expected_slot {
            anyhow::bail!(
                "mcp_audit_store_owner_reservation_path_mismatch:{}!={}",
                reservation.inner.canonical_slot.display(),
                expected_slot.display()
            );
        }
        let connection = Arc::new(McpAuditConnectionOwner::writable(reservation)?);
        let store = Self {
            db_path: expected_slot,
            connection: Some(connection),
            read_only: false,
            unavailable_reason: None,
            key,
            key_config,
            keyring,
            key_configs,
        };
        store.init_tables()?;
        Ok(store)
    }

    /// Seed a historical keychain-encrypted row without granting the legacy
    /// reference current writable-owner authority. D064 uses this test-only
    /// fixture to construct a real pre-migration SQLite artifact, then opens
    /// that artifact through the product constructor with a new active key.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn write_historical_keychain_fixture(
        db_path: impl Into<PathBuf>,
        material: AuditKeyMaterial,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> Result<()> {
        if material.config.mode != KeyMode::Keychain || material.config.key_ref.is_none() {
            anyhow::bail!("historical MCP audit fixture requires keychain material");
        }
        let store = Self::with_key_materials(db_path, vec![material])?;
        store.insert_log(tool_name, arguments, result, success, pii_found)?;
        Ok(())
    }

    pub fn open_read_only_existing_with_key_materials(
        db_path: impl Into<PathBuf>,
        mut materials: Vec<AuditKeyMaterial>,
    ) -> Result<Self> {
        if materials.is_empty() {
            anyhow::bail!("MCP audit key material is empty");
        }
        materials.sort_by_key(|material| material.config.epoch);
        for pair in materials.windows(2) {
            if pair[0].config.epoch == pair[1].config.epoch {
                anyhow::bail!("duplicate MCP audit key epoch");
            }
        }
        let active = materials.last().cloned().expect("non-empty key materials");
        if active.config.mode != KeyMode::Keychain || active.config.key_ref.is_none() {
            anyhow::bail!("active MCP audit key must be random keychain material");
        }
        let keyring = materials
            .iter()
            .map(|material| (material.config.epoch, material.key))
            .collect();
        let key_configs = materials
            .into_iter()
            .map(|material| material.config)
            .collect();
        Self::build_read_only(
            &db_path.into(),
            active.key,
            active.config,
            keyring,
            key_configs,
        )
    }

    fn build_read_only(
        db_path: &Path,
        key: [u8; 32],
        key_config: AuditKeyConfig,
        keyring: HashMap<u64, [u8; 32]>,
        key_configs: Vec<AuditKeyConfig>,
    ) -> Result<Self> {
        let (canonical_slot, connection) = McpAuditConnectionOwner::read_only(db_path)?;
        Ok(Self {
            db_path: canonical_slot,
            connection: Some(Arc::new(connection)),
            read_only: true,
            unavailable_reason: None,
            key,
            key_config,
            keyring,
            key_configs,
        })
    }

    pub fn unavailable_sentinel(reason: impl Into<String>) -> Self {
        Self {
            db_path: PathBuf::new(),
            connection: None,
            read_only: true,
            unavailable_reason: Some(reason.into()),
            key: [0; 32],
            key_config: AuditKeyConfig::default(),
            keyring: HashMap::new(),
            key_configs: Vec::new(),
        }
    }

    /// Hydrate a historical deterministic key solely so existing audit rows
    /// can be read and re-encrypted during migration. `with_key_materials`
    /// rejects this material as the active write epoch.
    pub fn legacy_read_only_key_material(config: AuditKeyConfig) -> Result<AuditKeyMaterial> {
        if config.mode == KeyMode::Keychain {
            anyhow::bail!("keychain audit config requires externally hydrated key material");
        }
        Ok(AuditKeyMaterial {
            key: Self::derive_key(&config),
            config,
        })
    }

    /// Derive the 32-byte AES key from the current key configuration.
    fn derive_key(config: &AuditKeyConfig) -> [u8; 32] {
        match config.mode {
            KeyMode::Derived => {
                let mut context = DigestContext::new(&SHA256);
                context.update(b"openlife_mcp_log_secret_v1");
                let digest = context.finish();
                let mut key_arr = [0u8; 32];
                key_arr.copy_from_slice(digest.as_ref());
                key_arr
            }
            KeyMode::Passphrase => {
                let salt = config
                    .salt_b64
                    .as_ref()
                    .and_then(|s| general_purpose::STANDARD.decode(s).ok())
                    .unwrap_or_else(|| b"openlife_default_salt".to_vec());
                let mut context = DigestContext::new(&SHA256);
                context.update(&salt);
                context.update(b"openlife_mcp_log_passphrase_v1");
                let digest = context.finish();
                let mut key_arr = [0u8; 32];
                key_arr.copy_from_slice(digest.as_ref());
                key_arr
            }
            KeyMode::Env => {
                let env_key = config
                    .env_var
                    .as_ref()
                    .and_then(|var| std::env::var(var).ok())
                    .unwrap_or_default();
                let mut context = DigestContext::new(&SHA256);
                context.update(env_key.as_bytes());
                context.update(b"openlife_mcp_log_env_v1");
                let digest = context.finish();
                let mut key_arr = [0u8; 32];
                key_arr.copy_from_slice(digest.as_ref());
                key_arr
            }
            KeyMode::Keychain => {
                panic!("keychain audit keys must be supplied as hydrated key material")
            }
        }
    }

    /// Rotate to a new key configuration for future writes.
    ///
    /// Existing entries remain readable while this store is initialized with
    /// the full keyring returned by `key_configs`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn rotate_key(&mut self, new_config: AuditKeyConfig) {
        assert_ne!(
            new_config.mode,
            KeyMode::Keychain,
            "keychain rotation requires rotate_key_material"
        );
        let new_key = Self::derive_key(&new_config);
        self.key = new_key;
        self.key_config = new_config;
        self.keyring.insert(self.key_config.epoch, new_key);
        self.key_configs
            .retain(|config| config.epoch != self.key_config.epoch);
        self.key_configs.push(self.key_config.clone());
        self.key_configs.sort_by_key(|config| config.epoch);
    }

    pub fn rotate_key_material(&mut self, material: AuditKeyMaterial) -> Result<()> {
        if self.read_only || self.unavailable_reason.is_some() {
            anyhow::bail!("MCP audit key rotation requires the canonical writable owner");
        }
        if material.config.epoch <= self.key_config.epoch {
            anyhow::bail!("MCP audit key epoch must increase monotonically");
        }
        if material.config.mode != KeyMode::Keychain || material.config.key_ref.is_none() {
            anyhow::bail!("MCP audit rotation requires a keychain reference");
        }
        self.key = material.key;
        self.key_config = material.config.clone();
        self.keyring.insert(material.config.epoch, material.key);
        self.key_configs.push(material.config);
        self.key_configs.sort_by_key(|config| config.epoch);
        Ok(())
    }

    pub fn key_config(&self) -> &AuditKeyConfig {
        &self.key_config
    }

    pub fn key_configs(&self) -> &[AuditKeyConfig] {
        &self.key_configs
    }

    pub fn database_path(&self) -> &Path {
        &self.db_path
    }

    /// Export decrypted audit logs for the given time range.
    pub fn export_logs(&self, days: i64) -> Result<AuditExport> {
        let entries = self.list_logs(10000)?;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let filtered: Vec<_> = entries
            .into_iter()
            .filter(|e| {
                chrono::DateTime::parse_from_rfc3339(&e.created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc) >= cutoff)
                    .unwrap_or(true)
            })
            .map(|e| ExportedAuditEntry {
                id: e.id,
                tool_name: e.tool_name,
                arguments: e.arguments,
                result: e.result,
                success: e.success,
                pii_found: e.pii_found,
                created_at: e.created_at,
            })
            .collect();
        Ok(AuditExport {
            exported_at: chrono::Utc::now().to_rfc3339(),
            entry_count: filtered.len(),
            days,
            entries: filtered,
        })
    }

    /// Cleanup strategy: remove logs older than retention_days and return count removed.
    pub fn cleanup(&self, retention_days: i64) -> Result<usize> {
        self.clear_old_logs(retention_days)
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        if let Some(reason) = &self.unavailable_reason {
            anyhow::bail!("mcp_audit_store_unavailable:{reason}");
        }
        self.connection
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_store_connection_unavailable"))?
            .lock()
    }

    fn init_tables(&self) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS mcp_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_name TEXT NOT NULL,
                arguments_encrypted TEXT NOT NULL,
                result_encrypted TEXT NOT NULL,
                success INTEGER NOT NULL,
                pii_found INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                key_epoch INTEGER NOT NULL DEFAULT 0,
                payload_minimized_version INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "mcp_log",
            "key_epoch",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "mcp_log",
            "payload_minimized_version",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        crate::sqlite_migration::record_schema_version(&tx, "mcp_audit_store", 3)?;
        tx.commit()?;
        drop(conn);
        self.migrate_legacy_payloads()?;
        Ok(())
    }

    fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key).map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let nonce_bytes = rand::random::<[u8; 12]>();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("encrypt failed: {:?}", e))?;
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(general_purpose::STANDARD.encode(&combined))
    }

    fn decrypt(&self, combined_b64: &str) -> Result<String> {
        self.decrypt_with_key(combined_b64, &self.key)
    }

    fn decrypt_for_epoch(&self, combined_b64: &str, key_epoch: u64) -> Result<String> {
        if let Some(key) = self.keyring.get(&key_epoch) {
            return self.decrypt_with_key(combined_b64, key);
        }
        self.decrypt(combined_b64)
    }

    fn decrypt_with_key(&self, combined_b64: &str, key: &[u8; 32]) -> Result<String> {
        let combined = general_purpose::STANDARD
            .decode(combined_b64)
            .context("invalid base64")?;
        if combined.len() < 12 {
            return Err(anyhow::anyhow!("ciphertext too short"));
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("decrypt failed: {:?}", e))?;
        String::from_utf8(plaintext).context("utf8 decode")
    }

    fn migrate_legacy_payloads(&self) -> Result<()> {
        let mut conn = self.conn()?;
        let rows = {
            let mut statement = conn.prepare(
                "SELECT id, arguments_encrypted, result_encrypted, key_epoch
                 FROM mcp_log
                 WHERE payload_minimized_version < ?1",
            )?;
            let rows = statement
                .query_map([MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?.max(0) as u64,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        if rows.is_empty() {
            return Ok(());
        }

        let mut migrated = Vec::with_capacity(rows.len());
        for (id, arguments_encrypted, result_encrypted, key_epoch) in rows {
            let arguments_plaintext = self
                .decrypt_for_epoch(&arguments_encrypted, key_epoch)
                .unwrap_or(arguments_encrypted);
            let arguments_receipt = serde_json::from_str::<Value>(&arguments_plaintext)
                .ok()
                .map(|value| audit_arguments_receipt(&value))
                .transpose()?
                .unwrap_or_else(|| {
                    audit_payload_receipt(
                        "arguments",
                        "unparseable_legacy",
                        arguments_plaintext.as_bytes(),
                    )
                });
            let result_plaintext = self
                .decrypt_for_epoch(&result_encrypted, key_epoch)
                .unwrap_or(result_encrypted);
            migrated.push((
                id,
                self.encrypt(&arguments_receipt)?,
                self.encrypt(&audit_result_receipt(&result_plaintext))?,
            ));
        }

        let transaction = conn.transaction()?;
        for (id, arguments_encrypted, result_encrypted) in migrated {
            transaction.execute(
                "UPDATE mcp_log
                 SET arguments_encrypted = ?1,
                     result_encrypted = ?2,
                     key_epoch = ?3,
                     payload_minimized_version = ?4
                 WHERE id = ?5",
                params![
                    arguments_encrypted,
                    result_encrypted,
                    self.key_config.epoch as i64,
                    MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                    id,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_log(
        &self,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> Result<i64> {
        let conn = self.conn()?;
        let args_enc = self.encrypt(&audit_arguments_receipt(arguments)?)?;
        let res_enc = self.encrypt(&audit_result_receipt(result))?;
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO mcp_log (
                tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                created_at, key_epoch, payload_minimized_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                tool_name,
                args_enc,
                res_enc,
                success as i32,
                pii_found as i32,
                created_at,
                self.key_config.epoch as i64,
                MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_logs(&self, limit: usize) -> Result<Vec<McpLogEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tool_name, arguments_encrypted, result_encrypted, success, pii_found, created_at, key_epoch
             FROM mcp_log
             ORDER BY id DESC
             LIMIT ?1"
        )?;
        let rows = stmt.query_map([limit], |row| {
            let id: i64 = row.get(0)?;
            let tool_name: String = row.get(1)?;
            let args_enc: String = row.get(2)?;
            let res_enc: String = row.get(3)?;
            let success: i32 = row.get(4)?;
            let pii_found: i32 = row.get(5)?;
            let created_at: String = row.get(6)?;
            let key_epoch: i64 = row.get(7)?;
            Ok((
                id,
                tool_name,
                args_enc,
                res_enc,
                success != 0,
                pii_found != 0,
                created_at,
                key_epoch as u64,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (id, tool_name, args_enc, res_enc, success, pii_found, created_at, key_epoch) = r?;
            let arguments = self
                .decrypt_for_epoch(&args_enc, key_epoch)
                .unwrap_or_else(|_| "[decrypt failed]".into());
            let result = self
                .decrypt_for_epoch(&res_enc, key_epoch)
                .unwrap_or_else(|_| "[decrypt failed]".into());
            out.push(McpLogEntry {
                id,
                tool_name,
                arguments,
                result,
                success,
                pii_found,
                created_at,
            });
        }
        Ok(out)
    }

    pub fn clear_old_logs(&self, days: i64) -> Result<usize> {
        let conn = self.conn()?;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let rows = conn.execute(
            "DELETE FROM mcp_log WHERE created_at < ?1",
            [cutoff.to_rfc3339()],
        )?;
        Ok(rows)
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Default for McpAuditStore {
    fn default() -> Self {
        let data_dir = openlife_default_data_dir();
        Self::new(data_dir.join("mcp_audit.db"))
    }
}

#[cfg(any(test, feature = "test-utils"))]
fn openlife_default_data_dir() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("OPENLIFE_DATA_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return std::path::PathBuf::from(trimmed);
        }
    }
    let profile = std::env::var("OPENLIFE_PROFILE")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "release".to_string());
    let app_dir_name = match profile.as_str() {
        "dev" => "ai.openlife.app.dev",
        "qa" => "ai.openlife.app.qa",
        _ => "ai.openlife.app",
    };
    dirs::data_dir()
        .map(|d| d.join(app_dir_name))
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap()
                .join(format!(".{}", app_dir_name))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keychain_material(epoch: u64, key: [u8; 32]) -> AuditKeyMaterial {
        AuditKeyMaterial {
            config: AuditKeyConfig {
                mode: KeyMode::Keychain,
                salt_b64: None,
                env_var: None,
                key_ref: Some(format!("test:mcp-audit-store:epoch:{epoch}")),
                epoch,
                created_at: "2026-07-13T00:00:00Z".into(),
            },
            key,
        }
    }

    #[test]
    fn legacy_key_material_cannot_become_the_active_product_write_epoch() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = McpAuditStore::legacy_read_only_key_material(AuditKeyConfig::default())
            .expect("legacy migration material");

        let error =
            match McpAuditStore::with_key_materials(dir.path().join("audit.db"), vec![legacy]) {
                Ok(_) => panic!("legacy key modes must never authorize new product writes"),
                Err(error) => error,
            };

        assert!(error
            .to_string()
            .contains("legacy modes are read-only migration keys"));
    }

    #[test]
    fn writable_owner_clone_retains_the_same_os_lease_until_final_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let material = keychain_material(91, [0x91; 32]);
        let first = McpAuditStore::with_key_materials(&path, vec![material.clone()]).unwrap();
        let retained_clone = first.clone();
        drop(first);

        let error = McpAuditStore::with_key_materials(&path, vec![material.clone()])
            .err()
            .expect("a retained clone must keep the canonical writer lease");
        assert!(error
            .to_string()
            .contains("mcp_audit_store_sqlite_slot_owner_lease_unavailable"));

        drop(retained_clone);
        let replacement = McpAuditStore::with_key_materials(&path, vec![material]).unwrap();
        replacement
            .insert_log("replacement", &serde_json::json!({}), "ok", true, false)
            .unwrap();
    }

    #[test]
    fn writable_owner_rejects_same_path_database_inode_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let displaced = dir.path().join("audit-displaced.db");
        let store =
            McpAuditStore::with_key_materials(&path, vec![keychain_material(92, [0x92; 32])])
                .unwrap();
        store
            .insert_log("original", &serde_json::json!({}), "ok", true, false)
            .unwrap();

        std::fs::rename(&path, &displaced).unwrap();
        Connection::open(&path)
            .unwrap()
            .execute("CREATE TABLE replacement(id INTEGER PRIMARY KEY)", [])
            .unwrap();

        let error = store
            .list_logs(10)
            .expect_err("the owner must not retarget to a replacement inode");
        assert!(error
            .to_string()
            .contains("mcp_audit_store_database_identity_changed"));
    }

    #[test]
    fn audit_store_key_rotation_keeps_old_logs_readable_with_keyring() {
        const PRIVATE_ARGUMENT: &str = "THERAPY-CASE-74291-ORCHID";
        const PRIVATE_RESULT: &str = "ACCOUNT-NOTE-55318-CEDAR";
        let dir = tempfile::tempdir().unwrap();
        let mut store = McpAuditStore::new(dir.path().join("audit.db"));
        store
            .insert_log(
                "test_tool",
                &serde_json::json!({"x": PRIVATE_ARGUMENT}),
                PRIVATE_RESULT,
                true,
                true,
            )
            .unwrap();

        let old_config = store.key_config().clone();
        let mut new_config = AuditKeyConfig::default();
        new_config.mode = KeyMode::Passphrase;
        new_config.salt_b64 = Some(general_purpose::STANDARD.encode(b"newsalt123456789"));
        new_config.epoch = old_config.epoch + 1;
        store.rotate_key(new_config.clone());

        // New writes use new key
        store
            .insert_log(
                "test_tool2",
                &serde_json::json!({"y": 2}),
                "done",
                true,
                false,
            )
            .unwrap();

        // All logs still readable
        let logs = store.list_logs(10).unwrap();
        assert_eq!(logs.len(), 2);
        let serialized = serde_json::to_string(&logs).unwrap();
        assert!(!serialized.contains(PRIVATE_ARGUMENT));
        assert!(!serialized.contains(PRIVATE_RESULT));
        assert!(serialized.contains("payloadStored"));
        assert!(serialized.contains("sha256:"));

        let restarted =
            McpAuditStore::with_keyring(dir.path().join("audit.db"), store.key_configs().to_vec());
        let restarted_logs = restarted.list_logs(10).unwrap();
        let restarted_serialized = serde_json::to_string(&restarted_logs).unwrap();
        assert!(!restarted_serialized.contains(PRIVATE_ARGUMENT));
        assert!(!restarted_serialized.contains(PRIVATE_RESULT));
        assert!(restarted_serialized.contains("payloadStored"));
    }

    #[test]
    fn audit_store_export_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let store = McpAuditStore::new(dir.path().join("audit.db"));
        store
            .insert_log("tool_a", &serde_json::json!({}), "result", true, false)
            .unwrap();

        let export = store.export_logs(30).unwrap();
        assert_eq!(export.entry_count, 1);
        assert_eq!(export.entries[0].tool_name, "tool_a");

        let cleaned = store.cleanup(0).unwrap();
        assert_eq!(cleaned, 1);
        let logs = store.list_logs(10).unwrap();
        assert!(logs.is_empty());
    }

    #[test]
    fn legacy_reversible_payloads_are_migrated_to_receipts_on_restart() {
        const LEGACY_ARGUMENT: &str = "MEDICAL-NOTE-31057-MAPLE";
        const LEGACY_RESULT: &str = "FINANCE-NOTE-88241-ASH";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        let arguments_encrypted = store
            .encrypt(&serde_json::json!({ "note": LEGACY_ARGUMENT }).to_string())
            .unwrap();
        let result_encrypted = store.encrypt(LEGACY_RESULT).unwrap();
        let configs = store.key_configs().to_vec();
        store
            .conn()
            .unwrap()
            .execute(
                "INSERT INTO mcp_log (
                    tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                    created_at, key_epoch, payload_minimized_version
                 ) VALUES (?1, ?2, ?3, 1, 1, ?4, ?5, 0)",
                params![
                    "legacy_tool",
                    arguments_encrypted,
                    result_encrypted,
                    chrono::Utc::now().to_rfc3339(),
                    store.key_config().epoch as i64,
                ],
            )
            .unwrap();
        drop(store);

        let restarted = McpAuditStore::with_keyring(&path, configs);
        let serialized = serde_json::to_string(&restarted.list_logs(10).unwrap()).unwrap();

        assert!(!serialized.contains(LEGACY_ARGUMENT));
        assert!(!serialized.contains(LEGACY_RESULT));
        assert!(serialized.contains("payloadStored"));
        assert!(serialized.contains("sha256:"));
        let version: i64 = restarted
            .conn()
            .unwrap()
            .query_row(
                "SELECT payload_minimized_version FROM mcp_log WHERE tool_name = 'legacy_tool'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION);
    }
}

/// Shareable handle.
pub type SharedMcpAuditStore = Arc<Mutex<McpAuditStore>>;
