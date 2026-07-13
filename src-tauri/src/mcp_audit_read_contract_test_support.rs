use crate::persistence_coordinator::{PersistenceCoordinator, PersistenceStoreMode};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) const D065_AUDIT_KEY_REFERENCE_STORE: &str = "McpAuditKeyReferenceStore";
pub(crate) const D065_AUDIT_STORE: &str = "McpAuditStore";
pub(crate) const D065_UNRELATED_STORE: &str = "MemoryStore";

pub(crate) fn assert_d065_store_mode(
    coordinator: &PersistenceCoordinator,
    store: &str,
    expected: PersistenceStoreMode,
) {
    let snapshot = coordinator.snapshot();
    let actual = snapshot
        .stores
        .iter()
        .find(|health| health.store == store)
        .unwrap_or_else(|| panic!("D065 fixture did not register exact store owner {store}"))
        .mode;
    assert_eq!(
        actual, expected,
        "D065 fixture must prove the exact owner mode for {store}"
    );
}

pub(crate) fn assert_d065_composite_read_owners(
    coordinator: &PersistenceCoordinator,
    key_reference_mode: PersistenceStoreMode,
    audit_store_mode: PersistenceStoreMode,
) {
    assert_d065_store_mode(
        coordinator,
        D065_AUDIT_KEY_REFERENCE_STORE,
        key_reference_mode,
    );
    assert_d065_store_mode(coordinator, D065_AUDIT_STORE, audit_store_mode);
    for owner in [D065_AUDIT_KEY_REFERENCE_STORE, D065_AUDIT_STORE] {
        assert!(
            coordinator.require_trusted_read(owner).is_ok(),
            "D065 composite audit-read owner {owner} must remain independently readable"
        );
    }
}

pub(crate) fn assert_d065_effects_blocked_independently(coordinator: &PersistenceCoordinator) {
    assert!(
        coordinator.require_effects_allowed().is_err(),
        "the fixture must independently prove that global effects are blocked"
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqliteFileSnapshot {
    pub path: PathBuf,
    pub exists: bool,
    pub bytes: Option<Vec<u8>>,
    pub len: Option<u64>,
    pub modified: Option<SystemTime>,
    pub permissions_read_only: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqliteFamilySnapshot {
    pub members: Vec<OsString>,
    pub files: Vec<SqliteFileSnapshot>,
}

fn sqlite_sibling_path(database_path: &Path, suffix: &str) -> PathBuf {
    let mut path = database_path.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
}

/// Capture the complete known SQLite file family plus any persistent sibling
/// beginning with the canonical database filename. Access time is excluded
/// because a read itself may update it on some filesystems; bytes, length,
/// modification time and permissions are stable read-only evidence.
pub(crate) fn sqlite_family_snapshot(database_path: &Path) -> SqliteFamilySnapshot {
    let parent = database_path.parent().expect("SQLite path has a parent");
    let database_name = database_path
        .file_name()
        .expect("SQLite path has a filename")
        .to_os_string();
    let database_prefix = {
        let mut prefix = database_name.clone();
        prefix.push("-");
        prefix
    };
    let mut members = std::fs::read_dir(parent)
        .expect("read SQLite family directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name())
        .filter(|name| {
            name == &database_name
                || name
                    .as_os_str()
                    .as_encoded_bytes()
                    .starts_with(database_prefix.as_encoded_bytes())
        })
        .collect::<Vec<_>>();
    members.sort();

    let files = ["", "-wal", "-shm", "-journal"]
        .into_iter()
        .map(|suffix| {
            let path = sqlite_sibling_path(database_path, suffix);
            if !path.exists() {
                return SqliteFileSnapshot {
                    path,
                    exists: false,
                    bytes: None,
                    len: None,
                    modified: None,
                    permissions_read_only: None,
                };
            }
            let metadata = std::fs::metadata(&path).expect("read SQLite family metadata");
            SqliteFileSnapshot {
                path: path.clone(),
                exists: true,
                bytes: Some(std::fs::read(&path).expect("read SQLite family bytes")),
                len: Some(metadata.len()),
                modified: metadata.modified().ok(),
                permissions_read_only: Some(metadata.permissions().readonly()),
            }
        })
        .collect();

    SqliteFamilySnapshot { members, files }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CiphertextColumn {
    Arguments,
    Result,
}

impl CiphertextColumn {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Arguments => "arguments_encrypted",
            Self::Result => "result_encrypted",
        }
    }
}

pub(crate) fn corrupt_ciphertext(database_path: &Path, column: CiphertextColumn) {
    let connection = rusqlite::Connection::open(database_path).expect("open D065 audit fixture");
    let statement = match column {
        CiphertextColumn::Arguments => {
            "UPDATE mcp_log SET arguments_encrypted = 'invalid-ciphertext' WHERE id = 1"
        }
        CiphertextColumn::Result => {
            "UPDATE mcp_log SET result_encrypted = 'invalid-ciphertext' WHERE id = 1"
        }
    };
    connection
        .execute(statement, [])
        .unwrap_or_else(|error| panic!("corrupt {} fixture: {error}", column.label()));
}
