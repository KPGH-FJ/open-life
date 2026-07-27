use crate::agent::ToolDispatchAttempt;
use crate::llm::{
    ProviderDataRoute, ProviderInvocationReceipt, ProviderInvocationStatus, ProviderPayloadPurpose,
    ProviderPolicyAuthority, ProviderPolicyAuthorization, ProviderPolicyReceiptEvidence,
};
#[cfg(any(test, feature = "test-utils"))]
use crate::scheduler::ProviderInvocationTerminalProof;
use crate::scheduler::{
    ScheduledProviderTruthAdmission, ScheduledProviderTruthRecord, ScheduledProviderTruthTransition,
};
use crate::tool_execution_receipt::{
    ToolActionEffect, ToolDispatchKind, ToolEffectStatus, ToolExecutionOutcome,
    ToolExecutionReceipt, ToolTransportStatus,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(any(test, feature = "test-utils"))]
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const TASK_STORE_SCHEMA_VERSION: i64 = 15;
const EXACT_PROVIDER_PROVENANCE_SCHEMA_VERSION: i64 = 10;
const MAX_LEGACY_SCHEDULED_TASK_BYTES: u64 = 8 * 1024 * 1024;
const MAX_OVERSIZED_LEGACY_FINGERPRINT_BYTES: u64 = 64 * 1024;
const MAX_LEGACY_SCHEDULED_TASK_ITEMS: usize = 10_000;
const MAX_SCHEDULED_TASK_DESCRIPTION_CHARS: usize = 262_144;
const SCHEDULER_POLICY_VERSION: &str = "scheduled-provider-grant-v2";
const SCHEDULER_POLICY_REASON: &str = "scheduled_content_requires_explicit_cloud_consent";
const SCHEDULED_PROVIDER_PAYLOAD_CONTRACT: &str =
    "scheduled_agent_loop_compiler_v1:runtime_compiled_messages:bounded_context:no_raw_canonical";
const TASK_STORE_IDENTITY_METADATA_KEY: &str = "canonical_task_store_identity_v1";
const TASK_STORE_SLOT_VERIFIER_METADATA_KEY: &str = "canonical_task_store_slot_verifier_v1";
const TASK_STORE_OWNER_LOCK_VERIFIER_METADATA_KEY: &str =
    "canonical_task_store_owner_lock_verifier_v1";
const TASK_STORE_PRE_V13_PURGE_COMPLETE_METADATA_KEY: &str = "pre_v13_physical_purge_complete_v1";
const TASK_STORE_SLOT_DERIVATION_DOMAIN: &str = "openlife-task-store-slot-derivation-v1";
const TASK_STORE_SLOT_VERIFIER_DOMAIN: &str = "openlife-task-store-slot-verifier-v1";
const TASK_STORE_OWNER_LOCK_VERIFIER_DOMAIN: &str = "openlife-task-store-owner-lock-verifier-v1";
const TASK_STORE_OWNER_ENVELOPE_DOMAIN: &str = "openlife-task-store-owner-envelope-v1";
const TASK_STORE_OWNER_ENVELOPE_SCHEMA: &str = "openlife.task-store.owner-envelope.v1";
const MAX_TASK_STORE_OWNER_ENVELOPE_BYTES: usize = 4 * 1024;
const TASK_STORE_CLAIM_SEAL_DOMAIN: &str = "openlife-task-store-claim-seal-v1";

/// Purpose-isolated root key for the canonical scheduled-task database slot.
/// Product bootstrap must hydrate it from an OS secret owner. The key is never
/// serialized into the SQLite database; a copied database therefore cannot
/// authenticate itself at another filesystem slot.
#[derive(Clone, PartialEq, Eq)]
pub struct TaskStoreAuthorityKey([u8; 32]);

impl std::fmt::Debug for TaskStoreAuthorityKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TaskStoreAuthorityKey([REDACTED])")
    }
}

impl TaskStoreAuthorityKey {
    pub fn from_key_material(material: &[u8]) -> Result<Self> {
        let key: [u8; 32] = material
            .try_into()
            .map_err(|_| anyhow::anyhow!("task_store_authority_key_must_be_32_bytes"))?;
        if key.iter().all(|byte| *byte == 0) {
            anyhow::bail!("task_store_authority_key_must_not_be_zero");
        }
        Ok(Self(key))
    }

    fn random() -> Result<Self> {
        loop {
            let key = rand::random::<[u8; 32]>();
            if key.iter().any(|byte| *byte != 0) {
                return Self::from_key_material(&key);
            }
        }
    }

    fn derive_for_database_slot(&self, slot_material: &[u8]) -> Result<Self> {
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &self.0);
        let mut material =
            Vec::with_capacity(TASK_STORE_SLOT_DERIVATION_DOMAIN.len() + slot_material.len() + 9);
        material.extend_from_slice(TASK_STORE_SLOT_DERIVATION_DOMAIN.as_bytes());
        material.push(0);
        material.extend_from_slice(&(slot_material.len() as u64).to_be_bytes());
        material.extend_from_slice(slot_material);
        Self::from_key_material(ring::hmac::sign(&key, &material).as_ref())
    }

    fn sign(&self, domain: &str, material: &[u8]) -> String {
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &self.0);
        let mut bound = Vec::with_capacity(domain.len() + material.len() + 9);
        bound.extend_from_slice(domain.as_bytes());
        bound.push(0);
        bound.extend_from_slice(&(material.len() as u64).to_be_bytes());
        bound.extend_from_slice(material);
        let encoded = ring::hmac::sign(&key, &bound)
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("hmac-sha256:{encoded}")
    }

    fn verify(&self, domain: &str, material: &[u8], expected: &str) -> bool {
        let Some(encoded) = expected.strip_prefix("hmac-sha256:") else {
            return false;
        };
        if encoded.len() != 64
            || !encoded
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return false;
        }
        let mut expected_bytes = Vec::with_capacity(32);
        for offset in (0..encoded.len()).step_by(2) {
            let Ok(byte) = u8::from_str_radix(&encoded[offset..offset + 2], 16) else {
                return false;
            };
            expected_bytes.push(byte);
        }
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &self.0);
        let mut bound = Vec::with_capacity(domain.len() + material.len() + 9);
        bound.extend_from_slice(domain.as_bytes());
        bound.push(0);
        bound.extend_from_slice(&(material.len() as u64).to_be_bytes());
        bound.extend_from_slice(material);
        ring::hmac::verify(&key, &bound, &expected_bytes).is_ok()
    }
}

struct TaskStoreDatabaseSlot {
    canonical_material: Vec<u8>,
    slot_key: TaskStoreAuthorityKey,
}

/// Pre-SQLite authentication state persisted on the exact owner-lock inode.
/// The outer HMAC binds every field; the two inner verifiers are retained so
/// the post-open database metadata can be cross-checked without translating
/// authority formats. Unknown JSON fields are rejected to keep the envelope
/// bounded and version-exact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct TaskStoreOwnerEnvelopeV1 {
    schema: String,
    canonical_store_identity: String,
    database_identity: String,
    owner_lock_identity: String,
    database_slot_verifier: String,
    owner_lock_verifier: String,
    envelope_hmac: String,
}

impl TaskStoreDatabaseSlot {
    fn for_canonical_path(canonical_path: &Path, root_key: &TaskStoreAuthorityKey) -> Result<Self> {
        let canonical_text = canonical_path.to_string_lossy();
        let mut canonical_material = Vec::with_capacity(canonical_text.len() + 48);
        canonical_material.extend_from_slice(b"openlife-task-store-canonical-path-v1");
        canonical_material.push(0);
        canonical_material.extend_from_slice(&(canonical_text.len() as u64).to_be_bytes());
        canonical_material.extend_from_slice(canonical_text.as_bytes());
        let slot_key = root_key.derive_for_database_slot(&canonical_material)?;
        Ok(Self {
            canonical_material,
            slot_key,
        })
    }

    fn ephemeral() -> Result<Self> {
        let root_key = TaskStoreAuthorityKey::random()?;
        let nonce = uuid::Uuid::new_v4().to_string();
        let mut canonical_material = b"openlife-task-store-in-memory-v1\0".to_vec();
        canonical_material.extend_from_slice(nonce.as_bytes());
        let slot_key = root_key.derive_for_database_slot(&canonical_material)?;
        Ok(Self {
            canonical_material,
            slot_key,
        })
    }

    fn verifier_material(&self, store_identity: &str) -> Vec<u8> {
        let mut material =
            Vec::with_capacity(self.canonical_material.len() + store_identity.len() + 16);
        material.extend_from_slice(&(store_identity.len() as u64).to_be_bytes());
        material.extend_from_slice(store_identity.as_bytes());
        material.extend_from_slice(&(self.canonical_material.len() as u64).to_be_bytes());
        material.extend_from_slice(&self.canonical_material);
        material
    }

    fn sign_store_identity(&self, store_identity: &str) -> String {
        self.slot_key.sign(
            TASK_STORE_SLOT_VERIFIER_DOMAIN,
            &self.verifier_material(store_identity),
        )
    }

    fn verify_store_identity(&self, store_identity: &str, verifier: &str) -> bool {
        self.slot_key.verify(
            TASK_STORE_SLOT_VERIFIER_DOMAIN,
            &self.verifier_material(store_identity),
            verifier,
        )
    }

    fn owner_lock_verifier_material(&self, owner_lock_identity_material: &str) -> Vec<u8> {
        let mut material = Vec::with_capacity(
            self.canonical_material.len() + owner_lock_identity_material.len() + 16,
        );
        material.extend_from_slice(&(owner_lock_identity_material.len() as u64).to_be_bytes());
        material.extend_from_slice(owner_lock_identity_material.as_bytes());
        material.extend_from_slice(&(self.canonical_material.len() as u64).to_be_bytes());
        material.extend_from_slice(&self.canonical_material);
        material
    }

    fn sign_owner_lock_identity(&self, owner_lock_identity_material: &str) -> String {
        self.slot_key.sign(
            TASK_STORE_OWNER_LOCK_VERIFIER_DOMAIN,
            &self.owner_lock_verifier_material(owner_lock_identity_material),
        )
    }

    fn verify_owner_lock_identity(
        &self,
        owner_lock_identity_material: &str,
        verifier: &str,
    ) -> bool {
        self.slot_key.verify(
            TASK_STORE_OWNER_LOCK_VERIFIER_DOMAIN,
            &self.owner_lock_verifier_material(owner_lock_identity_material),
            verifier,
        )
    }

    fn owner_envelope_material(
        &self,
        store_identity: &str,
        database_identity: &str,
        owner_lock_identity: &str,
        database_slot_verifier: &str,
        owner_lock_verifier: &str,
    ) -> Vec<u8> {
        let fields = [
            TASK_STORE_OWNER_ENVELOPE_SCHEMA,
            store_identity,
            database_identity,
            owner_lock_identity,
            database_slot_verifier,
            owner_lock_verifier,
        ];
        let mut material = Vec::new();
        for field in fields {
            material.extend_from_slice(&(field.len() as u64).to_be_bytes());
            material.extend_from_slice(field.as_bytes());
        }
        material
    }

    fn issue_owner_envelope(
        &self,
        store_identity: String,
        database_identity: String,
        owner_lock_identity: String,
    ) -> Result<TaskStoreOwnerEnvelopeV1> {
        validate_task_store_identity(&store_identity)?;
        let database_slot_verifier = self.sign_store_identity(&store_identity);
        let owner_lock_verifier = self.sign_owner_lock_identity(&owner_lock_identity);
        let envelope_hmac = self.slot_key.sign(
            TASK_STORE_OWNER_ENVELOPE_DOMAIN,
            &self.owner_envelope_material(
                &store_identity,
                &database_identity,
                &owner_lock_identity,
                &database_slot_verifier,
                &owner_lock_verifier,
            ),
        );
        Ok(TaskStoreOwnerEnvelopeV1 {
            schema: TASK_STORE_OWNER_ENVELOPE_SCHEMA.to_string(),
            canonical_store_identity: store_identity,
            database_identity,
            owner_lock_identity,
            database_slot_verifier,
            owner_lock_verifier,
            envelope_hmac,
        })
    }

    fn verify_owner_envelope(
        &self,
        envelope: &TaskStoreOwnerEnvelopeV1,
        database_identity: &str,
        owner_lock_identity: &str,
    ) -> Result<()> {
        if envelope.schema != TASK_STORE_OWNER_ENVELOPE_SCHEMA {
            anyhow::bail!("task_store_owner_envelope_schema_invalid");
        }
        validate_task_store_identity(&envelope.canonical_store_identity)?;
        if envelope.database_identity != database_identity
            || envelope.owner_lock_identity != owner_lock_identity
        {
            anyhow::bail!("task_store_owner_lock_authentication_failed:inode_binding");
        }
        if !self.verify_store_identity(
            &envelope.canonical_store_identity,
            &envelope.database_slot_verifier,
        ) || !self.verify_owner_lock_identity(
            &envelope.owner_lock_identity,
            &envelope.owner_lock_verifier,
        ) || !self.slot_key.verify(
            TASK_STORE_OWNER_ENVELOPE_DOMAIN,
            &self.owner_envelope_material(
                &envelope.canonical_store_identity,
                &envelope.database_identity,
                &envelope.owner_lock_identity,
                &envelope.database_slot_verifier,
                &envelope.owner_lock_verifier,
            ),
            &envelope.envelope_hmac,
        ) {
            anyhow::bail!("task_store_owner_lock_authentication_failed:envelope_hmac");
        }
        Ok(())
    }
}

fn canonical_task_store_database_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path).with_context(|| {
            format!(
                "canonicalize existing scheduled task database slot before open: {}",
                path.display()
            )
        });
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let canonical_parent = std::fs::canonicalize(parent.unwrap_or_else(|| Path::new(".")))
        .with_context(|| {
            format!(
                "canonicalize scheduled task database parent before open: {}",
                path.display()
            )
        })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("scheduled_task_store_database_file_name_missing"))?;
    Ok(canonical_parent.join(file_name))
}

fn open_task_store_writable_nofollow(canonical_path: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        canonical_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .with_context(|| {
        format!(
            "failed to open authenticated tasks db at {}",
            canonical_path.display()
        )
    })
}

#[cfg(test)]
fn open_task_store_database_with_stable_slot<F, G>(
    path: &Path,
    after_expected_before_open: F,
    after_open_before_validation: G,
) -> Result<(
    Connection,
    PathBuf,
    crate::sqlite_migration::SqliteSlotOwnerLease,
)>
where
    F: FnOnce(),
    G: FnOnce(),
{
    let expected_slot = canonical_task_store_database_path(path)?;
    let owner_lease = crate::sqlite_migration::SqliteSlotOwnerLease::acquire(
        &expected_slot,
        "scheduled_task_store",
    )?;
    after_expected_before_open();
    owner_lease.validate_database_identity()?;
    let conn = open_task_store_writable_nofollow(&expected_slot)?;
    owner_lease.validate_database_identity()?;
    after_open_before_validation();
    let observed_slot = crate::sqlite_migration::canonical_opened_main_database_path(
        &conn,
        "scheduled_task_store",
    )?
    .ok_or_else(|| anyhow::anyhow!("scheduled_task_store_persistent_database_path_missing"))?;
    if observed_slot != expected_slot {
        anyhow::bail!(
            "scheduled_task_store_database_slot_changed_during_open:{}!={}",
            expected_slot.display(),
            observed_slot.display()
        );
    }
    owner_lease.bind_opened_database_identity()?;
    Ok((conn, observed_slot, owner_lease))
}

#[cfg(test)]
fn configure_authenticated_task_store_connection<F>(
    connection: &crate::sqlite_migration::IdentityBoundSqliteConnection,
    after_preflight_before_configure: F,
) -> Result<()>
where
    F: FnOnce(),
{
    let conn = connection.lock()?;
    after_preflight_before_configure();
    // The preflight can be arbitrarily slow and the pathname can change after
    // it authenticates. Revalidate the retained database and owner-lock file
    // identities while the identity-bound connection guard is still held and
    // immediately before the first mutating PRAGMA.
    connection.validate_identity()?;
    configure_connection(&conn)
}

fn crosscheck_task_store_owner_envelope_after_sqlite_open(
    conn: &Connection,
    database_slot: &TaskStoreDatabaseSlot,
    envelope: &TaskStoreOwnerEnvelopeV1,
) -> Result<()> {
    let existing_schema_version = task_store_schema_version(conn)?.unwrap_or(0);
    if existing_schema_version > TASK_STORE_SCHEMA_VERSION {
        anyhow::bail!("task store schema is newer than this OpenLife build");
    }
    let metadata_exists = task_store_table_exists(conn, "task_store_metadata")?;
    if !metadata_exists {
        if existing_schema_version >= 13 {
            anyhow::bail!("task_store_canonical_authority_metadata_missing");
        }
        return Ok(());
    }

    let internal_identity = task_store_metadata_value(conn, TASK_STORE_IDENTITY_METADATA_KEY)?;
    let internal_slot_verifier =
        task_store_metadata_value(conn, TASK_STORE_SLOT_VERIFIER_METADATA_KEY)?;
    match (internal_identity, internal_slot_verifier) {
        (Some(identity), Some(verifier)) => {
            if identity != envelope.canonical_store_identity
                || verifier != envelope.database_slot_verifier
                || !database_slot.verify_store_identity(&identity, &verifier)
            {
                anyhow::bail!("task_store_database_slot_authentication_failed");
            }
        }
        (None, None) if existing_schema_version < 13 => {}
        (None, None) => anyhow::bail!("task_store_canonical_authority_metadata_missing"),
        _ => anyhow::bail!("task_store_canonical_authority_metadata_incomplete"),
    }

    match task_store_metadata_value(conn, TASK_STORE_OWNER_LOCK_VERIFIER_METADATA_KEY)? {
        Some(verifier)
            if verifier == envelope.owner_lock_verifier
                && database_slot
                    .verify_owner_lock_identity(&envelope.owner_lock_identity, &verifier) =>
        {
            Ok(())
        }
        Some(_) => anyhow::bail!("task_store_owner_lock_authentication_failed"),
        None if existing_schema_version < TASK_STORE_SCHEMA_VERSION => Ok(()),
        None => anyhow::bail!("task_store_owner_lock_authority_metadata_missing"),
    }
}

fn configure_preauthenticated_task_store_connection<F>(
    connection: &crate::sqlite_migration::IdentityBoundSqliteConnection,
    database_slot: &TaskStoreDatabaseSlot,
    envelope: &TaskStoreOwnerEnvelopeV1,
    after_preflight_before_configure: F,
) -> Result<()>
where
    F: FnOnce(),
{
    let conn = connection.lock()?;
    after_preflight_before_configure();
    // Keep the identity and internal-authority cross-check immediately adjacent
    // to the first mutating PRAGMA. SQLite has been opened only after the
    // external envelope authenticated the exact slot and inodes.
    connection.validate_identity()?;
    crosscheck_task_store_owner_envelope_after_sqlite_open(&conn, database_slot, envelope)?;
    configure_connection(&conn)
}

fn task_store_sidecar_family_present(expected_slot: &Path) -> Result<bool> {
    let mut present = false;
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = expected_slot.as_os_str().to_os_string();
        sidecar.push(suffix);
        match std::fs::symlink_metadata(PathBuf::from(sidecar)) {
            Ok(_) => present = true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(present)
}

/// Establish pre-SQLite authority for the exact database and lock inodes.
///
/// A complete envelope is verified without opening SQLite at all, including
/// when a live WAL/SHM family exists. The only compatibility exception is an
/// empty legacy lock file paired with a checkpointed database: immutable mode
/// performs one zero-write metadata read, then the signed envelope is fsynced.
/// An invalid/partial envelope is never treated as legacy, and a missing
/// envelope with sidecars fails closed because immutable SQLite would ignore
/// potentially newer committed frames.
fn preauthenticate_task_store_before_sqlite_open(
    expected_slot: &Path,
    owner_lease: &crate::sqlite_migration::SqliteSlotOwnerLease,
    database_slot: &TaskStoreDatabaseSlot,
    existing_nonempty_database: bool,
    database_was_created: bool,
) -> Result<TaskStoreOwnerEnvelopeV1> {
    owner_lease.validate_database_identity()?;
    let database_identity = owner_lease.database_identity_material()?;
    let owner_lock_identity = owner_lease.owner_lock_identity_material();
    let envelope_bytes =
        owner_lease.read_owner_lock_envelope(MAX_TASK_STORE_OWNER_ENVELOPE_BYTES)?;
    if !envelope_bytes.is_empty() {
        let envelope: TaskStoreOwnerEnvelopeV1 = serde_json::from_slice(&envelope_bytes)
            .map_err(|error| anyhow::anyhow!("task_store_owner_envelope_invalid:{error}"))?;
        database_slot.verify_owner_envelope(&envelope, &database_identity, &owner_lock_identity)?;
        owner_lease.validate_database_identity()?;
        return Ok(envelope);
    }

    if task_store_sidecar_family_present(expected_slot)? {
        anyhow::bail!(
            "task_store_owner_lock_authentication_failed:owner_envelope_missing_with_sidecars"
        );
    }
    if !existing_nonempty_database && !database_was_created {
        anyhow::bail!(
            "task_store_owner_lock_authentication_failed:owner_envelope_missing_existing_empty_slot"
        );
    }

    let store_identity = if existing_nonempty_database {
        let conn = crate::sqlite_migration::open_existing_immutable_read_only(
            expected_slot,
            "scheduled_task_store_legacy_owner_envelope_migration",
            &[],
        )?;
        let observed_slot = crate::sqlite_migration::canonical_opened_main_database_path(
            &conn,
            "scheduled_task_store_legacy_owner_envelope_migration",
        )?
        .ok_or_else(|| anyhow::anyhow!("scheduled_task_store_preflight_database_path_missing"))?;
        if observed_slot != expected_slot {
            anyhow::bail!(
                "scheduled_task_store_database_slot_changed_during_preflight:{}!={}",
                expected_slot.display(),
                observed_slot.display()
            );
        }
        let legacy_schema_version = task_store_schema_version(&conn)?.unwrap_or(0);
        if legacy_schema_version >= TASK_STORE_SCHEMA_VERSION {
            anyhow::bail!(
                "task_store_owner_lock_authentication_failed:owner_envelope_missing_current_schema"
            );
        }
        if task_store_table_exists(&conn, "task_store_metadata")?
            && task_store_metadata_value(&conn, TASK_STORE_OWNER_LOCK_VERIFIER_METADATA_KEY)?
                .is_some()
        {
            anyhow::bail!(
                "task_store_owner_lock_authentication_failed:legacy_owner_verifier_already_bound"
            );
        }
        preflight_existing_task_store_owner_lock_binding(
            &conn,
            database_slot,
            &owner_lock_identity,
        )?;
        if task_store_table_exists(&conn, "task_store_metadata")? {
            task_store_metadata_value(&conn, TASK_STORE_IDENTITY_METADATA_KEY)?
                .unwrap_or_else(|| format!("task-store:{}", uuid::Uuid::new_v4()))
        } else {
            format!("task-store:{}", uuid::Uuid::new_v4())
        }
    } else {
        format!("task-store:{}", uuid::Uuid::new_v4())
    };
    let envelope = database_slot.issue_owner_envelope(
        store_identity,
        database_identity,
        owner_lock_identity,
    )?;
    let encoded = serde_json::to_vec(&envelope)?;
    owner_lease.write_owner_lock_envelope(&encoded, MAX_TASK_STORE_OWNER_ENVELOPE_BYTES)?;
    owner_lease.validate_database_identity()?;
    Ok(envelope)
}

fn open_task_store_database_read_only_with_stable_slot<F, G>(
    path: &Path,
    after_expected_before_open: F,
    after_open_before_validation: G,
) -> Result<(
    Connection,
    PathBuf,
    crate::sqlite_migration::SqliteDatabaseIdentityGuard,
)>
where
    F: FnOnce(),
    G: FnOnce(),
{
    let expected_slot = canonical_task_store_database_path(path)?;
    let identity_guard = crate::sqlite_migration::SqliteDatabaseIdentityGuard::capture(
        &expected_slot,
        "scheduled_task_store_read_only",
    )?;
    after_expected_before_open();
    let conn =
        crate::sqlite_migration::open_existing_read_only(path, "scheduled_task_store", &["tasks"])?;
    after_open_before_validation();
    let observed_slot = crate::sqlite_migration::canonical_opened_main_database_path(
        &conn,
        "scheduled_task_store_read_only",
    )?
    .ok_or_else(|| anyhow::anyhow!("scheduled_task_store_read_only_database_path_missing"))?;
    if observed_slot != expected_slot {
        anyhow::bail!(
            "scheduled_task_store_database_slot_changed_during_read_only_open:{}!={}",
            expected_slot.display(),
            observed_slot.display()
        );
    }
    identity_guard.validate()?;
    Ok((conn, observed_slot, identity_guard))
}

#[derive(Debug, PartialEq, Eq)]
struct TaskStoreRuntimeAuthority {
    canonical_store_identity: Arc<str>,
    database_slot_verifier: Arc<str>,
    process_epoch_id: uuid::Uuid,
    writer_owner_generation_id: uuid::Uuid,
    claim_sealing_key: Option<TaskStoreAuthorityKey>,
    available: bool,
}

impl TaskStoreRuntimeAuthority {
    fn unavailable() -> Arc<Self> {
        Arc::new(Self {
            canonical_store_identity: Arc::from("task_store:unavailable"),
            database_slot_verifier: Arc::from("unavailable"),
            process_epoch_id: uuid::Uuid::nil(),
            writer_owner_generation_id: uuid::Uuid::nil(),
            claim_sealing_key: None,
            available: false,
        })
    }

    fn seal_claim(&self, material: &[u8]) -> Result<String> {
        let key = self
            .claim_sealing_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("scheduled claim sealing authority is unavailable"))?;
        Ok(key.sign(TASK_STORE_CLAIM_SEAL_DOMAIN, material))
    }

    fn verify_claim_seal(&self, material: &[u8], seal: &str) -> bool {
        self.claim_sealing_key
            .as_ref()
            .is_some_and(|key| key.verify(TASK_STORE_CLAIM_SEAL_DOMAIN, material, seal))
    }
}

fn task_store_process_epoch_id() -> uuid::Uuid {
    static PROCESS_EPOCH_ID: OnceLock<uuid::Uuid> = OnceLock::new();
    *PROCESS_EPOCH_ID.get_or_init(uuid::Uuid::new_v4)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ScheduledTaskGrantAuthorityProof {
    #[default]
    UntrustedSerializedState,
    CanonicalReviewedPolicyRoute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledTask {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub source_run_id: Option<String>,
    #[serde(default)]
    pub source_proposal_id: Option<String>,
    #[serde(default)]
    pub action_type: String,
    #[serde(default)]
    pub attempt_count: u32,
    #[serde(default)]
    pub claim_token: Option<String>,
    #[serde(default)]
    pub lease_expires_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub result_digest: Option<String>,
    #[serde(default)]
    pub result_ref: Option<String>,
    pub provider_grant: ScheduledProviderGrantV2,
    /// Capability provenance is deliberately not serializable. A caller can
    /// deserialize the public grant facts for display or transport, but only a
    /// live PolicyRouter decision (or a row reloaded from the canonical store)
    /// can carry cloud-execution authority.
    #[serde(skip)]
    provider_grant_authority: ScheduledTaskGrantAuthorityProof,
}

impl ScheduledTask {
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        due_date: Option<String>,
        priority: impl Into<String>,
    ) -> Self {
        let priority: String = priority.into();
        let id = uuid::Uuid::new_v4().to_string();
        let title = title.into();
        let description = description.into();
        let action_type = "scheduled_task".to_string();
        let provider_grant = ScheduledProviderGrantV2::deterministic_local_only(
            &id,
            &description,
            &action_type,
            due_date.as_deref(),
            None,
            None,
        );
        Self {
            id,
            title,
            description,
            due_date,
            priority: if priority.is_empty() {
                "medium".into()
            } else {
                priority
            },
            status: "pending".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            source_run_id: None,
            source_proposal_id: None,
            action_type,
            attempt_count: 0,
            claim_token: None,
            lease_expires_at: None,
            last_error: None,
            result_digest: None,
            result_ref: None,
            provider_grant,
            provider_grant_authority: ScheduledTaskGrantAuthorityProof::UntrustedSerializedState,
        }
    }

    /// Seal deterministic LocalOnly authority. Neither proposal payloads nor
    /// NetworkPolicy can widen this method to cloud.
    pub fn seal_deterministic_local_provider_grant(&mut self) {
        self.provider_grant = ScheduledProviderGrantV2::deterministic_local_only(
            &self.id,
            &self.description,
            &self.action_type,
            self.due_date.as_deref(),
            self.source_run_id.as_deref(),
            self.source_proposal_id.as_deref(),
        );
        self.provider_grant_authority = ScheduledTaskGrantAuthorityProof::UntrustedSerializedState;
    }

    pub fn seal_reviewed_cloud_provider_grant(
        &mut self,
        decision: &crate::agent::main_chat_agent_v1::ScheduledProviderRouteDecision,
    ) -> Result<()> {
        if decision.data_route() != ProviderDataRoute::PolicyAllowed
            || decision.subject_digest() != digest_content(&self.description)
            || decision.schedule_digest()
                != scheduled_task_schedule_digest(
                    &self.id,
                    &self.action_type,
                    self.due_date.as_deref(),
                )
        {
            anyhow::bail!("scheduled provider decision does not match the canonical task");
        }
        let mut grant = ScheduledProviderGrantV2 {
            grant_id: String::new(),
            policy_version: decision.policy_version().to_string(),
            policy_decision_digest: decision.decision_id().to_string(),
            data_route: decision.data_route(),
            reason_code: decision.reason_code().to_string(),
            subject_digest: decision.subject_digest().to_string(),
            schedule_digest: decision.schedule_digest().to_string(),
            payload_purpose: ProviderPayloadPurpose::AgentLoopStep,
            payload_contract_digest: digest_ref(SCHEDULED_PROVIDER_PAYLOAD_CONTRACT),
            source_ref_digest: digest_parts(&[
                "scheduled_provider_grant_source_v2",
                &self.id,
                self.source_run_id.as_deref().unwrap_or("none"),
                self.source_proposal_id.as_deref().unwrap_or("none"),
            ]),
            grant_scope: ScheduledProviderGrantScope::SingleExecution,
            grant_expires_at: Some(decision.grant_expires_at().to_rfc3339()),
            provider_digest: Some(decision.provider_digest().to_string()),
            model_digest: Some(decision.model_digest().to_string()),
            review_snapshot_digest: Some(decision.review_snapshot_digest().to_string()),
            review_dispatch_claim_digest: Some(decision.review_dispatch_claim_digest().to_string()),
        };
        grant.grant_id = grant.canonical_grant_id(&self.id, &self.action_type);
        grant.validate_for_task(self)?;
        self.provider_grant = grant;
        self.provider_grant_authority =
            ScheduledTaskGrantAuthorityProof::CanonicalReviewedPolicyRoute;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledProviderGrantScope {
    LocalOnlyDurable,
    SingleExecution,
}

impl ScheduledProviderGrantScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::LocalOnlyDurable => "local_only_durable",
            Self::SingleExecution => "single_execution",
        }
    }
}

/// Deterministic policy fact owned by the scheduled-execution control plane.
///
/// A scheduled task is not durable consent to send its future contents to a
/// cloud provider. This decision is persisted on every attempt and then bound
/// to every provider receipt. It contains no prompt or response content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledProviderGrantV2 {
    pub grant_id: String,
    pub policy_version: String,
    pub policy_decision_digest: String,
    pub data_route: ProviderDataRoute,
    pub reason_code: String,
    pub subject_digest: String,
    pub schedule_digest: String,
    pub payload_purpose: ProviderPayloadPurpose,
    pub payload_contract_digest: String,
    pub source_ref_digest: String,
    pub grant_scope: ScheduledProviderGrantScope,
    pub grant_expires_at: Option<String>,
    pub provider_digest: Option<String>,
    pub model_digest: Option<String>,
    pub review_snapshot_digest: Option<String>,
    pub review_dispatch_claim_digest: Option<String>,
}

impl ScheduledProviderGrantV2 {
    fn deterministic_local_only(
        task_id: &str,
        description: &str,
        action_type: &str,
        due_date: Option<&str>,
        source_run_id: Option<&str>,
        source_proposal_id: Option<&str>,
    ) -> Self {
        Self::deterministic_local_only_with_provider_digest(
            task_id,
            description,
            action_type,
            due_date,
            source_run_id,
            source_proposal_id,
            crate::agent::metadata_safe::metadata_safe_text_digest("ollama").1,
        )
    }

    /// v10/v11 sealed the local provider target with the generic
    /// length-prefixed reference digest. The provider adapter contract uses
    /// the digest of the exact target text instead. Keep this constructor
    /// private and migration-only so old canonical rows can be recognized
    /// exactly without accepting arbitrary caller-shaped state.
    fn deterministic_local_only_v11(
        task_id: &str,
        description: &str,
        action_type: &str,
        due_date: Option<&str>,
        source_run_id: Option<&str>,
        source_proposal_id: Option<&str>,
    ) -> Self {
        Self::deterministic_local_only_with_provider_digest(
            task_id,
            description,
            action_type,
            due_date,
            source_run_id,
            source_proposal_id,
            digest_ref("ollama"),
        )
    }

    fn deterministic_local_only_with_provider_digest(
        task_id: &str,
        description: &str,
        action_type: &str,
        due_date: Option<&str>,
        source_run_id: Option<&str>,
        source_proposal_id: Option<&str>,
        provider_digest: String,
    ) -> Self {
        let policy_version = SCHEDULER_POLICY_VERSION.to_string();
        let reason_code = SCHEDULER_POLICY_REASON.to_string();
        let data_route = ProviderDataRoute::LocalOnly;
        let subject_digest = digest_content(description);
        let schedule_digest = scheduled_task_schedule_digest(task_id, action_type, due_date);
        let payload_purpose = ProviderPayloadPurpose::AgentLoopStep;
        let payload_contract_digest = digest_ref(SCHEDULED_PROVIDER_PAYLOAD_CONTRACT);
        let source_ref_digest = digest_parts(&[
            "scheduled_provider_grant_source_v2",
            task_id,
            source_run_id.unwrap_or("none"),
            source_proposal_id.unwrap_or("none"),
        ]);
        let mut grant = Self {
            grant_id: String::new(),
            policy_version,
            policy_decision_digest: digest_parts(&[
                "scheduled_local_policy_decision_v2",
                task_id,
                &subject_digest,
                &schedule_digest,
            ]),
            data_route,
            reason_code,
            subject_digest,
            schedule_digest,
            payload_purpose,
            payload_contract_digest,
            source_ref_digest,
            grant_scope: ScheduledProviderGrantScope::LocalOnlyDurable,
            grant_expires_at: None,
            provider_digest: Some(provider_digest),
            model_digest: None,
            review_snapshot_digest: None,
            review_dispatch_claim_digest: None,
        };
        grant.grant_id = grant.canonical_grant_id(task_id, action_type);
        grant
    }

    pub fn allows_cloud(&self) -> bool {
        self.data_route == ProviderDataRoute::PolicyAllowed
    }

    fn validate_for_task(&self, task: &ScheduledTask) -> Result<()> {
        validate_digest(
            "scheduled provider policy decision",
            &self.policy_decision_digest,
        )?;
        if self.subject_digest != digest_content(&task.description)
            || self.schedule_digest
                != scheduled_task_schedule_digest(
                    &task.id,
                    &task.action_type,
                    task.due_date.as_deref(),
                )
            || self.payload_purpose != ProviderPayloadPurpose::AgentLoopStep
            || self.payload_contract_digest != digest_ref(SCHEDULED_PROVIDER_PAYLOAD_CONTRACT)
            || self.source_ref_digest
                != digest_parts(&[
                    "scheduled_provider_grant_source_v2",
                    &task.id,
                    task.source_run_id.as_deref().unwrap_or("none"),
                    task.source_proposal_id.as_deref().unwrap_or("none"),
                ])
            || self.grant_id != self.canonical_grant_id(&task.id, &task.action_type)
        {
            anyhow::bail!("scheduled provider grant does not match canonical task subject");
        }
        match self.data_route {
            ProviderDataRoute::LocalOnly => {
                let expected = Self::deterministic_local_only(
                    &task.id,
                    &task.description,
                    &task.action_type,
                    task.due_date.as_deref(),
                    task.source_run_id.as_deref(),
                    task.source_proposal_id.as_deref(),
                );
                if self != &expected {
                    anyhow::bail!("scheduled local-only grant is not deterministic");
                }
            }
            ProviderDataRoute::PolicyAllowed => {
                if self.policy_version != "scheduled_provider_policy_v2"
                    || self.reason_code != "reviewed_scheduled_cloud_single_execution"
                    || self.grant_scope != ScheduledProviderGrantScope::SingleExecution
                    || self.provider_digest.is_none()
                    || self.model_digest.is_none()
                    || self.review_snapshot_digest.is_none()
                    || self.review_dispatch_claim_digest.is_none()
                {
                    anyhow::bail!("scheduled cloud grant lacks reviewed exact scope");
                }
                let expires_at = self
                    .grant_expires_at
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("scheduled cloud grant has no expiry"))?;
                let expires_at = DateTime::parse_from_rfc3339(expires_at)
                    .context("scheduled cloud grant expiry is invalid")?
                    .with_timezone(&Utc);
                if task
                    .due_date
                    .as_deref()
                    .and_then(|due_at| DateTime::parse_from_rfc3339(due_at).ok())
                    .is_some_and(|due_at| due_at.with_timezone(&Utc) > expires_at)
                {
                    anyhow::bail!("scheduled cloud grant expires before its canonical due time");
                }
                for digest in [
                    self.provider_digest.as_deref(),
                    self.model_digest.as_deref(),
                    self.review_snapshot_digest.as_deref(),
                    self.review_dispatch_claim_digest.as_deref(),
                ] {
                    validate_digest("scheduled cloud grant provenance", digest.unwrap())?;
                }
                let expected_decision_digest = scheduled_provider_policy_decision_digest(
                    &task.id,
                    &self.subject_digest,
                    &self.schedule_digest,
                    self.provider_digest.as_deref().unwrap(),
                    self.model_digest.as_deref().unwrap(),
                    &expires_at,
                    self.review_snapshot_digest.as_deref().unwrap(),
                    self.review_dispatch_claim_digest.as_deref().unwrap(),
                    &self.reason_code,
                );
                if self.policy_decision_digest != expected_decision_digest {
                    anyhow::bail!("scheduled cloud policy decision digest is not canonical");
                }
            }
        }
        Ok(())
    }

    fn canonical_grant_id(&self, task_id: &str, action_type: &str) -> String {
        digest_parts(&[
            "scheduled_provider_grant_v2",
            &self.policy_version,
            &self.policy_decision_digest,
            task_id,
            action_type,
            provider_data_route_label(self.data_route),
            &self.reason_code,
            &self.subject_digest,
            &self.schedule_digest,
            self.payload_purpose.as_str(),
            &self.payload_contract_digest,
            &self.source_ref_digest,
            self.grant_scope.as_str(),
            self.grant_expires_at.as_deref().unwrap_or("none"),
            self.provider_digest.as_deref().unwrap_or("none"),
            self.model_digest.as_deref().unwrap_or("none"),
            self.review_snapshot_digest.as_deref().unwrap_or("none"),
            self.review_dispatch_claim_digest
                .as_deref()
                .unwrap_or("none"),
        ])
    }

    fn is_expired_at(&self, now: &DateTime<Utc>) -> Result<bool> {
        match self.grant_expires_at.as_deref() {
            Some(expires_at) => {
                Ok(DateTime::parse_from_rfc3339(expires_at)?.with_timezone(&Utc) <= *now)
            }
            None => Ok(false),
        }
    }

    fn target_matches(&self, provider: &str, model: &str) -> bool {
        let provider_digest = crate::agent::metadata_safe::metadata_safe_text_digest(provider).1;
        let model_digest = crate::agent::metadata_safe::metadata_safe_text_digest(model).1;
        self.provider_digest.as_deref() == Some(provider_digest.as_str())
            && self
                .model_digest
                .as_deref()
                .is_none_or(|expected| expected == model_digest)
    }
}

#[derive(PartialEq, Eq)]
struct ScheduledClaimAuthorityProof {
    store: Arc<TaskStoreRuntimeAuthority>,
    sealed_claim: String,
}

impl std::fmt::Debug for ScheduledClaimAuthorityProof {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScheduledClaimAuthorityProof")
            .field(
                "canonical_store_identity",
                &self.store.canonical_store_identity,
            )
            .field("process_epoch_id", &self.store.process_epoch_id)
            .field(
                "writer_owner_generation_id",
                &self.store.writer_owner_generation_id,
            )
            .field("database_slot_verifier", &"[HMAC-ONLY]")
            .field("sealed_claim", &"[HMAC-ONLY]")
            .finish()
    }
}

/// Immutable, runtime-issued authority for one exact scheduled attempt.
///
/// Portable task/grant DTOs remain cloneable for display, but the claim itself
/// deliberately exposes only shared getters and does not implement `Clone`.
/// Callers that need concurrent read access must share the same capability via
/// `Arc<ScheduledTaskClaim>` rather than minting another proof-bearing value.
#[derive(PartialEq, Eq)]
pub struct ScheduledTaskClaim {
    task: ScheduledTask,
    claim_token: String,
    attempt_id: String,
    attempt_number: u32,
    provider_grant: ScheduledProviderGrantV2,
    policy_authority_proof: ScheduledClaimAuthorityProof,
}

impl std::fmt::Debug for ScheduledTaskClaim {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScheduledTaskClaim")
            .field("task_id", &self.task.id)
            .field("attempt_id", &self.attempt_id)
            .field("attempt_number", &self.attempt_number)
            .field("claim_token", &"[REDACTED]")
            .field("task_body", &"[REDACTED]")
            .field(
                "provider_grant_id_digest",
                &digest_ref(&self.provider_grant.grant_id),
            )
            .field("provider_data_route", &self.provider_grant.data_route)
            .field("authority", &"[TASK_STORE_SEALED]")
            .finish()
    }
}

impl ScheduledTaskClaim {
    pub fn task(&self) -> &ScheduledTask {
        &self.task
    }

    pub fn claim_token(&self) -> &str {
        &self.claim_token
    }

    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    pub fn attempt_number(&self) -> u32 {
        self.attempt_number
    }

    pub fn provider_grant(&self) -> &ScheduledProviderGrantV2 {
        &self.provider_grant
    }

    pub(crate) fn validate_policy_authority(&self) -> Result<()> {
        if !self.policy_authority_proof.store.available
            || self.policy_authority_proof.store.process_epoch_id.is_nil()
            || self
                .policy_authority_proof
                .store
                .writer_owner_generation_id
                .is_nil()
        {
            anyhow::bail!("scheduled claim policy authority proof is unavailable");
        }
        let material = scheduled_claim_seal_material(
            &self.task,
            &self.claim_token,
            &self.attempt_id,
            self.attempt_number,
            &self.provider_grant,
            self.policy_authority_proof.store.process_epoch_id,
            self.policy_authority_proof.store.writer_owner_generation_id,
        )?;
        if !self
            .policy_authority_proof
            .store
            .verify_claim_seal(&material, &self.policy_authority_proof.sealed_claim)
        {
            anyhow::bail!("scheduled claim no longer matches its TaskStore-issued sealed revision");
        }
        self.provider_grant.validate_for_task(&self.task)?;
        if self.task.provider_grant != self.provider_grant {
            anyhow::bail!("scheduled attempt grant does not match its canonical task grant");
        }
        match self.provider_grant.data_route {
            ProviderDataRoute::PolicyAllowed
                if self.task.provider_grant_authority
                    != ScheduledTaskGrantAuthorityProof::CanonicalReviewedPolicyRoute =>
            {
                anyhow::bail!("scheduled cloud claim lost its canonical ReviewWorkflow authority");
            }
            ProviderDataRoute::LocalOnly
                if self.task.provider_grant_authority
                    != ScheduledTaskGrantAuthorityProof::UntrustedSerializedState =>
            {
                anyhow::bail!("scheduled local claim carries an invalid cloud authority marker");
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_store_authority(&self, authority: &Arc<TaskStoreRuntimeAuthority>) -> Result<()> {
        self.validate_policy_authority()?;
        if !Arc::ptr_eq(&self.policy_authority_proof.store, authority)
            || self.policy_authority_proof.store.canonical_store_identity
                != authority.canonical_store_identity
            || self.policy_authority_proof.store.database_slot_verifier
                != authority.database_slot_verifier
            || self.policy_authority_proof.store.process_epoch_id != authority.process_epoch_id
            || self.policy_authority_proof.store.writer_owner_generation_id
                != authority.writer_owner_generation_id
        {
            anyhow::bail!("scheduled claim belongs to another canonical TaskStore authority");
        }
        Ok(())
    }

    pub(crate) fn canonical_store_identity(&self) -> &str {
        &self.policy_authority_proof.store.canonical_store_identity
    }

    pub(crate) fn database_slot_verifier(&self) -> &str {
        &self.policy_authority_proof.store.database_slot_verifier
    }

    pub(crate) fn runtime_store_instance_id(&self) -> uuid::Uuid {
        self.policy_authority_proof.store.writer_owner_generation_id
    }

    /// Test-only read accessor for constructing metadata evidence fixtures.
    /// It returns only the exact digest derived by the canonical scheduled
    /// policy route; no PolicyAuthorization object or risk-lowering capability
    /// crosses the crate boundary.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_policy_subject_scope_digest(&self) -> Result<String> {
        Ok(ProviderPolicyAuthorization::from_scheduled_claim(self)?.subject_scope_digest())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledClaimSettlement {
    ReclaimedBeforeDispatch,
    GrantConsumedRequiresReview,
    FailedAfterObservedTerminal,
    UnknownRequiresReconciliation,
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScheduledReconciliationIssuer {
    ProviderAdapterReconciler,
    ToolGatewayReconciler,
    #[cfg(any(test, feature = "test-utils"))]
    NativeUserConfirmation,
}

#[cfg(any(test, feature = "test-utils"))]
impl ScheduledReconciliationIssuer {
    fn as_str(self) -> &'static str {
        match self {
            Self::ProviderAdapterReconciler => "provider_adapter_reconciler",
            Self::ToolGatewayReconciler => "tool_gateway_reconciler",
            #[cfg(any(test, feature = "test-utils"))]
            Self::NativeUserConfirmation => "native_user_confirmation",
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScheduledReconciliationEvidenceKind {
    #[cfg(any(test, feature = "test-utils"))]
    NoEffect,
    Failed,
    #[cfg(any(test, feature = "test-utils"))]
    Completed,
}

#[cfg(any(test, feature = "test-utils"))]
impl ScheduledReconciliationEvidenceKind {
    fn as_str(self) -> &'static str {
        match self {
            #[cfg(any(test, feature = "test-utils"))]
            Self::NoEffect => "no_effect_confirmed",
            Self::Failed => "failed_confirmed",
            #[cfg(any(test, feature = "test-utils"))]
            Self::Completed => "completed_confirmed",
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ScheduledUnknownAttemptBinding {
    store: Arc<TaskStoreRuntimeAuthority>,
    task_id: String,
    task_revision_digest: String,
    attempt_id: String,
    attempt_number: u32,
    claim_token_digest: String,
    provider_grant_id: String,
    provider_provenance_state: String,
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone)]
enum ScheduledReconciliationResolution {
    #[cfg(any(test, feature = "test-utils"))]
    RetrySafe,
    ConfirmedFailed {
        reason_code: String,
    },
    #[cfg(any(test, feature = "test-utils"))]
    ConfirmedCompleted {
        result_ref: String,
        result_digest: String,
    },
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone)]
struct ScheduledReconciliationRecord {
    binding: ScheduledUnknownAttemptBinding,
    resolution: ScheduledReconciliationResolution,
    evidence_id: String,
    issuer: ScheduledReconciliationIssuer,
    evidence_kind: ScheduledReconciliationEvidenceKind,
    evidence_ref: String,
    evidence_digest: String,
    issued_at: String,
    source_id: String,
}

/// Single-use runtime capability for resolving one exact unknown scheduler
/// attempt. It deliberately implements neither `Clone` nor serde. Portable
/// evidence fields and caller-selected strings cannot mint this capability.
///
/// ```compile_fail
/// use openlife_core::tasks::ScheduledReconciliationAdmission;
/// fn clone_is_not_authority(admission: ScheduledReconciliationAdmission) {
///     let copied = admission.clone();
///     drop((admission, copied));
/// }
/// ```
///
/// ```compile_fail
/// use openlife_core::tasks::ScheduledReconciliationAdmission;
/// fn serde_is_not_authority(admission: ScheduledReconciliationAdmission) {
///     let _ = serde_json::to_string(&admission).unwrap();
/// }
/// ```
#[cfg(any(test, feature = "test-utils"))]
pub struct ScheduledReconciliationAdmission {
    issuance_id: uuid::Uuid,
    record_digest: String,
    record: Option<ScheduledReconciliationRecord>,
    issued_sources: Arc<Mutex<HashSet<String>>>,
}

#[cfg(any(test, feature = "test-utils"))]
impl std::fmt::Debug for ScheduledReconciliationAdmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScheduledReconciliationAdmission")
            .field("issuance_id", &self.issuance_id)
            .field("record_digest", &self.record_digest)
            .field(
                "task_id",
                &self
                    .record
                    .as_ref()
                    .map(|record| record.binding.task_id.as_str()),
            )
            .field(
                "attempt_id",
                &self
                    .record
                    .as_ref()
                    .map(|record| record.binding.attempt_id.as_str()),
            )
            .finish()
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl ScheduledReconciliationAdmission {
    fn validate_for_store(&self, authority: &Arc<TaskStoreRuntimeAuthority>) -> Result<()> {
        let record = self.record.as_ref().ok_or_else(|| {
            anyhow::anyhow!("scheduled reconciliation admission already consumed")
        })?;
        if !Arc::ptr_eq(&record.binding.store, authority)
            || record.binding.store.process_epoch_id != authority.process_epoch_id
            || record.binding.store.writer_owner_generation_id
                != authority.writer_owner_generation_id
            || record.binding.store.canonical_store_identity != authority.canonical_store_identity
            || record.binding.store.database_slot_verifier != authority.database_slot_verifier
        {
            anyhow::bail!("scheduled reconciliation admission belongs to another TaskStore");
        }
        let expected_digest = scheduled_reconciliation_record_digest(record);
        if expected_digest != self.record_digest {
            anyhow::bail!("scheduled reconciliation admission digest mismatch");
        }
        let registered = self
            .issued_sources
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled reconciliation source registry poisoned"))?
            .contains(&record.source_id);
        if !registered {
            anyhow::bail!("scheduled reconciliation source was revoked or already unavailable");
        }
        Ok(())
    }

    fn finish_committed(&mut self) -> Result<()> {
        let record = self.record.as_ref().ok_or_else(|| {
            anyhow::anyhow!("scheduled reconciliation admission already consumed")
        })?;
        self.issued_sources
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled reconciliation source registry poisoned"))?
            .remove(&record.source_id);
        self.record.take();
        Ok(())
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Drop for ScheduledReconciliationAdmission {
    fn drop(&mut self) {
        let Some(record) = self.record.as_ref() else {
            return;
        };
        if let Ok(mut issued) = self.issued_sources.lock() {
            issued.remove(&record.source_id);
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduledReconciliationTestResolution {
    RetrySafe,
    ConfirmedFailed {
        reason_code: String,
    },
    ConfirmedCompleted {
        result_ref: String,
        result_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledAttemptRecord {
    pub attempt_id: String,
    pub task_id: String,
    pub claim_token: String,
    pub attempt_number: u32,
    pub status: String,
    pub provider_grant_id: String,
    pub policy_version: String,
    pub data_route: String,
    pub policy_reason_code: String,
    pub provider_subject_digest: String,
    pub provider_payload_purpose: String,
    pub provider_payload_contract_digest: String,
    pub provider_source_ref_digest: String,
    pub provider_provenance_state: String,
    pub migration_associated_grant_id: Option<String>,
    pub error_digest: Option<String>,
    pub reconciliation_evidence_digest: Option<String>,
    pub reconciliation_issuer: Option<String>,
    pub reconciliation_evidence_kind: Option<String>,
    pub reconciliation_evidence_ref: Option<String>,
    pub process_epoch_id: String,
    pub writer_owner_generation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledProviderReceiptRecord {
    pub request_id: String,
    pub attempt_id: String,
    pub task_id: String,
    pub claim_token: String,
    pub provider_grant_id: String,
    pub migration_associated_grant_id: Option<String>,
    pub provider_digest: String,
    pub model_digest: String,
    pub status: String,
    pub error_digest: Option<String>,
    pub simulated: Option<bool>,
    pub policy_evidence_state: String,
    pub policy_evidence_digest: Option<String>,
    pub subject_scope_digest: Option<String>,
    pub payload_purpose: Option<String>,
    pub unfiltered_payload_digest: Option<String>,
    pub context_manifest_digest: Option<String>,
    pub prepared_envelope_digest: Option<String>,
    pub prepared_request_digest: Option<String>,
    pub network_policy_decision_digest: Option<String>,
    pub process_epoch_id: String,
    pub writer_owner_generation_id: String,
}

/// Result of retiring the pre-TaskStore JSON owner.
///
/// SQLite keeps metadata/digests only. Raw content for provably not-yet-due
/// pending tasks is returned transiently so bootstrap can stage an exact
/// ReviewWorkflow Proposal; it is never written into the migration journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyScheduledTaskQuarantineReport {
    pub source_digest: Option<String>,
    pub item_count: usize,
    pub review_required_count: usize,
    pub historical_count: usize,
    pub quarantined_count: usize,
    pub source_malformed: bool,
    pub evidence_path: Option<PathBuf>,
}

impl LegacyScheduledTaskQuarantineReport {
    fn absent() -> Self {
        Self {
            source_digest: None,
            item_count: 0,
            review_required_count: 0,
            historical_count: 0,
            quarantined_count: 0,
            source_malformed: false,
            evidence_path: None,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LegacyScheduledTaskReviewCandidate {
    pub source_digest: String,
    pub source_ordinal: usize,
    pub item_digest: String,
    pub title: String,
    pub description: String,
    pub due_at: String,
    pub priority: String,
    pub action_type: String,
    pub source_run_id: Option<String>,
    pub source_proposal_id: Option<String>,
    pub review_created_at: String,
}

impl std::fmt::Debug for LegacyScheduledTaskReviewCandidate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LegacyScheduledTaskReviewCandidate")
            .field("source_digest", &self.source_digest)
            .field("source_ordinal", &self.source_ordinal)
            .field("item_digest", &self.item_digest)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct LegacyScheduledTaskMigrationRow {
    ordinal: usize,
    item_digest: String,
    legacy_task_id_digest: Option<String>,
    legacy_status: &'static str,
    reason_code: &'static str,
    effect_state: &'static str,
    terminal_detail_digest: Option<String>,
    review_candidate: Option<LegacyScheduledTaskReviewCandidate>,
}

pub struct TaskStore {
    conn: crate::sqlite_migration::IdentityBoundSqliteConnection,
    mutation_authority: TaskStoreMutationAuthority,
    writer_owner_generation_id: uuid::Uuid,
    owner_lock_identity_material: Option<Arc<str>>,
    database_slot: Option<TaskStoreDatabaseSlot>,
    preauthenticated_store_identity: Option<Arc<str>>,
    persistent_owner_lease: Option<crate::sqlite_migration::SqliteSlotOwnerLease>,
    persistent_owner_envelope: Option<TaskStoreOwnerEnvelopeV1>,
    owner_envelope_poisoned: Mutex<Option<String>>,
    authority: OnceLock<Arc<TaskStoreRuntimeAuthority>>,
    /// Process-local authority restored only from a non-serializable
    /// ReviewWorkflow proof. A task row and its publicly recomputable digests
    /// are never sufficient to populate this map.
    reviewed_cloud_authorities: Mutex<HashMap<String, String>>,
    /// Runtime source facts already consumed to mint reconciliation
    /// admissions. This is intentionally process-local; a process restart
    /// cannot recreate evidence authority from portable receipt fields.
    #[cfg(any(test, feature = "test-utils"))]
    reconciliation_sources: Arc<Mutex<HashSet<String>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TaskStoreMutationAuthority {
    WritableOwner,
    ReadOnlyObservation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AbandonedRuntimeScope {
    PreviousProcessEpoch,
    SameProcessWriterGeneration,
}

impl TaskStore {
    pub fn new_with_authority_key(
        db_path: impl Into<PathBuf>,
        authority_key: &TaskStoreAuthorityKey,
    ) -> Result<Self> {
        Self::new_with_authority_key_and_open_hooks(db_path, authority_key, || {}, || {})
    }

    fn new_with_authority_key_and_open_hooks<F, G>(
        db_path: impl Into<PathBuf>,
        authority_key: &TaskStoreAuthorityKey,
        after_preauth_before_writable_open: F,
        after_open_before_configure: G,
    ) -> Result<Self>
    where
        F: FnOnce(),
        G: FnOnce(),
    {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let canonical_db_path = canonical_task_store_database_path(&db_path)?;
        let owner_lease = crate::sqlite_migration::SqliteSlotOwnerLease::acquire(
            &canonical_db_path,
            "scheduled_task_store",
        )?;
        let existing_nonempty_database = owner_lease.database_len()? > 0;
        let writer_owner_generation_id = owner_lease.owner_generation_id();
        let owner_lock_identity_material: Arc<str> =
            Arc::from(owner_lease.owner_lock_identity_material());
        let database_slot =
            TaskStoreDatabaseSlot::for_canonical_path(&canonical_db_path, authority_key)?;
        let owner_envelope = preauthenticate_task_store_before_sqlite_open(
            &canonical_db_path,
            &owner_lease,
            &database_slot,
            existing_nonempty_database,
            owner_lease.database_was_created(),
        )?;
        after_preauth_before_writable_open();
        owner_lease.validate_database_identity()?;
        let conn = open_task_store_writable_nofollow(&canonical_db_path)?;
        // sqlite3_open_v2 has not executed schema SQL. Revalidate the exact
        // retained inode before the first PRAGMA/database-list query so a slot
        // replacement cannot make SQLite inspect or recover the replacement.
        owner_lease.validate_database_identity()?;
        let observed_slot = crate::sqlite_migration::canonical_opened_main_database_path(
            &conn,
            "scheduled_task_store",
        )?
        .ok_or_else(|| anyhow::anyhow!("scheduled_task_store_persistent_database_path_missing"))?;
        if observed_slot != canonical_db_path {
            anyhow::bail!(
                "scheduled_task_store_database_slot_changed_during_open:{}!={}",
                canonical_db_path.display(),
                observed_slot.display()
            );
        }
        owner_lease.bind_opened_database_identity()?;
        let persistent_owner_lease = owner_lease.clone();
        let connection =
            crate::sqlite_migration::IdentityBoundSqliteConnection::writable(conn, owner_lease);
        configure_preauthenticated_task_store_connection(
            &connection,
            &database_slot,
            &owner_envelope,
            after_open_before_configure,
        )?;
        let store = Self {
            conn: connection,
            mutation_authority: TaskStoreMutationAuthority::WritableOwner,
            writer_owner_generation_id,
            owner_lock_identity_material: Some(owner_lock_identity_material),
            database_slot: Some(database_slot),
            preauthenticated_store_identity: Some(Arc::from(
                owner_envelope.canonical_store_identity.clone(),
            )),
            persistent_owner_lease: Some(persistent_owner_lease),
            persistent_owner_envelope: Some(owner_envelope),
            owner_envelope_poisoned: Mutex::new(None),
            authority: OnceLock::new(),
            reviewed_cloud_authorities: Mutex::new(HashMap::new()),
            #[cfg(any(test, feature = "test-utils"))]
            reconciliation_sources: Arc::new(Mutex::new(HashSet::new())),
        };
        store.init_tables()?;
        Ok(store)
    }

    /// Test-only convenience. Release dependency graphs have no constructor
    /// that silently substitutes process-local key material for the OS-owned
    /// TaskStore authority slot.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let key = TaskStoreAuthorityKey::from_key_material(&[0x5a; 32])?;
        Self::new_with_authority_key(db_path, &key)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory tasks db")?;
        configure_connection(&conn)?;
        let store = Self {
            conn: crate::sqlite_migration::IdentityBoundSqliteConnection::in_memory(conn),
            mutation_authority: TaskStoreMutationAuthority::WritableOwner,
            writer_owner_generation_id: uuid::Uuid::new_v4(),
            owner_lock_identity_material: Some(Arc::from(format!(
                "in-memory:{}",
                uuid::Uuid::new_v4()
            ))),
            database_slot: Some(TaskStoreDatabaseSlot::ephemeral()?),
            preauthenticated_store_identity: None,
            persistent_owner_lease: None,
            persistent_owner_envelope: None,
            owner_envelope_poisoned: Mutex::new(None),
            authority: OnceLock::new(),
            reviewed_cloud_authorities: Mutex::new(HashMap::new()),
            #[cfg(any(test, feature = "test-utils"))]
            reconciliation_sources: Arc::new(Mutex::new(HashSet::new())),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_read_only_existing_with_authority_key(
        db_path: impl Into<PathBuf>,
        authority_key: &TaskStoreAuthorityKey,
    ) -> Result<Self> {
        let db_path = db_path.into();
        let (conn, canonical_db_path, identity_guard) =
            open_task_store_database_read_only_with_stable_slot(&db_path, || {}, || {})?;
        let database_slot =
            TaskStoreDatabaseSlot::for_canonical_path(&canonical_db_path, authority_key)?;
        let authority =
            load_existing_task_store_authority(&conn, &database_slot, uuid::Uuid::nil())?;
        let authority_cell = OnceLock::new();
        authority_cell
            .set(Arc::new(authority))
            .map_err(|_| anyhow::anyhow!("task_store_authority_initialized_twice"))?;
        Ok(Self {
            conn: crate::sqlite_migration::IdentityBoundSqliteConnection::read_only(
                conn,
                identity_guard,
            ),
            mutation_authority: TaskStoreMutationAuthority::ReadOnlyObservation,
            writer_owner_generation_id: uuid::Uuid::nil(),
            owner_lock_identity_material: None,
            database_slot: Some(database_slot),
            preauthenticated_store_identity: None,
            persistent_owner_lease: None,
            persistent_owner_envelope: None,
            owner_envelope_poisoned: Mutex::new(None),
            authority: authority_cell,
            reviewed_cloud_authorities: Mutex::new(HashMap::new()),
            #[cfg(any(test, feature = "test-utils"))]
            reconciliation_sources: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn unavailable_sentinel() -> Result<Self> {
        let authority = OnceLock::new();
        authority
            .set(TaskStoreRuntimeAuthority::unavailable())
            .map_err(|_| anyhow::anyhow!("task_store_authority_initialized_twice"))?;
        Ok(Self {
            conn: crate::sqlite_migration::IdentityBoundSqliteConnection::in_memory(
                crate::sqlite_migration::unavailable_read_only_sentinel("scheduled_task_store")?,
            ),
            mutation_authority: TaskStoreMutationAuthority::ReadOnlyObservation,
            writer_owner_generation_id: uuid::Uuid::nil(),
            owner_lock_identity_material: None,
            database_slot: None,
            preauthenticated_store_identity: None,
            persistent_owner_lease: None,
            persistent_owner_envelope: None,
            owner_envelope_poisoned: Mutex::new(None),
            authority,
            reviewed_cloud_authorities: Mutex::new(HashMap::new()),
            #[cfg(any(test, feature = "test-utils"))]
            reconciliation_sources: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    fn runtime_authority(&self) -> Result<&Arc<TaskStoreRuntimeAuthority>> {
        self.authority
            .get()
            .filter(|authority| authority.available)
            .ok_or_else(|| anyhow::anyhow!("task_store_canonical_authority_unavailable"))
    }

    fn validate_claim_authority(&self, claim: &ScheduledTaskClaim) -> Result<()> {
        claim.validate_store_authority(self.runtime_authority()?)
    }

    fn require_mutation_authority(&self, operation: &str) -> Result<()> {
        if self.mutation_authority != TaskStoreMutationAuthority::WritableOwner {
            anyhow::bail!("scheduled_task_store_write_authority_required:{operation}");
        }
        Ok(())
    }

    /// Retire the pre-SQLite `scheduled_tasks.json` owner without replaying an
    /// ambiguous effect.
    ///
    /// * `running` and already-due `pending` rows are quarantined as unknown;
    /// * old terminal labels become metadata-only historical reports, never a
    ///   canonical completion receipt;
    /// * a well-formed future `pending` row becomes `review_required`, never a
    ///   claimable task. Bootstrap must stage it through ReviewWorkflow.
    pub fn migrate_legacy_json_if_present(
        &self,
        legacy_path: &Path,
    ) -> Result<LegacyScheduledTaskQuarantineReport> {
        let mut conn = self.lock_writable_connection("migrate_legacy_json_if_present")?;
        let opened = match OpenedLegacyScheduledTaskSource::open(legacy_path) {
            Ok(opened) => opened,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
            {
                return Ok(LegacyScheduledTaskQuarantineReport::absent())
            }
            Err(error) => return Err(error),
        };
        let source_byte_len = opened.byte_len;
        let bytes = opened.bytes.as_deref();
        let source_digest = opened.source_digest.clone();
        let existing_cutoff = conn
            .query_row(
                "SELECT migration_cutoff_at FROM legacy_scheduled_task_sources
                 WHERE source_digest = ?1",
                params![source_digest],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let migration_cutoff = match existing_cutoff {
            Some(value) => DateTime::parse_from_rfc3339(&value)
                .context("legacy scheduled task migration cutoff is corrupt")?
                .with_timezone(&chrono::Utc),
            None => chrono::Utc::now(),
        };
        let migration_cutoff_text = migration_cutoff.to_rfc3339();
        let (rows, item_count, source_reason_code, source_malformed) = match bytes {
            None => (Vec::new(), 0, "legacy_source_oversized_unknown", true),
            Some(bytes) => match serde_json::from_slice::<serde_json::Value>(bytes) {
                Ok(serde_json::Value::Array(items))
                    if items.len() <= MAX_LEGACY_SCHEDULED_TASK_ITEMS =>
                {
                    let item_count = items.len();
                    let rows = items
                        .iter()
                        .enumerate()
                        .map(|(ordinal, item)| {
                            classify_legacy_scheduled_task(
                                &source_digest,
                                ordinal,
                                item,
                                migration_cutoff,
                                &migration_cutoff_text,
                            )
                        })
                        .collect::<Vec<_>>();
                    let reason = if rows.is_empty() {
                        "legacy_source_empty_retired"
                    } else {
                        "legacy_source_classified_without_replay"
                    };
                    (rows, item_count, reason, false)
                }
                Ok(serde_json::Value::Array(_)) => (
                    Vec::new(),
                    0,
                    "legacy_source_item_bound_exceeded_unknown",
                    true,
                ),
                Ok(_) | Err(_) => (Vec::new(), 0, "legacy_source_malformed_unknown", true),
            },
        };
        let review_required_count = rows
            .iter()
            .filter(|row| row.effect_state == "review_required")
            .count();
        let historical_count = rows
            .iter()
            .filter(|row| row.effect_state.starts_with("reported_"))
            .count();
        let quarantined_count = rows
            .iter()
            .filter(|row| row.effect_state == "unknown")
            .count()
            + usize::from(source_malformed);
        let evidence_file_name =
            legacy_evidence_file_name(legacy_path, &source_digest, quarantined_count > 0)?;
        let evidence_path = legacy_path.with_file_name(&evidence_file_name);
        let now = chrono::Utc::now().to_rfc3339();

        {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let existing = tx
                .query_row(
                    "SELECT source_byte_len, item_count, review_required_count,
                            historical_count, quarantine_count, source_reason_code,
                            evidence_file_name, migration_cutoff_at
                     FROM legacy_scheduled_task_sources WHERE source_digest = ?1",
                    params![source_digest],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, String>(7)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((
                existing_byte_len,
                existing_item_count,
                existing_review_required_count,
                existing_historical_count,
                existing_quarantine_count,
                existing_reason,
                existing_evidence_name,
                existing_cutoff,
            )) = existing
            {
                if existing_byte_len != source_byte_len as i64
                    || existing_item_count != item_count as i64
                    || existing_review_required_count != review_required_count as i64
                    || existing_historical_count != historical_count as i64
                    || existing_quarantine_count != quarantined_count as i64
                    || existing_reason != source_reason_code
                    || existing_evidence_name != evidence_file_name
                    || existing_cutoff != migration_cutoff_text
                {
                    anyhow::bail!(
                        "legacy scheduled task migration journal conflicts with source metadata"
                    );
                }
                let existing_rows: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM legacy_scheduled_task_migration_records
                     WHERE source_digest = ?1",
                    params![source_digest],
                    |row| row.get(0),
                )?;
                if existing_rows != rows.len() as i64 {
                    anyhow::bail!("legacy scheduled task migration journal is incomplete");
                }
            } else {
                tx.execute(
                    "INSERT INTO legacy_scheduled_task_sources (
                        source_digest, source_byte_len, item_count, review_required_count,
                        historical_count, quarantine_count, source_reason_code,
                        evidence_file_name, migration_cutoff_at, migrated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        source_digest,
                        source_byte_len as i64,
                        item_count as i64,
                        review_required_count as i64,
                        historical_count as i64,
                        quarantined_count as i64,
                        source_reason_code,
                        evidence_file_name,
                        migration_cutoff_text,
                        now,
                    ],
                )?;
                for row in &rows {
                    tx.execute(
                        "INSERT INTO legacy_scheduled_task_migration_records (
                            source_digest, source_ordinal, item_digest,
                            legacy_task_id_digest, legacy_status, reason_code,
                            effect_state, terminal_detail_digest, migrated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        params![
                            source_digest,
                            row.ordinal as i64,
                            row.item_digest,
                            row.legacy_task_id_digest,
                            row.legacy_status,
                            row.reason_code,
                            row.effect_state,
                            row.terminal_detail_digest,
                            now,
                        ],
                    )?;
                }
            }
            tx.commit()?;
        }

        retire_legacy_source_file(legacy_path, &evidence_path, &opened)?;
        Ok(LegacyScheduledTaskQuarantineReport {
            source_digest: Some(source_digest),
            item_count,
            review_required_count,
            historical_count,
            quarantined_count,
            source_malformed,
            evidence_path: Some(evidence_path),
        })
    }

    /// Reconstruct review-required candidates from the retained evidence after
    /// a crash between source retirement and Proposal staging.
    pub fn pending_legacy_review_candidates(
        &self,
        evidence_directory: &Path,
    ) -> Result<Vec<LegacyScheduledTaskReviewCandidate>> {
        let pending = {
            let conn = self.lock_writable_connection("pending_legacy_review_candidates")?;
            let mut statement = conn.prepare(
                "SELECT r.source_digest, r.source_ordinal, r.item_digest,
                        s.evidence_file_name, s.migration_cutoff_at
                 FROM legacy_scheduled_task_migration_records r
                 JOIN legacy_scheduled_task_sources s
                   ON s.source_digest = r.source_digest
                 WHERE r.effect_state = 'review_required'
                   AND r.review_proposal_id IS NULL
                 ORDER BY r.source_digest, r.source_ordinal",
            )?;
            let loaded = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            loaded
        };
        let mut candidates = Vec::with_capacity(pending.len());
        for (source_digest, ordinal, expected_item_digest, evidence_name, cutoff) in pending {
            if ordinal < 0 || Path::new(&evidence_name).components().count() != 1 {
                anyhow::bail!("legacy scheduled task migration metadata is invalid");
            }
            let evidence_path = evidence_directory.join(&evidence_name);
            let opened = OpenedLegacyScheduledTaskSource::open(&evidence_path)?;
            if opened.byte_len > MAX_LEGACY_SCHEDULED_TASK_BYTES
                || opened.source_digest != source_digest
            {
                anyhow::bail!("legacy scheduled task evidence failed integrity validation");
            }
            harden_legacy_evidence_permissions(&opened.file, &evidence_path)?;
            let bytes = opened.bytes.ok_or_else(|| {
                anyhow::anyhow!("legacy scheduled task evidence exceeds the review bound")
            })?;
            let items = serde_json::from_slice::<serde_json::Value>(&bytes)?;
            let items = items
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("legacy scheduled task evidence is not an array"))?;
            let item = items
                .get(ordinal as usize)
                .ok_or_else(|| anyhow::anyhow!("legacy scheduled task evidence item is missing"))?;
            let cutoff = DateTime::parse_from_rfc3339(&cutoff)?.with_timezone(&chrono::Utc);
            let row = classify_legacy_scheduled_task(
                &source_digest,
                ordinal as usize,
                item,
                cutoff,
                &cutoff.to_rfc3339(),
            );
            if row.item_digest != expected_item_digest || row.effect_state != "review_required" {
                anyhow::bail!("legacy scheduled task review candidate no longer matches evidence");
            }
            candidates.push(row.review_candidate.ok_or_else(|| {
                anyhow::anyhow!("legacy scheduled task review candidate payload is unavailable")
            })?);
        }
        Ok(candidates)
    }

    pub fn mark_legacy_review_proposal_staged(
        &self,
        candidate: &LegacyScheduledTaskReviewCandidate,
        proposal_id: &str,
    ) -> Result<bool> {
        validate_reference("legacy review proposal id", proposal_id)?;
        let conn = self.lock_writable_connection("mark_legacy_review_proposal_staged")?;
        let changed = conn.execute(
            "UPDATE legacy_scheduled_task_migration_records
             SET review_proposal_id = ?1
             WHERE source_digest = ?2 AND source_ordinal = ?3
               AND item_digest = ?4 AND effect_state = 'review_required'
               AND (review_proposal_id IS NULL OR review_proposal_id = ?1)",
            params![
                proposal_id,
                candidate.source_digest,
                candidate.source_ordinal as i64,
                candidate.item_digest,
            ],
        )?;
        if changed == 1 {
            return Ok(true);
        }
        Ok(conn.query_row(
            "SELECT COUNT(*) = 1 FROM legacy_scheduled_task_migration_records
             WHERE source_digest = ?1 AND source_ordinal = ?2 AND item_digest = ?3
               AND effect_state = 'review_required' AND review_proposal_id = ?4",
            params![
                candidate.source_digest,
                candidate.source_ordinal as i64,
                candidate.item_digest,
                proposal_id,
            ],
            |row| row.get(0),
        )?)
    }

    fn init_tables(&self) -> Result<()> {
        let mut conn = self.lock_writable_connection("init_tables")?;
        let tx = conn.transaction()?;
        let existing_schema_version = task_store_schema_version(&tx)?.unwrap_or(0);
        if existing_schema_version > TASK_STORE_SCHEMA_VERSION {
            anyhow::bail!("task store schema is newer than this OpenLife build");
        }
        // Capture the source schema's reportable execution facts before any
        // backfill or constraint migration can normalize them. The snapshot is
        // process memory only and deliberately excludes task prose, provider
        // payloads, request identifiers, claim tokens, and grants.
        let pre_authority_truth =
            capture_pre_authority_task_store_truth(&tx, existing_schema_version)?;
        let retiring_pre_authority_truth = (1..13).contains(&existing_schema_version);
        let purge_marker_complete = task_store_table_exists(&tx, "task_store_metadata")?
            && task_store_metadata_value(&tx, TASK_STORE_PRE_V13_PURGE_COMPLETE_METADATA_KEY)?
                .as_deref()
                == Some("complete");
        let physical_purge_required =
            retiring_pre_authority_truth || (existing_schema_version > 0 && !purge_marker_complete);
        if retiring_pre_authority_truth {
            // Retire untrusted pre-authority product rows before any backfill,
            // constraint rebuild, or status normalization can reinterpret or
            // copy them. The metadata-only snapshot above is the sole input to
            // the later quarantine journal in this same transaction.
            retire_pre_authority_task_store_rows(&tx)?;
        }
        // v11 adds exact tool-receipt identity. v12 adds the immutable prepared
        // provider-request digest and normalizes the deterministic local target
        // to the adapter's exact-text digest. v13 binds the entire store to an
        // OS-owned secret plus canonical filesystem slot. v14 binds attempts to
        // one process epoch so restart recovery never depends on wall-clock or
        // lease expiry. v15 separates that OS-process epoch from the exact
        // writable owner generation and binds both onto attempts and receipts.
        // Rows that predate
        // these bindings are retained only as quarantined/legacy evidence and
        // cannot silently regain execution or exact provider-truth credit.
        let migrating_from_legacy =
            existing_schema_version < EXACT_PROVIDER_PROVENANCE_SCHEMA_VERSION;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                due_date TEXT,
                priority TEXT NOT NULL DEFAULT 'medium',
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                completed_at TEXT,
                source_run_id TEXT,
                source_proposal_id TEXT,
                action_type TEXT NOT NULL DEFAULT 'scheduled_task',
                attempt_count INTEGER NOT NULL DEFAULT 0,
                claim_token TEXT,
                lease_expires_at TEXT,
                last_error TEXT,
                result_digest TEXT,
                result_ref TEXT,
                eligible_at TEXT,
                provider_grant_id TEXT NOT NULL DEFAULT '',
                provider_policy_version TEXT NOT NULL DEFAULT '',
                provider_data_route TEXT NOT NULL DEFAULT '',
                provider_reason_code TEXT NOT NULL DEFAULT '',
                provider_subject_digest TEXT NOT NULL DEFAULT '',
                provider_payload_purpose TEXT NOT NULL DEFAULT '',
                provider_payload_contract_digest TEXT NOT NULL DEFAULT '',
                provider_source_ref_digest TEXT NOT NULL DEFAULT '',
                provider_schedule_digest TEXT NOT NULL DEFAULT '',
                provider_grant_scope TEXT NOT NULL DEFAULT '',
                provider_grant_expires_at TEXT,
                provider_target_digest TEXT,
                model_target_digest TEXT,
                review_snapshot_digest TEXT,
                review_dispatch_claim_digest TEXT,
                provider_policy_decision_digest TEXT NOT NULL DEFAULT ''
            )",
            [],
        )?;
        crate::sqlite_migration::ensure_column(&tx, "tasks", "source_proposal_id", "TEXT")?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "tasks",
            "action_type",
            "TEXT NOT NULL DEFAULT 'scheduled_task'",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "tasks",
            "attempt_count",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        crate::sqlite_migration::ensure_column(&tx, "tasks", "claim_token", "TEXT")?;
        crate::sqlite_migration::ensure_column(&tx, "tasks", "lease_expires_at", "TEXT")?;
        crate::sqlite_migration::ensure_column(&tx, "tasks", "last_error", "TEXT")?;
        crate::sqlite_migration::ensure_column(&tx, "tasks", "result_digest", "TEXT")?;
        crate::sqlite_migration::ensure_column(&tx, "tasks", "result_ref", "TEXT")?;
        crate::sqlite_migration::ensure_column(&tx, "tasks", "eligible_at", "TEXT")?;
        for (column, definition) in [
            ("provider_grant_id", "TEXT NOT NULL DEFAULT ''"),
            ("provider_policy_version", "TEXT NOT NULL DEFAULT ''"),
            ("provider_data_route", "TEXT NOT NULL DEFAULT ''"),
            ("provider_reason_code", "TEXT NOT NULL DEFAULT ''"),
            ("provider_subject_digest", "TEXT NOT NULL DEFAULT ''"),
            ("provider_payload_purpose", "TEXT NOT NULL DEFAULT ''"),
            (
                "provider_payload_contract_digest",
                "TEXT NOT NULL DEFAULT ''",
            ),
            ("provider_source_ref_digest", "TEXT NOT NULL DEFAULT ''"),
            ("provider_schedule_digest", "TEXT NOT NULL DEFAULT ''"),
            ("provider_grant_scope", "TEXT NOT NULL DEFAULT ''"),
            ("provider_grant_expires_at", "TEXT"),
            ("provider_target_digest", "TEXT"),
            ("model_target_digest", "TEXT"),
            ("review_snapshot_digest", "TEXT"),
            ("review_dispatch_claim_digest", "TEXT"),
            (
                "provider_policy_decision_digest",
                "TEXT NOT NULL DEFAULT ''",
            ),
        ] {
            crate::sqlite_migration::ensure_column(&tx, "tasks", column, definition)?;
        }
        let local_provider_grant_migrations =
            backfill_task_provider_grants(&tx, migrating_from_legacy, existing_schema_version)?;
        tx.execute(
            "UPDATE tasks SET eligible_at = COALESCE(
                strftime('%Y-%m-%dT%H:%M:%f+00:00', due_date), due_date
             )
             WHERE eligible_at IS NULL AND due_date IS NOT NULL AND due_date != ''",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS legacy_scheduled_task_sources (
                source_digest TEXT PRIMARY KEY,
                source_byte_len INTEGER NOT NULL,
                item_count INTEGER NOT NULL,
                review_required_count INTEGER NOT NULL,
                historical_count INTEGER NOT NULL,
                quarantine_count INTEGER NOT NULL,
                source_reason_code TEXT NOT NULL,
                evidence_file_name TEXT NOT NULL,
                migration_cutoff_at TEXT NOT NULL,
                migrated_at TEXT NOT NULL
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS legacy_scheduled_task_migration_records (
                source_digest TEXT NOT NULL,
                source_ordinal INTEGER NOT NULL,
                item_digest TEXT NOT NULL,
                legacy_task_id_digest TEXT,
                legacy_status TEXT NOT NULL,
                reason_code TEXT NOT NULL,
                effect_state TEXT NOT NULL CHECK(effect_state IN (
                    'unknown', 'review_required', 'reported_completed',
                    'reported_failed', 'reported_cancelled'
                )),
                terminal_detail_digest TEXT,
                review_proposal_id TEXT,
                migrated_at TEXT NOT NULL,
                PRIMARY KEY(source_digest, source_ordinal),
                FOREIGN KEY(source_digest) REFERENCES legacy_scheduled_task_sources(source_digest)
                    ON DELETE RESTRICT
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS scheduler_attempts (
                attempt_id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                claim_token TEXT NOT NULL UNIQUE,
                attempt_number INTEGER NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'claimed', 'executing', 'completed', 'failed', 'unknown',
                    'pre_dispatch_failed', 'pre_dispatch_timeout', 'expired_before_execution',
                    'reconciled_retry_safe', 'reconciled_failed', 'reconciled_completed'
                )),
                provider_grant_id TEXT NOT NULL,
                policy_version TEXT NOT NULL,
                data_route TEXT NOT NULL CHECK(data_route IN ('local_only', 'policy_allowed')),
                policy_reason_code TEXT NOT NULL,
                provider_subject_digest TEXT NOT NULL,
                provider_payload_purpose TEXT NOT NULL,
                provider_payload_contract_digest TEXT NOT NULL,
                provider_source_ref_digest TEXT NOT NULL,
                provider_provenance_state TEXT NOT NULL DEFAULT 'exact' CHECK(
                    provider_provenance_state IN ('exact', 'legacy_unavailable')
                ),
                migration_associated_grant_id TEXT,
                process_epoch_id TEXT NOT NULL,
                writer_owner_generation_id TEXT NOT NULL,
                claimed_at TEXT NOT NULL,
                execution_started_at TEXT,
                finished_at TEXT,
                agent_run_ref_digest TEXT,
                error_digest TEXT,
                reconciliation_evidence_digest TEXT,
                reconciliation_issuer TEXT,
                reconciliation_evidence_kind TEXT,
                reconciliation_evidence_ref TEXT,
                reconciled_at TEXT,
                UNIQUE(task_id, attempt_number),
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
            )",
            [],
        )?;
        rename_column_if_needed(
            &tx,
            "scheduler_attempts",
            "policy_decision_id",
            "provider_grant_id",
        )?;
        for (column, definition) in [
            ("provider_subject_digest", "TEXT NOT NULL DEFAULT ''"),
            ("provider_payload_purpose", "TEXT NOT NULL DEFAULT ''"),
            (
                "provider_payload_contract_digest",
                "TEXT NOT NULL DEFAULT ''",
            ),
            ("provider_source_ref_digest", "TEXT NOT NULL DEFAULT ''"),
            (
                "provider_provenance_state",
                "TEXT NOT NULL DEFAULT 'legacy_unavailable'",
            ),
            ("migration_associated_grant_id", "TEXT"),
            (
                "process_epoch_id",
                "TEXT NOT NULL DEFAULT 'legacy_process_epoch_unknown'",
            ),
            (
                "writer_owner_generation_id",
                "TEXT NOT NULL DEFAULT 'legacy_writer_owner_generation_unknown'",
            ),
            ("reconciliation_issuer", "TEXT"),
            ("reconciliation_evidence_kind", "TEXT"),
            ("reconciliation_evidence_ref", "TEXT"),
        ] {
            crate::sqlite_migration::ensure_column(&tx, "scheduler_attempts", column, definition)?;
        }
        backfill_attempt_provider_grants(
            &tx,
            migrating_from_legacy,
            &local_provider_grant_migrations,
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS scheduler_provider_receipts (
                request_id TEXT PRIMARY KEY,
                attempt_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                claim_token TEXT NOT NULL,
                process_epoch_id TEXT NOT NULL,
                writer_owner_generation_id TEXT NOT NULL,
                provider_grant_id TEXT NOT NULL,
                migration_associated_grant_id TEXT,
                provider_digest TEXT NOT NULL,
                model_digest TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'started', 'completed', 'failed', 'remote_unknown'
                )),
                started_at TEXT NOT NULL,
                finished_at TEXT,
                error_digest TEXT,
                simulated INTEGER,
                policy_evidence_state TEXT NOT NULL CHECK(policy_evidence_state IN (
                    'exact', 'legacy_unavailable'
                )),
                policy_evidence_digest TEXT,
                subject_scope_digest TEXT,
                payload_purpose TEXT,
                unfiltered_payload_digest TEXT,
                context_manifest_digest TEXT,
                prepared_envelope_digest TEXT,
                prepared_request_digest TEXT,
                network_policy_decision_digest TEXT,
                FOREIGN KEY(attempt_id) REFERENCES scheduler_attempts(attempt_id) ON DELETE CASCADE,
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
            )",
            [],
        )?;
        rename_column_if_needed(
            &tx,
            "scheduler_provider_receipts",
            "policy_decision_id",
            "provider_grant_id",
        )?;
        for (column, definition) in [
            (
                "policy_evidence_state",
                "TEXT NOT NULL DEFAULT 'legacy_unavailable'",
            ),
            ("policy_evidence_digest", "TEXT"),
            ("subject_scope_digest", "TEXT"),
            ("payload_purpose", "TEXT"),
            ("unfiltered_payload_digest", "TEXT"),
            ("context_manifest_digest", "TEXT"),
            ("prepared_envelope_digest", "TEXT"),
            ("prepared_request_digest", "TEXT"),
            ("network_policy_decision_digest", "TEXT"),
            ("migration_associated_grant_id", "TEXT"),
            (
                "process_epoch_id",
                "TEXT NOT NULL DEFAULT 'legacy_process_epoch_unknown'",
            ),
            (
                "writer_owner_generation_id",
                "TEXT NOT NULL DEFAULT 'legacy_writer_owner_generation_unknown'",
            ),
        ] {
            crate::sqlite_migration::ensure_column(
                &tx,
                "scheduler_provider_receipts",
                column,
                definition,
            )?;
        }
        preserve_provider_receipt_provenance(
            &tx,
            migrating_from_legacy || !local_provider_grant_migrations.is_empty(),
        )?;
        if existing_schema_version < 7 {
            migrate_provider_receipt_remote_unknown_status(&tx)?;
        }
        crate::sqlite_migration::ensure_column(
            &tx,
            "scheduler_attempts",
            "reconciliation_evidence_digest",
            "TEXT",
        )?;
        crate::sqlite_migration::ensure_column(&tx, "scheduler_attempts", "reconciled_at", "TEXT")?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS scheduler_tool_dispatches (
                dispatch_id TEXT PRIMARY KEY,
                attempt_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                claim_token TEXT NOT NULL,
                process_epoch_id TEXT NOT NULL,
                writer_owner_generation_id TEXT NOT NULL,
                dispatch_index INTEGER NOT NULL,
                manifest_digest TEXT NOT NULL,
                manifest_contract_digest TEXT,
                tool_digest TEXT NOT NULL,
                input_hash TEXT,
                input_length_bytes INTEGER,
                source_run_ref_digest TEXT,
                identity_state TEXT NOT NULL DEFAULT 'legacy_unavailable' CHECK(
                    identity_state IN ('exact', 'legacy_unavailable')
                ),
                status TEXT NOT NULL CHECK(status IN ('started', 'returned', 'unknown')),
                observed_at TEXT NOT NULL,
                receipt_started_at TEXT,
                dispatched_at TEXT,
                finished_at TEXT,
                tool_receipt_id TEXT,
                request_digest TEXT,
                action_effect TEXT,
                idempotency_contract TEXT,
                dispatch_kind TEXT,
                dispatch_attempt_count INTEGER,
                transport_status TEXT,
                effect_status TEXT,
                execution_outcome TEXT,
                transport_observed_at TEXT,
                UNIQUE(attempt_id, dispatch_index),
                FOREIGN KEY(attempt_id) REFERENCES scheduler_attempts(attempt_id) ON DELETE CASCADE,
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
            )",
            [],
        )?;
        for (column, sql_type) in [
            ("manifest_contract_digest", "TEXT"),
            ("input_hash", "TEXT"),
            ("input_length_bytes", "INTEGER"),
            (
                "identity_state",
                "TEXT NOT NULL DEFAULT 'legacy_unavailable'",
            ),
            ("receipt_started_at", "TEXT"),
            ("dispatched_at", "TEXT"),
            ("tool_receipt_id", "TEXT"),
            ("request_digest", "TEXT"),
            ("action_effect", "TEXT"),
            ("idempotency_contract", "TEXT"),
            ("dispatch_kind", "TEXT"),
            ("dispatch_attempt_count", "INTEGER"),
            ("transport_status", "TEXT"),
            ("effect_status", "TEXT"),
            ("execution_outcome", "TEXT"),
            ("transport_observed_at", "TEXT"),
            (
                "process_epoch_id",
                "TEXT NOT NULL DEFAULT 'legacy_process_epoch_unknown'",
            ),
            (
                "writer_owner_generation_id",
                "TEXT NOT NULL DEFAULT 'legacy_writer_owner_generation_unknown'",
            ),
        ] {
            crate::sqlite_migration::ensure_column(
                &tx,
                "scheduler_tool_dispatches",
                column,
                sql_type,
            )?;
        }
        migrate_scheduler_attempt_policy_allowed_route(&tx)?;
        // The policy-route constraint rebuild predates the v15 runtime
        // identity split and recreates all three receipt tables. Restore both
        // bindings before any exact-identity validator queries those columns.
        for table in [
            "scheduler_attempts",
            "scheduler_provider_receipts",
            "scheduler_tool_dispatches",
        ] {
            crate::sqlite_migration::ensure_column(
                &tx,
                table,
                "process_epoch_id",
                "TEXT NOT NULL DEFAULT 'legacy_process_epoch_unknown'",
            )?;
            crate::sqlite_migration::ensure_column(
                &tx,
                table,
                "writer_owner_generation_id",
                "TEXT NOT NULL DEFAULT 'legacy_writer_owner_generation_unknown'",
            )?;
        }
        migrate_writer_owner_generation_v15(&tx, existing_schema_version)?;
        migrate_tool_dispatch_identity_v11(&tx, existing_schema_version)?;
        // Older constraint-rebuild migrations intentionally know nothing about
        // the v12 column, so enforce it after every possible rebuild.
        crate::sqlite_migration::ensure_column(
            &tx,
            "scheduler_provider_receipts",
            "prepared_request_digest",
            "TEXT",
        )?;
        migrate_provider_prepared_request_binding_v12(&tx, existing_schema_version)?;
        // Older constraint-rebuild migrations intentionally predate the v15
        // runtime identity split, so enforce both facts after every possible
        // table replacement.
        for table in [
            "scheduler_attempts",
            "scheduler_provider_receipts",
            "scheduler_tool_dispatches",
        ] {
            crate::sqlite_migration::ensure_column(
                &tx,
                table,
                "process_epoch_id",
                "TEXT NOT NULL DEFAULT 'legacy_process_epoch_unknown'",
            )?;
            crate::sqlite_migration::ensure_column(
                &tx,
                table,
                "writer_owner_generation_id",
                "TEXT NOT NULL DEFAULT 'legacy_writer_owner_generation_unknown'",
            )?;
        }
        preserve_provider_receipt_provenance(&tx, false)?;
        validate_exact_scheduler_runtime_identities(&tx)?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS scheduler_provider_grant_consumptions (
                grant_id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                attempt_id TEXT NOT NULL UNIQUE,
                consumed_at TEXT NOT NULL,
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE RESTRICT,
                FOREIGN KEY(attempt_id) REFERENCES scheduler_attempts(attempt_id)
                    ON DELETE RESTRICT
            )",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)",
            [],
        )?;
        tx.execute("DROP INDEX IF EXISTS idx_tasks_due_date", [])?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_due_claim
             ON tasks(status, eligible_at, created_at, id)",
            [],
        )?;
        tx.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_source_proposal
             ON tasks(source_proposal_id) WHERE source_proposal_id IS NOT NULL",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_scheduler_attempts_task_status
             ON scheduler_attempts(task_id, status, attempt_number DESC)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_scheduler_provider_attempt_status
             ON scheduler_provider_receipts(attempt_id, status)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_scheduler_tool_attempt_status
             ON scheduler_tool_dispatches(attempt_id, status)",
            [],
        )?;
        tx.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_scheduler_tool_receipt_id
             ON scheduler_tool_dispatches(tool_receipt_id)
             WHERE tool_receipt_id IS NOT NULL",
            [],
        )?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS task_store_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             ) WITHOUT ROWID;",
        )?;
        let database_slot = self
            .database_slot
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("task_store_database_slot_unavailable"))?;
        let owner_lock_identity_material = self
            .owner_lock_identity_material
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("task_store_owner_lock_identity_unavailable"))?;
        let bound_authority = bind_task_store_persistence_authority(
            &tx,
            database_slot,
            owner_lock_identity_material,
            self.preauthenticated_store_identity.as_deref(),
            existing_schema_version,
            &pre_authority_truth,
            self.writer_owner_generation_id,
        )?;
        // Recreate the immutable metadata guards transactionally so databases
        // written by an older build gain protection for the purge marker too.
        tx.execute_batch(
            "DROP TRIGGER IF EXISTS task_store_authority_metadata_immutable_update;
             DROP TRIGGER IF EXISTS task_store_authority_metadata_immutable_delete;",
        )?;
        if physical_purge_required {
            // A stale or attacker-supplied marker from a pre-authority schema
            // cannot bypass the post-commit physical purge.
            tx.execute(
                "DELETE FROM task_store_metadata WHERE key = ?1",
                [TASK_STORE_PRE_V13_PURGE_COMPLETE_METADATA_KEY],
            )?;
        } else {
            tx.execute(
                "INSERT OR IGNORE INTO task_store_metadata (key, value) VALUES (?1, 'complete')",
                [TASK_STORE_PRE_V13_PURGE_COMPLETE_METADATA_KEY],
            )?;
        }
        tx.execute_batch(
            "CREATE TRIGGER task_store_authority_metadata_immutable_update
             BEFORE UPDATE ON task_store_metadata
             WHEN OLD.key IN (
                'canonical_task_store_identity_v1',
                'canonical_task_store_slot_verifier_v1',
                'canonical_task_store_owner_lock_verifier_v1',
                'pre_v13_physical_purge_complete_v1'
             )
             BEGIN
                SELECT RAISE(ABORT, 'canonical TaskStore authority metadata is immutable');
             END;
             CREATE TRIGGER task_store_authority_metadata_immutable_delete
             BEFORE DELETE ON task_store_metadata
             WHEN OLD.key IN (
                'canonical_task_store_identity_v1',
                'canonical_task_store_slot_verifier_v1',
                'canonical_task_store_owner_lock_verifier_v1',
                'pre_v13_physical_purge_complete_v1'
             )
             BEGIN
                SELECT RAISE(ABORT, 'canonical TaskStore authority metadata is immutable');
             END;",
        )?;
        {
            let mut statement = tx.prepare("PRAGMA foreign_key_check")?;
            let mut violations = statement.query([])?;
            if violations.next()?.is_some() {
                anyhow::bail!("task store migration failed its foreign-key check");
            }
        }
        crate::sqlite_migration::record_schema_version(
            &tx,
            "task_store",
            TASK_STORE_SCHEMA_VERSION,
        )?;
        tx.commit()?;
        if physical_purge_required {
            complete_task_store_physical_purge(&mut conn)?;
        }
        self.authority
            .set(Arc::new(bound_authority))
            .map_err(|_| anyhow::anyhow!("task_store_authority_initialized_twice"))?;
        Ok(())
    }

    pub fn create_task_idempotent(&self, task: &ScheduledTask) -> Result<bool> {
        validate_new_task(task)?;
        let mut conn = self.lock_writable_connection("create_task_idempotent")?;
        let tx = conn.transaction()?;
        let changed = insert_task(&tx, task)?;
        if changed == 1 {
            tx.commit()?;
            self.remember_live_reviewed_cloud_authority(task)?;
            return Ok(true);
        }
        let existing_by_id = tx
            .query_row(
                "SELECT id FROM tasks WHERE id = ?1",
                params![task.id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let existing_by_proposal = match task.source_proposal_id.as_deref() {
            Some(source_proposal_id) => tx
                .query_row(
                    "SELECT id FROM tasks WHERE source_proposal_id = ?1",
                    params![source_proposal_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?,
            None => None,
        };
        let canonical_id = match (existing_by_id, existing_by_proposal) {
            (Some(by_id), Some(by_proposal)) if by_id != by_proposal => {
                anyhow::bail!(
                    "scheduled task idempotency keys resolve to different canonical tasks"
                )
            }
            (Some(canonical_id), _) | (_, Some(canonical_id)) => canonical_id,
            (None, None) => {
                anyhow::bail!("scheduled task insert was ignored without a canonical owner")
            }
        };
        let mut existing = load_task(&tx, &canonical_id)?;
        if task.provider_grant.data_route == ProviderDataRoute::PolicyAllowed
            && task.provider_grant_authority
                == ScheduledTaskGrantAuthorityProof::CanonicalReviewedPolicyRoute
        {
            existing.provider_grant_authority =
                ScheduledTaskGrantAuthorityProof::CanonicalReviewedPolicyRoute;
        }
        if &existing != task {
            anyhow::bail!(
                "scheduled task idempotency key was reused with a different canonical payload"
            );
        }
        tx.commit()?;
        self.remember_live_reviewed_cloud_authority(task)?;
        Ok(false)
    }

    /// Restore cloud execution authority from canonical ReviewWorkflow
    /// materialization evidence after a process restart. Public TaskStore rows
    /// and self-consistent digests cannot construct the proof argument.
    pub fn restore_reviewed_cloud_authority(
        &self,
        proof: &crate::agent::MaterializedReviewAcceptanceSnapshot,
    ) -> Result<()> {
        let conn = self.lock_writable_connection("restore_reviewed_cloud_authority")?;
        proof.validate()?;
        let proposal = proof.proposal();
        if proposal.proposal_type != crate::agent::ProposalType::ScheduledTask {
            anyhow::bail!("review materialization is not a scheduled task");
        }
        let task = conn
            .query_row(
                "SELECT id, title, description, due_date, priority, status, created_at,
                        completed_at, source_run_id, source_proposal_id, action_type,
                        attempt_count, claim_token, lease_expires_at, last_error,
                        result_digest, result_ref, provider_grant_id,
                        provider_policy_version, provider_data_route, provider_reason_code,
                        provider_subject_digest, provider_payload_purpose,
                        provider_payload_contract_digest, provider_source_ref_digest,
                        provider_schedule_digest, provider_grant_scope,
                        provider_grant_expires_at, provider_target_digest,
                        model_target_digest, review_snapshot_digest,
                        review_dispatch_claim_digest, provider_policy_decision_digest
                 FROM tasks WHERE id = ?1 AND source_proposal_id = ?1",
                [proposal.id.as_str()],
                map_row,
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("reviewed scheduled task is not canonical"))?;
        if task.provider_grant.data_route != ProviderDataRoute::PolicyAllowed
            || task.provider_grant.review_snapshot_digest.as_deref()
                != Some(proof.proposal_snapshot_digest())
            || task.provider_grant.review_dispatch_claim_digest.as_deref()
                != Some(proof.dispatch_claim_digest())
        {
            anyhow::bail!("scheduled cloud task is not bound to canonical review evidence");
        }
        task.provider_grant.validate_for_task(&task)?;
        self.reviewed_cloud_authorities
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?
            .insert(task.id, task.provider_grant.grant_id);
        Ok(())
    }

    /// A persisted cloud-shaped task that cannot be cross-proven by
    /// ProposalStore is not executable. Move it to an explicit fresh-review
    /// state instead of silently downgrading it to local or trusting its row.
    pub fn quarantine_unproven_reviewed_cloud_task(&self, task_id: &str) -> Result<bool> {
        validate_reference("scheduled task id", task_id)?;
        let conn = self.lock_writable_connection("quarantine_unproven_reviewed_cloud_task")?;
        Ok(conn.execute(
            "UPDATE tasks
             SET status = 'review_required',
                 last_error = 'scheduled_cloud_authority_unproven_requires_fresh_review',
                 eligible_at = NULL,
                 claim_token = NULL,
                 lease_expires_at = NULL
             WHERE id = ?1 AND status = 'pending'
               AND provider_data_route = 'policy_allowed'",
            [task_id],
        )? == 1)
    }

    fn remember_live_reviewed_cloud_authority(&self, task: &ScheduledTask) -> Result<()> {
        if task.provider_grant.data_route != ProviderDataRoute::PolicyAllowed {
            return Ok(());
        }
        if task.provider_grant_authority
            != ScheduledTaskGrantAuthorityProof::CanonicalReviewedPolicyRoute
        {
            anyhow::bail!("scheduled cloud task lacks live reviewed authority");
        }
        self.reviewed_cloud_authorities
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?
            .insert(task.id.clone(), task.provider_grant.grant_id.clone());
        Ok(())
    }

    fn apply_runtime_reviewed_cloud_authority(&self, task: &mut ScheduledTask) -> Result<()> {
        if task.provider_grant.data_route != ProviderDataRoute::PolicyAllowed {
            return Ok(());
        }
        let authorized = self
            .reviewed_cloud_authorities
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?
            .get(&task.id)
            .is_some_and(|grant_id| grant_id == &task.provider_grant.grant_id);
        if authorized {
            task.provider_grant_authority =
                ScheduledTaskGrantAuthorityProof::CanonicalReviewedPolicyRoute;
        }
        Ok(())
    }

    pub fn list_tasks(&self, status: Option<&str>) -> Result<Vec<ScheduledTask>> {
        let mut tasks = {
            let conn = self.lock_connection()?;
            let query = if status.is_some() {
                "SELECT id, title, description, due_date, priority, status, created_at, completed_at,
                    source_run_id, source_proposal_id, action_type, attempt_count, claim_token,
                    lease_expires_at, last_error, result_digest, result_ref, provider_grant_id,
                    provider_policy_version, provider_data_route, provider_reason_code,
                    provider_subject_digest, provider_payload_purpose,
                    provider_payload_contract_digest, provider_source_ref_digest,
                    provider_schedule_digest, provider_grant_scope, provider_grant_expires_at,
                    provider_target_digest, model_target_digest, review_snapshot_digest,
                    review_dispatch_claim_digest, provider_policy_decision_digest
             FROM tasks WHERE status = ?1 ORDER BY created_at DESC"
            } else {
                "SELECT id, title, description, due_date, priority, status, created_at, completed_at,
                    source_run_id, source_proposal_id, action_type, attempt_count, claim_token,
                    lease_expires_at, last_error, result_digest, result_ref, provider_grant_id,
                    provider_policy_version, provider_data_route, provider_reason_code,
                    provider_subject_digest, provider_payload_purpose,
                    provider_payload_contract_digest, provider_source_ref_digest,
                    provider_schedule_digest, provider_grant_scope, provider_grant_expires_at,
                    provider_target_digest, model_target_digest, review_snapshot_digest,
                    review_dispatch_claim_digest, provider_policy_decision_digest
             FROM tasks ORDER BY created_at DESC"
            };
            let mut stmt = conn.prepare(query)?;
            let rows = if let Some(status) = status {
                stmt.query_map(params![status], map_row)?
            } else {
                stmt.query_map([], map_row)?
            };
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for task in &mut tasks {
            self.apply_runtime_reviewed_cloud_authority(task)?;
        }
        Ok(tasks)
    }

    /// Atomically claims one due task and creates its immutable execution attempt
    /// with deterministic policy provenance.
    pub fn claim_next_due(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        lease_duration: chrono::Duration,
    ) -> Result<Option<ScheduledTaskClaim>> {
        let mut conn = self.lock_writable_connection("claim_next_due")?;
        let tx = conn.transaction()?;
        let now_text = now.to_rfc3339();
        tx.execute(
            "UPDATE tasks
             SET status = 'failed', last_error = 'scheduled_cloud_grant_expired', eligible_at = NULL
             WHERE status = 'pending' AND provider_data_route = 'policy_allowed'
               AND (provider_grant_expires_at IS NULL OR provider_grant_expires_at <= ?1)",
            params![now_text],
        )?;
        let candidate = tx
            .query_row(
                "SELECT id, title, description, due_date, priority, status, created_at, completed_at,
                        source_run_id, source_proposal_id, action_type, attempt_count, claim_token,
                        lease_expires_at, last_error, result_digest, result_ref, provider_grant_id,
                        provider_policy_version, provider_data_route, provider_reason_code,
                        provider_subject_digest, provider_payload_purpose,
                        provider_payload_contract_digest, provider_source_ref_digest,
                        provider_schedule_digest, provider_grant_scope, provider_grant_expires_at,
                        provider_target_digest, model_target_digest, review_snapshot_digest,
                        review_dispatch_claim_digest, provider_policy_decision_digest
                 FROM tasks
                 WHERE status = 'pending' AND eligible_at IS NOT NULL AND eligible_at != ''
                   AND eligible_at <= ?1
                   AND (
                        provider_data_route = 'local_only'
                        OR NOT EXISTS (
                            SELECT 1 FROM scheduler_provider_grant_consumptions c
                            WHERE c.grant_id = tasks.provider_grant_id
                        )
                   )
                 ORDER BY eligible_at ASC, created_at ASC, id ASC LIMIT 1",
                params![now_text],
                map_row,
            )
            .optional()?;
        let Some(mut candidate) = candidate else {
            tx.commit()?;
            return Ok(None);
        };
        self.apply_runtime_reviewed_cloud_authority(&mut candidate)?;
        if candidate.provider_grant.data_route == ProviderDataRoute::PolicyAllowed
            && candidate.provider_grant_authority
                != ScheduledTaskGrantAuthorityProof::CanonicalReviewedPolicyRoute
        {
            anyhow::bail!(
                "scheduled cloud task lacks canonical ReviewWorkflow authority for this process"
            );
        }
        let attempt_number = candidate
            .attempt_count
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("scheduled task attempt counter overflow"))?;
        candidate.provider_grant.validate_for_task(&candidate)?;
        if candidate.provider_grant.is_expired_at(&now)? {
            anyhow::bail!("scheduled provider grant expired before claim");
        }
        let provider_grant = candidate.provider_grant.clone();
        let claim_token = uuid::Uuid::new_v4().to_string();
        let attempt_id = digest_parts(&[
            "scheduled_attempt_v1",
            &candidate.id,
            &attempt_number.to_string(),
        ]);
        let lease_expires_at = (now + lease_duration).to_rfc3339();
        let changed = tx.execute(
            "UPDATE tasks SET status = 'running', claim_token = ?1, lease_expires_at = ?2,
                    attempt_count = ?3, last_error = NULL
             WHERE id = ?4 AND status = 'pending' AND attempt_count = ?5",
            params![
                claim_token,
                lease_expires_at,
                attempt_number,
                candidate.id,
                candidate.attempt_count,
            ],
        )?;
        if changed != 1 {
            tx.commit()?;
            return Ok(None);
        }
        tx.execute(
            "INSERT INTO scheduler_attempts (
                attempt_id, task_id, claim_token, attempt_number, status,
                provider_grant_id, policy_version, data_route, policy_reason_code,
                provider_subject_digest, provider_payload_purpose,
                provider_payload_contract_digest, provider_source_ref_digest,
                provider_provenance_state, process_epoch_id,
                writer_owner_generation_id, claimed_at
             ) VALUES (?1, ?2, ?3, ?4, 'claimed', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                       'exact', ?13, ?14, ?15)",
            params![
                attempt_id,
                candidate.id,
                claim_token,
                attempt_number,
                provider_grant.grant_id,
                provider_grant.policy_version,
                provider_data_route_label(provider_grant.data_route),
                provider_grant.reason_code,
                provider_grant.subject_digest,
                provider_grant.payload_purpose.as_str(),
                provider_grant.payload_contract_digest,
                provider_grant.source_ref_digest,
                self.runtime_authority()?.process_epoch_id.to_string(),
                self.runtime_authority()?
                    .writer_owner_generation_id
                    .to_string(),
                now_text,
            ],
        )?;
        let mut task = load_task(&tx, &candidate.id)?;
        self.apply_runtime_reviewed_cloud_authority(&mut task)?;
        let authority = Arc::clone(self.runtime_authority()?);
        let sealed_claim = authority.seal_claim(&scheduled_claim_seal_material(
            &task,
            &claim_token,
            &attempt_id,
            attempt_number,
            &provider_grant,
            authority.process_epoch_id,
            authority.writer_owner_generation_id,
        )?)?;
        // Construct every fallible part of the in-process capability before the
        // transaction commits. A committed `running` attempt without a claim
        // capability would otherwise be stranded until process reconciliation.
        tx.commit()?;
        Ok(Some(ScheduledTaskClaim {
            task,
            claim_token,
            attempt_id,
            attempt_number,
            provider_grant,
            policy_authority_proof: ScheduledClaimAuthorityProof {
                store: authority,
                sealed_claim,
            },
        }))
    }

    /// Marks the execution boundary before any provider or tool can be entered.
    /// An explicit error before a receipt is recorded can still be reclaimed;
    /// a process death after this boundary is reconciled conservatively.
    pub fn begin_claim_execution(&self, claim: &ScheduledTaskClaim) -> Result<bool> {
        let mut conn = self.lock_writable_connection("begin_claim_execution")?;
        self.validate_claim_authority(claim)?;
        let now = chrono::Utc::now();
        if claim.provider_grant.is_expired_at(&now)? {
            anyhow::bail!("scheduled provider grant expired before execution");
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE scheduler_attempts
             SET status = 'executing', execution_started_at = COALESCE(execution_started_at, ?1)
             WHERE attempt_id = ?2 AND task_id = ?3 AND claim_token = ?4 AND status = 'claimed'
               AND process_epoch_id = ?5 AND writer_owner_generation_id = ?6
               AND EXISTS (
                    SELECT 1 FROM tasks
                    WHERE id = ?3 AND status = 'running' AND claim_token = ?4
               )",
            params![
                now.to_rfc3339(),
                claim.attempt_id,
                claim.task.id,
                claim.claim_token,
                claim
                    .policy_authority_proof
                    .store
                    .process_epoch_id
                    .to_string(),
                claim
                    .policy_authority_proof
                    .store
                    .writer_owner_generation_id
                    .to_string(),
            ],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(true);
        }
        let already_executing = active_attempt_status(&tx, claim)?.as_deref() == Some("executing");
        tx.commit()?;
        Ok(already_executing)
    }

    /// Read-only ownership check used by the scheduler execution owner before
    /// entering a separate canonical domain transaction such as Review Center
    /// Proposal creation.
    pub fn owns_executing_claim(&self, claim: &ScheduledTaskClaim) -> Result<bool> {
        self.validate_claim_authority(claim)?;
        let conn = self.lock_connection()?;
        executing_attempt_active(&conn, claim)
    }

    /// Consume one runtime-issued, non-cloneable provider-truth admission and
    /// persist exactly the transition it carries. Public progress/receipt DTOs
    /// are deliberately not accepted as write authority.
    pub fn record_provider_truth(
        &self,
        claim: &ScheduledTaskClaim,
        admission: ScheduledProviderTruthAdmission,
    ) -> Result<bool> {
        // Reject observation-only handles before consuming the non-cloneable
        // provider admission. The concrete persistence branch revalidates the
        // writable owner and database identity immediately before its SQL.
        drop(self.lock_writable_connection("record_provider_truth")?);
        self.validate_claim_authority(claim)?;
        let record = admission.consume_for_claim(claim)?;
        validate_provider_truth_record_for_claim(claim, &record)?;
        match record.transition() {
            ScheduledProviderTruthTransition::Started => {
                self.persist_provider_started_record(claim, &record)
            }
            ScheduledProviderTruthTransition::Completed
            | ScheduledProviderTruthTransition::Failed
            | ScheduledProviderTruthTransition::RemoteUnknown => {
                self.persist_provider_terminal_record(claim, &record)
            }
        }
    }

    fn persist_provider_started_record(
        &self,
        claim: &ScheduledTaskClaim,
        record: &ScheduledProviderTruthRecord,
    ) -> Result<bool> {
        if record.transition() != ScheduledProviderTruthTransition::Started {
            anyhow::bail!("scheduled provider start persistence received another transition");
        }
        let request_id = record.request_id();
        let provider = record.provider();
        let model = record.model();
        let started_at = record.started_at();
        let policy_evidence = record.policy_evidence();
        claim.validate_policy_authority()?;
        validate_provider_evidence_for_claim(claim, policy_evidence)?;
        if claim.provider_grant.is_expired_at(&started_at)? {
            anyhow::bail!("scheduled provider grant expired at the adapter dispatch boundary");
        }
        validate_reference("provider request id", request_id)?;
        validate_reference("provider", provider)?;
        validate_reference("model", model)?;
        if claim.provider_grant.data_route == ProviderDataRoute::LocalOnly && provider != "ollama" {
            anyhow::bail!("local-only scheduled policy rejected a cloud provider start");
        }
        if !claim.provider_grant.target_matches(provider, model) {
            anyhow::bail!("scheduled provider start does not match its reviewed provider/model");
        }
        let mut conn = self.lock_writable_connection("persist_provider_started_record")?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_exact_provider_truth_owner(&tx, claim)?;
        let provider_digest = digest_ref(provider);
        let model_digest = digest_ref(model);
        let policy_evidence_digest = policy_evidence.evidence_digest()?;
        let payload_purpose = policy_evidence.payload_purpose.ok_or_else(|| {
            anyhow::anyhow!("scheduled provider start is missing payload purpose")
        })?;
        if claim.provider_grant.data_route == ProviderDataRoute::PolicyAllowed {
            let inserted = tx.execute(
                "INSERT OR IGNORE INTO scheduler_provider_grant_consumptions (
                    grant_id, task_id, attempt_id, consumed_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    claim.provider_grant.grant_id,
                    claim.task.id,
                    claim.attempt_id,
                    started_at.to_rfc3339(),
                ],
            )?;
            if inserted == 0 {
                let exact: bool = tx.query_row(
                    "SELECT COUNT(*) = 1 FROM scheduler_provider_grant_consumptions
                     WHERE grant_id = ?1 AND task_id = ?2 AND attempt_id = ?3",
                    params![
                        claim.provider_grant.grant_id,
                        claim.task.id,
                        claim.attempt_id,
                    ],
                    |row| row.get(0),
                )?;
                if !exact {
                    anyhow::bail!(
                        "scheduled provider grant was already consumed by another attempt"
                    );
                }
            }
        }
        let changed = tx.execute(
            "INSERT OR IGNORE INTO scheduler_provider_receipts (
                request_id, attempt_id, task_id, claim_token, process_epoch_id,
                writer_owner_generation_id, provider_grant_id,
                provider_digest, model_digest, status, started_at, policy_evidence_state,
                policy_evidence_digest, subject_scope_digest, payload_purpose,
                unfiltered_payload_digest, context_manifest_digest, prepared_envelope_digest,
                prepared_request_digest, network_policy_decision_digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'started', ?10, 'exact',
                       ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                request_id,
                claim.attempt_id,
                claim.task.id,
                claim.claim_token,
                claim
                    .policy_authority_proof
                    .store
                    .process_epoch_id
                    .to_string(),
                claim
                    .policy_authority_proof
                    .store
                    .writer_owner_generation_id
                    .to_string(),
                claim.provider_grant.grant_id,
                provider_digest,
                model_digest,
                started_at.to_rfc3339(),
                policy_evidence_digest,
                policy_evidence.subject_scope_digest,
                payload_purpose.as_str(),
                policy_evidence.unfiltered_payload_digest,
                policy_evidence.context_manifest_digest,
                policy_evidence.prepared_envelope_digest,
                record.prepared_request_digest(),
                policy_evidence.network_policy_decision_digest,
            ],
        )?;
        if changed == 0 {
            let matches: bool = tx.query_row(
                "SELECT COUNT(*) = 1 FROM scheduler_provider_receipts
                 WHERE request_id = ?1 AND attempt_id = ?2 AND task_id = ?3 AND claim_token = ?4
                   AND process_epoch_id = ?5 AND writer_owner_generation_id = ?6
                   AND provider_grant_id = ?7 AND provider_digest = ?8 AND model_digest = ?9
                   AND started_at = ?10 AND policy_evidence_state = 'exact'
                   AND policy_evidence_digest = ?11 AND subject_scope_digest = ?12
                   AND payload_purpose = ?13 AND unfiltered_payload_digest IS ?14
                   AND context_manifest_digest = ?15 AND prepared_envelope_digest IS ?16
                   AND prepared_request_digest = ?17
                   AND network_policy_decision_digest = ?18
                   AND finished_at IS NULL AND error_digest IS NULL AND simulated IS NULL",
                params![
                    request_id,
                    claim.attempt_id,
                    claim.task.id,
                    claim.claim_token,
                    claim
                        .policy_authority_proof
                        .store
                        .process_epoch_id
                        .to_string(),
                    claim
                        .policy_authority_proof
                        .store
                        .writer_owner_generation_id
                        .to_string(),
                    claim.provider_grant.grant_id,
                    provider_digest,
                    model_digest,
                    started_at.to_rfc3339(),
                    policy_evidence_digest,
                    policy_evidence.subject_scope_digest,
                    payload_purpose.as_str(),
                    policy_evidence.unfiltered_payload_digest,
                    policy_evidence.context_manifest_digest,
                    policy_evidence.prepared_envelope_digest,
                    record.prepared_request_digest(),
                    policy_evidence.network_policy_decision_digest,
                ],
                |row| row.get(0),
            )?;
            if !matches {
                anyhow::bail!("provider request id is already bound to different scheduler facts");
            }
        }
        tx.commit()?;
        Ok(changed == 1)
    }

    fn persist_provider_terminal_record(
        &self,
        claim: &ScheduledTaskClaim,
        record: &ScheduledProviderTruthRecord,
    ) -> Result<bool> {
        if record.transition() == ScheduledProviderTruthTransition::Started {
            anyhow::bail!("scheduled provider terminal persistence received start transition");
        }
        let receipt = ProviderInvocationReceipt {
            request_id: record.request_id().to_string(),
            provider: record.provider().to_string(),
            model: record.model().to_string(),
            status: match record.transition() {
                ScheduledProviderTruthTransition::Completed => ProviderInvocationStatus::Completed,
                ScheduledProviderTruthTransition::Failed => ProviderInvocationStatus::Failed,
                ScheduledProviderTruthTransition::RemoteUnknown => {
                    ProviderInvocationStatus::RemoteUnknown
                }
                ScheduledProviderTruthTransition::Started => unreachable!(
                    "start transition was rejected before terminal receipt construction"
                ),
            },
            started_at: record.started_at(),
            finished_at: record
                .finished_at()
                .ok_or_else(|| anyhow::anyhow!("scheduled provider terminal has no finished_at"))?,
            error_digest: record.error_digest().map(str::to_string),
            simulated: false,
            policy_evidence: Some(record.policy_evidence().clone()),
        };
        claim.validate_policy_authority()?;
        let policy_evidence = receipt.policy_evidence.as_ref().ok_or_else(|| {
            anyhow::anyhow!("scheduled provider terminal is missing exact policy evidence")
        })?;
        validate_provider_evidence_for_claim(claim, policy_evidence)?;
        let policy_evidence_digest = policy_evidence.evidence_digest()?;
        if claim.provider_grant.data_route == ProviderDataRoute::LocalOnly
            && receipt.provider != "ollama"
        {
            anyhow::bail!("local-only scheduled policy rejected a cloud provider receipt");
        }
        if !claim
            .provider_grant
            .target_matches(&receipt.provider, &receipt.model)
        {
            anyhow::bail!("scheduled provider receipt does not match its reviewed provider/model");
        }
        if let Some(error_digest) = receipt.error_digest.as_deref() {
            validate_digest("provider error digest", error_digest)?;
        }
        match receipt.status {
            ProviderInvocationStatus::Completed if receipt.error_digest.is_some() => {
                anyhow::bail!("completed provider receipt cannot contain an error digest");
            }
            ProviderInvocationStatus::Failed if receipt.error_digest.is_none() => {
                anyhow::bail!("failed provider receipt requires an error digest");
            }
            ProviderInvocationStatus::RemoteUnknown if receipt.error_digest.is_none() => {
                anyhow::bail!("remote-unknown provider receipt requires a reason digest");
            }
            _ => {}
        }
        let status = match receipt.status {
            ProviderInvocationStatus::Completed => "completed",
            ProviderInvocationStatus::Failed => "failed",
            ProviderInvocationStatus::RemoteUnknown => "remote_unknown",
        };
        let mut conn = self.lock_writable_connection("persist_provider_terminal_record")?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_exact_provider_truth_owner(&tx, claim)?;
        let provider_digest = digest_ref(&receipt.provider);
        let model_digest = digest_ref(&receipt.model);
        let changed = tx.execute(
            "UPDATE scheduler_provider_receipts
             SET status = ?1, finished_at = ?2, error_digest = ?3, simulated = ?4
             WHERE request_id = ?5 AND attempt_id = ?6 AND task_id = ?7 AND claim_token = ?8
               AND process_epoch_id = ?9 AND writer_owner_generation_id = ?10
               AND provider_grant_id = ?11 AND provider_digest = ?12 AND model_digest = ?13
               AND started_at = ?14 AND status = 'started' AND policy_evidence_state = 'exact'
               AND policy_evidence_digest = ?15 AND subject_scope_digest = ?16
               AND payload_purpose = ?17 AND unfiltered_payload_digest IS ?18
               AND context_manifest_digest = ?19 AND prepared_envelope_digest IS ?20
               AND prepared_request_digest = ?21
               AND network_policy_decision_digest = ?22",
            params![
                status,
                receipt.finished_at.to_rfc3339(),
                receipt.error_digest,
                receipt.simulated,
                receipt.request_id,
                claim.attempt_id,
                claim.task.id,
                claim.claim_token,
                claim
                    .policy_authority_proof
                    .store
                    .process_epoch_id
                    .to_string(),
                claim
                    .policy_authority_proof
                    .store
                    .writer_owner_generation_id
                    .to_string(),
                claim.provider_grant.grant_id,
                provider_digest,
                model_digest,
                receipt.started_at.to_rfc3339(),
                policy_evidence_digest,
                policy_evidence.subject_scope_digest,
                policy_evidence
                    .payload_purpose
                    .map(|purpose| purpose.as_str()),
                policy_evidence.unfiltered_payload_digest,
                policy_evidence.context_manifest_digest,
                policy_evidence.prepared_envelope_digest,
                record.prepared_request_digest(),
                policy_evidence.network_policy_decision_digest,
            ],
        )?;
        if changed == 0 {
            anyhow::bail!(
                "scheduled provider first terminal already won or lost its exact durable start"
            );
        }
        tx.commit()?;
        Ok(changed == 1)
    }

    /// Persists the first adapter-owned dispatch transition together with the
    /// exact preflight manifest/input binding. The preflight DTO alone is not
    /// dispatch evidence, and the receipt alone does not contain the manifest
    /// contract or bounded input binding, so both are required here.
    pub fn record_tool_dispatch_started(
        &self,
        claim: &ScheduledTaskClaim,
        attempt: &ToolDispatchAttempt,
        receipt: &ToolExecutionReceipt,
    ) -> Result<String> {
        let mut conn = self.lock_writable_connection("record_tool_dispatch_started")?;
        self.validate_claim_authority(claim)?;
        validate_tool_dispatch_started_identity(attempt, receipt)?;
        let input_length_bytes = i64::try_from(attempt.input_length_bytes)
            .context("tool input length exceeds sqlite integer range")?;
        let dispatched_at = receipt
            .dispatched_at
            .ok_or_else(|| anyhow::anyhow!("tool dispatch start has no dispatched_at"))?;
        let manifest_digest = digest_ref(&attempt.manifest_id);
        let tool_digest = digest_ref(&attempt.tool_name);
        let source_run_digest = attempt.source_run_id.as_deref().map(digest_ref);
        let receipt_started_at = receipt.started_at.to_rfc3339();
        let dispatched_at = dispatched_at.to_rfc3339();
        let tx = conn.transaction()?;
        ensure_executing_attempt(&tx, claim)?;

        if let Some(dispatch_id) = tx
            .query_row(
                "SELECT dispatch_id FROM scheduler_tool_dispatches
                 WHERE tool_receipt_id = ?1",
                params![receipt.receipt_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            let exact: bool = tx.query_row(
                "SELECT COUNT(*) = 1 FROM scheduler_tool_dispatches
                 WHERE dispatch_id = ?1 AND attempt_id = ?2 AND task_id = ?3
                   AND claim_token = ?4 AND manifest_digest = ?5
                   AND manifest_contract_digest = ?6 AND tool_digest = ?7
                   AND input_hash = ?8 AND input_length_bytes = ?9
                   AND source_run_ref_digest IS ?10 AND identity_state = 'exact'
                   AND receipt_started_at = ?11 AND dispatched_at = ?12
                   AND request_digest = ?13 AND action_effect = ?14
                   AND idempotency_contract = ?15 AND dispatch_kind = ?16
                   AND dispatch_attempt_count = ?17
                   AND process_epoch_id = ?18 AND writer_owner_generation_id = ?19",
                params![
                    dispatch_id,
                    claim.attempt_id,
                    claim.task.id,
                    claim.claim_token,
                    manifest_digest,
                    attempt.manifest_contract_digest,
                    tool_digest,
                    attempt.input_hash,
                    input_length_bytes,
                    source_run_digest,
                    receipt_started_at,
                    dispatched_at,
                    receipt.request_digest,
                    receipt.action_effect.as_str(),
                    receipt.idempotency_contract.as_str(),
                    receipt.dispatch_kind.as_str(),
                    i64::from(receipt.dispatch_attempt_count),
                    claim
                        .policy_authority_proof
                        .store
                        .process_epoch_id
                        .to_string(),
                    claim
                        .policy_authority_proof
                        .store
                        .writer_owner_generation_id
                        .to_string(),
                ],
                |row| row.get(0),
            )?;
            if !exact {
                anyhow::bail!("tool dispatch receipt id is rebound to a different start fact");
            }
            tx.commit()?;
            return Ok(dispatch_id);
        }

        let dispatch_index: u32 = tx.query_row(
            "SELECT COALESCE(MAX(dispatch_index), 0) + 1
             FROM scheduler_tool_dispatches WHERE attempt_id = ?1",
            params![claim.attempt_id],
            |row| row.get(0),
        )?;
        let dispatch_id = digest_parts(&[
            "scheduled_tool_dispatch_v1",
            &claim.attempt_id,
            &dispatch_index.to_string(),
        ]);
        tx.execute(
            "INSERT INTO scheduler_tool_dispatches (
                dispatch_id, attempt_id, task_id, claim_token, dispatch_index,
                manifest_digest, manifest_contract_digest, tool_digest, input_hash,
                input_length_bytes, source_run_ref_digest, identity_state, status,
                observed_at, receipt_started_at, dispatched_at, tool_receipt_id,
                request_digest, action_effect, idempotency_contract, dispatch_kind,
                dispatch_attempt_count, transport_status, effect_status, execution_outcome,
                process_epoch_id, writer_owner_generation_id
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'exact', 'started',
                ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
                ?24, ?25
             )",
            params![
                dispatch_id,
                claim.attempt_id,
                claim.task.id,
                claim.claim_token,
                dispatch_index,
                manifest_digest,
                attempt.manifest_contract_digest,
                tool_digest,
                attempt.input_hash,
                input_length_bytes,
                source_run_digest,
                chrono::Utc::now().to_rfc3339(),
                receipt_started_at,
                dispatched_at,
                receipt.receipt_id,
                receipt.request_digest,
                receipt.action_effect.as_str(),
                receipt.idempotency_contract.as_str(),
                receipt.dispatch_kind.as_str(),
                i64::from(receipt.dispatch_attempt_count),
                receipt.transport_status.as_str(),
                receipt.effect_status.as_str(),
                receipt.execution_outcome.as_str(),
                claim
                    .policy_authority_proof
                    .store
                    .process_epoch_id
                    .to_string(),
                claim
                    .policy_authority_proof
                    .store
                    .writer_owner_generation_id
                    .to_string(),
            ],
        )?;
        tx.commit()?;
        Ok(dispatch_id)
    }

    /// Projects the ToolGateway-owned minimal terminal receipt onto the exact
    /// scheduler attempt. No tool arguments or output are copied.
    pub fn record_tool_terminal(
        &self,
        claim: &ScheduledTaskClaim,
        receipt: &ToolExecutionReceipt,
    ) -> Result<bool> {
        let mut conn = self.lock_writable_connection("record_tool_terminal")?;
        self.validate_claim_authority(claim)?;
        receipt
            .mechanically_valid_terminal()
            .map_err(|reason| anyhow::anyhow!(reason))?;
        validate_reference("tool receipt id", &receipt.receipt_id)?;
        validate_digest("tool request digest", &receipt.request_digest)?;
        let manifest_id = receipt
            .manifest_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("dispatched tool receipt is missing manifest id"))?;
        validate_reference("tool manifest id", manifest_id)?;
        if let Some(source_run_id) = receipt.source_run_id.as_deref() {
            validate_reference("tool source run id", source_run_id)?;
        }
        let finished_at = receipt.finished_at.expect("validated terminal timestamp");
        if receipt.transport_status == ToolTransportStatus::NotAttempted {
            anyhow::bail!("not-attempted tool receipt cannot terminate a dispatch record");
        }
        let dispatched_at = receipt
            .dispatched_at
            .ok_or_else(|| anyhow::anyhow!("dispatched tool terminal has no dispatch time"))?;
        let ambiguous = receipt.effect_status == ToolEffectStatus::Unknown
            || matches!(
                receipt.transport_status,
                ToolTransportStatus::LocalAborted | ToolTransportStatus::RemoteUnknown
            );
        let status = if ambiguous { "unknown" } else { "returned" };
        let source_run_digest = receipt.source_run_id.as_deref().map(digest_ref);
        let tx = conn.transaction()?;
        ensure_executing_attempt(&tx, claim)?;
        let changed = tx.execute(
            "UPDATE scheduler_tool_dispatches
             SET status = ?1, finished_at = ?2, transport_status = ?3,
                 effect_status = ?4, execution_outcome = ?5,
                 transport_observed_at = ?6
             WHERE attempt_id = ?7 AND task_id = ?8 AND claim_token = ?9
               AND tool_receipt_id = ?10 AND identity_state = 'exact'
               AND status = 'started' AND manifest_digest = ?11
               AND source_run_ref_digest IS ?12 AND request_digest = ?13
               AND action_effect = ?14 AND idempotency_contract = ?15
               AND dispatch_kind = ?16 AND dispatch_attempt_count = ?17
               AND receipt_started_at = ?18 AND dispatched_at = ?19
               AND manifest_contract_digest IS NOT NULL AND input_hash IS NOT NULL
               AND input_length_bytes IS NOT NULL AND finished_at IS NULL
               AND transport_status = 'dispatched' AND effect_status = 'not_attempted'
               AND execution_outcome = 'not_observed' AND transport_observed_at IS NULL
               AND process_epoch_id = ?20 AND writer_owner_generation_id = ?21",
            params![
                status,
                finished_at.to_rfc3339(),
                receipt.transport_status.as_str(),
                receipt.effect_status.as_str(),
                receipt.execution_outcome.as_str(),
                receipt.response_observed_at.map(|value| value.to_rfc3339()),
                claim.attempt_id,
                claim.task.id,
                claim.claim_token,
                receipt.receipt_id,
                digest_ref(manifest_id),
                source_run_digest,
                receipt.request_digest,
                receipt.action_effect.as_str(),
                receipt.idempotency_contract.as_str(),
                receipt.dispatch_kind.as_str(),
                i64::from(receipt.dispatch_attempt_count),
                receipt.started_at.to_rfc3339(),
                dispatched_at.to_rfc3339(),
                claim
                    .policy_authority_proof
                    .store
                    .process_epoch_id
                    .to_string(),
                claim
                    .policy_authority_proof
                    .store
                    .writer_owner_generation_id
                    .to_string(),
            ],
        )?;
        if changed == 0 {
            let exact_replay: bool = tx.query_row(
                "SELECT COUNT(*) = 1 FROM scheduler_tool_dispatches
                 WHERE attempt_id = ?1 AND task_id = ?2 AND claim_token = ?3
                   AND tool_receipt_id = ?4 AND identity_state = 'exact'
                   AND status = ?5 AND manifest_digest = ?6
                   AND source_run_ref_digest IS ?7 AND request_digest = ?8
                   AND action_effect = ?9 AND idempotency_contract = ?10
                   AND dispatch_kind = ?11 AND dispatch_attempt_count = ?12
                   AND receipt_started_at = ?13 AND dispatched_at = ?14
                   AND finished_at = ?15 AND transport_status = ?16
                   AND effect_status = ?17 AND execution_outcome = ?18
                   AND transport_observed_at IS ?19
                   AND process_epoch_id = ?20 AND writer_owner_generation_id = ?21",
                params![
                    claim.attempt_id,
                    claim.task.id,
                    claim.claim_token,
                    receipt.receipt_id,
                    status,
                    digest_ref(manifest_id),
                    source_run_digest,
                    receipt.request_digest,
                    receipt.action_effect.as_str(),
                    receipt.idempotency_contract.as_str(),
                    receipt.dispatch_kind.as_str(),
                    i64::from(receipt.dispatch_attempt_count),
                    receipt.started_at.to_rfc3339(),
                    dispatched_at.to_rfc3339(),
                    finished_at.to_rfc3339(),
                    receipt.transport_status.as_str(),
                    receipt.effect_status.as_str(),
                    receipt.execution_outcome.as_str(),
                    receipt.response_observed_at.map(|value| value.to_rfc3339()),
                    claim
                        .policy_authority_proof
                        .store
                        .process_epoch_id
                        .to_string(),
                    claim
                        .policy_authority_proof
                        .store
                        .writer_owner_generation_id
                        .to_string(),
                ],
                |row| row.get(0),
            )?;
            if !exact_replay {
                anyhow::bail!("tool terminal receipt does not match its exact durable start fact");
            }
        }
        tx.commit()?;
        Ok(status == "returned")
    }

    /// Binds the canonical conversation delivery to the exact active attempt
    /// before terminal task projection. The body remains owned by the
    /// Conversation store; TaskStore keeps only its reference and digest.
    pub fn stage_claim_result_delivery(
        &self,
        claim: &ScheduledTaskClaim,
        result_ref: &str,
        result_digest: &str,
    ) -> Result<bool> {
        let mut conn = self.lock_writable_connection("stage_claim_result_delivery")?;
        self.validate_claim_authority(claim)?;
        validate_scheduled_result_ref(result_ref)?;
        validate_digest("scheduled result digest", result_digest)?;
        let tx = conn.transaction()?;
        ensure_executing_attempt(&tx, claim)?;
        let changed = tx.execute(
            "UPDATE tasks SET result_ref = ?1, result_digest = ?2
             WHERE id = ?3 AND status = 'running' AND claim_token = ?4
               AND (result_ref IS NULL OR (result_ref = ?1 AND result_digest = ?2))",
            params![result_ref, result_digest, claim.task.id, claim.claim_token],
        )?;
        if changed != 1 {
            anyhow::bail!("scheduled result delivery conflicts with the canonical attempt");
        }
        tx.commit()?;
        Ok(true)
    }

    /// Completes the task only from durable provider truth. An in-memory
    /// collector or successful prose is not sufficient evidence.
    pub fn complete_claim(
        &self,
        claim: &ScheduledTaskClaim,
        agent_run_id: &str,
        result_ref: &str,
        result_digest: &str,
    ) -> Result<bool> {
        let mut conn = self.lock_writable_connection("complete_claim")?;
        self.validate_claim_authority(claim)?;
        validate_reference("agent run id", agent_run_id)?;
        validate_scheduled_result_ref(result_ref)?;
        validate_digest("scheduled result digest", result_digest)?;
        let tx = conn.transaction()?;
        ensure_executing_attempt(&tx, claim)?;
        let evidence = attempt_dispatch_evidence(&tx, &claim.attempt_id)?;
        if evidence.provider_completed == 0
            || evidence.provider_started > 0
            || evidence.provider_unknown > 0
            || evidence.provider_legacy > 0
            || evidence.tool_ambiguous() > 0
        {
            anyhow::bail!("scheduled completion lacks durable completed provider truth");
        }
        let now = chrono::Utc::now().to_rfc3339();
        let changed = tx.execute(
            "UPDATE tasks SET status = 'completed', completed_at = ?1, result_ref = ?2,
                    result_digest = ?3,
                    last_error = NULL, claim_token = NULL, lease_expires_at = NULL
             WHERE id = ?4 AND status = 'running' AND claim_token = ?5
               AND result_ref = ?2 AND result_digest = ?3",
            params![
                now,
                result_ref,
                result_digest,
                claim.task.id,
                claim.claim_token
            ],
        )?;
        if changed != 1 {
            tx.commit()?;
            return Ok(false);
        }
        let attempt_changed = tx.execute(
            "UPDATE scheduler_attempts
             SET status = 'completed', finished_at = ?1, agent_run_ref_digest = ?2
             WHERE attempt_id = ?3 AND task_id = ?4 AND claim_token = ?5 AND status = 'executing'",
            params![
                now,
                digest_ref(agent_run_id),
                claim.attempt_id,
                claim.task.id,
                claim.claim_token,
            ],
        )?;
        if attempt_changed != 1 {
            anyhow::bail!("scheduled attempt completion lost its canonical claim");
        }
        tx.commit()?;
        Ok(true)
    }

    /// Settles an explicit execution error according to durable dispatch facts.
    /// Only a factually pre-dispatch failure is automatically reclaimed.
    pub fn settle_claim_after_error(
        &self,
        claim: &ScheduledTaskClaim,
        reason_code: &str,
        error_digest: Option<&str>,
    ) -> Result<ScheduledClaimSettlement> {
        let mut conn = self.lock_writable_connection("settle_claim_after_error")?;
        self.validate_claim_authority(claim)?;
        validate_reason_code(reason_code)?;
        if let Some(error_digest) = error_digest {
            validate_digest("scheduled error digest", error_digest)?;
        }
        let tx = conn.transaction()?;
        ensure_executing_attempt(&tx, claim)?;
        let evidence = attempt_dispatch_evidence(&tx, &claim.attempt_id)?;
        let grant_consumed = scheduled_cloud_grant_consumed(&tx, claim)?;
        let settlement = if evidence.total_dispatches() == 0 && grant_consumed {
            settle_active_claim(
                &tx,
                claim,
                "failed",
                "pre_dispatch_failed",
                "scheduled_cloud_grant_consumed_requires_review",
                error_digest,
                false,
            )?;
            ScheduledClaimSettlement::GrantConsumedRequiresReview
        } else if evidence.total_dispatches() == 0 {
            settle_active_claim(
                &tx,
                claim,
                "pending",
                "pre_dispatch_failed",
                reason_code,
                error_digest,
                true,
            )?;
            ScheduledClaimSettlement::ReclaimedBeforeDispatch
        } else if evidence.provider_started > 0
            || evidence.provider_unknown > 0
            || evidence.provider_legacy > 0
            || evidence.tool_ambiguous() > 0
        {
            settle_active_claim(
                &tx,
                claim,
                "unknown",
                "unknown",
                "dispatch_result_unknown",
                error_digest,
                false,
            )?;
            ScheduledClaimSettlement::UnknownRequiresReconciliation
        } else {
            settle_active_claim(
                &tx,
                claim,
                "failed",
                "failed",
                reason_code,
                error_digest,
                false,
            )?;
            ScheduledClaimSettlement::FailedAfterObservedTerminal
        };
        tx.commit()?;
        Ok(settlement)
    }

    /// Timeout is stricter than a normal terminal error: once any provider or
    /// tool dispatch was observed, the overall result remains unknown until a
    /// reconciler proves a safe resolution.
    pub fn settle_claim_after_timeout(
        &self,
        claim: &ScheduledTaskClaim,
    ) -> Result<ScheduledClaimSettlement> {
        let mut conn = self.lock_writable_connection("settle_claim_after_timeout")?;
        self.validate_claim_authority(claim)?;
        let tx = conn.transaction()?;
        ensure_executing_attempt(&tx, claim)?;
        let evidence = attempt_dispatch_evidence(&tx, &claim.attempt_id)?;
        let grant_consumed = scheduled_cloud_grant_consumed(&tx, claim)?;
        let settlement = if evidence.total_dispatches() == 0 && grant_consumed {
            settle_active_claim(
                &tx,
                claim,
                "failed",
                "pre_dispatch_timeout",
                "scheduled_cloud_grant_consumed_requires_review",
                None,
                false,
            )?;
            ScheduledClaimSettlement::GrantConsumedRequiresReview
        } else if evidence.total_dispatches() == 0 {
            settle_active_claim(
                &tx,
                claim,
                "pending",
                "pre_dispatch_timeout",
                "pre_dispatch_timeout",
                None,
                true,
            )?;
            ScheduledClaimSettlement::ReclaimedBeforeDispatch
        } else {
            settle_active_claim(
                &tx,
                claim,
                "unknown",
                "unknown",
                "local_timeout_remote_state_unknown",
                None,
                false,
            )?;
            ScheduledClaimSettlement::UnknownRequiresReconciliation
        };
        tx.commit()?;
        Ok(settlement)
    }

    /// Conservatively quarantines a claim when an adapter-edge receipt could
    /// not be persisted. This is intentionally not an automatic retry path.
    pub fn quarantine_claim_unknown(
        &self,
        claim: &ScheduledTaskClaim,
        reason_code: &str,
    ) -> Result<bool> {
        let mut conn = self.lock_writable_connection("quarantine_claim_unknown")?;
        self.validate_claim_authority(claim)?;
        validate_reason_code(reason_code)?;
        let tx = conn.transaction()?;
        if !executing_attempt_active(&tx, claim)? {
            tx.commit()?;
            return Ok(false);
        }
        settle_active_claim(&tx, claim, "unknown", "unknown", reason_code, None, false)?;
        tx.commit()?;
        Ok(true)
    }

    /// Startup compatibility entrypoint. Previous-process and same-process
    /// abandoned-writer facts are selected and labelled by two disjoint
    /// reconcilers; a writer generation is never reinterpreted as a process
    /// epoch.
    pub fn reconcile_previous_process_claims(
        &self,
        observed_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<usize> {
        let previous = self.reconcile_abandoned_runtime_claims(
            observed_at,
            AbandonedRuntimeScope::PreviousProcessEpoch,
        )?;
        let abandoned_writer = self.reconcile_abandoned_runtime_claims(
            observed_at,
            AbandonedRuntimeScope::SameProcessWriterGeneration,
        )?;
        Ok(previous + abandoned_writer)
    }

    pub fn reconcile_abandoned_writer_generation(
        &self,
        observed_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<usize> {
        self.reconcile_abandoned_runtime_claims(
            observed_at,
            AbandonedRuntimeScope::SameProcessWriterGeneration,
        )
    }

    fn reconcile_abandoned_runtime_claims(
        &self,
        observed_at: chrono::DateTime<chrono::Utc>,
        scope: AbandonedRuntimeScope,
    ) -> Result<usize> {
        let authority = self.runtime_authority()?;
        let current_epoch = authority.process_epoch_id.to_string();
        let current_writer_generation = authority.writer_owner_generation_id.to_string();
        let operation = match scope {
            AbandonedRuntimeScope::PreviousProcessEpoch => "reconcile_previous_process_claims",
            AbandonedRuntimeScope::SameProcessWriterGeneration => {
                "reconcile_abandoned_writer_generation"
            }
        };
        let mut conn = self.lock_writable_connection(operation)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let abandoned = match scope {
            AbandonedRuntimeScope::PreviousProcessEpoch => {
                let mut statement = tx.prepare(
                    "SELECT t.id, t.claim_token, a.attempt_id, a.status, a.attempt_number
                     FROM tasks t JOIN scheduler_attempts a ON a.claim_token = t.claim_token
                     WHERE t.status = 'running' AND a.status IN ('claimed', 'executing')
                       AND a.process_epoch_id != ?1",
                )?;
                let rows = statement
                    .query_map([current_epoch], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, u32>(4)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            }
            AbandonedRuntimeScope::SameProcessWriterGeneration => {
                let mut statement = tx.prepare(
                    "SELECT t.id, t.claim_token, a.attempt_id, a.status, a.attempt_number
                     FROM tasks t JOIN scheduler_attempts a ON a.claim_token = t.claim_token
                     WHERE t.status = 'running' AND a.status IN ('claimed', 'executing')
                       AND a.process_epoch_id = ?1
                       AND a.writer_owner_generation_id != ?2",
                )?;
                let rows = statement
                    .query_map(params![current_epoch, current_writer_generation], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, u32>(4)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            }
        };
        let observed_at_text = observed_at.to_rfc3339();
        let mut changed = 0usize;
        for (task_id, claim_token, attempt_id, attempt_status, attempt_number) in abandoned {
            let (claimed_reason, executing_reason, remote_reason, owner_label) = match scope {
                AbandonedRuntimeScope::PreviousProcessEpoch => (
                    "previous_process_claim_reclaimed_before_execution",
                    "previous_process_execution_state_unknown",
                    "scheduler_process_exit_remote_state_unknown",
                    "previous-process",
                ),
                AbandonedRuntimeScope::SameProcessWriterGeneration => (
                    "abandoned_writer_generation_claim_reclaimed_before_execution",
                    "abandoned_writer_generation_execution_state_unknown",
                    "scheduler_writer_owner_exit_remote_state_unknown",
                    "abandoned-writer-generation",
                ),
            };
            let (task_status, attempt_terminal, reason, eligible_at) =
                if attempt_status == "claimed" {
                    (
                        "pending",
                        "expired_before_execution",
                        claimed_reason,
                        Some(retry_eligible_at(observed_at, attempt_number).to_rfc3339()),
                    )
                } else {
                    ("unknown", "unknown", executing_reason, None)
                };
            let task_changed = tx.execute(
                "UPDATE tasks SET status = ?1, last_error = ?2, claim_token = NULL,
                        lease_expires_at = NULL, eligible_at = ?3
                 WHERE id = ?4 AND status = 'running' AND claim_token = ?5",
                params![task_status, reason, eligible_at, task_id, claim_token],
            )?;
            if task_changed != 1 {
                anyhow::bail!("{owner_label} scheduler claim lost its canonical task owner");
            }
            let attempt_changed = tx.execute(
                "UPDATE scheduler_attempts SET status = ?1, finished_at = ?2,
                        error_digest = COALESCE(error_digest, ?3)
                 WHERE attempt_id = ?4 AND task_id = ?5 AND claim_token = ?6
                   AND status = ?7",
                params![
                    attempt_terminal,
                    observed_at_text,
                    digest_ref(reason),
                    attempt_id,
                    task_id,
                    claim_token,
                    attempt_status,
                ],
            )?;
            if attempt_changed != 1 {
                anyhow::bail!("{owner_label} scheduler claim lost its canonical attempt owner");
            }
            if attempt_terminal == "unknown" {
                tx.execute(
                    "UPDATE scheduler_provider_receipts
                     SET status = 'remote_unknown', finished_at = COALESCE(finished_at, ?1),
                         error_digest = COALESCE(error_digest, ?2)
                     WHERE attempt_id = ?3 AND status = 'started'",
                    params![observed_at_text, digest_ref(remote_reason), attempt_id,],
                )?;
                tx.execute(
                    "UPDATE scheduler_tool_dispatches
                     SET status = 'unknown', finished_at = COALESCE(finished_at, ?1),
                         transport_status = CASE
                            WHEN dispatch_kind = 'local' THEN 'local_aborted'
                            ELSE 'remote_unknown'
                         END,
                         effect_status = CASE
                            WHEN action_effect = 'read_only' THEN 'not_attempted'
                            ELSE 'unknown'
                         END,
                         execution_outcome = 'unknown'
                     WHERE attempt_id = ?2 AND status = 'started'",
                    params![observed_at_text, attempt_id],
                )?;
            }
            changed += 1;
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Reconciles expired leases transactionally. Claims that never crossed
    /// the execution boundary are safe to reclaim. Once execution began, a
    /// process crash is conservatively unknown. The fallible start observer
    /// prevents dispatch when its durable start cannot be recorded, but a
    /// crash after a recorded start still leaves remote state unknown.
    pub fn reconcile_expired_claims(&self, now: chrono::DateTime<chrono::Utc>) -> Result<usize> {
        let mut conn = self.lock_writable_connection("reconcile_expired_claims")?;
        let tx = conn.transaction()?;
        let expired = {
            let mut stmt = tx.prepare(
                "SELECT t.id, t.claim_token, a.attempt_id, a.status, a.attempt_number
                 FROM tasks t
                 LEFT JOIN scheduler_attempts a ON a.claim_token = t.claim_token
                 WHERE t.status = 'running' AND t.lease_expires_at IS NOT NULL
                   AND t.lease_expires_at <= ?1",
            )?;
            let rows = stmt.query_map(params![now.to_rfc3339()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<u32>>(4)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut changed = 0usize;
        for (task_id, claim_token, attempt_id, attempt_status, attempt_number) in expired {
            let (task_status, attempt_terminal, reason) =
                if attempt_status.as_deref() == Some("claimed") {
                    (
                        "pending",
                        "expired_before_execution",
                        "lease_expired_before_execution",
                    )
                } else {
                    ("unknown", "unknown", "lease_expired_during_execution")
                };
            let eligible_at = (task_status == "pending")
                .then(|| retry_eligible_at(now, attempt_number.unwrap_or(1)).to_rfc3339());
            changed += tx.execute(
                "UPDATE tasks SET status = ?1, last_error = ?2, claim_token = NULL,
                        lease_expires_at = NULL, eligible_at = ?3
                 WHERE id = ?4 AND status = 'running' AND claim_token IS ?5",
                params![task_status, reason, eligible_at, task_id, claim_token],
            )?;
            if let Some(attempt_id) = attempt_id {
                tx.execute(
                    "UPDATE scheduler_attempts SET status = ?1, finished_at = ?2,
                            error_digest = ?3
                     WHERE attempt_id = ?4 AND status IN ('claimed', 'executing')",
                    params![
                        attempt_terminal,
                        now.to_rfc3339(),
                        digest_ref(reason),
                        attempt_id,
                    ],
                )?;
                if attempt_terminal == "unknown" {
                    tx.execute(
                        "UPDATE scheduler_provider_receipts
                         SET status = 'remote_unknown', finished_at = COALESCE(finished_at, ?1),
                             error_digest = COALESCE(error_digest, ?2)
                         WHERE attempt_id = ?3 AND status = 'started'",
                        params![
                            now.to_rfc3339(),
                            digest_ref("scheduler_process_exit_remote_state_unknown"),
                            attempt_id,
                        ],
                    )?;
                    tx.execute(
                        "UPDATE scheduler_tool_dispatches
                         SET status = 'unknown', finished_at = COALESCE(finished_at, ?1),
                             transport_status = CASE
                                WHEN dispatch_kind = 'local' THEN 'local_aborted'
                                ELSE 'remote_unknown'
                             END,
                             effect_status = CASE
                                WHEN action_effect = 'read_only' THEN 'not_attempted'
                                ELSE 'unknown'
                             END,
                             execution_outcome = 'unknown'
                         WHERE attempt_id = ?2 AND status = 'started'",
                        params![now.to_rfc3339(), attempt_id],
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Issue a failure-only reconciliation capability from a real provider
    /// adapter terminal. A completed response still requires a separately
    /// proven canonical conversation result owner, and `remote_unknown` proves
    /// nothing new, so neither can resolve an unknown task through this API.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn issue_provider_failed_reconciliation(
        &self,
        task_id: &str,
        attempt_id: &str,
        proof: ProviderInvocationTerminalProof,
    ) -> Result<ScheduledReconciliationAdmission> {
        drop(self.lock_writable_connection("issue_provider_failed_reconciliation")?);
        if !proof.is_runtime_adapter_terminal()
            || proof.receipt().status != ProviderInvocationStatus::Failed
        {
            anyhow::bail!(
                "provider reconciliation requires a runtime adapter confirmed-failure terminal"
            );
        }
        let receipt = proof.receipt();
        let policy_evidence = receipt.policy_evidence.as_ref().ok_or_else(|| {
            anyhow::anyhow!("provider reconciliation terminal has no policy evidence")
        })?;
        let binding = {
            let conn = self.lock_writable_connection("issue_provider_failed_reconciliation")?;
            let binding = load_unknown_attempt_binding(
                &conn,
                Arc::clone(self.runtime_authority()?),
                task_id,
                attempt_id,
            )?
            .ok_or_else(|| anyhow::anyhow!("scheduled unknown attempt is unavailable"))?;
            if binding.provider_provenance_state != "exact" {
                anyhow::bail!("legacy provider execution cannot mint runtime reconciliation");
            }
            let exact: bool = conn.query_row(
                "SELECT COUNT(*) = 1 FROM scheduler_provider_receipts
                 WHERE attempt_id = ?1 AND task_id = ?2 AND request_id = ?3
                   AND provider_digest = ?4 AND model_digest = ?5
                   AND started_at = ?6 AND policy_evidence_state = 'exact'
                   AND policy_evidence_digest = ?7
                   AND status IN ('started', 'remote_unknown')",
                params![
                    attempt_id,
                    task_id,
                    receipt.request_id,
                    digest_ref(&receipt.provider),
                    digest_ref(&receipt.model),
                    receipt.started_at.to_rfc3339(),
                    policy_evidence.evidence_digest()?,
                ],
                |row| row.get(0),
            )?;
            if !exact {
                anyhow::bail!(
                    "provider reconciliation terminal does not match the exact unknown dispatch"
                );
            }
            binding
        };
        let source_id = proof.reconciliation_source_id()?;
        let evidence_ref = format!("provider-runtime-terminal://{}", receipt.request_id);
        let evidence_digest =
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "schema": "scheduled_provider_reconciliation_source_v1",
                "sourceId": source_id,
                "requestId": receipt.request_id,
                "provider": receipt.provider,
                "model": receipt.model,
                "status": receipt.status,
                "startedAt": receipt.started_at.to_rfc3339(),
                "finishedAt": receipt.finished_at.to_rfc3339(),
                "errorDigest": receipt.error_digest,
                "policyEvidenceDigest": policy_evidence.evidence_digest()?,
                "unknownAttemptDigest": scheduled_unknown_attempt_binding_digest(&binding),
            }))
            .1;
        self.issue_reconciliation_record(ScheduledReconciliationRecord {
            binding,
            resolution: ScheduledReconciliationResolution::ConfirmedFailed {
                reason_code: "provider_adapter_reconciled_confirmed_failure".into(),
            },
            evidence_id: uuid::Uuid::new_v4().to_string(),
            issuer: ScheduledReconciliationIssuer::ProviderAdapterReconciler,
            evidence_kind: ScheduledReconciliationEvidenceKind::Failed,
            evidence_ref,
            evidence_digest,
            issued_at: chrono::Utc::now().to_rfc3339(),
            source_id,
        })
    }

    /// Issue a failure-only reconciliation capability from a live ToolGateway
    /// receipt whose exact durable dispatch row was already quarantined.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn issue_tool_failed_reconciliation(
        &self,
        task_id: &str,
        attempt_id: &str,
        receipt: ToolExecutionReceipt,
    ) -> Result<ScheduledReconciliationAdmission> {
        drop(self.lock_writable_connection("issue_tool_failed_reconciliation")?);
        receipt
            .mechanically_valid_terminal()
            .map_err(|reason| anyhow::anyhow!(reason))?;
        if !receipt.is_runtime_issued()
            || receipt.execution_outcome != ToolExecutionOutcome::Failed
            || receipt.effect_status != ToolEffectStatus::NotAttempted
            || receipt.transport_status != ToolTransportStatus::ResponseObserved
        {
            anyhow::bail!(
                "tool reconciliation requires a live ToolGateway definite-failure terminal"
            );
        }
        let manifest_id = receipt
            .manifest_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("tool reconciliation receipt has no manifest owner"))?;
        let source_run_digest = receipt.source_run_id.as_deref().map(digest_ref);
        let binding = {
            let conn = self.lock_writable_connection("issue_tool_failed_reconciliation")?;
            let binding = load_unknown_attempt_binding(
                &conn,
                Arc::clone(self.runtime_authority()?),
                task_id,
                attempt_id,
            )?
            .ok_or_else(|| anyhow::anyhow!("scheduled unknown attempt is unavailable"))?;
            if binding.provider_provenance_state != "exact" {
                anyhow::bail!("legacy tool execution cannot mint runtime reconciliation");
            }
            let exact: bool = conn.query_row(
                "SELECT COUNT(*) = 1 FROM scheduler_tool_dispatches
                 WHERE attempt_id = ?1 AND task_id = ?2 AND tool_receipt_id = ?3
                   AND request_digest = ?4 AND identity_state = 'exact'
                   AND receipt_started_at = ?5 AND dispatched_at IS ?6
                   AND manifest_digest = ?7 AND source_run_ref_digest IS ?8
                   AND action_effect = ?9 AND idempotency_contract = ?10
                   AND dispatch_kind = ?11 AND dispatch_attempt_count = ?12
                   AND status = 'unknown'",
                params![
                    attempt_id,
                    task_id,
                    receipt.receipt_id,
                    receipt.request_digest,
                    receipt.started_at.to_rfc3339(),
                    receipt.dispatched_at.map(|value| value.to_rfc3339()),
                    digest_ref(manifest_id),
                    source_run_digest,
                    receipt.action_effect.as_str(),
                    receipt.idempotency_contract.as_str(),
                    receipt.dispatch_kind.as_str(),
                    i64::from(receipt.dispatch_attempt_count),
                ],
                |row| row.get(0),
            )?;
            if !exact {
                anyhow::bail!(
                    "tool reconciliation receipt does not match the exact unknown dispatch"
                );
            }
            binding
        };
        let source_id = digest_parts(&[
            "scheduled_tool_reconciliation_runtime_source_v1",
            &receipt.receipt_id,
            &receipt.request_digest,
            &receipt
                .finished_at
                .expect("validated terminal")
                .to_rfc3339(),
        ]);
        let evidence_ref = format!("tool-gateway-terminal://{}", receipt.receipt_id);
        let evidence_digest =
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "schema": "scheduled_tool_reconciliation_source_v1",
                "sourceId": source_id,
                "receipt": receipt,
                "unknownAttemptDigest": scheduled_unknown_attempt_binding_digest(&binding),
            }))
            .1;
        self.issue_reconciliation_record(ScheduledReconciliationRecord {
            binding,
            resolution: ScheduledReconciliationResolution::ConfirmedFailed {
                reason_code: "tool_gateway_reconciled_confirmed_failure".into(),
            },
            evidence_id: uuid::Uuid::new_v4().to_string(),
            issuer: ScheduledReconciliationIssuer::ToolGatewayReconciler,
            evidence_kind: ScheduledReconciliationEvidenceKind::Failed,
            evidence_ref,
            evidence_digest,
            issued_at: chrono::Utc::now().to_rfc3339(),
            source_id,
        })
    }

    #[cfg(any(test, feature = "test-utils"))]
    fn issue_reconciliation_record(
        &self,
        record: ScheduledReconciliationRecord,
    ) -> Result<ScheduledReconciliationAdmission> {
        // Revalidate the canonical owner immediately before changing the
        // process-local one-shot capability registry.
        drop(self.lock_writable_connection("issue_reconciliation_record")?);
        let record_digest = scheduled_reconciliation_record_digest(&record);
        let mut issued = self
            .reconciliation_sources
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled reconciliation source registry poisoned"))?;
        if !issued.insert(record.source_id.clone()) {
            anyhow::bail!("scheduled reconciliation runtime source was already admitted");
        }
        drop(issued);
        Ok(ScheduledReconciliationAdmission {
            issuance_id: uuid::Uuid::new_v4(),
            record_digest,
            record: Some(record),
            issued_sources: Arc::clone(&self.reconciliation_sources),
        })
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn issue_scheduled_reconciliation_test_admission(
        &self,
        task_id: &str,
        attempt_id: &str,
        resolution: ScheduledReconciliationTestResolution,
    ) -> Result<ScheduledReconciliationAdmission> {
        drop(self.lock_writable_connection("issue_scheduled_reconciliation_test_admission")?);
        let binding = {
            let conn =
                self.lock_writable_connection("issue_scheduled_reconciliation_test_admission")?;
            load_unknown_attempt_binding(
                &conn,
                Arc::clone(self.runtime_authority()?),
                task_id,
                attempt_id,
            )?
            .ok_or_else(|| anyhow::anyhow!("scheduled unknown attempt is unavailable"))?
        };
        let (resolution, evidence_kind) = match resolution {
            ScheduledReconciliationTestResolution::RetrySafe => (
                ScheduledReconciliationResolution::RetrySafe,
                ScheduledReconciliationEvidenceKind::NoEffect,
            ),
            ScheduledReconciliationTestResolution::ConfirmedFailed { reason_code } => (
                ScheduledReconciliationResolution::ConfirmedFailed { reason_code },
                ScheduledReconciliationEvidenceKind::Failed,
            ),
            ScheduledReconciliationTestResolution::ConfirmedCompleted {
                result_ref,
                result_digest,
            } => (
                ScheduledReconciliationResolution::ConfirmedCompleted {
                    result_ref,
                    result_digest,
                },
                ScheduledReconciliationEvidenceKind::Completed,
            ),
        };
        let source_id = digest_parts(&[
            "scheduled_reconciliation_test_source_v1",
            task_id,
            attempt_id,
            evidence_kind.as_str(),
            &scheduled_unknown_attempt_binding_digest(&binding),
        ]);
        let evidence_digest = digest_parts(&[
            "scheduled_reconciliation_test_evidence_v1",
            &scheduled_unknown_attempt_binding_digest(&binding),
            evidence_kind.as_str(),
            &source_id,
        ]);
        self.issue_reconciliation_record(ScheduledReconciliationRecord {
            binding,
            resolution,
            evidence_id: uuid::Uuid::new_v4().to_string(),
            issuer: ScheduledReconciliationIssuer::NativeUserConfirmation,
            evidence_kind,
            evidence_ref: format!("test-runtime-reconciliation://{source_id}"),
            evidence_digest,
            issued_at: chrono::Utc::now().to_rfc3339(),
            source_id,
        })
    }

    /// Unknown effects can leave quarantine only by consuming a one-shot
    /// runtime admission issued from a real canonical observation owner.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn reconcile_unknown_attempt(
        &self,
        mut admission: ScheduledReconciliationAdmission,
    ) -> Result<bool> {
        let mut conn = self.lock_writable_connection("reconcile_unknown_attempt")?;
        admission.validate_for_store(self.runtime_authority()?)?;
        let record = admission
            .record
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("scheduled reconciliation admission already consumed"))?
            .clone();
        validate_reconciliation_record(&record)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current_binding = load_unknown_attempt_binding(
            &tx,
            Arc::clone(self.runtime_authority()?),
            &record.binding.task_id,
            &record.binding.attempt_id,
        )?;
        let Some(current_binding) = current_binding else {
            tx.commit()?;
            admission.finish_committed()?;
            return Ok(false);
        };
        if current_binding != record.binding {
            anyhow::bail!("scheduled reconciliation admission lost its exact unknown attempt");
        }
        let reconciliation_context = tx
            .query_row(
                "SELECT a.provider_provenance_state,
                        CASE WHEN t.provider_data_route = 'policy_allowed'
                                  AND EXISTS (
                                      SELECT 1
                                      FROM scheduler_provider_grant_consumptions c
                                      WHERE c.task_id = t.id
                                        AND c.attempt_id = a.attempt_id
                                        AND c.grant_id = t.provider_grant_id
                                  )
                             THEN 1 ELSE 0 END
                 FROM tasks t JOIN scheduler_attempts a ON a.task_id = t.id
                 WHERE t.id = ?1
                   AND t.status IN ('unknown', 'unknown_legacy_execution_state')
                   AND a.attempt_id = ?2 AND a.status = 'unknown'",
                params![record.binding.task_id, record.binding.attempt_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
            )
            .optional()?;
        let Some((_provenance_state, _reviewed_cloud_grant_consumed)) = reconciliation_context
        else {
            tx.commit()?;
            admission.finish_committed()?;
            return Ok(false);
        };
        let now_value = chrono::Utc::now();
        let now = now_value.to_rfc3339();
        type ScheduledReconciliationTransition = (
            &'static str,
            &'static str,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let (
            task_status,
            attempt_status,
            reason_code,
            result_ref,
            result_digest,
            completed_at,
            eligible_at,
        ): ScheduledReconciliationTransition = match record.resolution {
            #[cfg(any(test, feature = "test-utils"))]
            ScheduledReconciliationResolution::RetrySafe => {
                if record.evidence_kind != ScheduledReconciliationEvidenceKind::NoEffect {
                    anyhow::bail!("scheduled retry-safe reconciliation evidence kind mismatch");
                }
                if _reviewed_cloud_grant_consumed {
                    (
                        "review_required",
                        "reconciled_retry_safe",
                        "scheduled_cloud_grant_consumed_requires_fresh_review".to_string(),
                        None,
                        None,
                        None,
                        None,
                    )
                } else {
                    (
                        "pending",
                        "reconciled_retry_safe",
                        "reconciled_retry_safe".to_string(),
                        None,
                        None,
                        None,
                        Some(now.clone()),
                    )
                }
            }
            ScheduledReconciliationResolution::ConfirmedFailed { reason_code } => {
                validate_reason_code(&reason_code)?;
                if record.evidence_kind != ScheduledReconciliationEvidenceKind::Failed {
                    anyhow::bail!("scheduled failed reconciliation evidence kind mismatch");
                }
                (
                    "failed",
                    "reconciled_failed",
                    reason_code,
                    None,
                    None,
                    None,
                    None,
                )
            }
            #[cfg(any(test, feature = "test-utils"))]
            ScheduledReconciliationResolution::ConfirmedCompleted {
                result_ref,
                result_digest,
            } => {
                validate_scheduled_result_ref(&result_ref)?;
                validate_digest("reconciled result digest", &result_digest)?;
                if record.evidence_kind != ScheduledReconciliationEvidenceKind::Completed {
                    anyhow::bail!("scheduled completed reconciliation evidence kind mismatch");
                }
                (
                    "completed",
                    "reconciled_completed",
                    "reconciled_completed".to_string(),
                    Some(result_ref),
                    Some(result_digest),
                    Some(now.clone()),
                    None,
                )
            }
        };
        let task_changed = tx.execute(
            "UPDATE tasks SET status = ?1, last_error = ?2, result_ref = ?3,
                    result_digest = ?4, completed_at = ?5, eligible_at = ?6,
                    claim_token = NULL, lease_expires_at = NULL
             WHERE id = ?7 AND status IN ('unknown', 'unknown_legacy_execution_state')",
            params![
                task_status,
                reason_code,
                result_ref,
                result_digest,
                completed_at,
                eligible_at,
                record.binding.task_id
            ],
        )?;
        let attempt_changed = tx.execute(
            "UPDATE scheduler_attempts SET status = ?1, finished_at = COALESCE(finished_at, ?2),
                    reconciliation_evidence_digest = ?3, reconciliation_issuer = ?4,
                    reconciliation_evidence_kind = ?5, reconciliation_evidence_ref = ?6,
                    reconciled_at = ?2
             WHERE attempt_id = ?7 AND task_id = ?8 AND status = 'unknown'",
            params![
                attempt_status,
                now,
                record.evidence_digest,
                record.issuer.as_str(),
                record.evidence_kind.as_str(),
                record.evidence_ref,
                record.binding.attempt_id,
                record.binding.task_id
            ],
        )?;
        if task_changed != 1 || attempt_changed != 1 {
            anyhow::bail!("scheduled reconciliation lost its canonical unknown owner");
        }
        tx.commit()?;
        admission.finish_committed()?;
        Ok(true)
    }

    pub fn latest_attempt_for_task(&self, task_id: &str) -> Result<Option<ScheduledAttemptRecord>> {
        let conn = self.lock_connection()?;
        conn.query_row(
            "SELECT attempt_id, task_id, claim_token, attempt_number, status,
                    provider_grant_id, policy_version, data_route, policy_reason_code,
                    provider_subject_digest, provider_payload_purpose,
                    provider_payload_contract_digest, provider_source_ref_digest, error_digest,
                    reconciliation_evidence_digest, provider_provenance_state,
                    migration_associated_grant_id, reconciliation_issuer,
                    reconciliation_evidence_kind, reconciliation_evidence_ref,
                    process_epoch_id, writer_owner_generation_id
             FROM scheduler_attempts WHERE task_id = ?1
             ORDER BY attempt_number DESC LIMIT 1",
            params![task_id],
            map_attempt_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn provider_receipts_for_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<ScheduledProviderReceiptRecord>> {
        let conn = self.lock_connection()?;
        let mut stmt = conn.prepare(
            "SELECT request_id, attempt_id, task_id, claim_token, provider_grant_id,
                    provider_digest, model_digest, status, error_digest, simulated,
                    policy_evidence_state, policy_evidence_digest, subject_scope_digest,
                    payload_purpose, unfiltered_payload_digest, context_manifest_digest,
                    prepared_envelope_digest, prepared_request_digest,
                    network_policy_decision_digest,
                    migration_associated_grant_id, process_epoch_id,
                    writer_owner_generation_id
             FROM scheduler_provider_receipts WHERE attempt_id = ?1 ORDER BY started_at, request_id",
        )?;
        let rows = stmt.query_map(params![attempt_id], map_provider_receipt_row)?;
        rows.map(|row| row.map_err(anyhow::Error::from)).collect()
    }

    /// Single authority gate for every TaskStore mutation. The observation
    /// handle is rejected before the SQLite mutex/connection is touched; a
    /// writable handle then revalidates the retained database and owner-lock
    /// identities before returning the connection guard.
    fn lock_writable_connection(
        &self,
        operation: &str,
    ) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.require_mutation_authority(operation)?;
        self.validate_persistent_owner_envelope()?;
        self.conn.lock()
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.validate_persistent_owner_envelope()?;
        self.conn.lock()
    }

    fn validate_persistent_owner_envelope(&self) -> Result<()> {
        let (Some(owner_lease), Some(database_slot), Some(expected)) = (
            self.persistent_owner_lease.as_ref(),
            self.database_slot.as_ref(),
            self.persistent_owner_envelope.as_ref(),
        ) else {
            return Ok(());
        };
        if let Some(reason) = self
            .owner_envelope_poisoned
            .lock()
            .map_err(|error| anyhow::anyhow!("task_store_owner_envelope_mutex_poisoned:{error}"))?
            .clone()
        {
            anyhow::bail!("task_store_owner_envelope_poisoned:{reason}");
        }
        let validation = (|| {
            let bytes =
                owner_lease.read_owner_lock_envelope(MAX_TASK_STORE_OWNER_ENVELOPE_BYTES)?;
            if bytes.is_empty() {
                anyhow::bail!("task_store_owner_envelope_missing");
            }
            let observed: TaskStoreOwnerEnvelopeV1 = serde_json::from_slice(&bytes)
                .map_err(|error| anyhow::anyhow!("task_store_owner_envelope_invalid:{error}"))?;
            if observed != *expected {
                anyhow::bail!("task_store_owner_envelope_changed");
            }
            database_slot.verify_owner_envelope(
                &observed,
                &owner_lease.database_identity_material()?,
                &owner_lease.owner_lock_identity_material(),
            )
        })();
        if let Err(error) = validation {
            let reason = error.to_string();
            *self.owner_envelope_poisoned.lock().map_err(|poison| {
                anyhow::anyhow!("task_store_owner_envelope_mutex_poisoned:{poison}")
            })? = Some(reason.clone());
            anyhow::bail!("task_store_owner_envelope_poisoned:{reason}");
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct AttemptDispatchEvidence {
    provider_started: usize,
    provider_completed: usize,
    provider_failed: usize,
    provider_unknown: usize,
    provider_legacy: usize,
    tool_started: usize,
    tool_returned: usize,
    tool_unknown: usize,
}

impl AttemptDispatchEvidence {
    fn tool_ambiguous(&self) -> usize {
        self.tool_started + self.tool_unknown
    }

    fn total_dispatches(&self) -> usize {
        self.provider_started
            + self.provider_completed
            + self.provider_failed
            + self.provider_unknown
            + self.provider_legacy
            + self.tool_started
            + self.tool_returned
            + self.tool_unknown
    }
}

fn attempt_dispatch_evidence(
    tx: &Transaction<'_>,
    attempt_id: &str,
) -> Result<AttemptDispatchEvidence> {
    let (provider_started, provider_completed, provider_failed, provider_unknown, provider_legacy) =
        tx.query_row(
            "SELECT
            SUM(CASE WHEN status = 'started' AND policy_evidence_state = 'exact'
                      AND prepared_request_digest IS NOT NULL THEN 1 ELSE 0 END),
            SUM(CASE WHEN status = 'completed' AND simulated = 0
                      AND policy_evidence_state = 'exact'
                      AND prepared_request_digest IS NOT NULL THEN 1 ELSE 0 END),
            SUM(CASE WHEN status = 'failed' AND policy_evidence_state = 'exact'
                      AND prepared_request_digest IS NOT NULL THEN 1 ELSE 0 END),
            SUM(CASE WHEN status = 'remote_unknown' AND policy_evidence_state = 'exact'
                      AND prepared_request_digest IS NOT NULL THEN 1 ELSE 0 END),
            SUM(CASE WHEN policy_evidence_state = 'legacy_unavailable'
                      OR prepared_request_digest IS NULL THEN 1 ELSE 0 END)
         FROM scheduler_provider_receipts WHERE attempt_id = ?1",
            params![attempt_id],
            |row| {
                Ok((
                    row.get::<_, Option<usize>>(0)?.unwrap_or(0),
                    row.get::<_, Option<usize>>(1)?.unwrap_or(0),
                    row.get::<_, Option<usize>>(2)?.unwrap_or(0),
                    row.get::<_, Option<usize>>(3)?.unwrap_or(0),
                    row.get::<_, Option<usize>>(4)?.unwrap_or(0),
                ))
            },
        )?;
    let (tool_started, tool_returned, tool_unknown) = tx.query_row(
        "SELECT
            SUM(CASE WHEN status = 'started' THEN 1 ELSE 0 END),
            SUM(CASE WHEN status = 'returned' THEN 1 ELSE 0 END),
            SUM(CASE WHEN status = 'unknown' THEN 1 ELSE 0 END)
         FROM scheduler_tool_dispatches WHERE attempt_id = ?1",
        params![attempt_id],
        |row| {
            Ok((
                row.get::<_, Option<usize>>(0)?.unwrap_or(0),
                row.get::<_, Option<usize>>(1)?.unwrap_or(0),
                row.get::<_, Option<usize>>(2)?.unwrap_or(0),
            ))
        },
    )?;
    Ok(AttemptDispatchEvidence {
        provider_started,
        provider_completed,
        provider_failed,
        provider_unknown,
        provider_legacy,
        tool_started,
        tool_returned,
        tool_unknown,
    })
}

fn settle_active_claim(
    tx: &Transaction<'_>,
    claim: &ScheduledTaskClaim,
    task_status: &str,
    attempt_status: &str,
    reason_code: &str,
    error_digest: Option<&str>,
    retry_after_backoff: bool,
) -> Result<()> {
    let now_value = chrono::Utc::now();
    let now = now_value.to_rfc3339();
    let eligible_at = retry_after_backoff
        .then(|| retry_eligible_at(now_value, claim.attempt_number).to_rfc3339());
    let task_changed = tx.execute(
        "UPDATE tasks SET status = ?1, last_error = ?2, claim_token = NULL,
                lease_expires_at = NULL, eligible_at = ?3
         WHERE id = ?4 AND status = 'running' AND claim_token = ?5",
        params![
            task_status,
            reason_code,
            eligible_at,
            claim.task.id,
            claim.claim_token
        ],
    )?;
    let attempt_changed = tx.execute(
        "UPDATE scheduler_attempts SET status = ?1, finished_at = ?2, error_digest = ?3
         WHERE attempt_id = ?4 AND task_id = ?5 AND claim_token = ?6 AND status = 'executing'
           AND process_epoch_id = ?7 AND writer_owner_generation_id = ?8",
        params![
            attempt_status,
            now,
            error_digest,
            claim.attempt_id,
            claim.task.id,
            claim.claim_token,
            claim
                .policy_authority_proof
                .store
                .process_epoch_id
                .to_string(),
            claim
                .policy_authority_proof
                .store
                .writer_owner_generation_id
                .to_string(),
        ],
    )?;
    if task_changed != 1 || attempt_changed != 1 {
        anyhow::bail!("scheduled claim settlement lost its canonical owner");
    }
    Ok(())
}

fn ensure_executing_attempt(tx: &Transaction<'_>, claim: &ScheduledTaskClaim) -> Result<()> {
    if !executing_attempt_active(tx, claim)? {
        anyhow::bail!("scheduled execution no longer owns its durable claim");
    }
    Ok(())
}

fn scheduled_cloud_grant_consumed(conn: &Connection, claim: &ScheduledTaskClaim) -> Result<bool> {
    if claim.provider_grant.data_route != ProviderDataRoute::PolicyAllowed {
        return Ok(false);
    }
    conn.query_row(
        "SELECT COUNT(*) = 1 FROM scheduler_provider_grant_consumptions
         WHERE grant_id = ?1 AND task_id = ?2 AND attempt_id = ?3",
        params![
            claim.provider_grant.grant_id,
            claim.task.id,
            claim.attempt_id,
        ],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn validate_provider_evidence_for_claim(
    claim: &ScheduledTaskClaim,
    evidence: &ProviderPolicyReceiptEvidence,
) -> Result<()> {
    claim.validate_policy_authority()?;
    evidence.validate_minimal_truth()?;
    let expected_subject =
        ProviderPolicyAuthorization::from_scheduled_claim(claim)?.subject_scope_digest();
    if evidence.issuing_authority != ProviderPolicyAuthority::ScheduledPolicy
        || evidence.decision_id != claim.provider_grant.policy_decision_digest
        || evidence.policy_version != claim.provider_grant.policy_version
        || evidence.effective_data_route != claim.provider_grant.data_route
        || evidence.subject_scope_digest != expected_subject
        || evidence.payload_purpose != Some(claim.provider_grant.payload_purpose)
        || evidence.declared_payload_categories
            != [crate::llm::ProviderPayloadCategory::RuntimeCompiledMessages]
    {
        anyhow::bail!("scheduled provider policy evidence does not match its exact attempt grant");
    }
    Ok(())
}

fn validate_provider_truth_record_for_claim(
    claim: &ScheduledTaskClaim,
    record: &ScheduledProviderTruthRecord,
) -> Result<()> {
    claim.validate_policy_authority()?;
    validate_provider_evidence_for_claim(claim, record.policy_evidence())?;
    validate_reference("provider request id", record.request_id())?;
    validate_reference("provider", record.provider())?;
    validate_reference("model", record.model())?;
    validate_digest(
        "scheduled prepared provider request",
        record.prepared_request_digest(),
    )?;
    if claim.provider_grant.is_expired_at(&record.started_at())? {
        anyhow::bail!("scheduled provider grant expired at the adapter dispatch boundary");
    }
    if claim.provider_grant.data_route == ProviderDataRoute::LocalOnly
        && record.provider() != "ollama"
    {
        anyhow::bail!("local-only scheduled policy rejected a cloud provider transition");
    }
    if !claim
        .provider_grant
        .target_matches(record.provider(), record.model())
    {
        anyhow::bail!("scheduled provider transition differs from its reviewed provider/model");
    }
    match record.transition() {
        ScheduledProviderTruthTransition::Started => {
            if record.finished_at().is_some() || record.error_digest().is_some() {
                anyhow::bail!("scheduled provider start carries terminal fields");
            }
        }
        ScheduledProviderTruthTransition::Completed => {
            if record.finished_at().is_none() || record.error_digest().is_some() {
                anyhow::bail!("scheduled provider completion shape is invalid");
            }
        }
        ScheduledProviderTruthTransition::Failed
        | ScheduledProviderTruthTransition::RemoteUnknown => {
            if record.finished_at().is_none() || record.error_digest().is_none() {
                anyhow::bail!("scheduled provider non-success terminal shape is invalid");
            }
        }
    }
    if let Some(error_digest) = record.error_digest() {
        validate_digest("provider error digest", error_digest)?;
    }
    Ok(())
}

fn ensure_exact_provider_truth_owner(
    tx: &Transaction<'_>,
    claim: &ScheduledTaskClaim,
) -> Result<()> {
    claim.validate_policy_authority()?;
    let current = load_task(tx, &claim.task.id)?;
    let expected = &claim.task;
    let exact_task_revision = current.id == expected.id
        && current.title == expected.title
        && current.description == expected.description
        && current.due_date == expected.due_date
        && current.priority == expected.priority
        && current.status == expected.status
        && current.created_at == expected.created_at
        && current.completed_at == expected.completed_at
        && current.source_run_id == expected.source_run_id
        && current.source_proposal_id == expected.source_proposal_id
        && current.action_type == expected.action_type
        && current.attempt_count == expected.attempt_count
        && current.claim_token.as_deref() == Some(claim.claim_token.as_str())
        && current.claim_token == expected.claim_token
        && current.lease_expires_at == expected.lease_expires_at
        && current.last_error == expected.last_error
        && current.result_digest == expected.result_digest
        && current.result_ref == expected.result_ref
        && current.provider_grant == claim.provider_grant
        && expected.provider_grant == claim.provider_grant;
    if !exact_task_revision {
        anyhow::bail!("scheduled provider truth lost its exact canonical task revision");
    }
    let exact_attempt: bool = tx.query_row(
        "SELECT COUNT(*) = 1 FROM scheduler_attempts
         WHERE attempt_id = ?1 AND task_id = ?2 AND claim_token = ?3
           AND attempt_number = ?4 AND status = 'executing'
           AND provider_grant_id = ?5 AND policy_version = ?6 AND data_route = ?7
           AND policy_reason_code = ?8 AND provider_subject_digest = ?9
           AND provider_payload_purpose = ?10
           AND provider_payload_contract_digest = ?11
           AND provider_source_ref_digest = ?12 AND provider_provenance_state = 'exact'
           AND process_epoch_id = ?13 AND writer_owner_generation_id = ?14",
        params![
            claim.attempt_id,
            claim.task.id,
            claim.claim_token,
            claim.attempt_number,
            claim.provider_grant.grant_id,
            claim.provider_grant.policy_version,
            provider_data_route_label(claim.provider_grant.data_route),
            claim.provider_grant.reason_code,
            claim.provider_grant.subject_digest,
            claim.provider_grant.payload_purpose.as_str(),
            claim.provider_grant.payload_contract_digest,
            claim.provider_grant.source_ref_digest,
            claim
                .policy_authority_proof
                .store
                .process_epoch_id
                .to_string(),
            claim
                .policy_authority_proof
                .store
                .writer_owner_generation_id
                .to_string(),
        ],
        |row| row.get(0),
    )?;
    if !exact_attempt {
        anyhow::bail!("scheduled provider truth lost its exact canonical attempt/grant");
    }
    Ok(())
}

fn executing_attempt_active(conn: &Connection, claim: &ScheduledTaskClaim) -> Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) = 1
         FROM tasks t JOIN scheduler_attempts a ON a.task_id = t.id
         WHERE t.id = ?1 AND t.status = 'running' AND t.claim_token = ?2
           AND a.attempt_id = ?3 AND a.claim_token = ?2 AND a.status = 'executing'
           AND a.provider_grant_id = ?4 AND a.process_epoch_id = ?5
           AND a.writer_owner_generation_id = ?6",
        params![
            claim.task.id,
            claim.claim_token,
            claim.attempt_id,
            claim.provider_grant.grant_id,
            claim
                .policy_authority_proof
                .store
                .process_epoch_id
                .to_string(),
            claim
                .policy_authority_proof
                .store
                .writer_owner_generation_id
                .to_string(),
        ],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn active_attempt_status(conn: &Connection, claim: &ScheduledTaskClaim) -> Result<Option<String>> {
    conn.query_row(
        "SELECT a.status
         FROM tasks t JOIN scheduler_attempts a ON a.task_id = t.id
         WHERE t.id = ?1 AND t.status = 'running' AND t.claim_token = ?2
           AND a.attempt_id = ?3 AND a.claim_token = ?2
           AND a.process_epoch_id = ?4 AND a.writer_owner_generation_id = ?5",
        params![
            claim.task.id,
            claim.claim_token,
            claim.attempt_id,
            claim
                .policy_authority_proof
                .store
                .process_epoch_id
                .to_string(),
            claim
                .policy_authority_proof
                .store
                .writer_owner_generation_id
                .to_string(),
        ],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn insert_task(conn: &Connection, task: &ScheduledTask) -> Result<usize> {
    let eligible_at = task
        .due_date
        .as_deref()
        .map(|due_date| {
            chrono::DateTime::parse_from_rfc3339(due_date)
                .map(|value| value.with_timezone(&chrono::Utc).to_rfc3339())
        })
        .transpose()
        .context("scheduled task due_date must be RFC3339")?;
    Ok(conn.execute(
        "INSERT OR IGNORE INTO tasks (
                id, title, description, due_date, priority, status, created_at,
                completed_at, source_run_id, source_proposal_id, action_type,
                attempt_count, claim_token, lease_expires_at, last_error, result_digest, result_ref,
                eligible_at, provider_grant_id, provider_policy_version, provider_data_route,
                provider_reason_code, provider_subject_digest, provider_payload_purpose,
                provider_payload_contract_digest, provider_source_ref_digest,
                provider_schedule_digest, provider_grant_scope, provider_grant_expires_at,
                provider_target_digest, model_target_digest, review_snapshot_digest,
                review_dispatch_claim_digest, provider_policy_decision_digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                       ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26,
                       ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34)",
        params![
            task.id,
            task.title,
            task.description,
            task.due_date,
            task.priority,
            task.status,
            task.created_at,
            task.completed_at,
            task.source_run_id,
            task.source_proposal_id,
            task.action_type,
            task.attempt_count,
            task.claim_token,
            task.lease_expires_at,
            task.last_error,
            task.result_digest,
            task.result_ref,
            eligible_at,
            task.provider_grant.grant_id,
            task.provider_grant.policy_version,
            provider_data_route_label(task.provider_grant.data_route),
            task.provider_grant.reason_code,
            task.provider_grant.subject_digest,
            task.provider_grant.payload_purpose.as_str(),
            task.provider_grant.payload_contract_digest,
            task.provider_grant.source_ref_digest,
            task.provider_grant.schedule_digest,
            task.provider_grant.grant_scope.as_str(),
            task.provider_grant.grant_expires_at,
            task.provider_grant.provider_digest,
            task.provider_grant.model_digest,
            task.provider_grant.review_snapshot_digest,
            task.provider_grant.review_dispatch_claim_digest,
            task.provider_grant.policy_decision_digest,
        ],
    )?)
}

fn load_task(tx: &Transaction<'_>, task_id: &str) -> Result<ScheduledTask> {
    load_task_from_connection(tx, task_id)
}

fn load_task_from_connection(conn: &Connection, task_id: &str) -> Result<ScheduledTask> {
    conn.query_row(
        "SELECT id, title, description, due_date, priority, status, created_at, completed_at,
                source_run_id, source_proposal_id, action_type, attempt_count, claim_token,
                lease_expires_at, last_error, result_digest, result_ref, provider_grant_id,
                provider_policy_version, provider_data_route, provider_reason_code,
                provider_subject_digest, provider_payload_purpose,
                provider_payload_contract_digest, provider_source_ref_digest,
                provider_schedule_digest, provider_grant_scope, provider_grant_expires_at,
                provider_target_digest, model_target_digest, review_snapshot_digest,
                review_dispatch_claim_digest, provider_policy_decision_digest
         FROM tasks WHERE id = ?1",
        params![task_id],
        map_row,
    )
    .map_err(Into::into)
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "secure_delete", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    Ok(())
}

fn complete_task_store_physical_purge(conn: &mut Connection) -> Result<()> {
    // Logical DELETE is not sufficient for a privacy quarantine: old SQLite
    // pages and WAL frames can retain task prose or a malicious oversized
    // status. The completion marker is deliberately written only after both
    // the committed migration and the physical rewrite succeed. A crash before
    // that write leaves the marker absent, so the next open repeats this step.
    require_complete_task_store_wal_truncate(conn)?;
    conn.execute_batch("VACUUM")?;
    require_complete_task_store_wal_truncate(conn)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(
        "INSERT INTO task_store_metadata (key, value) VALUES (?1, 'complete')",
        [TASK_STORE_PRE_V13_PURGE_COMPLETE_METADATA_KEY],
    )?;
    tx.commit()?;
    require_complete_task_store_wal_truncate(conn)?;
    Ok(())
}

fn require_complete_task_store_wal_truncate(conn: &Connection) -> Result<()> {
    let (busy, log_frames, checkpointed_frames) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
    if busy != 0 || log_frames != 0 || checkpointed_frames != 0 {
        anyhow::bail!(
            "task_store_wal_checkpoint_incomplete:busy={busy}:log_frames={log_frames}:checkpointed_frames={checkpointed_frames}"
        );
    }
    Ok(())
}

fn task_store_schema_version(conn: &Connection) -> Result<Option<i64>> {
    let versions_table_exists: bool = conn.query_row(
        "SELECT COUNT(*) = 1 FROM sqlite_master
         WHERE type = 'table' AND name = 'openlife_schema_versions'",
        [],
        |row| row.get(0),
    )?;
    if !versions_table_exists {
        return Ok(None);
    }
    conn.query_row(
        "SELECT version FROM openlife_schema_versions WHERE component = 'task_store'",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

/// Authenticate an already-current persistent TaskStore's owner-lock inode
/// before any connection PRAGMA, migration, or other writable SQLite action.
/// Older schemas may bind their first lock identity only inside the versioned
/// migration transaction; a current schema may never silently rebind it.
fn preflight_existing_task_store_owner_lock_binding(
    conn: &Connection,
    database_slot: &TaskStoreDatabaseSlot,
    owner_lock_identity_material: &str,
) -> Result<()> {
    let existing_schema_version = task_store_schema_version(conn)?.unwrap_or(0);
    if existing_schema_version > TASK_STORE_SCHEMA_VERSION {
        anyhow::bail!("task store schema is newer than this OpenLife build");
    }
    if !task_store_table_exists(conn, "task_store_metadata")? {
        if existing_schema_version >= TASK_STORE_SCHEMA_VERSION {
            anyhow::bail!("task_store_owner_lock_authority_metadata_missing");
        }
        return Ok(());
    }
    let existing_identity = task_store_metadata_value(conn, TASK_STORE_IDENTITY_METADATA_KEY)?;
    let existing_slot_verifier =
        task_store_metadata_value(conn, TASK_STORE_SLOT_VERIFIER_METADATA_KEY)?;
    match (existing_identity, existing_slot_verifier) {
        (Some(store_identity), Some(verifier)) => {
            validate_task_store_identity(&store_identity)?;
            if !database_slot.verify_store_identity(&store_identity, &verifier) {
                anyhow::bail!("task_store_database_slot_authentication_failed");
            }
        }
        (None, None) if existing_schema_version < 13 => {}
        (None, None) => anyhow::bail!("task_store_canonical_authority_metadata_missing"),
        _ => anyhow::bail!("task_store_canonical_authority_metadata_incomplete"),
    }
    match task_store_metadata_value(conn, TASK_STORE_OWNER_LOCK_VERIFIER_METADATA_KEY)? {
        Some(verifier)
            if database_slot
                .verify_owner_lock_identity(owner_lock_identity_material, &verifier) =>
        {
            Ok(())
        }
        Some(_) => anyhow::bail!("task_store_owner_lock_authentication_failed"),
        None if existing_schema_version < TASK_STORE_SCHEMA_VERSION => Ok(()),
        None => anyhow::bail!("task_store_owner_lock_authority_metadata_missing"),
    }
}

fn validate_task_store_identity(value: &str) -> Result<()> {
    let encoded = value
        .strip_prefix("task-store:")
        .ok_or_else(|| anyhow::anyhow!("task_store_canonical_identity_invalid"))?;
    let parsed = uuid::Uuid::parse_str(encoded)
        .map_err(|_| anyhow::anyhow!("task_store_canonical_identity_invalid"))?;
    if parsed.get_version_num() != 4 || parsed.to_string() != encoded {
        anyhow::bail!("task_store_canonical_identity_invalid");
    }
    Ok(())
}

fn task_store_metadata_value(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM task_store_metadata WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn runtime_task_store_authority(
    store_identity: String,
    database_slot_verifier: String,
    writer_owner_generation_id: uuid::Uuid,
) -> Result<TaskStoreRuntimeAuthority> {
    Ok(TaskStoreRuntimeAuthority {
        canonical_store_identity: Arc::from(store_identity),
        database_slot_verifier: Arc::from(database_slot_verifier),
        process_epoch_id: task_store_process_epoch_id(),
        writer_owner_generation_id,
        claim_sealing_key: Some(TaskStoreAuthorityKey::random()?),
        available: true,
    })
}

struct PreAuthorityTaskStoreTruth {
    task_id: String,
    task_status: LegacyStatusSnapshot,
    completed_at: Option<String>,
    result_digest: Option<String>,
    result_ref: Option<String>,
    last_error: Option<String>,
    attempt_id: Option<String>,
    attempt_status: Option<LegacyStatusSnapshot>,
    attempt_error_digest: Option<String>,
    provider_statuses: Vec<LegacyStatusSnapshot>,
    tool_statuses: Vec<LegacyStatusSnapshot>,
}

struct LegacyStatusSnapshot {
    category: String,
    bounded_source_digest: String,
}

#[derive(Clone, Copy)]
enum LegacyStatusDomain {
    Task,
    Attempt,
    Provider,
    Tool,
}

fn legacy_status_category_sql(domain: LegacyStatusDomain) -> &'static str {
    match domain {
        LegacyStatusDomain::Task => {
            "CASE status
                WHEN 'pending' THEN 'pending'
                WHEN 'running' THEN 'running'
                WHEN 'completed' THEN 'completed'
                WHEN 'failed' THEN 'failed'
                WHEN 'unknown' THEN 'unknown'
                WHEN 'unknown_legacy_execution_state' THEN 'unknown_legacy_execution_state'
                WHEN 'review_required' THEN 'review_required'
                WHEN 'cancelled' THEN 'cancelled'
                ELSE 'unknown_legacy_status' END"
        }
        LegacyStatusDomain::Attempt => {
            "CASE status
                WHEN 'claimed' THEN 'claimed'
                WHEN 'executing' THEN 'executing'
                WHEN 'completed' THEN 'completed'
                WHEN 'failed' THEN 'failed'
                WHEN 'unknown' THEN 'unknown'
                WHEN 'pre_dispatch_failed' THEN 'pre_dispatch_failed'
                WHEN 'pre_dispatch_timeout' THEN 'pre_dispatch_timeout'
                WHEN 'expired_before_execution' THEN 'expired_before_execution'
                WHEN 'reconciled_retry_safe' THEN 'reconciled_retry_safe'
                WHEN 'reconciled_failed' THEN 'reconciled_failed'
                WHEN 'reconciled_completed' THEN 'reconciled_completed'
                ELSE 'unknown_legacy_status' END"
        }
        LegacyStatusDomain::Provider => {
            "CASE status
                WHEN 'started' THEN 'started'
                WHEN 'completed' THEN 'completed'
                WHEN 'failed' THEN 'failed'
                WHEN 'remote_unknown' THEN 'remote_unknown'
                ELSE 'unknown_legacy_status' END"
        }
        LegacyStatusDomain::Tool => {
            "CASE status
                WHEN 'started' THEN 'started'
                WHEN 'returned' THEN 'returned'
                WHEN 'unknown' THEN 'unknown'
                ELSE 'unknown_legacy_status' END"
        }
    }
}

fn bounded_legacy_status_source_digest(prefix_hex: &str, byte_length: i64) -> String {
    crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
        "schema": "bounded_legacy_status_source_v1",
        "byteLength": byte_length.max(0),
        "prefixHex": prefix_hex,
    }))
    .1
}

fn legacy_status_snapshot_from_row(
    row: &rusqlite::Row<'_>,
    category_index: usize,
    prefix_index: usize,
    length_index: usize,
) -> rusqlite::Result<LegacyStatusSnapshot> {
    let category = row.get::<_, String>(category_index)?;
    let prefix_hex = row.get::<_, String>(prefix_index)?;
    let byte_length = row.get::<_, i64>(length_index)?;
    Ok(LegacyStatusSnapshot {
        category,
        bounded_source_digest: bounded_legacy_status_source_digest(&prefix_hex, byte_length),
    })
}

fn legacy_optional_column_expression(
    conn: &Connection,
    table: &str,
    column: &str,
) -> Result<&'static str> {
    if table_has_column(conn, table, column)? {
        Ok(match column {
            "completed_at" => "completed_at",
            "result_digest" => "result_digest",
            "result_ref" => "result_ref",
            "last_error" => "last_error",
            "error_digest" => "error_digest",
            _ => anyhow::bail!("unsupported legacy task-store snapshot column"),
        })
    } else {
        Ok("NULL")
    }
}

fn task_store_table_exists(conn: &Connection, table: &str) -> Result<bool> {
    if !table
        .bytes()
        .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
    {
        anyhow::bail!("invalid task-store migration table identifier");
    }
    conn.query_row(
        "SELECT COUNT(*) = 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn retire_pre_authority_task_store_rows(conn: &Connection) -> Result<()> {
    for table in [
        "scheduler_provider_grant_consumptions",
        "scheduler_provider_receipts",
        "scheduler_tool_dispatches",
        "scheduler_attempts",
        "tasks",
    ] {
        if task_store_table_exists(conn, table)? {
            conn.execute(&format!("DELETE FROM {table}"), [])?;
        }
    }
    Ok(())
}

fn legacy_statuses_for_attempt(
    conn: &Connection,
    table: &str,
    task_id: &str,
    attempt_id: Option<&str>,
    domain: LegacyStatusDomain,
) -> Result<Vec<LegacyStatusSnapshot>> {
    if !table_has_column(conn, table, "status")? || !table_has_column(conn, table, "task_id")? {
        return Ok(Vec::new());
    }
    let category = legacy_status_category_sql(domain);
    let rows = match attempt_id {
        Some(attempt_id) if table_has_column(conn, table, "attempt_id")? => {
            let mut statement = conn.prepare(&format!(
                "SELECT {category}, hex(substr(CAST(status AS BLOB), 1, 256)),
                        COALESCE(length(CAST(status AS BLOB)), 0)
                 FROM {table} WHERE task_id = ?1 AND attempt_id = ?2 ORDER BY 1, 2, 3"
            ))?;
            let statuses = statement
                .query_map(params![task_id, attempt_id], |row| {
                    legacy_status_snapshot_from_row(row, 0, 1, 2)
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            statuses
        }
        _ => {
            let mut statement = conn.prepare(&format!(
                "SELECT {category}, hex(substr(CAST(status AS BLOB), 1, 256)),
                        COALESCE(length(CAST(status AS BLOB)), 0)
                 FROM {table} WHERE task_id = ?1 ORDER BY 1, 2, 3"
            ))?;
            let statuses = statement
                .query_map([task_id], |row| {
                    legacy_status_snapshot_from_row(row, 0, 1, 2)
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            statuses
        }
    };
    Ok(rows)
}

fn capture_pre_authority_task_store_truth(
    conn: &Connection,
    existing_schema_version: i64,
) -> Result<Vec<PreAuthorityTaskStoreTruth>> {
    if !(1..13).contains(&existing_schema_version) {
        return Ok(Vec::new());
    }
    if !table_has_column(conn, "tasks", "id")? || !table_has_column(conn, "tasks", "status")? {
        anyhow::bail!("pre-v13 task store is missing canonical task identity or status");
    }
    let completed_at = legacy_optional_column_expression(conn, "tasks", "completed_at")?;
    let result_digest = legacy_optional_column_expression(conn, "tasks", "result_digest")?;
    let result_ref = legacy_optional_column_expression(conn, "tasks", "result_ref")?;
    let last_error = legacy_optional_column_expression(conn, "tasks", "last_error")?;
    let task_status_category = legacy_status_category_sql(LegacyStatusDomain::Task);
    let task_rows = {
        let mut statement = conn.prepare(&format!(
            "SELECT id, {task_status_category},
                    hex(substr(CAST(status AS BLOB), 1, 256)),
                    COALESCE(length(CAST(status AS BLOB)), 0),
                    {completed_at}, {result_digest}, {result_ref}, {last_error}
             FROM tasks ORDER BY id"
        ))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    legacy_status_snapshot_from_row(row, 1, 2, 3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    let attempts_available = table_has_column(conn, "scheduler_attempts", "attempt_id")?
        && table_has_column(conn, "scheduler_attempts", "task_id")?
        && table_has_column(conn, "scheduler_attempts", "status")?;
    let attempt_error = if attempts_available {
        legacy_optional_column_expression(conn, "scheduler_attempts", "error_digest")?
    } else {
        "NULL"
    };
    let attempt_status_category = legacy_status_category_sql(LegacyStatusDomain::Attempt);
    let mut snapshot = Vec::new();
    for (task_id, task_status, completed_at, result_digest, result_ref, last_error) in task_rows {
        let attempts = if attempts_available {
            let mut statement = conn.prepare(&format!(
                "SELECT attempt_id, {attempt_status_category},
                        hex(substr(CAST(status AS BLOB), 1, 256)),
                        COALESCE(length(CAST(status AS BLOB)), 0), {attempt_error}
                 FROM scheduler_attempts WHERE task_id = ?1 ORDER BY attempt_id"
            ))?;
            let rows = statement
                .query_map([task_id.as_str()], |row| {
                    Ok((
                        Some(row.get::<_, String>(0)?),
                        Some(legacy_status_snapshot_from_row(row, 1, 2, 3)?),
                        row.get::<_, Option<String>>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        } else {
            Vec::new()
        };
        let attempts = if attempts.is_empty() {
            vec![(None, None, None)]
        } else {
            attempts
        };
        for (attempt_id, attempt_status, attempt_error_digest) in attempts {
            let provider_statuses = legacy_statuses_for_attempt(
                conn,
                "scheduler_provider_receipts",
                &task_id,
                attempt_id.as_deref(),
                LegacyStatusDomain::Provider,
            )?;
            let tool_statuses = legacy_statuses_for_attempt(
                conn,
                "scheduler_tool_dispatches",
                &task_id,
                attempt_id.as_deref(),
                LegacyStatusDomain::Tool,
            )?;
            snapshot.push(PreAuthorityTaskStoreTruth {
                task_id: task_id.clone(),
                task_status: LegacyStatusSnapshot {
                    category: task_status.category.clone(),
                    bounded_source_digest: task_status.bounded_source_digest.clone(),
                },
                completed_at: completed_at.clone(),
                result_digest: result_digest.clone(),
                result_ref: result_ref.clone(),
                last_error: last_error.clone(),
                attempt_id,
                attempt_status,
                attempt_error_digest,
                provider_statuses,
                tool_statuses,
            });
        }
    }
    Ok(snapshot)
}

fn bind_task_store_persistence_authority(
    conn: &Connection,
    database_slot: &TaskStoreDatabaseSlot,
    owner_lock_identity_material: &str,
    preauthenticated_store_identity: Option<&str>,
    existing_schema_version: i64,
    pre_authority_truth: &[PreAuthorityTaskStoreTruth],
    writer_owner_generation_id: uuid::Uuid,
) -> Result<TaskStoreRuntimeAuthority> {
    if (1..13).contains(&existing_schema_version) {
        quarantine_pre_authority_task_store_truth(
            conn,
            existing_schema_version,
            pre_authority_truth,
        )?;
    }
    let existing_identity = task_store_metadata_value(conn, TASK_STORE_IDENTITY_METADATA_KEY)?;
    let existing_verifier = task_store_metadata_value(conn, TASK_STORE_SLOT_VERIFIER_METADATA_KEY)?;
    let expected_owner_lock_verifier =
        database_slot.sign_owner_lock_identity(owner_lock_identity_material);
    let existing_owner_lock_verifier =
        task_store_metadata_value(conn, TASK_STORE_OWNER_LOCK_VERIFIER_METADATA_KEY)?;
    match existing_owner_lock_verifier {
        Some(verifier)
            if database_slot
                .verify_owner_lock_identity(owner_lock_identity_material, &verifier) => {}
        Some(_) => anyhow::bail!("task_store_owner_lock_authentication_failed"),
        None if existing_schema_version < TASK_STORE_SCHEMA_VERSION => {
            conn.execute(
                "INSERT INTO task_store_metadata (key, value) VALUES (?1, ?2)",
                params![
                    TASK_STORE_OWNER_LOCK_VERIFIER_METADATA_KEY,
                    expected_owner_lock_verifier
                ],
            )?;
        }
        None => anyhow::bail!("task_store_owner_lock_authority_metadata_missing"),
    }
    match (existing_identity, existing_verifier) {
        (Some(store_identity), Some(verifier)) => {
            validate_task_store_identity(&store_identity)?;
            if preauthenticated_store_identity.is_some_and(|expected| expected != store_identity)
                || !database_slot.verify_store_identity(&store_identity, &verifier)
            {
                anyhow::bail!("task_store_database_slot_authentication_failed");
            }
            runtime_task_store_authority(store_identity, verifier, writer_owner_generation_id)
        }
        (None, None) if existing_schema_version == 0 => {
            let store_identity = preauthenticated_store_identity
                .map(str::to_string)
                .unwrap_or_else(|| format!("task-store:{}", uuid::Uuid::new_v4()));
            let verifier = database_slot.sign_store_identity(&store_identity);
            conn.execute(
                "INSERT INTO task_store_metadata (key, value) VALUES (?1, ?2)",
                params![TASK_STORE_IDENTITY_METADATA_KEY, store_identity],
            )?;
            conn.execute(
                "INSERT INTO task_store_metadata (key, value) VALUES (?1, ?2)",
                params![TASK_STORE_SLOT_VERIFIER_METADATA_KEY, verifier],
            )?;
            runtime_task_store_authority(store_identity, verifier, writer_owner_generation_id)
        }
        (None, None) if existing_schema_version < 13 => {
            let store_identity = preauthenticated_store_identity
                .map(str::to_string)
                .unwrap_or_else(|| format!("task-store:{}", uuid::Uuid::new_v4()));
            let verifier = database_slot.sign_store_identity(&store_identity);
            conn.execute(
                "INSERT INTO task_store_metadata (key, value) VALUES (?1, ?2)",
                params![TASK_STORE_IDENTITY_METADATA_KEY, store_identity],
            )?;
            conn.execute(
                "INSERT INTO task_store_metadata (key, value) VALUES (?1, ?2)",
                params![TASK_STORE_SLOT_VERIFIER_METADATA_KEY, verifier],
            )?;
            runtime_task_store_authority(store_identity, verifier, writer_owner_generation_id)
        }
        (None, None) => anyhow::bail!("task_store_canonical_authority_metadata_missing"),
        _ => anyhow::bail!("task_store_canonical_authority_metadata_incomplete"),
    }
}

fn quarantine_pre_authority_task_store_truth(
    conn: &Connection,
    existing_schema_version: i64,
    snapshot: &[PreAuthorityTaskStoreTruth],
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let reason = "pre_v13_task_store_truth_quarantined_requires_fresh_review";
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS legacy_task_store_truth_quarantine (
            record_id TEXT PRIMARY KEY,
            source_schema_version INTEGER NOT NULL,
            task_id_digest TEXT NOT NULL,
            task_status TEXT NOT NULL,
            task_status_source_digest TEXT NOT NULL,
            attempt_id_digest TEXT,
            attempt_status TEXT,
            attempt_status_source_digest TEXT,
            terminal_truth_digest TEXT NOT NULL,
            provider_receipt_count INTEGER NOT NULL,
            provider_status_digest TEXT NOT NULL,
            tool_dispatch_count INTEGER NOT NULL,
            tool_status_digest TEXT NOT NULL,
            reason_code TEXT NOT NULL,
            quarantined_at TEXT NOT NULL
         ) WITHOUT ROWID;",
    )?;
    for row in snapshot {
        let task_id_digest = digest_ref(&row.task_id);
        let attempt_id_digest = row.attempt_id.as_deref().map(digest_ref);
        let provider_status_facts = row
            .provider_statuses
            .iter()
            .map(|status| {
                serde_json::json!({
                    "category": status.category,
                    "boundedSourceDigest": status.bounded_source_digest,
                })
            })
            .collect::<Vec<_>>();
        let tool_status_facts = row
            .tool_statuses
            .iter()
            .map(|status| {
                serde_json::json!({
                    "category": status.category,
                    "boundedSourceDigest": status.bounded_source_digest,
                })
            })
            .collect::<Vec<_>>();
        let provider_status_digest = crate::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::json!(provider_status_facts),
        )
        .1;
        let tool_status_digest = crate::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::json!(tool_status_facts),
        )
        .1;
        let attempt_status_category = row
            .attempt_status
            .as_ref()
            .map(|status| status.category.as_str());
        let attempt_status_source_digest = row
            .attempt_status
            .as_ref()
            .map(|status| status.bounded_source_digest.as_str());
        let terminal_truth_digest =
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "schema": "pre_v13_task_store_terminal_report_v1",
                "taskStatus": row.task_status.category,
                "taskStatusSourceDigest": row.task_status.bounded_source_digest,
                "completedAtDigest": row.completed_at.as_deref().map(digest_ref),
                "resultDigest": row.result_digest,
                "resultRefDigest": row.result_ref.as_deref().map(digest_ref),
                "lastErrorDigest": row.last_error.as_deref().map(digest_ref),
                "attemptStatus": attempt_status_category,
                "attemptStatusSourceDigest": attempt_status_source_digest,
                "attemptErrorDigest": row.attempt_error_digest,
                "providerStatusDigest": provider_status_digest,
                "toolStatusDigest": tool_status_digest,
            }))
            .1;
        let record_id = digest_parts(&[
            "pre_v13_task_store_quarantine_v1",
            &existing_schema_version.to_string(),
            &task_id_digest,
            attempt_id_digest.as_deref().unwrap_or("no_attempt"),
        ]);
        conn.execute(
            "INSERT OR IGNORE INTO legacy_task_store_truth_quarantine (
                record_id, source_schema_version, task_id_digest, task_status,
                task_status_source_digest, attempt_id_digest, attempt_status,
                attempt_status_source_digest, terminal_truth_digest,
                provider_receipt_count, provider_status_digest,
                tool_dispatch_count, tool_status_digest, reason_code, quarantined_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                record_id,
                existing_schema_version,
                task_id_digest,
                row.task_status.category,
                row.task_status.bounded_source_digest,
                attempt_id_digest,
                attempt_status_category,
                attempt_status_source_digest,
                terminal_truth_digest,
                row.provider_statuses.len() as i64,
                provider_status_digest,
                row.tool_statuses.len() as i64,
                tool_status_digest,
                reason,
                now,
            ],
        )?;
    }
    // Remove every pre-v13 product owner after its metadata-only quarantine row
    // is durable in this same transaction. No pending/running/terminal label is
    // allowed to survive as current canonical scheduler truth.
    conn.execute("DELETE FROM scheduler_provider_grant_consumptions", [])?;
    conn.execute("DELETE FROM scheduler_provider_receipts", [])?;
    conn.execute("DELETE FROM scheduler_tool_dispatches", [])?;
    conn.execute("DELETE FROM scheduler_attempts", [])?;
    conn.execute("DELETE FROM tasks", [])?;
    Ok(())
}

fn load_existing_task_store_authority(
    conn: &Connection,
    database_slot: &TaskStoreDatabaseSlot,
    writer_owner_generation_id: uuid::Uuid,
) -> Result<TaskStoreRuntimeAuthority> {
    let schema_version = task_store_schema_version(conn)?
        .ok_or_else(|| anyhow::anyhow!("task_store_schema_version_missing"))?;
    if schema_version != TASK_STORE_SCHEMA_VERSION {
        anyhow::bail!("task_store_read_only_authority_schema_not_current");
    }
    if task_store_metadata_value(conn, TASK_STORE_PRE_V13_PURGE_COMPLETE_METADATA_KEY)?.as_deref()
        != Some("complete")
    {
        anyhow::bail!("task_store_physical_purge_incomplete");
    }
    let store_identity = task_store_metadata_value(conn, TASK_STORE_IDENTITY_METADATA_KEY)?
        .ok_or_else(|| anyhow::anyhow!("task_store_canonical_authority_metadata_missing"))?;
    let verifier = task_store_metadata_value(conn, TASK_STORE_SLOT_VERIFIER_METADATA_KEY)?
        .ok_or_else(|| anyhow::anyhow!("task_store_canonical_authority_metadata_missing"))?;
    validate_task_store_identity(&store_identity)?;
    if !database_slot.verify_store_identity(&store_identity, &verifier) {
        anyhow::bail!("task_store_database_slot_authentication_failed");
    }
    runtime_task_store_authority(store_identity, verifier, writer_owner_generation_id)
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    if !table
        .bytes()
        .chain(column.bytes())
        .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
    {
        anyhow::bail!("invalid scheduler migration identifier");
    }
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rename_column_if_needed(
    conn: &Connection,
    table: &str,
    legacy_column: &str,
    canonical_column: &str,
) -> Result<()> {
    if table_has_column(conn, table, canonical_column)? {
        return Ok(());
    }
    if !table_has_column(conn, table, legacy_column)? {
        anyhow::bail!(
            "scheduler migration is missing both {legacy_column} and {canonical_column} on {table}"
        );
    }
    conn.execute(
        &format!("ALTER TABLE {table} RENAME COLUMN {legacy_column} TO {canonical_column}"),
        [],
    )?;
    Ok(())
}

#[derive(Debug)]
struct LocalProviderGrantDigestMigration {
    task_id: String,
    legacy_grant: ScheduledProviderGrantV2,
}

fn backfill_task_provider_grants(
    conn: &Connection,
    migrating_from_legacy: bool,
    existing_schema_version: i64,
) -> Result<Vec<LocalProviderGrantDigestMigration>> {
    let mut local_provider_grant_migrations = Vec::new();
    let rows = {
        let mut statement = conn.prepare(
            "SELECT id, description, action_type, due_date, source_run_id, source_proposal_id,
                    provider_grant_id, provider_policy_version, provider_data_route,
                    provider_reason_code, provider_subject_digest, provider_payload_purpose,
                    provider_payload_contract_digest, provider_source_ref_digest,
                    provider_schedule_digest, provider_grant_scope, provider_grant_expires_at,
                    provider_target_digest, model_target_digest, review_snapshot_digest,
                    review_dispatch_claim_digest, provider_policy_decision_digest
             FROM tasks",
        )?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, String>(14)?,
                    row.get::<_, Option<String>>(15)?,
                    row.get::<_, Option<String>>(16)?,
                    row.get::<_, Option<String>>(17)?,
                    row.get::<_, Option<String>>(18)?,
                    row.get::<_, Option<String>>(19)?,
                    row.get::<_, Option<String>>(20)?,
                    row.get::<_, String>(21)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        collected
    };
    for (
        task_id,
        description,
        action_type,
        due_date,
        source_run_id,
        source_proposal_id,
        stored_grant_id,
        stored_policy_version,
        stored_data_route,
        stored_reason_code,
        stored_subject_digest,
        stored_payload_purpose,
        stored_payload_contract_digest,
        stored_source_ref_digest,
        stored_schedule_digest,
        stored_grant_scope,
        stored_grant_expires_at,
        stored_provider_target_digest,
        stored_model_target_digest,
        stored_review_snapshot_digest,
        stored_review_dispatch_claim_digest,
        stored_policy_decision_digest,
    ) in rows
    {
        if stored_data_route == "policy_allowed" {
            if migrating_from_legacy {
                anyhow::bail!(
                    "pre-v10 scheduled cloud-shaped state has no canonical ReviewWorkflow authority"
                );
            }
            if let Some(due_at) = due_date.as_deref() {
                DateTime::parse_from_rfc3339(due_at)
                    .context("stored scheduled cloud due time is invalid")?;
            }
            let grant = ScheduledProviderGrantV2 {
                grant_id: stored_grant_id,
                policy_version: stored_policy_version,
                policy_decision_digest: stored_policy_decision_digest,
                data_route: ProviderDataRoute::PolicyAllowed,
                reason_code: stored_reason_code,
                subject_digest: stored_subject_digest,
                schedule_digest: stored_schedule_digest,
                payload_purpose: parse_provider_payload_purpose(stored_payload_purpose)?,
                payload_contract_digest: stored_payload_contract_digest,
                source_ref_digest: stored_source_ref_digest,
                grant_scope: parse_scheduled_provider_grant_scope(stored_grant_scope.ok_or_else(
                    || anyhow::anyhow!("stored scheduled cloud grant scope is missing"),
                )?)?,
                grant_expires_at: stored_grant_expires_at,
                provider_digest: stored_provider_target_digest,
                model_digest: stored_model_target_digest,
                review_snapshot_digest: stored_review_snapshot_digest,
                review_dispatch_claim_digest: stored_review_dispatch_claim_digest,
            };
            let task = ScheduledTask {
                id: task_id,
                title: "stored-scheduled-cloud-task".into(),
                description,
                due_date,
                priority: "medium".into(),
                status: "pending".into(),
                created_at: chrono::Utc::now().to_rfc3339(),
                completed_at: None,
                source_run_id,
                source_proposal_id,
                action_type,
                attempt_count: 0,
                claim_token: None,
                lease_expires_at: None,
                last_error: None,
                result_digest: None,
                result_ref: None,
                provider_grant: grant,
                provider_grant_authority:
                    ScheduledTaskGrantAuthorityProof::UntrustedSerializedState,
            };
            task.provider_grant.validate_for_task(&task)?;
            continue;
        }
        let expected = ScheduledProviderGrantV2::deterministic_local_only(
            &task_id,
            &description,
            &action_type,
            due_date.as_deref(),
            source_run_id.as_deref(),
            source_proposal_id.as_deref(),
        );
        if !migrating_from_legacy {
            let stored = ScheduledProviderGrantV2 {
                grant_id: stored_grant_id,
                policy_version: stored_policy_version,
                policy_decision_digest: stored_policy_decision_digest,
                data_route: parse_provider_data_route(stored_data_route)?,
                reason_code: stored_reason_code,
                subject_digest: stored_subject_digest,
                schedule_digest: stored_schedule_digest,
                payload_purpose: parse_provider_payload_purpose(stored_payload_purpose)?,
                payload_contract_digest: stored_payload_contract_digest,
                source_ref_digest: stored_source_ref_digest,
                grant_scope: parse_scheduled_provider_grant_scope(stored_grant_scope.ok_or_else(
                    || anyhow::anyhow!("stored scheduled local grant scope is missing"),
                )?)?,
                grant_expires_at: stored_grant_expires_at,
                provider_digest: stored_provider_target_digest,
                model_digest: stored_model_target_digest,
                review_snapshot_digest: stored_review_snapshot_digest,
                review_dispatch_claim_digest: stored_review_dispatch_claim_digest,
            };
            if stored == expected {
                continue;
            }
            let legacy = ScheduledProviderGrantV2::deterministic_local_only_v11(
                &task_id,
                &description,
                &action_type,
                due_date.as_deref(),
                source_run_id.as_deref(),
                source_proposal_id.as_deref(),
            );
            if !(EXACT_PROVIDER_PROVENANCE_SCHEMA_VERSION..TASK_STORE_SCHEMA_VERSION)
                .contains(&existing_schema_version)
                || stored != legacy
            {
                anyhow::bail!("canonical scheduled task provider grant is corrupt or rebound");
            }
            local_provider_grant_migrations.push(LocalProviderGrantDigestMigration {
                task_id: task_id.clone(),
                legacy_grant: legacy,
            });
        }
        conn.execute(
            "UPDATE tasks SET provider_grant_id = ?1, provider_policy_version = ?2,
                    provider_data_route = ?3, provider_reason_code = ?4,
                    provider_subject_digest = ?5, provider_payload_purpose = ?6,
                    provider_payload_contract_digest = ?7, provider_source_ref_digest = ?8,
                    provider_schedule_digest = ?9, provider_grant_scope = ?10,
                    provider_grant_expires_at = ?11, provider_target_digest = ?12,
                    model_target_digest = ?13, review_snapshot_digest = ?14,
                    review_dispatch_claim_digest = ?15,
                    provider_policy_decision_digest = ?16
             WHERE id = ?17",
            params![
                expected.grant_id,
                expected.policy_version,
                provider_data_route_label(expected.data_route),
                expected.reason_code,
                expected.subject_digest,
                expected.payload_purpose.as_str(),
                expected.payload_contract_digest,
                expected.source_ref_digest,
                expected.schedule_digest,
                expected.grant_scope.as_str(),
                expected.grant_expires_at,
                expected.provider_digest,
                expected.model_digest,
                expected.review_snapshot_digest,
                expected.review_dispatch_claim_digest,
                expected.policy_decision_digest,
                task_id,
            ],
        )?;
    }
    Ok(local_provider_grant_migrations)
}

fn backfill_attempt_provider_grants(
    conn: &Connection,
    migrating_from_legacy: bool,
    local_provider_grant_migrations: &[LocalProviderGrantDigestMigration],
) -> Result<()> {
    if migrating_from_legacy {
        // Preserve the decision actually recorded on every historical attempt.
        // A current task grant is only a migration association; it is never
        // rewritten into an older attempt as if it had authorized that call.
        conn.execute(
            "UPDATE scheduler_attempts
             SET provider_provenance_state = CASE
                    WHEN provider_grant_id = (
                            SELECT provider_grant_id FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                     AND policy_version = (
                            SELECT provider_policy_version FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                     AND data_route = (
                            SELECT provider_data_route FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                     AND policy_reason_code = (
                            SELECT provider_reason_code FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                     AND provider_subject_digest = (
                            SELECT provider_subject_digest FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                     AND provider_payload_purpose = (
                            SELECT provider_payload_purpose FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                     AND provider_payload_contract_digest = (
                            SELECT provider_payload_contract_digest FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                     AND provider_source_ref_digest = (
                            SELECT provider_source_ref_digest FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                    THEN 'exact' ELSE 'legacy_unavailable' END,
                 migration_associated_grant_id = CASE
                    WHEN provider_grant_id = (
                            SELECT provider_grant_id FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                     AND provider_subject_digest = (
                            SELECT provider_subject_digest FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                     AND provider_payload_contract_digest = (
                            SELECT provider_payload_contract_digest FROM tasks WHERE id = scheduler_attempts.task_id
                         )
                    THEN NULL
                    ELSE (SELECT provider_grant_id FROM tasks
                          WHERE id = scheduler_attempts.task_id) END",
            [],
        )?;
    }
    for migration in local_provider_grant_migrations {
        let legacy = &migration.legacy_grant;
        conn.execute(
            "UPDATE scheduler_attempts
             SET provider_provenance_state = 'legacy_unavailable',
                 migration_associated_grant_id = (
                    SELECT provider_grant_id FROM tasks WHERE id = scheduler_attempts.task_id
                 )
             WHERE task_id = ?1 AND provider_provenance_state = 'exact'
               AND provider_grant_id = ?2 AND policy_version = ?3
               AND data_route = ?4 AND policy_reason_code = ?5
               AND provider_subject_digest = ?6 AND provider_payload_purpose = ?7
               AND provider_payload_contract_digest = ?8
               AND provider_source_ref_digest = ?9",
            params![
                migration.task_id,
                legacy.grant_id,
                legacy.policy_version,
                provider_data_route_label(legacy.data_route),
                legacy.reason_code,
                legacy.subject_digest,
                legacy.payload_purpose.as_str(),
                legacy.payload_contract_digest,
                legacy.source_ref_digest,
            ],
        )?;
        conn.execute(
            "UPDATE scheduler_attempts
             SET migration_associated_grant_id = (
                    SELECT provider_grant_id FROM tasks WHERE id = scheduler_attempts.task_id
                 )
             WHERE task_id = ?1 AND provider_provenance_state = 'legacy_unavailable'
               AND migration_associated_grant_id = ?2",
            params![migration.task_id, legacy.grant_id],
        )?;
    }
    if migrating_from_legacy || !local_provider_grant_migrations.is_empty() {
        let migration_reason = "unknown_legacy_execution_state";
        conn.execute(
            "UPDATE tasks
             SET status = 'unknown_legacy_execution_state', last_error = ?1,
                 eligible_at = NULL
             WHERE id IN (
                SELECT task_id FROM scheduler_attempts
                WHERE provider_provenance_state = 'legacy_unavailable'
                  AND status IN ('claimed', 'executing', 'unknown')
             )",
            [migration_reason],
        )?;
        conn.execute(
            "UPDATE scheduler_attempts
             SET status = 'unknown', error_digest = COALESCE(error_digest, ?1)
             WHERE provider_provenance_state = 'legacy_unavailable'
               AND status IN ('claimed', 'executing')",
            [digest_ref(migration_reason)],
        )?;
    }
    let invalid_exact: i64 = conn.query_row(
        "SELECT COUNT(*) FROM scheduler_attempts a JOIN tasks t ON t.id = a.task_id
         WHERE a.provider_provenance_state = 'exact' AND (
               a.provider_grant_id != t.provider_grant_id
            OR a.policy_version != t.provider_policy_version
            OR a.data_route != t.provider_data_route
            OR a.policy_reason_code != t.provider_reason_code
            OR a.provider_subject_digest != t.provider_subject_digest
            OR a.provider_payload_purpose != t.provider_payload_purpose
            OR a.provider_payload_contract_digest != t.provider_payload_contract_digest
            OR a.provider_source_ref_digest != t.provider_source_ref_digest
         )",
        [],
        |row| row.get(0),
    )?;
    if invalid_exact != 0 {
        anyhow::bail!("exact scheduled attempt provider provenance is corrupt or rebound");
    }
    let invalid_legacy: i64 = conn.query_row(
        "SELECT COUNT(*) FROM scheduler_attempts a JOIN tasks t ON t.id = a.task_id
         WHERE a.provider_provenance_state = 'legacy_unavailable'
           AND (a.migration_associated_grant_id IS NULL
                OR a.migration_associated_grant_id != t.provider_grant_id)",
        [],
        |row| row.get(0),
    )?;
    if invalid_legacy != 0 {
        anyhow::bail!("legacy scheduled attempt is missing its explicit migration association");
    }
    Ok(())
}

fn preserve_provider_receipt_provenance(
    conn: &Connection,
    migrating_from_legacy: bool,
) -> Result<()> {
    if migrating_from_legacy {
        // A v5 store produced by an earlier build may already have copied the
        // current task grant onto an older attempt. A legacy receipt is the
        // remaining mechanical evidence that the original authorization was
        // unavailable; keep it unavailable rather than upgrading it to exact.
        conn.execute(
            "UPDATE scheduler_attempts
             SET provider_provenance_state = 'legacy_unavailable',
                 migration_associated_grant_id = (
                    SELECT t.provider_grant_id FROM tasks t
                    WHERE t.id = scheduler_attempts.task_id
                 )
             WHERE EXISTS (
                SELECT 1 FROM scheduler_provider_receipts r
                WHERE r.attempt_id = scheduler_attempts.attempt_id
                  AND r.policy_evidence_state = 'legacy_unavailable'
             )",
            [],
        )?;
        conn.execute(
            "UPDATE scheduler_provider_receipts
             SET policy_evidence_state = 'legacy_unavailable',
                 migration_associated_grant_id = (
                    SELECT t.provider_grant_id
                    FROM tasks t WHERE t.id = scheduler_provider_receipts.task_id
                 )
             WHERE policy_evidence_digest IS NULL
                OR EXISTS (
                    SELECT 1 FROM scheduler_attempts a
                    WHERE a.attempt_id = scheduler_provider_receipts.attempt_id
                      AND a.provider_provenance_state = 'legacy_unavailable'
                )",
            [],
        )?;
        let migration_reason = "unknown_legacy_execution_state";
        conn.execute(
            "UPDATE tasks
             SET status = 'unknown_legacy_execution_state', last_error = ?1,
                 eligible_at = NULL
             WHERE id IN (
                SELECT task_id FROM scheduler_attempts
                WHERE provider_provenance_state = 'legacy_unavailable'
                  AND status IN ('claimed', 'executing', 'unknown')
             )",
            [migration_reason],
        )?;
        conn.execute(
            "UPDATE scheduler_attempts
             SET status = 'unknown', error_digest = COALESCE(error_digest, ?1)
             WHERE provider_provenance_state = 'legacy_unavailable'
               AND status IN ('claimed', 'executing')",
            [digest_ref(migration_reason)],
        )?;
    }
    let invalid_exact: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM scheduler_provider_receipts r
         JOIN scheduler_attempts a ON a.attempt_id = r.attempt_id
         WHERE r.policy_evidence_state = 'exact'
           AND (a.provider_provenance_state != 'exact'
                OR r.provider_grant_id != a.provider_grant_id
                OR r.process_epoch_id != a.process_epoch_id
                OR r.writer_owner_generation_id != a.writer_owner_generation_id
                OR r.migration_associated_grant_id IS NOT NULL)",
        [],
        |row| row.get(0),
    )?;
    if invalid_exact != 0 {
        anyhow::bail!("exact scheduled provider receipt provenance is corrupt or rebound");
    }
    let invalid_legacy: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM scheduler_provider_receipts r
         JOIN tasks t ON t.id = r.task_id
         WHERE r.policy_evidence_state = 'legacy_unavailable'
           AND (r.migration_associated_grant_id IS NULL
                OR r.migration_associated_grant_id != t.provider_grant_id)",
        [],
        |row| row.get(0),
    )?;
    if invalid_legacy != 0 {
        anyhow::bail!("legacy provider receipt is missing its migration association");
    }
    Ok(())
}

fn migrate_scheduler_attempt_policy_allowed_route(conn: &Connection) -> Result<()> {
    let schema_sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master
         WHERE type = 'table' AND name = 'scheduler_attempts'",
        [],
        |row| row.get(0),
    )?;
    if schema_sql.contains("'policy_allowed'") {
        return Ok(());
    }
    let grant_consumptions_exist: bool = conn.query_row(
        "SELECT COUNT(*) = 1 FROM sqlite_master
         WHERE type = 'table' AND name = 'scheduler_provider_grant_consumptions'",
        [],
        |row| row.get(0),
    )?;

    conn.execute(
        "DROP TABLE IF EXISTS scheduler_provider_grant_consumptions_v10",
        [],
    )?;
    conn.execute("DROP TABLE IF EXISTS scheduler_tool_dispatches_v10", [])?;
    conn.execute("DROP TABLE IF EXISTS scheduler_provider_receipts_v10", [])?;
    conn.execute("DROP TABLE IF EXISTS scheduler_attempts_v10", [])?;
    conn.execute(
        "CREATE TABLE scheduler_attempts_v10 (
            attempt_id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            claim_token TEXT NOT NULL UNIQUE,
            attempt_number INTEGER NOT NULL,
            status TEXT NOT NULL CHECK(status IN (
                'claimed', 'executing', 'completed', 'failed', 'unknown',
                'pre_dispatch_failed', 'pre_dispatch_timeout', 'expired_before_execution',
                'reconciled_retry_safe', 'reconciled_failed', 'reconciled_completed'
            )),
            provider_grant_id TEXT NOT NULL,
            policy_version TEXT NOT NULL,
            data_route TEXT NOT NULL CHECK(data_route IN ('local_only', 'policy_allowed')),
            policy_reason_code TEXT NOT NULL,
            provider_subject_digest TEXT NOT NULL,
            provider_payload_purpose TEXT NOT NULL,
            provider_payload_contract_digest TEXT NOT NULL,
            provider_source_ref_digest TEXT NOT NULL,
            provider_provenance_state TEXT NOT NULL DEFAULT 'exact' CHECK(
                provider_provenance_state IN ('exact', 'legacy_unavailable')
            ),
            migration_associated_grant_id TEXT,
            claimed_at TEXT NOT NULL,
            execution_started_at TEXT,
            finished_at TEXT,
            agent_run_ref_digest TEXT,
            error_digest TEXT,
            reconciliation_evidence_digest TEXT,
            reconciliation_issuer TEXT,
            reconciliation_evidence_kind TEXT,
            reconciliation_evidence_ref TEXT,
            reconciled_at TEXT,
            UNIQUE(task_id, attempt_number),
            FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
        )",
        [],
    )?;
    conn.execute(
        "INSERT INTO scheduler_attempts_v10 (
            attempt_id, task_id, claim_token, attempt_number, status,
            provider_grant_id, policy_version, data_route, policy_reason_code,
            provider_subject_digest, provider_payload_purpose,
            provider_payload_contract_digest, provider_source_ref_digest,
            provider_provenance_state, migration_associated_grant_id, claimed_at,
            execution_started_at, finished_at, agent_run_ref_digest, error_digest,
            reconciliation_evidence_digest, reconciliation_issuer,
            reconciliation_evidence_kind, reconciliation_evidence_ref, reconciled_at
         ) SELECT attempt_id, task_id, claim_token, attempt_number, status,
            provider_grant_id, policy_version, data_route, policy_reason_code,
            provider_subject_digest, provider_payload_purpose,
            provider_payload_contract_digest, provider_source_ref_digest,
            provider_provenance_state, migration_associated_grant_id, claimed_at,
            execution_started_at, finished_at, agent_run_ref_digest, error_digest,
            reconciliation_evidence_digest, reconciliation_issuer,
            reconciliation_evidence_kind, reconciliation_evidence_ref, reconciled_at
         FROM scheduler_attempts",
        [],
    )?;
    conn.execute(
        "CREATE TABLE scheduler_provider_receipts_v10 (
            request_id TEXT PRIMARY KEY,
            attempt_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            claim_token TEXT NOT NULL,
            provider_grant_id TEXT NOT NULL,
            migration_associated_grant_id TEXT,
            provider_digest TEXT NOT NULL,
            model_digest TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN (
                'started', 'completed', 'failed', 'remote_unknown'
            )),
            started_at TEXT NOT NULL,
            finished_at TEXT,
            error_digest TEXT,
            simulated INTEGER,
            policy_evidence_state TEXT NOT NULL CHECK(policy_evidence_state IN (
                'exact', 'legacy_unavailable'
            )),
            policy_evidence_digest TEXT,
            subject_scope_digest TEXT,
            payload_purpose TEXT,
            unfiltered_payload_digest TEXT,
            context_manifest_digest TEXT,
            prepared_envelope_digest TEXT,
            network_policy_decision_digest TEXT,
            FOREIGN KEY(attempt_id) REFERENCES scheduler_attempts_v10(attempt_id) ON DELETE CASCADE,
            FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
        )",
        [],
    )?;
    conn.execute(
        "INSERT INTO scheduler_provider_receipts_v10 (
            request_id, attempt_id, task_id, claim_token, provider_grant_id,
            migration_associated_grant_id, provider_digest, model_digest, status,
            started_at, finished_at, error_digest, simulated, policy_evidence_state,
            policy_evidence_digest, subject_scope_digest, payload_purpose,
            unfiltered_payload_digest, context_manifest_digest, prepared_envelope_digest,
            network_policy_decision_digest
         ) SELECT request_id, attempt_id, task_id, claim_token, provider_grant_id,
            migration_associated_grant_id, provider_digest, model_digest, status,
            started_at, finished_at, error_digest, simulated, policy_evidence_state,
            policy_evidence_digest, subject_scope_digest, payload_purpose,
            unfiltered_payload_digest, context_manifest_digest, prepared_envelope_digest,
            network_policy_decision_digest
         FROM scheduler_provider_receipts",
        [],
    )?;
    conn.execute(
        "CREATE TABLE scheduler_tool_dispatches_v10 (
            dispatch_id TEXT PRIMARY KEY,
            attempt_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            claim_token TEXT NOT NULL,
            dispatch_index INTEGER NOT NULL,
            manifest_digest TEXT NOT NULL,
            manifest_contract_digest TEXT,
            tool_digest TEXT NOT NULL,
            input_hash TEXT,
            input_length_bytes INTEGER,
            source_run_ref_digest TEXT,
            identity_state TEXT NOT NULL DEFAULT 'legacy_unavailable' CHECK(
                identity_state IN ('exact', 'legacy_unavailable')
            ),
            status TEXT NOT NULL CHECK(status IN ('started', 'returned', 'unknown')),
            observed_at TEXT NOT NULL,
            receipt_started_at TEXT,
            dispatched_at TEXT,
            finished_at TEXT,
            tool_receipt_id TEXT,
            request_digest TEXT,
            action_effect TEXT,
            idempotency_contract TEXT,
            dispatch_kind TEXT,
            dispatch_attempt_count INTEGER,
            transport_status TEXT,
            effect_status TEXT,
            execution_outcome TEXT,
            transport_observed_at TEXT,
            UNIQUE(attempt_id, dispatch_index),
            FOREIGN KEY(attempt_id) REFERENCES scheduler_attempts_v10(attempt_id) ON DELETE CASCADE,
            FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
        )",
        [],
    )?;
    conn.execute(
        "INSERT INTO scheduler_tool_dispatches_v10 (
            dispatch_id, attempt_id, task_id, claim_token, dispatch_index,
            manifest_digest, manifest_contract_digest, tool_digest, input_hash,
            input_length_bytes, source_run_ref_digest, identity_state, status, observed_at,
            receipt_started_at, dispatched_at, finished_at, tool_receipt_id, request_digest,
            action_effect, idempotency_contract, dispatch_kind, dispatch_attempt_count,
            transport_status, effect_status, execution_outcome, transport_observed_at
         ) SELECT dispatch_id, attempt_id, task_id, claim_token, dispatch_index,
            manifest_digest, manifest_contract_digest, tool_digest, input_hash,
            input_length_bytes, source_run_ref_digest, identity_state, status, observed_at,
            receipt_started_at, dispatched_at, finished_at, tool_receipt_id, request_digest,
            action_effect, idempotency_contract, dispatch_kind, dispatch_attempt_count,
            transport_status, effect_status, execution_outcome, transport_observed_at
         FROM scheduler_tool_dispatches",
        [],
    )?;
    if grant_consumptions_exist {
        conn.execute(
            "CREATE TABLE scheduler_provider_grant_consumptions_v10 (
                grant_id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                attempt_id TEXT NOT NULL UNIQUE,
                consumed_at TEXT NOT NULL,
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE RESTRICT,
                FOREIGN KEY(attempt_id) REFERENCES scheduler_attempts_v10(attempt_id)
                    ON DELETE RESTRICT
            )",
            [],
        )?;
        conn.execute(
            "INSERT INTO scheduler_provider_grant_consumptions_v10 (
                grant_id, task_id, attempt_id, consumed_at
             ) SELECT grant_id, task_id, attempt_id, consumed_at
               FROM scheduler_provider_grant_consumptions",
            [],
        )?;
    }
    conn.execute("DROP TABLE scheduler_tool_dispatches", [])?;
    conn.execute("DROP TABLE scheduler_provider_receipts", [])?;
    if grant_consumptions_exist {
        conn.execute("DROP TABLE scheduler_provider_grant_consumptions", [])?;
    }
    conn.execute("DROP TABLE scheduler_attempts", [])?;
    conn.execute(
        "ALTER TABLE scheduler_attempts_v10 RENAME TO scheduler_attempts",
        [],
    )?;
    conn.execute(
        "ALTER TABLE scheduler_provider_receipts_v10 RENAME TO scheduler_provider_receipts",
        [],
    )?;
    conn.execute(
        "ALTER TABLE scheduler_tool_dispatches_v10 RENAME TO scheduler_tool_dispatches",
        [],
    )?;
    if grant_consumptions_exist {
        conn.execute(
            "ALTER TABLE scheduler_provider_grant_consumptions_v10
             RENAME TO scheduler_provider_grant_consumptions",
            [],
        )?;
    }
    Ok(())
}

fn migrate_provider_prepared_request_binding_v12(
    conn: &Connection,
    existing_schema_version: i64,
) -> Result<()> {
    if existing_schema_version >= 12 {
        return Ok(());
    }
    conn.execute(
        "UPDATE scheduler_attempts
         SET provider_provenance_state = 'legacy_unavailable',
             migration_associated_grant_id = (
                SELECT t.provider_grant_id FROM tasks t
                WHERE t.id = scheduler_attempts.task_id
             )
         WHERE EXISTS (
            SELECT 1 FROM scheduler_provider_receipts r
            WHERE r.attempt_id = scheduler_attempts.attempt_id
         )",
        [],
    )?;
    conn.execute(
        "UPDATE scheduler_provider_receipts
         SET policy_evidence_state = 'legacy_unavailable',
             migration_associated_grant_id = (
                SELECT t.provider_grant_id FROM tasks t
                WHERE t.id = scheduler_provider_receipts.task_id
             )
         WHERE EXISTS (
            SELECT 1 FROM scheduler_attempts a
            WHERE a.attempt_id = scheduler_provider_receipts.attempt_id
              AND a.provider_provenance_state = 'legacy_unavailable'
         )",
        [],
    )?;
    let migration_reason = "unknown_legacy_provider_prepared_binding";
    conn.execute(
        "UPDATE tasks
         SET status = 'unknown_legacy_execution_state', last_error = ?1,
             eligible_at = NULL
         WHERE id IN (
            SELECT task_id FROM scheduler_attempts
            WHERE provider_provenance_state = 'legacy_unavailable'
              AND status IN ('claimed', 'executing', 'unknown')
         )",
        [migration_reason],
    )?;
    conn.execute(
        "UPDATE scheduler_attempts
         SET status = 'unknown', error_digest = COALESCE(error_digest, ?1)
         WHERE provider_provenance_state = 'legacy_unavailable'
           AND status IN ('claimed', 'executing')",
        [digest_ref(migration_reason)],
    )?;
    preserve_provider_receipt_provenance(conn, false)?;
    Ok(())
}

/// Schema v14 observed an OS process epoch but did not observe the exact
/// writable-owner generation. Adding the v15 column with one shared sentinel
/// cannot upgrade those rows into exact execution truth: equality between two
/// sentinels proves only that both facts are missing. Retain metadata, consume
/// no grant again, and conservatively quarantine every affected execution.
fn migrate_writer_owner_generation_v15(
    conn: &Connection,
    existing_schema_version: i64,
) -> Result<()> {
    if existing_schema_version >= 15 {
        return Ok(());
    }
    let now = chrono::Utc::now().to_rfc3339();
    let reason = "legacy_writer_owner_generation_unobserved";
    let reason_digest = digest_ref(reason);
    conn.execute(
        "UPDATE tasks
         SET status = 'unknown_legacy_execution_state', last_error = ?1,
             completed_at = NULL, eligible_at = NULL, claim_token = NULL,
             lease_expires_at = NULL
         WHERE EXISTS (
            SELECT 1 FROM scheduler_attempts a WHERE a.task_id = tasks.id
         )",
        [reason],
    )?;
    conn.execute(
        "UPDATE scheduler_attempts
         SET status = 'unknown', provider_provenance_state = 'legacy_unavailable',
             migration_associated_grant_id = (
                SELECT t.provider_grant_id FROM tasks t
                WHERE t.id = scheduler_attempts.task_id
             ),
             finished_at = COALESCE(finished_at, ?1),
             error_digest = COALESCE(error_digest, ?2)",
        params![now, reason_digest],
    )?;
    conn.execute(
        "UPDATE scheduler_provider_receipts
         SET status = 'remote_unknown', policy_evidence_state = 'legacy_unavailable',
             migration_associated_grant_id = (
                SELECT t.provider_grant_id FROM tasks t
                WHERE t.id = scheduler_provider_receipts.task_id
             ),
             finished_at = COALESCE(finished_at, ?1),
             error_digest = COALESCE(error_digest, ?2), simulated = NULL",
        params![now, reason_digest],
    )?;
    conn.execute(
        "UPDATE scheduler_tool_dispatches
         SET status = 'unknown', identity_state = 'legacy_unavailable',
             finished_at = COALESCE(finished_at, ?1),
             transport_status = 'remote_unknown', effect_status = 'unknown',
             execution_outcome = 'unknown'",
        [now],
    )?;
    Ok(())
}

fn valid_runtime_identity_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|parsed| {
        !parsed.is_nil() && parsed.get_version_num() == 4 && parsed.to_string() == value
    })
}

fn validate_exact_scheduler_runtime_identities(conn: &Connection) -> Result<()> {
    let mut attempts = conn.prepare(
        "SELECT attempt_id, process_epoch_id, writer_owner_generation_id
         FROM scheduler_attempts WHERE provider_provenance_state = 'exact'",
    )?;
    let attempt_rows = attempts.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in attempt_rows {
        let (attempt_id, process_epoch, writer_generation) = row?;
        if !valid_runtime_identity_uuid(&process_epoch)
            || !valid_runtime_identity_uuid(&writer_generation)
        {
            anyhow::bail!("exact scheduler attempt runtime identity is invalid:{attempt_id}");
        }
    }

    let mut provider_receipts = conn.prepare(
        "SELECT r.request_id, r.process_epoch_id, r.writer_owner_generation_id,
                a.process_epoch_id, a.writer_owner_generation_id
         FROM scheduler_provider_receipts r
         JOIN scheduler_attempts a ON a.attempt_id = r.attempt_id
         WHERE r.policy_evidence_state = 'exact'",
    )?;
    let provider_rows = provider_receipts.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    for row in provider_rows {
        let (request_id, receipt_epoch, receipt_generation, attempt_epoch, attempt_generation) =
            row?;
        if !valid_runtime_identity_uuid(&receipt_epoch)
            || !valid_runtime_identity_uuid(&receipt_generation)
            || !valid_runtime_identity_uuid(&attempt_epoch)
            || !valid_runtime_identity_uuid(&attempt_generation)
            || receipt_epoch != attempt_epoch
            || receipt_generation != attempt_generation
        {
            anyhow::bail!("exact scheduler provider runtime identity is invalid:{request_id}");
        }
    }

    let mut tool_dispatches = conn.prepare(
        "SELECT d.dispatch_id, d.process_epoch_id, d.writer_owner_generation_id,
                a.process_epoch_id, a.writer_owner_generation_id
         FROM scheduler_tool_dispatches d
         JOIN scheduler_attempts a ON a.attempt_id = d.attempt_id
         WHERE d.identity_state = 'exact'",
    )?;
    let tool_rows = tool_dispatches.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    for row in tool_rows {
        let (dispatch_id, receipt_epoch, receipt_generation, attempt_epoch, attempt_generation) =
            row?;
        if !valid_runtime_identity_uuid(&receipt_epoch)
            || !valid_runtime_identity_uuid(&receipt_generation)
            || !valid_runtime_identity_uuid(&attempt_epoch)
            || !valid_runtime_identity_uuid(&attempt_generation)
            || receipt_epoch != attempt_epoch
            || receipt_generation != attempt_generation
        {
            anyhow::bail!("exact scheduler tool runtime identity is invalid:{dispatch_id}");
        }
    }
    Ok(())
}

/// Version 10 and older tool rows were selected for terminal projection by
/// `(attempt, manifest, source_run, earliest_index)`. They therefore cannot be
/// upgraded into exact receipt identity by backfilling public DTO fields. Keep
/// the minimal historical evidence, but quarantine every such row as unknown;
/// only a post-v11 adapter-start observation may create `identity_state=exact`.
fn migrate_tool_dispatch_identity_v11(
    conn: &Connection,
    existing_schema_version: i64,
) -> Result<()> {
    if existing_schema_version < 11 {
        conn.execute(
            "UPDATE scheduler_tool_dispatches
             SET identity_state = 'legacy_unavailable', status = 'unknown'
             WHERE identity_state != 'exact' OR identity_state IS NULL",
            [],
        )?;
    }

    let invalid_exact: i64 = conn.query_row(
        "SELECT COUNT(*) FROM scheduler_tool_dispatches
         WHERE identity_state = 'exact' AND (
            tool_receipt_id IS NULL OR request_digest IS NULL
            OR manifest_contract_digest IS NULL OR input_hash IS NULL
            OR input_length_bytes IS NULL OR receipt_started_at IS NULL
            OR dispatched_at IS NULL OR action_effect IS NULL
            OR idempotency_contract IS NULL OR dispatch_kind IS NULL
            OR dispatch_attempt_count IS NULL
            OR NOT EXISTS (
                SELECT 1 FROM scheduler_attempts a
                WHERE a.attempt_id = scheduler_tool_dispatches.attempt_id
                  AND a.process_epoch_id = scheduler_tool_dispatches.process_epoch_id
                  AND a.writer_owner_generation_id =
                      scheduler_tool_dispatches.writer_owner_generation_id
            )
         )",
        [],
        |row| row.get(0),
    )?;
    if invalid_exact != 0 {
        anyhow::bail!("exact scheduler tool dispatch identity is incomplete");
    }

    let actionable_legacy: i64 = conn.query_row(
        "SELECT COUNT(*) FROM scheduler_tool_dispatches
         WHERE identity_state = 'legacy_unavailable' AND status != 'unknown'",
        [],
        |row| row.get(0),
    )?;
    if actionable_legacy != 0 {
        anyhow::bail!("legacy scheduler tool dispatch cannot retain actionable status");
    }
    Ok(())
}

fn migrate_provider_receipt_remote_unknown_status(conn: &Connection) -> Result<()> {
    let schema_sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master
         WHERE type = 'table' AND name = 'scheduler_provider_receipts'",
        [],
        |row| row.get(0),
    )?;
    if schema_sql.contains("'remote_unknown'") {
        return Ok(());
    }

    conn.execute("DROP TABLE IF EXISTS scheduler_provider_receipts_v7", [])?;
    conn.execute(
        "CREATE TABLE scheduler_provider_receipts_v7 (
            request_id TEXT PRIMARY KEY,
            attempt_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            claim_token TEXT NOT NULL,
            provider_grant_id TEXT NOT NULL,
            migration_associated_grant_id TEXT,
            provider_digest TEXT NOT NULL,
            model_digest TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN (
                'started', 'completed', 'failed', 'remote_unknown'
            )),
            started_at TEXT NOT NULL,
            finished_at TEXT,
            error_digest TEXT,
            simulated INTEGER,
            policy_evidence_state TEXT NOT NULL CHECK(policy_evidence_state IN (
                'exact', 'legacy_unavailable'
            )),
            policy_evidence_digest TEXT,
            subject_scope_digest TEXT,
            payload_purpose TEXT,
            unfiltered_payload_digest TEXT,
            context_manifest_digest TEXT,
            prepared_envelope_digest TEXT,
            network_policy_decision_digest TEXT,
            FOREIGN KEY(attempt_id) REFERENCES scheduler_attempts(attempt_id) ON DELETE CASCADE,
            FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
        )",
        [],
    )?;
    conn.execute(
        "INSERT INTO scheduler_provider_receipts_v7 (
            request_id, attempt_id, task_id, claim_token, provider_grant_id,
            migration_associated_grant_id, provider_digest, model_digest, status,
            started_at, finished_at, error_digest, simulated, policy_evidence_state,
            policy_evidence_digest, subject_scope_digest, payload_purpose,
            unfiltered_payload_digest, context_manifest_digest, prepared_envelope_digest,
            network_policy_decision_digest
         )
         SELECT request_id, attempt_id, task_id, claim_token, provider_grant_id,
            migration_associated_grant_id, provider_digest, model_digest, status,
            started_at, finished_at, error_digest, simulated, policy_evidence_state,
            policy_evidence_digest, subject_scope_digest, payload_purpose,
            unfiltered_payload_digest, context_manifest_digest, prepared_envelope_digest,
            network_policy_decision_digest
         FROM scheduler_provider_receipts",
        [],
    )?;
    conn.execute("DROP TABLE scheduler_provider_receipts", [])?;
    conn.execute(
        "ALTER TABLE scheduler_provider_receipts_v7 RENAME TO scheduler_provider_receipts",
        [],
    )?;
    Ok(())
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduledTask> {
    let data_route = parse_provider_data_route(row.get::<_, String>(19)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            19,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })?;
    Ok(ScheduledTask {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        due_date: row.get(3)?,
        priority: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        completed_at: row.get(7)?,
        source_run_id: row.get(8)?,
        source_proposal_id: row.get(9)?,
        action_type: row.get(10)?,
        attempt_count: row.get(11)?,
        claim_token: row.get(12)?,
        lease_expires_at: row.get(13)?,
        last_error: row.get(14)?,
        result_digest: row.get(15)?,
        result_ref: row.get(16)?,
        provider_grant: ScheduledProviderGrantV2 {
            grant_id: row.get(17)?,
            policy_version: row.get(18)?,
            policy_decision_digest: row.get(32)?,
            data_route,
            reason_code: row.get(20)?,
            subject_digest: row.get(21)?,
            payload_purpose: parse_provider_payload_purpose(row.get::<_, String>(22)?).map_err(
                |error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        22,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            error.to_string(),
                        )),
                    )
                },
            )?,
            payload_contract_digest: row.get(23)?,
            source_ref_digest: row.get(24)?,
            schedule_digest: row.get(25)?,
            grant_scope: parse_scheduled_provider_grant_scope(row.get::<_, String>(26)?).map_err(
                |error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        26,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            error.to_string(),
                        )),
                    )
                },
            )?,
            grant_expires_at: row.get(27)?,
            provider_digest: row.get(28)?,
            model_digest: row.get(29)?,
            review_snapshot_digest: row.get(30)?,
            review_dispatch_claim_digest: row.get(31)?,
        },
        // Canonical rows preserve facts, not authority. Cloud authority is
        // restored separately from ProposalStore/ReviewWorkflow evidence for
        // this process and exact grant id.
        provider_grant_authority: ScheduledTaskGrantAuthorityProof::UntrustedSerializedState,
    })
}

fn map_attempt_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduledAttemptRecord> {
    Ok(ScheduledAttemptRecord {
        attempt_id: row.get(0)?,
        task_id: row.get(1)?,
        claim_token: row.get(2)?,
        attempt_number: row.get(3)?,
        status: row.get(4)?,
        provider_grant_id: row.get(5)?,
        policy_version: row.get(6)?,
        data_route: row.get(7)?,
        policy_reason_code: row.get(8)?,
        provider_subject_digest: row.get(9)?,
        provider_payload_purpose: row.get(10)?,
        provider_payload_contract_digest: row.get(11)?,
        provider_source_ref_digest: row.get(12)?,
        error_digest: row.get(13)?,
        reconciliation_evidence_digest: row.get(14)?,
        provider_provenance_state: row.get(15)?,
        migration_associated_grant_id: row.get(16)?,
        reconciliation_issuer: row.get(17)?,
        reconciliation_evidence_kind: row.get(18)?,
        reconciliation_evidence_ref: row.get(19)?,
        process_epoch_id: row.get(20)?,
        writer_owner_generation_id: row.get(21)?,
    })
}

fn map_provider_receipt_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ScheduledProviderReceiptRecord> {
    Ok(ScheduledProviderReceiptRecord {
        request_id: row.get(0)?,
        attempt_id: row.get(1)?,
        task_id: row.get(2)?,
        claim_token: row.get(3)?,
        provider_grant_id: row.get(4)?,
        provider_digest: row.get(5)?,
        model_digest: row.get(6)?,
        status: row.get(7)?,
        error_digest: row.get(8)?,
        simulated: row.get::<_, Option<i64>>(9)?.map(|value| value != 0),
        policy_evidence_state: row.get(10)?,
        policy_evidence_digest: row.get(11)?,
        subject_scope_digest: row.get(12)?,
        payload_purpose: row.get(13)?,
        unfiltered_payload_digest: row.get(14)?,
        context_manifest_digest: row.get(15)?,
        prepared_envelope_digest: row.get(16)?,
        prepared_request_digest: row.get(17)?,
        network_policy_decision_digest: row.get(18)?,
        migration_associated_grant_id: row.get(19)?,
        process_epoch_id: row.get(20)?,
        writer_owner_generation_id: row.get(21)?,
    })
}

fn classify_legacy_scheduled_task(
    source_digest: &str,
    ordinal: usize,
    item: &serde_json::Value,
    migration_cutoff: DateTime<Utc>,
    migration_cutoff_text: &str,
) -> LegacyScheduledTaskMigrationRow {
    let item_bytes = serde_json::to_vec(item).unwrap_or_else(|_| b"legacy-json-value".to_vec());
    let item_digest = digest_bytes(&item_bytes);
    let legacy_status = match item.get("status").and_then(serde_json::Value::as_str) {
        Some("pending") => "pending",
        Some("running") => "running",
        Some("completed") => "completed",
        Some("failed") => "failed",
        Some("cancelled") => "cancelled",
        _ => "unknown",
    };
    let legacy_task_id_digest = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(digest_content);
    let (reason_code, effect_state, terminal_detail_digest, review_candidate) = match legacy_status
    {
        "pending" => {
            let scheduled_at = item.get("scheduled_at").and_then(serde_json::Value::as_str);
            let parsed_due = scheduled_at.and_then(|value| {
                DateTime::parse_from_rfc3339(value)
                    .ok()
                    .map(|time| time.with_timezone(&Utc))
            });
            let title = legacy_bounded_nonempty_text(item, "title", 512);
            let description = item
                .get("prompt")
                .and_then(serde_json::Value::as_str)
                .filter(|value| value.chars().count() <= MAX_SCHEDULED_TASK_DESCRIPTION_CHARS);
            let action_type = item
                .get("action_type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("scheduled_task");
            let action_type_valid = !action_type.trim().is_empty()
                && action_type.chars().count() <= 512
                && !action_type.chars().any(char::is_control);
            let priority = match item.get("priority").and_then(serde_json::Value::as_str) {
                Some("low") => "low",
                Some("high") => "high",
                _ => "medium",
            };
            let provably_not_due = scheduled_at.zip(parsed_due).is_some_and(|(raw, parsed)| {
                raw > migration_cutoff_text && parsed > migration_cutoff
            });
            if let (true, Some(title), Some(description), Some(parsed_due)) = (
                provably_not_due && action_type_valid,
                title,
                description,
                parsed_due,
            ) {
                let candidate = LegacyScheduledTaskReviewCandidate {
                    source_digest: source_digest.to_string(),
                    source_ordinal: ordinal,
                    item_digest: item_digest.clone(),
                    title: title.to_string(),
                    description: description.to_string(),
                    due_at: parsed_due.to_rfc3339(),
                    priority: priority.to_string(),
                    action_type: action_type.to_string(),
                    source_run_id: legacy_bounded_optional_reference(item, "source_run_id"),
                    source_proposal_id: legacy_bounded_optional_reference(
                        item,
                        "source_proposal_id",
                    ),
                    review_created_at: migration_cutoff_text.to_string(),
                };
                (
                    "legacy_future_pending_requires_fresh_review",
                    "review_required",
                    None,
                    Some(candidate),
                )
            } else if scheduled_at.zip(parsed_due).is_some_and(|(raw, parsed)| {
                raw <= migration_cutoff_text || parsed <= migration_cutoff
            }) {
                (
                    "legacy_due_pending_dispatch_state_unknown",
                    "unknown",
                    None,
                    None,
                )
            } else {
                (
                    "legacy_pending_schema_unmappable_unknown",
                    "unknown",
                    None,
                    None,
                )
            }
        }
        "running" => (
            "legacy_running_dispatch_state_unknown",
            "unknown",
            None,
            None,
        ),
        "completed" => (
            "legacy_reported_terminal_without_canonical_receipt",
            "reported_completed",
            item.get("result_preview")
                .map(|value| digest_bytes(&serde_json::to_vec(value).unwrap_or_default())),
            None,
        ),
        "failed" => (
            "legacy_reported_terminal_without_canonical_receipt",
            "reported_failed",
            item.get("error")
                .map(|value| digest_bytes(&serde_json::to_vec(value).unwrap_or_default())),
            None,
        ),
        "cancelled" => (
            "legacy_reported_terminal_without_canonical_receipt",
            "reported_cancelled",
            None,
            None,
        ),
        _ => ("legacy_schema_unmappable_unknown", "unknown", None, None),
    };
    LegacyScheduledTaskMigrationRow {
        ordinal,
        item_digest,
        legacy_task_id_digest,
        legacy_status,
        reason_code,
        effect_state,
        terminal_detail_digest,
        review_candidate,
    }
}

fn legacy_bounded_nonempty_text<'a>(
    item: &'a serde_json::Value,
    key: &str,
    max_chars: usize,
) -> Option<&'a str> {
    item.get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty() && value.chars().count() <= max_chars)
}

fn legacy_bounded_optional_reference(item: &serde_json::Value, key: &str) -> Option<String> {
    legacy_bounded_nonempty_text(item, key, 512).map(str::to_string)
}

fn legacy_evidence_file_name(
    legacy_path: &Path,
    source_digest: &str,
    quarantined: bool,
) -> Result<String> {
    validate_digest("legacy scheduled task source digest", source_digest)?;
    let source_name = legacy_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("legacy scheduled task source has an invalid file name"))?;
    let digest_suffix = source_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow::anyhow!("legacy scheduled task source digest is invalid"))?;
    let disposition = if quarantined {
        "quarantined"
    } else {
        "imported"
    };
    Ok(format!(
        "{source_name}.{disposition}.sha256_{}",
        &digest_suffix[..16]
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LegacyFileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    byte_len: u64,
    modified_marker: i128,
}

impl LegacyFileIdentity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Result<Self> {
        if !metadata.file_type().is_file() {
            anyhow::bail!("legacy scheduled task source must be a regular file");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                byte_len: metadata.len(),
                modified_marker: i128::from(metadata.mtime()) * 1_000_000_000
                    + i128::from(metadata.mtime_nsec()),
            })
        }
        #[cfg(not(unix))]
        {
            let modified_marker = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_nanos() as i128)
                .unwrap_or_default();
            Ok(Self {
                byte_len: metadata.len(),
                modified_marker,
            })
        }
    }

    fn same_file_object(&self, other: &Self) -> bool {
        #[cfg(unix)]
        {
            self.device == other.device && self.inode == other.inode
        }
        #[cfg(not(unix))]
        {
            self == other
        }
    }
}

struct OpenedLegacyScheduledTaskSource {
    file: File,
    identity: LegacyFileIdentity,
    byte_len: u64,
    bytes: Option<Vec<u8>>,
    source_digest: String,
}

impl OpenedLegacyScheduledTaskSource {
    fn open(path: &Path) -> Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let mut file = options.open(path).with_context(|| {
            format!(
                "open legacy scheduled task source without following links {}",
                path.display()
            )
        })?;
        let before = LegacyFileIdentity::from_metadata(&file.metadata()?)?;
        let byte_len = before.byte_len;
        let (bytes, source_digest) = if byte_len <= MAX_LEGACY_SCHEDULED_TASK_BYTES {
            let mut bytes = Vec::with_capacity(byte_len.min(1024 * 1024) as usize);
            file.by_ref()
                .take(MAX_LEGACY_SCHEDULED_TASK_BYTES + 1)
                .read_to_end(&mut bytes)
                .with_context(|| {
                    format!(
                        "read bounded legacy scheduled task source {}",
                        path.display()
                    )
                })?;
            if bytes.len() as u64 != byte_len {
                anyhow::bail!("legacy scheduled task source changed length while being read");
            }
            let digest = digest_bytes(&bytes);
            (Some(bytes), digest)
        } else {
            let mut prefix = Vec::with_capacity(MAX_OVERSIZED_LEGACY_FINGERPRINT_BYTES as usize);
            file.by_ref()
                .take(MAX_OVERSIZED_LEGACY_FINGERPRINT_BYTES)
                .read_to_end(&mut prefix)
                .with_context(|| {
                    format!(
                        "read bounded oversized legacy scheduled task fingerprint {}",
                        path.display()
                    )
                })?;
            let prefix_digest = digest_bytes(&prefix);
            let digest = digest_parts(&[
                "legacy_oversized_source_bounded_fingerprint_v1",
                &byte_len.to_string(),
                &before.modified_marker.to_string(),
                &prefix_digest,
            ]);
            (None, digest)
        };
        let after = LegacyFileIdentity::from_metadata(&file.metadata()?)?;
        if before != after {
            anyhow::bail!("legacy scheduled task source changed while being read");
        }
        Ok(Self {
            file,
            identity: before,
            byte_len,
            bytes,
            source_digest,
        })
    }

    fn verify_path_identity(&self, path: &Path) -> Result<()> {
        let reopened = Self::open(path)?;
        if !self.identity.same_file_object(&reopened.identity)
            || self.identity != reopened.identity
            || self.source_digest != reopened.source_digest
        {
            anyhow::bail!("legacy scheduled task path no longer names the opened source file");
        }
        Ok(())
    }
}

fn retire_legacy_source_file(
    source_path: &Path,
    evidence_path: &Path,
    opened_source: &OpenedLegacyScheduledTaskSource,
) -> Result<()> {
    opened_source.verify_path_identity(source_path)?;

    match std::fs::symlink_metadata(evidence_path) {
        Ok(_) => {
            let evidence = OpenedLegacyScheduledTaskSource::open(evidence_path)?;
            if evidence.source_digest != opened_source.source_digest
                || opened_source.bytes.is_none()
            {
                anyhow::bail!("legacy scheduled task evidence target has conflicting identity");
            }
            opened_source.verify_path_identity(source_path)?;
            let source_name = source_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("scheduled_tasks.json");
            let retired_duplicate = source_path.with_file_name(format!(
                ".{source_name}.retired-duplicate-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::rename(source_path, &retired_duplicate).with_context(|| {
                format!(
                    "atomically detach duplicate legacy scheduled task owner {}",
                    source_path.display(),
                )
            })?;
            let detached = OpenedLegacyScheduledTaskSource::open(&retired_duplicate)?;
            if !opened_source.identity.same_file_object(&detached.identity)
                || opened_source.identity != detached.identity
                || opened_source.source_digest != detached.source_digest
            {
                anyhow::bail!("detached legacy scheduled task owner changed file identity");
            }
            std::fs::remove_file(&retired_duplicate).with_context(|| {
                format!(
                    "remove verified detached legacy scheduled task owner {}",
                    retired_duplicate.display()
                )
            })?;
            harden_legacy_evidence_permissions(&evidence.file, evidence_path)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            opened_source.verify_path_identity(source_path)?;
            std::fs::rename(source_path, evidence_path).with_context(|| {
                format!(
                    "atomically retire legacy scheduled task source {} to {}",
                    source_path.display(),
                    evidence_path.display()
                )
            })?;
            let evidence = OpenedLegacyScheduledTaskSource::open(evidence_path)?;
            if !opened_source.identity.same_file_object(&evidence.identity)
                || opened_source.identity != evidence.identity
                || opened_source.source_digest != evidence.source_digest
            {
                anyhow::bail!(
                    "retired legacy scheduled task evidence is not the opened source identity"
                );
            }
            harden_legacy_evidence_permissions(&evidence.file, evidence_path)?;
        }
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    if let Some(parent) = source_path.parent() {
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| {
                format!(
                    "fsync legacy scheduled task evidence directory {}",
                    parent.display()
                )
            })?;
    }
    Ok(())
}

fn harden_legacy_evidence_permissions(file: &File, evidence_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .with_context(|| {
                format!(
                    "restrict legacy scheduled task evidence permissions {}",
                    evidence_path.display()
                )
            })?;
    }
    Ok(())
}

fn digest_bytes(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    let hex = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn validate_reference(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().count() > 512 {
        anyhow::bail!("{label} is missing or exceeds the metadata bound");
    }
    Ok(())
}

fn validate_new_task(task: &ScheduledTask) -> Result<()> {
    validate_reference("scheduled task id", &task.id)?;
    validate_reference("scheduled task title", &task.title)?;
    validate_reference("scheduled task action type", &task.action_type)?;
    if task.description.chars().count() > MAX_SCHEDULED_TASK_DESCRIPTION_CHARS {
        anyhow::bail!("scheduled task description exceeds the provider payload bound");
    }
    if !matches!(task.priority.as_str(), "low" | "medium" | "high") {
        anyhow::bail!("scheduled task priority is not canonical");
    }
    if task.status != "pending"
        || task.attempt_count != 0
        || task.claim_token.is_some()
        || task.lease_expires_at.is_some()
        || task.completed_at.is_some()
        || task.last_error.is_some()
        || task.result_digest.is_some()
        || task.result_ref.is_some()
    {
        anyhow::bail!("new scheduled task must start from pristine pending state");
    }
    chrono::DateTime::parse_from_rfc3339(&task.created_at)
        .context("scheduled task created_at must be RFC3339")?;
    if let Some(due_date) = task.due_date.as_deref() {
        chrono::DateTime::parse_from_rfc3339(due_date)
            .context("scheduled task due_date must be RFC3339")?;
    }
    if let Some(source_proposal_id) = task.source_proposal_id.as_deref() {
        validate_reference("scheduled source proposal id", source_proposal_id)?;
    }
    if let Some(source_run_id) = task.source_run_id.as_deref() {
        validate_reference("scheduled source run id", source_run_id)?;
    }
    task.provider_grant.validate_for_task(task)?;
    if task.provider_grant.data_route == ProviderDataRoute::PolicyAllowed
        && task.provider_grant_authority
            != ScheduledTaskGrantAuthorityProof::CanonicalReviewedPolicyRoute
    {
        anyhow::bail!(
            "deserialized scheduled cloud grant has facts but no canonical review authority"
        );
    }
    Ok(())
}

fn validate_reason_code(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_:-".contains(&byte)
        })
    {
        anyhow::bail!("scheduled reason must be a bounded machine reason code");
    }
    Ok(())
}

fn validate_digest(label: &str, value: &str) -> Result<()> {
    let hex = value.strip_prefix("sha256:").unwrap_or_default();
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        anyhow::bail!("{label} must be a sha256 digest");
    }
    Ok(())
}

fn validate_scheduled_result_ref(value: &str) -> Result<()> {
    if value.len() > 768
        || !value.starts_with("conversation://")
        || !value.contains("/message/")
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        anyhow::bail!("scheduled result ref must identify a canonical conversation message");
    }
    Ok(())
}

#[cfg(any(test, feature = "test-utils"))]
fn load_unknown_attempt_binding(
    conn: &Connection,
    authority: Arc<TaskStoreRuntimeAuthority>,
    task_id: &str,
    attempt_id: &str,
) -> Result<Option<ScheduledUnknownAttemptBinding>> {
    let attempt = conn
        .query_row(
            "SELECT a.claim_token, a.attempt_number, a.provider_grant_id,
                    a.provider_provenance_state
             FROM tasks t JOIN scheduler_attempts a ON a.task_id = t.id
             WHERE t.id = ?1
               AND t.status IN ('unknown', 'unknown_legacy_execution_state')
               AND a.attempt_id = ?2 AND a.status = 'unknown'",
            params![task_id, attempt_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((claim_token, attempt_number, provider_grant_id, provider_provenance_state)) = attempt
    else {
        return Ok(None);
    };
    let task = load_task_from_connection(conn, task_id)?;
    let task_revision_digest =
        crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "schema": "scheduled_unknown_task_revision_v1",
            "task": task,
            "attemptId": attempt_id,
            "attemptNumber": attempt_number,
            "claimTokenDigest": digest_ref(&claim_token),
            "providerGrantId": provider_grant_id,
            "providerProvenanceState": provider_provenance_state,
        }))
        .1;
    Ok(Some(ScheduledUnknownAttemptBinding {
        store: authority,
        task_id: task_id.to_string(),
        task_revision_digest,
        attempt_id: attempt_id.to_string(),
        attempt_number,
        claim_token_digest: digest_ref(&claim_token),
        provider_grant_id,
        provider_provenance_state,
    }))
}

#[cfg(any(test, feature = "test-utils"))]
fn scheduled_unknown_attempt_binding_digest(binding: &ScheduledUnknownAttemptBinding) -> String {
    crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
        "schema": "scheduled_unknown_attempt_binding_v1",
        "canonicalStoreIdentity": binding.store.canonical_store_identity.as_ref(),
        "databaseSlotVerifier": binding.store.database_slot_verifier.as_ref(),
        "processEpochId": binding.store.process_epoch_id,
        "writerOwnerGenerationId": binding.store.writer_owner_generation_id,
        "taskId": binding.task_id,
        "taskRevisionDigest": binding.task_revision_digest,
        "attemptId": binding.attempt_id,
        "attemptNumber": binding.attempt_number,
        "claimTokenDigest": binding.claim_token_digest,
        "providerGrantId": binding.provider_grant_id,
        "providerProvenanceState": binding.provider_provenance_state,
    }))
    .1
}

#[cfg(any(test, feature = "test-utils"))]
fn scheduled_reconciliation_record_digest(record: &ScheduledReconciliationRecord) -> String {
    let resolution = match &record.resolution {
        #[cfg(any(test, feature = "test-utils"))]
        ScheduledReconciliationResolution::RetrySafe => serde_json::json!({
            "kind": "retry_safe",
        }),
        ScheduledReconciliationResolution::ConfirmedFailed { reason_code } => {
            serde_json::json!({
                "kind": "confirmed_failed",
                "reasonCode": reason_code,
            })
        }
        #[cfg(any(test, feature = "test-utils"))]
        ScheduledReconciliationResolution::ConfirmedCompleted {
            result_ref,
            result_digest,
        } => serde_json::json!({
            "kind": "confirmed_completed",
            "resultRef": result_ref,
            "resultDigest": result_digest,
        }),
    };
    crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
        "schema": "scheduled_reconciliation_admission_v1",
        "unknownAttemptDigest": scheduled_unknown_attempt_binding_digest(&record.binding),
        "resolution": resolution,
        "evidenceId": record.evidence_id,
        "issuer": record.issuer.as_str(),
        "evidenceKind": record.evidence_kind.as_str(),
        "evidenceRef": record.evidence_ref,
        "evidenceDigest": record.evidence_digest,
        "issuedAt": record.issued_at,
        "sourceId": record.source_id,
    }))
    .1
}

#[cfg(any(test, feature = "test-utils"))]
fn validate_reconciliation_record(record: &ScheduledReconciliationRecord) -> Result<()> {
    validate_reference("reconciliation evidence id", &record.evidence_id)?;
    validate_reference("reconciliation evidence ref", &record.evidence_ref)?;
    validate_digest("reconciliation evidence digest", &record.evidence_digest)?;
    validate_digest("reconciliation source id", &record.source_id)?;
    validate_digest(
        "reconciliation task revision",
        &record.binding.task_revision_digest,
    )?;
    validate_digest(
        "reconciliation claim token",
        &record.binding.claim_token_digest,
    )?;
    chrono::DateTime::parse_from_rfc3339(&record.issued_at)
        .context("reconciliation evidence issued_at must be RFC3339")?;
    if !record.binding.store.available
        || record.binding.store.process_epoch_id.is_nil()
        || record.binding.store.writer_owner_generation_id.is_nil()
        || record.binding.task_id.trim().is_empty()
        || record.binding.attempt_id.trim().is_empty()
        || record.binding.provider_grant_id.trim().is_empty()
    {
        anyhow::bail!("scheduled reconciliation binding is incomplete");
    }
    Ok(())
}

fn provider_data_route_label(route: ProviderDataRoute) -> &'static str {
    match route {
        ProviderDataRoute::LocalOnly => "local_only",
        ProviderDataRoute::PolicyAllowed => "policy_allowed",
    }
}

fn parse_provider_data_route(value: String) -> Result<ProviderDataRoute> {
    match value.as_str() {
        "local_only" => Ok(ProviderDataRoute::LocalOnly),
        "policy_allowed" => Ok(ProviderDataRoute::PolicyAllowed),
        _ => anyhow::bail!("unknown scheduled provider data route"),
    }
}

fn parse_scheduled_provider_grant_scope(value: String) -> Result<ScheduledProviderGrantScope> {
    match value.as_str() {
        "local_only_durable" => Ok(ScheduledProviderGrantScope::LocalOnlyDurable),
        "single_execution" => Ok(ScheduledProviderGrantScope::SingleExecution),
        _ => anyhow::bail!("unknown scheduled provider grant scope"),
    }
}

fn parse_provider_payload_purpose(value: String) -> Result<ProviderPayloadPurpose> {
    match value.as_str() {
        "agent_loop_step" => Ok(ProviderPayloadPurpose::AgentLoopStep),
        _ => anyhow::bail!("unknown scheduled provider payload purpose"),
    }
}

fn digest_ref(value: &str) -> String {
    digest_parts(&[value])
}

fn scheduled_claim_seal_material(
    task: &ScheduledTask,
    claim_token: &str,
    attempt_id: &str,
    attempt_number: u32,
    provider_grant: &ScheduledProviderGrantV2,
    process_epoch_id: uuid::Uuid,
    writer_owner_generation_id: uuid::Uuid,
) -> Result<Vec<u8>> {
    let grant_authority = match task.provider_grant_authority {
        ScheduledTaskGrantAuthorityProof::UntrustedSerializedState => "untrusted_serialized_state",
        ScheduledTaskGrantAuthorityProof::CanonicalReviewedPolicyRoute => {
            "canonical_reviewed_policy_route"
        }
    };
    serde_json::to_vec(&serde_json::json!({
        "schema": "scheduled_task_claim_seal_v2",
        "task": task,
        "taskGrantAuthority": grant_authority,
        "claimToken": claim_token,
        "attemptId": attempt_id,
        "attemptNumber": attempt_number,
        "attemptProviderGrant": provider_grant,
        "processEpochId": process_epoch_id,
        "writerOwnerGenerationId": writer_owner_generation_id,
    }))
    .map_err(|error| anyhow::anyhow!("scheduled claim seal material failed: {error}"))
}

pub(crate) fn scheduled_task_schedule_digest(
    task_id: &str,
    action_type: &str,
    due_date: Option<&str>,
) -> String {
    crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
        "taskId": task_id,
        "actionType": action_type,
        "dueAt": due_date,
    }))
    .1
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
pub(crate) fn scheduled_provider_policy_decision_digest(
    task_id: &str,
    subject_digest: &str,
    schedule_digest: &str,
    provider_digest: &str,
    model_digest: &str,
    grant_expires_at: &DateTime<Utc>,
    review_snapshot_digest: &str,
    review_dispatch_claim_digest: &str,
    reason_code: &str,
) -> String {
    crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
        "schema": "openlife.scheduledProviderRouteDecision.v2",
        "policyVersion": "scheduled_provider_policy_v2",
        "taskId": task_id,
        "subjectDigest": subject_digest,
        "scheduleDigest": schedule_digest,
        "dataRoute": "policy_allowed",
        "providerDigest": provider_digest,
        "modelDigest": model_digest,
        "grantScope": "single_execution",
        "expiresAt": grant_expires_at.to_rfc3339(),
        "reviewSnapshotDigest": review_snapshot_digest,
        "reviewDispatchClaimDigest": review_dispatch_claim_digest,
        "reasonCode": reason_code,
    }))
    .1
}

fn digest_content(value: &str) -> String {
    let bytes = ring::digest::digest(&ring::digest::SHA256, value.as_bytes());
    let hex = bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn retry_eligible_at(
    now: chrono::DateTime<chrono::Utc>,
    attempt_number: u32,
) -> chrono::DateTime<chrono::Utc> {
    let exponent = attempt_number.saturating_sub(1).min(3);
    let seconds = 60_i64.saturating_mul(1_i64 << exponent);
    now + chrono::Duration::seconds(seconds)
}

fn validate_tool_dispatch_started_identity(
    attempt: &ToolDispatchAttempt,
    receipt: &ToolExecutionReceipt,
) -> Result<()> {
    validate_reference("tool receipt id", &attempt.receipt_id)?;
    validate_reference("tool manifest id", &attempt.manifest_id)?;
    validate_reference("tool name", &attempt.tool_name)?;
    validate_digest(
        "tool manifest contract digest",
        &attempt.manifest_contract_digest,
    )?;
    validate_digest("tool input hash", &attempt.input_hash)?;
    validate_digest("tool request digest", &attempt.request_digest)?;
    if i64::try_from(attempt.input_length_bytes).is_err() {
        anyhow::bail!("tool input length exceeds sqlite integer range");
    }
    if uuid::Uuid::parse_str(&attempt.receipt_id).is_err()
        || uuid::Uuid::parse_str(&receipt.receipt_id).is_err()
    {
        anyhow::bail!("tool receipt id is not a UUID");
    }
    if let Some(source_run_id) = attempt.source_run_id.as_deref() {
        validate_reference("tool source run id", source_run_id)?;
    }
    if attempt.receipt_id != receipt.receipt_id
        || receipt.manifest_id.as_deref() != Some(attempt.manifest_id.as_str())
        || attempt.source_run_id != receipt.source_run_id
        || attempt.request_digest != receipt.request_digest
        || attempt.action_effect != receipt.action_effect
        || attempt.idempotency_contract != receipt.idempotency_contract
    {
        anyhow::bail!("tool dispatch start differs from its prepared immutable identity");
    }
    if receipt.action_effect == ToolActionEffect::Unknown
        || receipt.idempotency_contract
            == crate::tool_manifest::ToolIdempotencyContract::Unspecified
    {
        anyhow::bail!("tool dispatch start lacks an explicit effect/idempotency contract");
    }
    let dispatched_at = receipt
        .dispatched_at
        .ok_or_else(|| anyhow::anyhow!("tool dispatch start has no dispatched_at"))?;
    if receipt.dispatch_kind == ToolDispatchKind::NotAttempted
        || receipt.dispatch_kind == ToolDispatchKind::Unknown
        || receipt.dispatch_attempt_count != 1
        || receipt.transport_status != ToolTransportStatus::Dispatched
        || receipt.effect_status != ToolEffectStatus::NotAttempted
        || receipt.execution_outcome != ToolExecutionOutcome::NotObserved
        || receipt.started_at > dispatched_at
        || receipt.response_observed_at.is_some()
        || receipt.finished_at.is_some()
    {
        anyhow::bail!("tool dispatch start is not the first adapter-owned transition");
    }
    Ok(())
}

fn digest_parts(parts: &[&str]) -> String {
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    for part in parts {
        context.update(&(part.len() as u64).to_be_bytes());
        context.update(part.as_bytes());
    }
    let hex = context
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keeps caller-shaped fixtures confined to the test build while forcing
    /// every persistence assertion through the production one-shot admission
    /// API. Product code has no equivalent extension trait.
    trait TaskStoreProviderTruthFixtureExt {
        fn record_provider_started(
            &self,
            claim: &ScheduledTaskClaim,
            request_id: &str,
            provider: &str,
            model: &str,
            started_at: chrono::DateTime<chrono::Utc>,
            policy_evidence: &ProviderPolicyReceiptEvidence,
        ) -> Result<bool>;

        fn record_provider_terminal(
            &self,
            claim: &ScheduledTaskClaim,
            receipt: &ProviderInvocationReceipt,
        ) -> Result<bool>;
    }

    impl TaskStoreProviderTruthFixtureExt for TaskStore {
        fn record_provider_started(
            &self,
            claim: &ScheduledTaskClaim,
            request_id: &str,
            provider: &str,
            model: &str,
            started_at: chrono::DateTime<chrono::Utc>,
            policy_evidence: &ProviderPolicyReceiptEvidence,
        ) -> Result<bool> {
            drop(self.lock_writable_connection("record_provider_truth")?);
            let progress = crate::scheduler::ProviderInvocationProgress::Started {
                request_id: request_id.to_string(),
                provider: provider.to_string(),
                model: model.to_string(),
                started_at,
                policy_evidence: policy_evidence.clone(),
            };
            let admission =
                crate::scheduler::issue_scheduled_provider_truth_test_admission(claim, &progress)?;
            self.record_provider_truth(claim, admission)
        }

        fn record_provider_terminal(
            &self,
            claim: &ScheduledTaskClaim,
            receipt: &ProviderInvocationReceipt,
        ) -> Result<bool> {
            drop(self.lock_writable_connection("record_provider_truth")?);
            let progress = match receipt.status {
                ProviderInvocationStatus::Completed => {
                    crate::scheduler::ProviderInvocationProgress::Completed(receipt.clone())
                }
                ProviderInvocationStatus::Failed => {
                    crate::scheduler::ProviderInvocationProgress::Failed(receipt.clone())
                }
                ProviderInvocationStatus::RemoteUnknown => {
                    crate::scheduler::ProviderInvocationProgress::RemoteUnknown(receipt.clone())
                }
            };
            let admission =
                crate::scheduler::issue_scheduled_provider_truth_test_admission(claim, &progress)?;
            self.record_provider_truth(claim, admission)
        }
    }

    #[test]
    fn production_task_store_exposes_only_the_admission_owned_provider_truth_write() {
        let production = include_str!("tasks.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production TaskStore source");
        let removed_start = ["pub fn record_provider_", "started("].concat();
        let removed_terminal = ["pub fn record_provider_", "terminal("].concat();
        assert!(!production.contains(&removed_start));
        assert!(!production.contains(&removed_terminal));
        assert!(production.contains("pub fn record_provider_truth("));
        assert!(production.contains("admission.consume_for_claim(claim)?"));
    }

    #[test]
    fn production_reconciliation_has_no_caller_constructible_resolution_or_evidence_lane() {
        let source = include_str!("tasks.rs").replace("\r\n", "\n");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production TaskStore source");
        assert!(!production.contains("pub struct ScheduledReconciliationEvidenceV1"));
        assert!(!production.contains("pub enum ScheduledUnknownResolution"));
        assert!(production.contains("pub struct ScheduledReconciliationAdmission"));
    }

    fn due_task(id: &str, proposal_id: &str) -> ScheduledTask {
        let mut task = ScheduledTask::new(
            "Review today",
            "Prepare a short review",
            Some((chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339()),
            "medium",
        );
        task.id = id.into();
        task.source_run_id = Some("run-1".into());
        task.source_proposal_id = Some(proposal_id.into());
        task.seal_deterministic_local_provider_grant();
        task
    }

    fn reviewed_cloud_fixture(
        id: &str,
        provider: &str,
        model: &str,
    ) -> (
        ScheduledTask,
        crate::agent::ProposalStore,
        crate::agent::AgentProposal,
        String,
    ) {
        reviewed_cloud_fixture_with_store(
            id,
            provider,
            model,
            crate::agent::ProposalStore::new_in_memory().unwrap(),
        )
    }

    fn reviewed_cloud_fixture_with_store(
        id: &str,
        provider: &str,
        model: &str,
        proposal_store: crate::agent::ProposalStore,
    ) -> (
        ScheduledTask,
        crate::agent::ProposalStore,
        crate::agent::AgentProposal,
        String,
    ) {
        let due_at = chrono::Utc::now() - chrono::Duration::minutes(1);
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(2);
        let mut proposal = crate::agent::AgentProposal::new(
            crate::agent::ProposalType::ScheduledTask,
            "tasks.reviewed_cloud",
            serde_json::json!({
                "title": "Reviewed cloud schedule",
                "description": "Prepare a short review",
                "scheduled_at": due_at.to_rfc3339(),
                "priority": "medium",
                "tool": "scheduled_task",
                "provider_route": {
                    "data_route": "policy_allowed",
                    "provider": provider,
                    "model": model,
                    "grant_scope": "single_execution",
                    "consent_scope": "scheduled_provider_once",
                    "expires_at": expires_at.to_rfc3339(),
                }
            }),
            "Explicitly review one scheduled cloud execution.",
            1.0,
            crate::agent::RiskLevel::Medium,
            crate::agent::ProposalSource::Manual,
        );
        proposal.id = id.into();
        proposal_store.create_proposal(&proposal).unwrap();
        let claim_id = proposal_store
            .claim_dispatch(&proposal.id)
            .unwrap()
            .unwrap();
        let review = crate::agent::ReviewWorkflow::new(&proposal_store)
            .claimed_acceptance_snapshot(&proposal.id, &claim_id)
            .unwrap();
        let decision = crate::agent::main_chat_agent_v1::PolicyRouter
            .authorize_scheduled_provider_route(
                &review,
                crate::agent::main_chat_agent_v1::ScheduledProviderRouteRequest {
                    task_id: proposal.id.clone(),
                    description: "Prepare a short review".into(),
                    action_type: "scheduled_task".into(),
                    due_at,
                    provider: provider.into(),
                    model: model.into(),
                    requested_data_route: ProviderDataRoute::PolicyAllowed,
                    grant_expires_at: expires_at,
                },
            )
            .unwrap();
        let mut task = ScheduledTask::new(
            "Reviewed cloud schedule",
            "Prepare a short review",
            Some(due_at.to_rfc3339()),
            "medium",
        );
        task.id = proposal.id.clone();
        task.source_proposal_id = Some(proposal.id.clone());
        task.action_type = "scheduled_task".into();
        task.seal_reviewed_cloud_provider_grant(&decision).unwrap();
        (task, proposal_store, proposal, claim_id)
    }

    fn reviewed_cloud_task(id: &str, provider: &str, model: &str) -> ScheduledTask {
        reviewed_cloud_fixture(id, provider, model).0
    }

    fn set_reviewed_cloud_grant_expiry(
        task: &mut ScheduledTask,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) {
        let decision_digest = scheduled_provider_policy_decision_digest(
            &task.id,
            &task.provider_grant.subject_digest,
            &task.provider_grant.schedule_digest,
            task.provider_grant.provider_digest.as_deref().unwrap(),
            task.provider_grant.model_digest.as_deref().unwrap(),
            &expires_at,
            task.provider_grant
                .review_snapshot_digest
                .as_deref()
                .unwrap(),
            task.provider_grant
                .review_dispatch_claim_digest
                .as_deref()
                .unwrap(),
            &task.provider_grant.reason_code,
        );
        task.provider_grant.grant_expires_at = Some(expires_at.to_rfc3339());
        task.provider_grant.policy_decision_digest = decision_digest;
        task.provider_grant.grant_id = task
            .provider_grant
            .canonical_grant_id(&task.id, &task.action_type);
    }

    fn claim_and_begin(store: &TaskStore, task_id: &str, proposal_id: &str) -> ScheduledTaskClaim {
        store
            .create_task_idempotent(&due_task(task_id, proposal_id))
            .unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        assert!(store.begin_claim_execution(&claim).unwrap());
        claim
    }

    #[test]
    fn task_store_claim_seal_rejects_a_self_consistent_post_issue_rewrite() {
        let store = TaskStore::new_in_memory().unwrap();
        let mut claim = claim_and_begin(
            &store,
            "task-claim-seal-rewrite",
            "proposal-claim-seal-rewrite",
        );
        claim.validate_policy_authority().unwrap();

        claim
            .task
            .description
            .push_str(" rewritten after TaskStore issuance");
        claim.task.seal_deterministic_local_provider_grant();
        claim.provider_grant = claim.task.provider_grant.clone();

        let error = claim.validate_policy_authority().unwrap_err();
        assert!(error.to_string().contains("sealed revision"));
        assert!(store.owns_executing_claim(&claim).is_err());
    }

    #[test]
    fn scheduled_claim_debug_redacts_body_token_and_grant_material() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(&store, "task-debug-redaction", "proposal-debug-redaction");
        let rendered = format!("{claim:?}");

        assert!(rendered.contains(&claim.task.id));
        assert!(rendered.contains(&claim.attempt_id));
        assert!(rendered.contains(&claim.attempt_number.to_string()));
        assert!(!rendered.contains(&claim.task.title));
        assert!(!rendered.contains(&claim.task.description));
        assert!(!rendered.contains(&claim.claim_token));
        assert!(!rendered.contains(&claim.provider_grant.grant_id));
        assert!(rendered.contains("[REDACTED]"));
    }

    fn scheduled_provider_evidence(claim: &ScheduledTaskClaim) -> ProviderPolicyReceiptEvidence {
        ProviderPolicyReceiptEvidence {
            decision_id: claim.provider_grant.policy_decision_digest.clone(),
            policy_version: claim.provider_grant.policy_version.clone(),
            issuing_authority: ProviderPolicyAuthority::ScheduledPolicy,
            effective_data_route: claim.provider_grant.data_route,
            effective_local_restriction: None,
            subject_scope_digest: ProviderPolicyAuthorization::from_scheduled_claim(claim)
                .unwrap()
                .subject_scope_digest(),
            payload_purpose: Some(ProviderPayloadPurpose::AgentLoopStep),
            unfiltered_payload_digest: Some(digest_ref("scheduled unfiltered payload")),
            context_manifest_digest: digest_ref("scheduled context manifest"),
            prepared_envelope_digest: Some(digest_ref("scheduled prepared envelope")),
            provider_config_generation: "test-scheduled-provider-generation".into(),
            network_policy_decision_digest: digest_ref("scheduled network decision"),
            selected_context_refs: Vec::new(),
            included_context_categories: Vec::new(),
            declared_payload_categories: vec![
                crate::llm::ProviderPayloadCategory::RuntimeCompiledMessages,
            ],
            policy_provenance_refs: Vec::new(),
            raw_life_model_included: false,
            raw_unbounded_memory_included: false,
        }
    }

    fn completed_receipt(
        claim: &ScheduledTaskClaim,
        request_id: &str,
        started_at: chrono::DateTime<chrono::Utc>,
    ) -> ProviderInvocationReceipt {
        ProviderInvocationReceipt {
            request_id: request_id.into(),
            provider: "ollama".into(),
            model: "local-model".into(),
            status: ProviderInvocationStatus::Completed,
            started_at,
            finished_at: started_at + chrono::Duration::milliseconds(20),
            error_digest: None,
            simulated: false,
            policy_evidence: Some(scheduled_provider_evidence(claim)),
        }
    }

    fn failed_receipt(
        claim: &ScheduledTaskClaim,
        request_id: &str,
        started_at: chrono::DateTime<chrono::Utc>,
    ) -> ProviderInvocationReceipt {
        ProviderInvocationReceipt {
            request_id: request_id.into(),
            provider: "ollama".into(),
            model: "local-model".into(),
            status: ProviderInvocationStatus::Failed,
            started_at,
            finished_at: started_at + chrono::Duration::milliseconds(20),
            error_digest: Some(digest_ref("provider failed")),
            simulated: false,
            policy_evidence: Some(scheduled_provider_evidence(claim)),
        }
    }

    fn observed_tool_receipt_fixture(
        manifest_id: &str,
        source_run_id: &str,
        request_material: &str,
    ) -> (
        ToolDispatchAttempt,
        ToolExecutionReceipt,
        ToolExecutionReceipt,
    ) {
        let tracker = crate::tool_execution_receipt::ToolExecutionReceiptTracker::new(
            Some(source_run_id.into()),
            Some(manifest_id.into()),
            request_material.into(),
            ToolActionEffect::ReadOnly,
            crate::tool_manifest::ToolIdempotencyContract::Idempotent,
        );
        let prepared = tracker.snapshot();
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_text_digest(request_material);
        let attempt = ToolDispatchAttempt {
            receipt_id: prepared.receipt_id.clone(),
            manifest_id: manifest_id.into(),
            tool_name: "memory.search".into(),
            manifest_contract_digest: digest_ref("manifest contract"),
            input_hash,
            input_length_bytes: input_length_bytes as u64,
            source_run_id: Some(source_run_id.into()),
            request_digest: prepared.request_digest.clone(),
            action_effect: prepared.action_effect,
            idempotency_contract: prepared.idempotency_contract,
            process_risk:
                crate::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess,
            effect_may_survive_local_process: false,
        };
        tracker.mark_mcp_dispatched();
        let started = tracker.snapshot();
        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.finish();
        let terminal = tracker.snapshot();
        (attempt, started, terminal)
    }

    fn remote_unknown_tool_receipt_fixture(
        manifest_id: &str,
        source_run_id: &str,
        request_material: &str,
    ) -> (
        ToolDispatchAttempt,
        ToolExecutionReceipt,
        ToolExecutionReceipt,
    ) {
        let tracker = crate::tool_execution_receipt::ToolExecutionReceiptTracker::new(
            Some(source_run_id.into()),
            Some(manifest_id.into()),
            request_material.into(),
            ToolActionEffect::ExternalMutation,
            crate::tool_manifest::ToolIdempotencyContract::NonIdempotent,
        );
        let prepared = tracker.snapshot();
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_text_digest(request_material);
        let attempt = ToolDispatchAttempt {
            receipt_id: prepared.receipt_id.clone(),
            manifest_id: manifest_id.into(),
            tool_name: "external.write".into(),
            manifest_contract_digest: digest_ref("external manifest contract"),
            input_hash,
            input_length_bytes: input_length_bytes as u64,
            source_run_id: Some(source_run_id.into()),
            request_digest: prepared.request_digest.clone(),
            action_effect: prepared.action_effect,
            idempotency_contract: prepared.idempotency_contract,
            process_risk:
                crate::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess,
            effect_may_survive_local_process: true,
        };
        tracker.mark_network_dispatched();
        let started = tracker.snapshot();
        tracker.mark_remote_unknown();
        tracker.finish();
        let terminal = tracker.snapshot();
        (attempt, started, terminal)
    }

    #[cfg(unix)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ExactFileState {
        exists: bool,
        bytes: Vec<u8>,
        modified: Option<std::time::SystemTime>,
        mode: Option<u32>,
    }

    #[cfg(unix)]
    fn exact_file_state(path: &Path) -> ExactFileState {
        use std::os::unix::fs::PermissionsExt;

        match std::fs::metadata(path) {
            Ok(metadata) => ExactFileState {
                exists: true,
                bytes: std::fs::read(path).unwrap(),
                modified: metadata.modified().ok(),
                mode: Some(metadata.permissions().mode()),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ExactFileState {
                exists: false,
                bytes: Vec::new(),
                modified: None,
                mode: None,
            },
            Err(error) => panic!("capture exact file state for {}: {error}", path.display()),
        }
    }

    #[cfg(unix)]
    fn sqlite_family_states(path: &Path) -> Vec<(PathBuf, ExactFileState)> {
        [
            path.to_path_buf(),
            PathBuf::from(format!("{}-wal", path.display())),
            PathBuf::from(format!("{}-shm", path.display())),
        ]
        .into_iter()
        .map(|member| {
            let state = exact_file_state(&member);
            (member, state)
        })
        .collect()
    }

    #[cfg(unix)]
    fn assert_file_states_unchanged(before: &[(PathBuf, ExactFileState)]) {
        for (path, expected) in before {
            assert_eq!(
                exact_file_state(path),
                *expected,
                "file state changed at {}",
                path.display()
            );
        }
    }

    fn downgrade_current_task_store_to_v14_without_writer_generation(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "DROP TRIGGER task_store_authority_metadata_immutable_update;
             DROP TRIGGER task_store_authority_metadata_immutable_delete;
             DELETE FROM task_store_metadata
             WHERE key = 'canonical_task_store_owner_lock_verifier_v1';
             UPDATE openlife_schema_versions SET version = 14
             WHERE component = 'task_store';
             ALTER TABLE scheduler_provider_receipts
             DROP COLUMN writer_owner_generation_id;
             ALTER TABLE scheduler_tool_dispatches
             DROP COLUMN writer_owner_generation_id;
             ALTER TABLE scheduler_attempts
             DROP COLUMN writer_owner_generation_id;",
        )
        .unwrap();
    }

    #[test]
    fn source_proposal_is_idempotent_claim_is_single_owner_and_policy_is_deterministic() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = due_task("task-1", "proposal-1");
        let expected_grant = task.provider_grant.clone();
        assert_eq!(expected_grant.data_route, ProviderDataRoute::LocalOnly);
        assert!(!expected_grant.allows_cloud());
        assert!(store.create_task_idempotent(&task).unwrap());
        assert!(!store.create_task_idempotent(&task).unwrap());

        let first = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        assert_eq!(first.provider_grant, expected_grant);
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_none());
        let attempt = store
            .latest_attempt_for_task(&first.task.id)
            .unwrap()
            .unwrap();
        assert_eq!(attempt.status, "claimed");
        assert_eq!(attempt.data_route, "local_only");
        assert_eq!(attempt.provider_grant_id, expected_grant.grant_id);
    }

    #[test]
    fn idempotency_key_rejects_every_canonical_payload_drift() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = due_task("task-idempotency-payload", "proposal-idempotency-payload");
        assert!(store.create_task_idempotent(&task).unwrap());

        let mut changed_title = task.clone();
        changed_title.title = "Different canonical title".into();
        assert!(store.create_task_idempotent(&changed_title).is_err());

        let mut changed_id = task.clone();
        changed_id.id = "task-idempotency-payload-rebound".into();
        changed_id.seal_deterministic_local_provider_grant();
        assert!(store.create_task_idempotent(&changed_id).is_err());

        assert_eq!(store.list_tasks(None).unwrap(), vec![task]);
    }

    #[test]
    fn concurrent_exact_task_replay_has_one_insert_and_one_canonical_payload() {
        let store = std::sync::Arc::new(TaskStore::new_in_memory().unwrap());
        let task = due_task("task-idempotency-race", "proposal-idempotency-race");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let inserted = (0..8)
            .map(|_| {
                let store = std::sync::Arc::clone(&store);
                let task = task.clone();
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    store.create_task_idempotent(&task).unwrap()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(|inserted| *inserted)
            .count();

        assert_eq!(inserted, 1);
        assert_eq!(store.list_tasks(None).unwrap(), vec![task]);
    }

    #[test]
    fn canonical_provider_grant_is_bound_before_claim_and_cannot_be_reissued_from_mutated_task() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = due_task("task-grant-bound", "proposal-grant-bound");
        let expected_grant = task.provider_grant.clone();
        assert_eq!(expected_grant.data_route, ProviderDataRoute::LocalOnly);
        assert!(!expected_grant.allows_cloud());
        assert!(store.create_task_idempotent(&task).unwrap());

        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        assert_eq!(claim.provider_grant, expected_grant);

        let mut rebound = due_task("task-grant-rebound", "proposal-grant-rebound");
        rebound.description = "A different provider subject".into();
        assert!(store.create_task_idempotent(&rebound).is_err());
    }

    #[test]
    fn reviewed_cloud_grant_is_task_provider_model_expiry_and_single_use_bound() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = reviewed_cloud_task("task-reviewed-cloud", "openai", "gpt-reviewed");
        assert_eq!(
            task.provider_grant.data_route,
            ProviderDataRoute::PolicyAllowed
        );
        assert_eq!(
            task.provider_grant.grant_scope,
            ScheduledProviderGrantScope::SingleExecution
        );
        assert!(task.provider_grant.grant_expires_at.is_some());
        assert!(store.create_task_idempotent(&task).unwrap());

        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        assert!(store.begin_claim_execution(&claim).unwrap());
        assert!(store
            .record_provider_started(
                &claim,
                "wrong-provider-request",
                "openrouter",
                "gpt-reviewed",
                chrono::Utc::now(),
                &scheduled_provider_evidence(&claim),
            )
            .is_err());
        assert!(store
            .record_provider_started(
                &claim,
                "wrong-model-request",
                "openai",
                "different-model",
                chrono::Utc::now(),
                &scheduled_provider_evidence(&claim),
            )
            .is_err());
        assert!(store
            .record_provider_started(
                &claim,
                "reviewed-provider-request",
                "openai",
                "gpt-reviewed",
                chrono::Utc::now(),
                &scheduled_provider_evidence(&claim),
            )
            .unwrap());
    }

    #[test]
    fn reviewed_cloud_grant_restart_requires_canonical_review_reproof() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scheduled-cloud-restart.sqlite");
        let proposal_path = directory.path().join("proposals.sqlite");
        let (task, proposal_store, mut proposal, claim_id) = reviewed_cloud_fixture_with_store(
            "task-reviewed-cloud-restart",
            "openai",
            "gpt-reviewed",
            crate::agent::ProposalStore::new(&proposal_path).unwrap(),
        );
        let expected_grant = task.provider_grant.clone();
        {
            let store = TaskStore::new(&path).unwrap();
            assert!(store.create_task_idempotent(&task).unwrap());
        }
        assert!(proposal_store
            .mark_effect_confirmed_projection_pending(&proposal.id, &claim_id)
            .unwrap());
        proposal.accept();
        assert!(proposal_store
            .project_confirmed_effect(&proposal, &claim_id)
            .unwrap());
        drop(proposal_store);

        let reopened = TaskStore::new(&path).unwrap();
        let reopened_proposal_store = crate::agent::ProposalStore::new(&proposal_path).unwrap();
        let persisted = reopened.list_tasks(None).unwrap().remove(0);
        assert_eq!(persisted.provider_grant, expected_grant);
        assert_eq!(
            persisted.provider_grant.data_route,
            ProviderDataRoute::PolicyAllowed
        );
        let error = reopened
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap_err()
            .to_string();
        assert!(error.contains("lacks canonical ReviewWorkflow authority"));
        let proof = crate::agent::ReviewWorkflow::new(&reopened_proposal_store)
            .materialized_acceptance_snapshot(&proposal.id)
            .unwrap();
        let read_only = TaskStore::open_read_only_existing_with_authority_key(
            &path,
            &TaskStoreAuthorityKey::from_key_material(&[0x5a; 32]).unwrap(),
        )
        .unwrap();
        let restore_error = read_only
            .restore_reviewed_cloud_authority(&proof)
            .unwrap_err()
            .to_string();
        assert_eq!(
            restore_error,
            "scheduled_task_store_write_authority_required:restore_reviewed_cloud_authority"
        );
        assert_eq!(
            read_only.list_tasks(None).unwrap()[0].provider_grant_authority,
            ScheduledTaskGrantAuthorityProof::UntrustedSerializedState
        );
        drop(read_only);
        reopened.restore_reviewed_cloud_authority(&proof).unwrap();
        assert!(reopened
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_some());
    }

    #[test]
    fn public_cloud_task_row_cannot_recompute_restart_authority() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scheduled-cloud-unproven.sqlite");
        let task = reviewed_cloud_task("task-cloud-unproven", "openai", "gpt-reviewed");
        {
            let store = TaskStore::new(&path).unwrap();
            store.create_task_idempotent(&task).unwrap();
        }

        let reopened = TaskStore::new(&path).unwrap();
        let public_roundtrip: ScheduledTask = serde_json::from_value(
            serde_json::to_value(reopened.list_tasks(None).unwrap().remove(0)).unwrap(),
        )
        .unwrap();
        assert!(reopened.create_task_idempotent(&public_roundtrip).is_err());
        assert!(reopened
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .is_err());
        assert!(reopened
            .quarantine_unproven_reviewed_cloud_task(&task.id)
            .unwrap());
        let review_required = reopened.list_tasks(Some("review_required")).unwrap();
        assert_eq!(review_required.len(), 1);
        assert_eq!(
            review_required[0].last_error.as_deref(),
            Some("scheduled_cloud_authority_unproven_requires_fresh_review")
        );
    }

    #[test]
    fn serialized_cloud_grant_tamper_and_task_rebinding_are_rejected() {
        let task = reviewed_cloud_task("task-cloud-tamper", "openai", "gpt-reviewed");
        let serialized = serde_json::to_value(&task).unwrap();
        let untrusted_round_trip: ScheduledTask =
            serde_json::from_value(serialized.clone()).unwrap();
        let store = TaskStore::new_in_memory().unwrap();
        assert!(store.create_task_idempotent(&untrusted_round_trip).is_err());

        let mut serialized = serialized;
        serialized["provider_grant"]["provider_digest"] =
            serde_json::Value::String(digest_ref("attacker-provider"));
        let tampered: ScheduledTask = serde_json::from_value(serialized).unwrap();
        assert!(store.create_task_idempotent(&tampered).is_err());

        let mut tampered_live_decision = task.clone();
        tampered_live_decision.provider_grant.policy_decision_digest =
            digest_ref("attacker policy decision");
        tampered_live_decision.provider_grant.grant_id =
            tampered_live_decision.provider_grant.canonical_grant_id(
                &tampered_live_decision.id,
                &tampered_live_decision.action_type,
            );
        assert!(store
            .create_task_idempotent(&tampered_live_decision)
            .is_err());

        let mut rebound = task;
        rebound.id = "different-task-id".into();
        assert!(store.create_task_idempotent(&rebound).is_err());
        assert!(store.list_tasks(None).unwrap().is_empty());
    }

    #[test]
    fn expired_cloud_grant_fails_closed_before_claim() {
        let mut task = reviewed_cloud_task("task-cloud-expired", "openai", "gpt-reviewed");
        set_reviewed_cloud_grant_expiry(
            &mut task,
            chrono::Utc::now() - chrono::Duration::minutes(1),
        );
        let store = TaskStore::new_in_memory().unwrap();
        assert!(store.create_task_idempotent(&task).unwrap());

        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_none());
        let failed = store.list_tasks(Some("failed")).unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(
            failed[0].last_error.as_deref(),
            Some("scheduled_cloud_grant_expired")
        );
    }

    #[test]
    fn cloud_grant_expiry_is_rechecked_at_the_actual_provider_start_boundary() {
        let mut task = reviewed_cloud_task("task-cloud-start-expiry", "openai", "gpt-reviewed");
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);
        set_reviewed_cloud_grant_expiry(&mut task, expires_at);
        let store = TaskStore::new_in_memory().unwrap();
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        assert!(store.begin_claim_execution(&claim).unwrap());

        assert!(store
            .record_provider_started(
                &claim,
                "expired-at-provider-boundary",
                "openai",
                "gpt-reviewed",
                expires_at + chrono::Duration::seconds(1),
                &scheduled_provider_evidence(&claim),
            )
            .is_err());
        assert!(store
            .provider_receipts_for_attempt(&claim.attempt_id)
            .unwrap()
            .is_empty());
        let conn = store.lock_connection().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM scheduler_provider_grant_consumptions",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn cloud_grant_is_not_consumed_by_local_preflight_failure() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = reviewed_cloud_task("task-cloud-preflight", "openai", "gpt-reviewed");
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        assert!(store.begin_claim_execution(&claim).unwrap());
        assert_eq!(
            store
                .settle_claim_after_error(
                    &claim,
                    "provider_preflight_failed",
                    Some(&digest_ref("preflight failed")),
                )
                .unwrap(),
            ScheduledClaimSettlement::ReclaimedBeforeDispatch
        );
        let conn = store.lock_connection().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM scheduler_provider_grant_consumptions",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
        drop(conn);
        assert_eq!(store.list_tasks(Some("pending")).unwrap().len(), 1);
    }

    #[test]
    fn concurrent_cloud_provider_start_consumes_grant_with_one_durable_owner() {
        let store = std::sync::Arc::new(TaskStore::new_in_memory().unwrap());
        let task = reviewed_cloud_task("task-cloud-consume", "openai", "gpt-reviewed");
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        assert!(store.begin_claim_execution(&claim).unwrap());
        let claim = std::sync::Arc::new(claim);
        let policy_evidence = scheduled_provider_evidence(&claim);
        let started_at = chrono::Utc::now();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let inserted = (0..8)
            .map(|_| {
                let store = std::sync::Arc::clone(&store);
                let barrier = std::sync::Arc::clone(&barrier);
                let claim = std::sync::Arc::clone(&claim);
                let policy_evidence = policy_evidence.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .record_provider_started(
                            &claim,
                            "reviewed-provider-request",
                            "openai",
                            "gpt-reviewed",
                            started_at,
                            &policy_evidence,
                        )
                        .unwrap()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(|result| *result)
            .count();
        assert_eq!(inserted, 1, "same start fact is durably idempotent");
        let conn = store.lock_connection().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM scheduler_provider_grant_consumptions",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        drop(conn);
        assert_eq!(
            store
                .provider_receipts_for_attempt(&claim.attempt_id)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn task_creation_cannot_forge_running_or_terminal_scheduler_state() {
        let store = TaskStore::new_in_memory().unwrap();
        let mut forged = due_task("task-forged", "proposal-forged");
        forged.status = "running".into();
        forged.attempt_count = 1;
        forged.claim_token = Some("forged-claim".into());

        assert!(store.create_task_idempotent(&forged).is_err());
        assert!(store.list_tasks(None).unwrap().is_empty());
    }

    #[test]
    fn claim_and_attempt_insert_roll_back_together_on_fault() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = due_task("task-claim-fault", "proposal-claim-fault");
        store.create_task_idempotent(&task).unwrap();
        let attempt_id = digest_parts(&["scheduled_attempt_v1", &task.id, "1"]);
        {
            let conn = store.lock_connection().unwrap();
            conn.execute(
                "INSERT INTO scheduler_attempts (
                    attempt_id, task_id, claim_token, attempt_number, status,
                    provider_grant_id, policy_version, data_route, policy_reason_code,
                    provider_subject_digest, provider_payload_purpose,
                    provider_payload_contract_digest, provider_source_ref_digest,
                    process_epoch_id, writer_owner_generation_id, claimed_at
                 ) VALUES (?1, ?2, 'fault-token', 1, 'claimed', ?3, ?4, 'local_only',
                           ?5, ?6, ?7, ?8, ?9, 'fault-process-epoch',
                           'fault-writer-owner-generation', ?10)",
                params![
                    attempt_id,
                    task.id,
                    task.provider_grant.grant_id,
                    task.provider_grant.policy_version,
                    task.provider_grant.reason_code,
                    task.provider_grant.subject_digest,
                    task.provider_grant.payload_purpose.as_str(),
                    task.provider_grant.payload_contract_digest,
                    task.provider_grant.source_ref_digest,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .unwrap();
        }

        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .is_err());
        let persisted = store.list_tasks(Some("pending")).unwrap().remove(0);
        assert_eq!(persisted.attempt_count, 0);
        assert!(persisted.claim_token.is_none());
    }

    #[test]
    fn expired_claim_before_execution_is_reclaimed_for_a_fresh_attempt() {
        let store = TaskStore::new_in_memory().unwrap();
        store
            .create_task_idempotent(&due_task("task-pre-dispatch", "proposal-pre-dispatch"))
            .unwrap();
        let first = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(-1))
            .unwrap()
            .unwrap();

        assert_eq!(
            store.reconcile_expired_claims(chrono::Utc::now()).unwrap(),
            1
        );
        assert!(store.list_tasks(Some("unknown")).unwrap().is_empty());
        let second = store
            .claim_next_due(
                chrono::Utc::now() + chrono::Duration::minutes(10),
                chrono::Duration::seconds(30),
            )
            .unwrap()
            .unwrap();
        assert_eq!(first.attempt_number, 1);
        assert_eq!(second.attempt_number, 2);
    }

    #[test]
    fn explicit_pre_dispatch_failure_reclaims_but_timeout_after_dispatch_is_unknown() {
        let store = TaskStore::new_in_memory().unwrap();
        let first = claim_and_begin(&store, "task-settlement", "proposal-settlement");
        assert_eq!(
            store
                .settle_claim_after_error(
                    &first,
                    "local_model_unavailable",
                    Some(&digest_ref("adapter unavailable")),
                )
                .unwrap(),
            ScheduledClaimSettlement::ReclaimedBeforeDispatch
        );
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_none());

        let second = store
            .claim_next_due(
                chrono::Utc::now() + chrono::Duration::minutes(10),
                chrono::Duration::seconds(30),
            )
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&second).unwrap();
        let started_at = chrono::Utc::now();
        store
            .record_provider_started(
                &second,
                "provider-request-1",
                "ollama",
                "local-model",
                started_at,
                &scheduled_provider_evidence(&second),
            )
            .unwrap();
        assert_eq!(
            store.settle_claim_after_timeout(&second).unwrap(),
            ScheduledClaimSettlement::UnknownRequiresReconciliation
        );
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_none());

        let admission = store
            .issue_scheduled_reconciliation_test_admission(
                &second.task.id,
                &second.attempt_id,
                ScheduledReconciliationTestResolution::RetrySafe,
            )
            .unwrap();
        assert!(store.reconcile_unknown_attempt(admission).unwrap());
        let reconciled = store
            .latest_attempt_for_task(&second.task.id)
            .unwrap()
            .unwrap();
        assert_eq!(reconciled.status, "reconciled_retry_safe");
        assert!(reconciled.reconciliation_evidence_digest.is_some());
        assert_eq!(
            reconciled.reconciliation_issuer.as_deref(),
            Some("native_user_confirmation")
        );
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_some());
    }

    #[test]
    fn reconciliation_source_is_reissuable_after_transaction_failure() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(
            &store,
            "task-reconciliation-rollback",
            "proposal-reconciliation-rollback",
        );
        store
            .record_provider_started(
                &claim,
                "provider-request-reconciliation-rollback",
                "ollama",
                "local-model",
                chrono::Utc::now(),
                &scheduled_provider_evidence(&claim),
            )
            .unwrap();
        assert_eq!(
            store.settle_claim_after_timeout(&claim).unwrap(),
            ScheduledClaimSettlement::UnknownRequiresReconciliation
        );
        {
            let conn = store.lock_connection().unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_reconciliation_for_fault_test
                 BEFORE UPDATE OF status ON tasks
                 WHEN OLD.status = 'unknown' AND NEW.status = 'pending'
                 BEGIN
                    SELECT RAISE(ABORT, 'fault-injected reconciliation failure');
                 END;",
            )
            .unwrap();
        }
        let first = store
            .issue_scheduled_reconciliation_test_admission(
                &claim.task.id,
                &claim.attempt_id,
                ScheduledReconciliationTestResolution::RetrySafe,
            )
            .unwrap();
        assert!(store.reconcile_unknown_attempt(first).is_err());
        {
            let conn = store.lock_connection().unwrap();
            conn.execute_batch("DROP TRIGGER reject_reconciliation_for_fault_test;")
                .unwrap();
        }

        // The exact deterministic source id can be issued again only if the
        // failed transaction's RAII drop released the process-local registry.
        let retry = store
            .issue_scheduled_reconciliation_test_admission(
                &claim.task.id,
                &claim.attempt_id,
                ScheduledReconciliationTestResolution::RetrySafe,
            )
            .unwrap();
        assert!(store.reconcile_unknown_attempt(retry).unwrap());
    }

    #[test]
    fn consumed_cloud_grant_reconciliation_requires_fresh_review_not_fake_pending() {
        let store = TaskStore::new_in_memory().unwrap();
        let task = reviewed_cloud_task(
            "task-cloud-unknown-reconciliation",
            "openai",
            "gpt-reviewed",
        );
        store.create_task_idempotent(&task).unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&claim).unwrap();
        store
            .record_provider_started(
                &claim,
                "cloud-unknown-request",
                "openai",
                "gpt-reviewed",
                chrono::Utc::now(),
                &scheduled_provider_evidence(&claim),
            )
            .unwrap();
        assert_eq!(
            store.settle_claim_after_timeout(&claim).unwrap(),
            ScheduledClaimSettlement::UnknownRequiresReconciliation
        );
        let admission = store
            .issue_scheduled_reconciliation_test_admission(
                &claim.task.id,
                &claim.attempt_id,
                ScheduledReconciliationTestResolution::RetrySafe,
            )
            .unwrap();
        assert!(store.reconcile_unknown_attempt(admission).unwrap());

        assert!(store.list_tasks(Some("pending")).unwrap().is_empty());
        let review_required = store.list_tasks(Some("review_required")).unwrap();
        assert_eq!(review_required.len(), 1);
        assert_eq!(
            review_required[0].last_error.as_deref(),
            Some("scheduled_cloud_grant_consumed_requires_fresh_review")
        );
        assert!(store
            .claim_next_due(
                chrono::Utc::now() + chrono::Duration::days(1),
                chrono::Duration::seconds(30)
            )
            .unwrap()
            .is_none());
    }

    #[test]
    fn completed_claim_requires_durable_minimal_provider_receipt_bound_to_policy() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(&store, "task-receipt", "proposal-receipt");
        let result_digest = digest_ref("canonical result");
        let result_ref = "conversation://scheduled/task-receipt/message/1";
        assert!(store
            .complete_claim(&claim, "agent-run-1", result_ref, &result_digest)
            .is_err());

        let started_at = chrono::Utc::now();
        store
            .record_provider_started(
                &claim,
                "provider-request-completed",
                "ollama",
                "local-model",
                started_at,
                &scheduled_provider_evidence(&claim),
            )
            .unwrap();
        store
            .record_provider_terminal(
                &claim,
                &completed_receipt(&claim, "provider-request-completed", started_at),
            )
            .unwrap();
        store
            .stage_claim_result_delivery(&claim, result_ref, &result_digest)
            .unwrap();
        assert!(store
            .complete_claim(&claim, "agent-run-1", result_ref, &result_digest)
            .unwrap());

        let receipts = store
            .provider_receipts_for_attempt(&claim.attempt_id)
            .unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].status, "completed");
        assert_eq!(receipts[0].provider_grant_id, claim.provider_grant.grant_id);
        assert_eq!(receipts[0].provider_digest, digest_ref("ollama"));
        assert_eq!(receipts[0].model_digest, digest_ref("local-model"));
        assert!(!receipts[0].provider_digest.contains("ollama"));
        assert!(!receipts[0].model_digest.contains("local-model"));
    }

    #[test]
    fn scheduled_provider_terminal_survives_wall_clock_rollback() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(
            &store,
            "task-provider-clock-rollback",
            "proposal-provider-clock-rollback",
        );
        let started_at = chrono::Utc::now();
        store
            .record_provider_started(
                &claim,
                "provider-request-clock-rollback",
                "ollama",
                "local-model",
                started_at,
                &scheduled_provider_evidence(&claim),
            )
            .expect("record exact scheduled provider start");
        let mut terminal = completed_receipt(&claim, "provider-request-clock-rollback", started_at);
        terminal.finished_at = started_at - chrono::Duration::milliseconds(1);

        assert!(store
            .record_provider_terminal(&claim, &terminal)
            .expect("typed scheduled terminal order survives wall-clock rollback"));
        assert!(store.record_provider_terminal(&claim, &terminal).is_err());
        let alternate_terminal =
            failed_receipt(&claim, "provider-request-clock-rollback", started_at);
        assert!(store
            .record_provider_terminal(&claim, &alternate_terminal)
            .is_err());
        let receipt = store
            .provider_receipts_for_attempt(&claim.attempt_id)
            .unwrap()
            .remove(0);
        assert_eq!(receipt.status, "completed");
        assert_eq!(receipt.error_digest, None);
        assert_eq!(receipt.simulated, Some(false));
        assert_eq!(receipt.policy_evidence_state, "exact");
    }

    #[test]
    fn provider_terminal_cannot_replace_the_exact_start_policy_evidence() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(&store, "task-evidence-match", "proposal-evidence-match");
        let started_at = chrono::Utc::now();
        let start_evidence = scheduled_provider_evidence(&claim);
        store
            .record_provider_started(
                &claim,
                "provider-request-evidence-match",
                "ollama",
                "local-model",
                started_at,
                &start_evidence,
            )
            .unwrap();

        let mut terminal = completed_receipt(&claim, "provider-request-evidence-match", started_at);
        terminal
            .policy_evidence
            .as_mut()
            .unwrap()
            .network_policy_decision_digest = digest_ref("different network decision");
        assert!(store.record_provider_terminal(&claim, &terminal).is_err());

        let receipt = store
            .provider_receipts_for_attempt(&claim.attempt_id)
            .unwrap()
            .remove(0);
        assert_eq!(receipt.status, "started");
        assert_eq!(receipt.policy_evidence_state, "exact");
        assert_eq!(
            receipt.policy_evidence_digest,
            Some(start_evidence.evidence_digest().unwrap())
        );
    }

    #[test]
    fn provider_truth_admission_rejects_stale_task_revision_and_attempt_grant_rows() {
        let task_store = TaskStore::new_in_memory().unwrap();
        let task_claim = claim_and_begin(
            &task_store,
            "task-stale-provider-revision",
            "proposal-stale-provider-revision",
        );
        let task_progress = crate::scheduler::ProviderInvocationProgress::Started {
            request_id: "provider-stale-task-revision".into(),
            provider: "ollama".into(),
            model: "local-model".into(),
            started_at: chrono::Utc::now(),
            policy_evidence: scheduled_provider_evidence(&task_claim),
        };
        let task_admission = crate::scheduler::issue_scheduled_provider_truth_test_admission(
            &task_claim,
            &task_progress,
        )
        .unwrap();
        task_store
            .lock_connection()
            .unwrap()
            .execute(
                "UPDATE tasks SET description = 'changed after provider scope binding' WHERE id = ?1",
                params![task_claim.task.id],
            )
            .unwrap();
        let error = task_store
            .record_provider_truth(&task_claim, task_admission)
            .unwrap_err()
            .to_string();
        assert!(error.contains("canonical task revision"), "{error}");
        assert!(task_store
            .provider_receipts_for_attempt(&task_claim.attempt_id)
            .unwrap()
            .is_empty());

        let attempt_store = TaskStore::new_in_memory().unwrap();
        let attempt_claim = claim_and_begin(
            &attempt_store,
            "task-stale-attempt-grant",
            "proposal-stale-attempt-grant",
        );
        let attempt_progress = crate::scheduler::ProviderInvocationProgress::Started {
            request_id: "provider-stale-attempt-grant".into(),
            provider: "ollama".into(),
            model: "local-model".into(),
            started_at: chrono::Utc::now(),
            policy_evidence: scheduled_provider_evidence(&attempt_claim),
        };
        let attempt_admission = crate::scheduler::issue_scheduled_provider_truth_test_admission(
            &attempt_claim,
            &attempt_progress,
        )
        .unwrap();
        attempt_store
            .lock_connection()
            .unwrap()
            .execute(
                "UPDATE scheduler_attempts SET policy_reason_code = 'mutated_reason' WHERE attempt_id = ?1",
                params![attempt_claim.attempt_id],
            )
            .unwrap();
        let error = attempt_store
            .record_provider_truth(&attempt_claim, attempt_admission)
            .unwrap_err()
            .to_string();
        assert!(error.contains("canonical attempt/grant"), "{error}");
        assert!(attempt_store
            .provider_receipts_for_attempt(&attempt_claim.attempt_id)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn provider_terminal_cas_binds_the_exact_prepared_request_digest() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(
            &store,
            "task-prepared-request-cas",
            "proposal-prepared-request-cas",
        );
        let started_at = chrono::Utc::now();
        store
            .record_provider_started(
                &claim,
                "provider-prepared-request-cas",
                "ollama",
                "local-model",
                started_at,
                &scheduled_provider_evidence(&claim),
            )
            .unwrap();
        let original_digest = store
            .provider_receipts_for_attempt(&claim.attempt_id)
            .unwrap()
            .remove(0)
            .prepared_request_digest
            .expect("new provider truth must persist its prepared request digest");
        store
            .lock_connection()
            .unwrap()
            .execute(
                "UPDATE scheduler_provider_receipts SET prepared_request_digest = ?2 WHERE request_id = ?1",
                params![
                    "provider-prepared-request-cas",
                    digest_ref("another prepared request")
                ],
            )
            .unwrap();

        let terminal = completed_receipt(&claim, "provider-prepared-request-cas", started_at);
        assert!(store.record_provider_terminal(&claim, &terminal).is_err());
        let row = store
            .provider_receipts_for_attempt(&claim.attempt_id)
            .unwrap()
            .remove(0);
        assert_eq!(row.status, "started");
        assert_ne!(
            row.prepared_request_digest.as_deref(),
            Some(original_digest.as_str())
        );
    }

    #[test]
    fn provider_start_evidence_cannot_replay_across_attempts_of_the_same_task() {
        let store = TaskStore::new_in_memory().unwrap();
        let first = claim_and_begin(&store, "task-attempt-scope", "proposal-attempt-scope");
        let first_evidence = scheduled_provider_evidence(&first);
        assert_eq!(
            store
                .settle_claim_after_error(
                    &first,
                    "pre_dispatch_test_failure",
                    Some(&digest_ref("pre dispatch")),
                )
                .unwrap(),
            ScheduledClaimSettlement::ReclaimedBeforeDispatch
        );
        let second = store
            .claim_next_due(
                chrono::Utc::now() + chrono::Duration::minutes(10),
                chrono::Duration::seconds(30),
            )
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&second).unwrap();

        assert!(store
            .record_provider_started(
                &second,
                "provider-request-replayed-attempt",
                "ollama",
                "local-model",
                chrono::Utc::now(),
                &first_evidence,
            )
            .is_err());
        assert!(store
            .provider_receipts_for_attempt(&second.attempt_id)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn local_only_policy_rejects_cloud_receipt_before_it_can_be_canonical_truth() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(&store, "task-policy", "proposal-policy");
        assert!(store
            .record_provider_started(
                &claim,
                "cloud-request",
                "openai",
                "gpt-test",
                chrono::Utc::now(),
                &scheduled_provider_evidence(&claim),
            )
            .is_err());
        assert!(store
            .provider_receipts_for_attempt(&claim.attempt_id)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn tool_dispatch_timeout_is_unknown_and_needs_explicit_reconciliation() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(&store, "task-tool", "proposal-tool");
        let (attempt, started, _) =
            observed_tool_receipt_fixture("manifest-1", "agent-run-tool", "tool request");
        store
            .record_tool_dispatch_started(&claim, &attempt, &started)
            .unwrap();
        assert_eq!(
            store.settle_claim_after_timeout(&claim).unwrap(),
            ScheduledClaimSettlement::UnknownRequiresReconciliation
        );
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_none());
    }

    #[test]
    fn typed_tool_terminal_receipt_resolves_dispatch_without_guessing() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(&store, "task-tool-returned", "proposal-tool-returned");
        let (attempt, started, terminal) =
            observed_tool_receipt_fixture("manifest-1", "agent-run-tool", "tool request");
        store
            .record_tool_dispatch_started(&claim, &attempt, &started)
            .unwrap();
        let (dispatch_process_epoch, dispatch_writer_generation): (String, String) = store
            .lock_connection()
            .unwrap()
            .query_row(
                "SELECT process_epoch_id, writer_owner_generation_id
                 FROM scheduler_tool_dispatches WHERE tool_receipt_id = ?1",
                [started.receipt_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            dispatch_process_epoch,
            claim
                .policy_authority_proof
                .store
                .process_epoch_id
                .to_string()
        );
        assert_eq!(
            dispatch_writer_generation,
            claim
                .policy_authority_proof
                .store
                .writer_owner_generation_id
                .to_string()
        );
        assert!(store.record_tool_terminal(&claim, &terminal).unwrap());
        let provider_started_at = chrono::Utc::now();
        store
            .record_provider_started(
                &claim,
                "provider-after-tool",
                "ollama",
                "local-model",
                provider_started_at,
                &scheduled_provider_evidence(&claim),
            )
            .unwrap();
        store
            .record_provider_terminal(
                &claim,
                &completed_receipt(&claim, "provider-after-tool", provider_started_at),
            )
            .unwrap();
        let result_ref = "conversation://scheduled/task-tool-returned/message/1";
        let result_digest = digest_ref("completed after known tool response");
        store
            .stage_claim_result_delivery(&claim, result_ref, &result_digest)
            .unwrap();
        assert!(store
            .complete_claim(&claim, "agent-run-tool", result_ref, &result_digest,)
            .unwrap());
    }

    #[test]
    fn same_manifest_and_run_terminals_bind_by_receipt_id_even_in_reverse_order() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(
            &store,
            "task-tool-reverse-terminal",
            "proposal-tool-reverse-terminal",
        );
        let (first_attempt, first_started, first_terminal) =
            observed_tool_receipt_fixture("manifest-shared", "agent-run-shared", "first request");
        let (second_attempt, second_started, second_terminal) =
            observed_tool_receipt_fixture("manifest-shared", "agent-run-shared", "second request");
        let first_dispatch_id = store
            .record_tool_dispatch_started(&claim, &first_attempt, &first_started)
            .unwrap();
        assert_eq!(
            store
                .record_tool_dispatch_started(&claim, &first_attempt, &first_started)
                .unwrap(),
            first_dispatch_id,
            "an exact repeated adapter-start callback is idempotent"
        );
        let mut rebound_attempt = first_attempt.clone();
        rebound_attempt.input_hash = digest_ref("rebound input");
        assert!(store
            .record_tool_dispatch_started(&claim, &rebound_attempt, &first_started)
            .is_err());
        store
            .record_tool_dispatch_started(&claim, &second_attempt, &second_started)
            .unwrap();

        let second_terminal: ToolExecutionReceipt =
            serde_json::from_value(serde_json::to_value(&second_terminal).unwrap()).unwrap();
        let first_terminal: ToolExecutionReceipt =
            serde_json::from_value(serde_json::to_value(&first_terminal).unwrap()).unwrap();
        assert!(store
            .record_tool_terminal(&claim, &second_terminal)
            .unwrap());
        assert!(store.record_tool_terminal(&claim, &first_terminal).unwrap());

        let conn = store.lock_connection().unwrap();
        let rows = conn
            .prepare(
                "SELECT tool_receipt_id, request_digest, status
                 FROM scheduler_tool_dispatches
                 WHERE attempt_id = ?1 ORDER BY dispatch_index",
            )
            .unwrap()
            .query_map(params![claim.attempt_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            rows,
            vec![
                (
                    first_terminal.receipt_id,
                    first_terminal.request_digest,
                    "returned".into(),
                ),
                (
                    second_terminal.receipt_id,
                    second_terminal.request_digest,
                    "returned".into(),
                ),
            ]
        );
    }

    #[test]
    fn terminal_dto_field_drift_cannot_mutate_the_exact_started_row() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(
            &store,
            "task-tool-terminal-drift",
            "proposal-tool-terminal-drift",
        );
        let (attempt, started, terminal) =
            observed_tool_receipt_fixture("manifest-drift", "agent-run-drift", "stable request");
        store
            .record_tool_dispatch_started(&claim, &attempt, &started)
            .unwrap();

        let mut receipt_id_drift = terminal.clone();
        receipt_id_drift.receipt_id = uuid::Uuid::new_v4().to_string();
        let mut request_drift = terminal.clone();
        request_drift.request_digest = digest_ref("different request digest");
        let mut manifest_drift = terminal.clone();
        manifest_drift.manifest_id = Some("manifest-other".into());
        let mut run_drift = terminal.clone();
        run_drift.source_run_id = Some("agent-run-other".into());
        let mut idempotency_drift = terminal.clone();
        idempotency_drift.idempotency_contract =
            crate::tool_manifest::ToolIdempotencyContract::NonIdempotent;
        let mut dispatch_kind_drift = terminal.clone();
        dispatch_kind_drift.dispatch_kind = ToolDispatchKind::Network;
        let mut dispatch_count_drift = terminal.clone();
        dispatch_count_drift.dispatch_attempt_count = 2;
        let mut started_at_drift = terminal.clone();
        started_at_drift.started_at -= chrono::Duration::milliseconds(1);
        let mut action_effect_drift = terminal.clone();
        action_effect_drift.action_effect = ToolActionEffect::LocalMutation;
        action_effect_drift.effect_status = ToolEffectStatus::Confirmed;

        for (field, drifted) in [
            ("receipt_id", receipt_id_drift),
            ("request_digest", request_drift),
            ("manifest_id", manifest_drift),
            ("source_run_id", run_drift),
            ("idempotency_contract", idempotency_drift),
            ("dispatch_kind", dispatch_kind_drift),
            ("dispatch_attempt_count", dispatch_count_drift),
            ("started_at", started_at_drift),
            ("action_effect", action_effect_drift),
        ] {
            assert_eq!(
                drifted.mechanically_valid_terminal(),
                Ok(()),
                "{field} counterexample must reach the identity CAS"
            );
            assert!(
                store.record_tool_terminal(&claim, &drifted).is_err(),
                "{field} drift must fail closed"
            );
        }

        let conn = store.lock_connection().unwrap();
        let row: (String, Option<String>, String) = conn
            .query_row(
                "SELECT status, finished_at, transport_status
                 FROM scheduler_tool_dispatches WHERE tool_receipt_id = ?1",
                params![terminal.receipt_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, ("started".into(), None, "dispatched".into()));
    }

    #[test]
    fn mechanically_invalid_terminal_preserves_started_and_unknown_truth() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(
            &store,
            "task-tool-invalid-terminal",
            "proposal-tool-invalid-terminal",
        );
        let (started_attempt, started_receipt, _) = observed_tool_receipt_fixture(
            "manifest-invalid-started",
            "agent-run-invalid",
            "invalid started request",
        );
        store
            .record_tool_dispatch_started(&claim, &started_attempt, &started_receipt)
            .unwrap();
        let mut invalid_started_terminal = started_receipt.clone();
        invalid_started_terminal.finished_at = Some(chrono::Utc::now());
        assert_eq!(
            invalid_started_terminal.mechanically_valid_terminal(),
            Err("tool_receipt_dispatch_has_no_terminal_certainty")
        );
        assert!(store
            .record_tool_terminal(&claim, &invalid_started_terminal)
            .is_err());

        let (unknown_attempt, unknown_started, unknown_terminal) =
            remote_unknown_tool_receipt_fixture(
                "manifest-valid-unknown",
                "agent-run-invalid",
                "unknown request",
            );
        store
            .record_tool_dispatch_started(&claim, &unknown_attempt, &unknown_started)
            .unwrap();
        assert!(!store
            .record_tool_terminal(&claim, &unknown_terminal)
            .unwrap());
        let mut invalid_unknown_replay = unknown_terminal.clone();
        invalid_unknown_replay.finished_at = None;
        assert!(store
            .record_tool_terminal(&claim, &invalid_unknown_replay)
            .is_err());

        let conn = store.lock_connection().unwrap();
        let states = conn
            .prepare(
                "SELECT tool_receipt_id, status FROM scheduler_tool_dispatches
                 WHERE attempt_id = ?1 ORDER BY dispatch_index",
            )
            .unwrap()
            .query_map(params![claim.attempt_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            states,
            vec![
                (started_receipt.receipt_id, "started".into()),
                (unknown_terminal.receipt_id, "unknown".into()),
            ]
        );
    }

    #[test]
    fn v10_tool_dispatch_without_exact_identity_is_quarantined_not_reinterpreted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        {
            let store = TaskStore::new(&path).unwrap();
            let claim = claim_and_begin(
                &store,
                "task-tool-v10-migration",
                "proposal-tool-v10-migration",
            );
            let (attempt, started, terminal) =
                observed_tool_receipt_fixture("manifest-v10", "agent-run-v10", "legacy request");
            store
                .record_tool_dispatch_started(&claim, &attempt, &started)
                .unwrap();
            {
                let conn = store.lock_connection().unwrap();
                conn.execute(
                    "UPDATE scheduler_tool_dispatches
                     SET identity_state = 'legacy_unavailable',
                         manifest_contract_digest = NULL, input_hash = NULL,
                         input_length_bytes = NULL, receipt_started_at = NULL,
                         dispatched_at = NULL, idempotency_contract = NULL,
                         dispatch_kind = NULL, dispatch_attempt_count = NULL
                     WHERE tool_receipt_id = ?1",
                    params![terminal.receipt_id],
                )
                .unwrap();
                conn.execute(
                    "UPDATE openlife_schema_versions SET version = 10
                     WHERE component = 'task_store'",
                    [],
                )
                .unwrap();
            }
        }

        let reopened = TaskStore::new(&path).unwrap();
        assert!(reopened.list_tasks(None).unwrap().is_empty());
        let conn = reopened.lock_connection().unwrap();
        let row: (i64, String, String, i64, String) = conn
            .query_row(
                "SELECT source_schema_version, task_status, attempt_status,
                        tool_dispatch_count, tool_status_digest
                 FROM legacy_task_store_truth_quarantine",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(row.0, 10);
        assert_eq!(row.1, "running");
        assert_eq!(row.2, "executing");
        assert_eq!(row.3, 1);
        assert!(row.4.starts_with("sha256:"));
        assert_eq!(
            task_store_schema_version(&conn).unwrap(),
            Some(TASK_STORE_SCHEMA_VERSION)
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM scheduler_tool_dispatches",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn scheduler_receipt_tables_never_add_prompt_or_response_body_columns() {
        let store = TaskStore::new_in_memory().unwrap();
        let conn = store.lock_connection().unwrap();
        for table in [
            "scheduler_attempts",
            "scheduler_provider_receipts",
            "scheduler_tool_dispatches",
        ] {
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .unwrap();
            let columns = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert!(!columns.iter().any(|column| {
                let column = column.to_ascii_lowercase();
                column.contains("prompt")
                    || column.contains("response")
                    || column.contains("content")
                    || column == "payload"
                    || column.ends_with("_body")
            }));
        }
    }

    #[test]
    fn concurrent_claimers_have_exactly_one_owner() {
        let store = std::sync::Arc::new(TaskStore::new_in_memory().unwrap());
        store
            .create_task_idempotent(&due_task("task-concurrent", "proposal-concurrent"))
            .unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let owners = (0..8)
            .map(|_| {
                let store = std::sync::Arc::clone(&store);
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
                        .unwrap()
                        .is_some()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(|owned| *owned)
            .count();
        assert_eq!(owners, 1);
    }

    #[test]
    fn expired_execution_boundary_is_unknown_until_reconciled() {
        let store = TaskStore::new_in_memory().unwrap();
        store
            .create_task_idempotent(&due_task("task-crash", "proposal-crash"))
            .unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(-1))
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&claim).unwrap();

        assert_eq!(
            store.reconcile_expired_claims(chrono::Utc::now()).unwrap(),
            1
        );
        assert_eq!(store.list_tasks(Some("unknown")).unwrap().len(), 1);
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_none());
    }

    #[test]
    fn expired_process_provider_start_becomes_remote_unknown_not_perpetually_inflight() {
        let store = TaskStore::new_in_memory().unwrap();
        store
            .create_task_idempotent(&due_task(
                "task-crash-provider-start",
                "proposal-crash-provider-start",
            ))
            .unwrap();
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(-1))
            .unwrap()
            .unwrap();
        store.begin_claim_execution(&claim).unwrap();
        store
            .record_provider_started(
                &claim,
                "provider-request-crash",
                "ollama",
                "local-model",
                chrono::Utc::now(),
                &scheduled_provider_evidence(&claim),
            )
            .unwrap();

        assert_eq!(
            store.reconcile_expired_claims(chrono::Utc::now()).unwrap(),
            1
        );
        let receipt = store
            .provider_receipts_for_attempt(&claim.attempt_id)
            .unwrap()
            .remove(0);
        assert_eq!(receipt.status, "remote_unknown");
        assert!(receipt.error_digest.is_some());
        assert!(store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .is_none());
    }

    #[test]
    fn previous_process_provider_start_is_unknown_immediately_even_if_clock_moves_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task-store-process-epoch.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x63; 32]).unwrap();
        let started_at = chrono::Utc::now();
        let attempt_id = {
            let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
            store
                .create_task_idempotent(&due_task("task-process-epoch", "proposal-process-epoch"))
                .unwrap();
            let claim = store
                .claim_next_due(started_at, chrono::Duration::days(7))
                .unwrap()
                .unwrap();
            store.begin_claim_execution(&claim).unwrap();
            store
                .record_provider_started(
                    &claim,
                    "provider-request-process-epoch",
                    "ollama",
                    "local-model",
                    started_at,
                    &scheduled_provider_evidence(&claim),
                )
                .unwrap();
            claim.attempt_id().to_string()
        };

        let restarted = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        let rolled_back_clock = started_at - chrono::Duration::days(30);
        assert_eq!(
            restarted
                .reconcile_previous_process_claims(rolled_back_clock)
                .unwrap(),
            1
        );
        let receipt = restarted
            .provider_receipts_for_attempt(&attempt_id)
            .unwrap()
            .remove(0);
        assert_eq!(receipt.status, "remote_unknown");
        assert_eq!(restarted.list_tasks(Some("unknown")).unwrap().len(), 1);
    }

    #[test]
    fn observed_provider_failure_is_failed_not_unknown() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(&store, "task-provider-failed", "proposal-provider-failed");
        let started_at = chrono::Utc::now();
        store
            .record_provider_started(
                &claim,
                "provider-request-failed",
                "ollama",
                "local-model",
                started_at,
                &scheduled_provider_evidence(&claim),
            )
            .unwrap();
        store
            .record_provider_terminal(
                &claim,
                &failed_receipt(&claim, "provider-request-failed", started_at),
            )
            .unwrap();

        assert_eq!(
            store
                .settle_claim_after_error(
                    &claim,
                    "provider_terminal_failure",
                    Some(&digest_ref("known provider failure")),
                )
                .unwrap(),
            ScheduledClaimSettlement::FailedAfterObservedTerminal
        );
        assert_eq!(store.list_tasks(Some("failed")).unwrap().len(), 1);
        assert!(store.list_tasks(Some("unknown")).unwrap().is_empty());
    }

    #[test]
    fn simulated_provider_receipt_cannot_complete_a_product_task() {
        let store = TaskStore::new_in_memory().unwrap();
        let claim = claim_and_begin(&store, "task-simulated", "proposal-simulated");
        let started_at = chrono::Utc::now();
        store
            .record_provider_started(
                &claim,
                "provider-request-simulated",
                "ollama",
                "local-model",
                started_at,
                &scheduled_provider_evidence(&claim),
            )
            .unwrap();
        let mut receipt = completed_receipt(&claim, "provider-request-simulated", started_at);
        receipt.simulated = true;
        assert!(store.record_provider_terminal(&claim, &receipt).is_err());
        let result_ref = "conversation://scheduled/task-simulated/message/1";
        let result_digest = digest_ref("fixture response");
        store
            .stage_claim_result_delivery(&claim, result_ref, &result_digest)
            .unwrap();

        assert!(store
            .complete_claim(&claim, "agent-run-simulated", result_ref, &result_digest)
            .is_err());
    }

    #[test]
    fn v11_provider_rows_without_prepared_request_binding_are_quarantined() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks-v11-provider.db");
        let attempt_id = {
            let store = TaskStore::new(&path).unwrap();
            let claim = claim_and_begin(
                &store,
                "task-provider-v11-migration",
                "proposal-provider-v11-migration",
            );
            store
                .record_provider_started(
                    &claim,
                    "provider-v11-unbound-request",
                    "ollama",
                    "local-model",
                    chrono::Utc::now(),
                    &scheduled_provider_evidence(&claim),
                )
                .unwrap();
            {
                let conn = store.lock_connection().unwrap();
                let legacy_grant = ScheduledProviderGrantV2::deterministic_local_only_v11(
                    &claim.task.id,
                    &claim.task.description,
                    &claim.task.action_type,
                    claim.task.due_date.as_deref(),
                    claim.task.source_run_id.as_deref(),
                    claim.task.source_proposal_id.as_deref(),
                );
                conn.execute(
                    "UPDATE tasks
                     SET provider_grant_id = ?2, provider_target_digest = ?3
                     WHERE id = ?1",
                    params![
                        claim.task.id,
                        legacy_grant.grant_id,
                        legacy_grant.provider_digest,
                    ],
                )
                .unwrap();
                conn.execute(
                    "UPDATE scheduler_attempts SET provider_grant_id = ?2
                     WHERE attempt_id = ?1",
                    params![claim.attempt_id, legacy_grant.grant_id],
                )
                .unwrap();
                conn.execute(
                    "UPDATE scheduler_provider_receipts
                     SET provider_grant_id = ?1, prepared_request_digest = NULL
                     WHERE request_id = 'provider-v11-unbound-request'",
                    params![legacy_grant.grant_id],
                )
                .unwrap();
                conn.execute(
                    "UPDATE openlife_schema_versions SET version = 11
                     WHERE component = 'task_store'",
                    [],
                )
                .unwrap();
            }
            claim.attempt_id
        };

        let reopened = TaskStore::new(&path).unwrap();
        assert!(reopened.list_tasks(None).unwrap().is_empty());
        assert!(reopened
            .provider_receipts_for_attempt(&attempt_id)
            .unwrap()
            .is_empty());
        let conn = reopened.lock_connection().unwrap();
        let quarantine: (i64, String, String, i64, String) = conn
            .query_row(
                "SELECT source_schema_version, task_status, attempt_status,
                        provider_receipt_count, provider_status_digest
                 FROM legacy_task_store_truth_quarantine",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(quarantine.0, 11);
        assert_eq!(quarantine.1, "running");
        assert_eq!(quarantine.2, "executing");
        assert_eq!(quarantine.3, 1);
        assert!(quarantine.4.starts_with("sha256:"));
        assert_eq!(
            task_store_schema_version(&conn).unwrap(),
            Some(TASK_STORE_SCHEMA_VERSION)
        );
    }

    #[test]
    fn v11_noncanonical_provider_rebinding_is_quarantined_without_reexecution() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks-v11-provider-rebound.db");
        {
            let store = TaskStore::new(&path).unwrap();
            let task = due_task("task-provider-v11-rebound", "proposal-provider-v11-rebound");
            store.create_task_idempotent(&task).unwrap();
            let rebound = ScheduledProviderGrantV2::deterministic_local_only_with_provider_digest(
                &task.id,
                &task.description,
                &task.action_type,
                task.due_date.as_deref(),
                task.source_run_id.as_deref(),
                task.source_proposal_id.as_deref(),
                digest_ref("caller-selected-provider"),
            );
            let conn = store.lock_connection().unwrap();
            conn.execute(
                "UPDATE tasks
                 SET provider_grant_id = ?2, provider_target_digest = ?3
                 WHERE id = ?1",
                params![task.id, rebound.grant_id, rebound.provider_digest],
            )
            .unwrap();
            conn.execute(
                "UPDATE openlife_schema_versions SET version = 11
                 WHERE component = 'task_store'",
                [],
            )
            .unwrap();
        }

        let reopened = TaskStore::new(&path).unwrap();
        assert!(reopened.list_tasks(None).unwrap().is_empty());
        assert!(reopened
            .claim_next_due(
                chrono::Utc::now() + chrono::Duration::days(1),
                chrono::Duration::seconds(30),
            )
            .unwrap()
            .is_none());
        let conn = reopened.lock_connection().unwrap();
        let quarantine: (i64, String, Option<String>, i64) = conn
            .query_row(
                "SELECT source_schema_version, task_status, attempt_status,
                        provider_receipt_count
                 FROM legacy_task_store_truth_quarantine",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(quarantine, (11, "pending".into(), None, 0));
    }

    #[test]
    fn due_claim_query_is_backed_by_the_composite_due_index() {
        let store = TaskStore::new_in_memory().unwrap();
        let conn = store.lock_connection().unwrap();
        let mut stmt = conn
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT id FROM tasks
                 WHERE status = 'pending' AND eligible_at IS NOT NULL AND eligible_at != ''
                   AND eligible_at <= ?1
                 ORDER BY eligible_at ASC, created_at ASC, id ASC LIMIT 1",
            )
            .unwrap();
        let details = stmt
            .query_map(params![chrono::Utc::now().to_rfc3339()], |row| {
                row.get::<_, String>(3)
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(details
            .iter()
            .any(|detail| detail.contains("idx_tasks_due_claim")));
    }

    #[test]
    fn future_legacy_pending_is_preserved_as_review_required_but_never_claimable() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        let due_at = (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339();
        let raw_prompt = "future capability must survive behind fresh review";
        std::fs::write(
            &legacy_path,
            serde_json::to_vec(&serde_json::json!([{
                "id": "legacy-future",
                "title": "Future legacy task",
                "prompt": raw_prompt,
                "scheduled_at": due_at,
                "status": "pending",
                "created_at": chrono::Utc::now().to_rfc3339(),
                "source_proposal_id": "legacy-proposal-future",
                "action_type": "scheduled_task"
            }]))
            .unwrap(),
        )
        .unwrap();
        let store = TaskStore::new(directory.path().join("tasks.db")).unwrap();

        let report = store.migrate_legacy_json_if_present(&legacy_path).unwrap();

        assert_eq!(report.item_count, 1);
        assert_eq!(report.review_required_count, 1);
        assert_eq!(report.historical_count, 0);
        assert_eq!(report.quarantined_count, 0);
        assert!(!legacy_path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(report.evidence_path.as_ref().unwrap())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0);
        }
        assert!(store.list_tasks(None).unwrap().is_empty());
        assert!(store
            .claim_next_due(
                chrono::Utc::now() + chrono::Duration::days(30),
                chrono::Duration::seconds(30),
            )
            .unwrap()
            .is_none());

        let pending = store
            .pending_legacy_review_candidates(directory.path())
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].title, "Future legacy task");
        assert_eq!(pending[0].description, raw_prompt);
        assert!(store
            .mark_legacy_review_proposal_staged(&pending[0], "legacy-review-proposal")
            .unwrap());
        assert!(store
            .pending_legacy_review_candidates(directory.path())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn due_pending_and_running_legacy_rows_are_unknown_crash_counterexamples() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        let due_at = (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
        std::fs::write(
            &legacy_path,
            serde_json::to_vec(&serde_json::json!([
                {
                    "id": "legacy-due-pending",
                    "title": "May have dispatched before old file write",
                    "prompt": "ambiguous pending",
                    "scheduled_at": due_at,
                    "status": "pending"
                },
                {
                    "id": "legacy-running",
                    "title": "Old running",
                    "prompt": "ambiguous running",
                    "scheduled_at": due_at,
                    "status": "running"
                }
            ]))
            .unwrap(),
        )
        .unwrap();
        let store = TaskStore::new(directory.path().join("tasks.db")).unwrap();

        let report = store.migrate_legacy_json_if_present(&legacy_path).unwrap();

        assert_eq!(report.quarantined_count, 2);
        assert_eq!(report.review_required_count, 0);
        assert!(store
            .pending_legacy_review_candidates(directory.path())
            .unwrap()
            .is_empty());
        let conn = store.lock_connection().unwrap();
        let reasons = conn
            .prepare(
                "SELECT reason_code FROM legacy_scheduled_task_migration_records
                 ORDER BY source_ordinal",
            )
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            reasons,
            vec![
                "legacy_due_pending_dispatch_state_unknown",
                "legacy_running_dispatch_state_unknown",
            ]
        );
    }

    #[test]
    fn legacy_terminal_labels_are_metadata_only_history_not_completion_receipts() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        let raw_terminal = "unproven terminal prose must not enter metadata";
        std::fs::write(
            &legacy_path,
            serde_json::to_vec(&serde_json::json!([
                {"id":"legacy-completed", "status":"completed", "result_preview":raw_terminal},
                {"id":"legacy-failed", "status":"failed", "error":raw_terminal}
            ]))
            .unwrap(),
        )
        .unwrap();
        let store = TaskStore::new(directory.path().join("tasks.db")).unwrap();

        let report = store.migrate_legacy_json_if_present(&legacy_path).unwrap();

        assert_eq!(report.historical_count, 2);
        assert_eq!(report.quarantined_count, 0);
        assert!(store.list_tasks(Some("completed")).unwrap().is_empty());
        let conn = store.lock_connection().unwrap();
        let metadata: String = conn
            .query_row(
                "SELECT GROUP_CONCAT(
                    source_digest || ':' || item_digest || ':' ||
                    COALESCE(legacy_task_id_digest, '') || ':' || legacy_status || ':' ||
                    reason_code || ':' || effect_state || ':' ||
                    COALESCE(terminal_detail_digest, '')
                 ) FROM legacy_scheduled_task_migration_records",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!metadata.contains(raw_terminal));
        assert!(!metadata.contains("legacy-completed"));
        assert!(metadata.contains("reported_completed"));
        assert!(metadata.contains("reported_failed"));
    }

    #[test]
    fn legacy_json_quarantine_is_idempotent_after_commit_before_source_retirement_retry() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        let bytes = serde_json::to_vec(&serde_json::json!([{
            "id": "legacy-idempotent",
            "title": "Legacy idempotent",
            "prompt": "do not duplicate",
            "scheduled_at": (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339(),
            "status": "pending",
            "created_at": chrono::Utc::now().to_rfc3339()
        }]))
        .unwrap();
        std::fs::write(&legacy_path, &bytes).unwrap();
        let store = TaskStore::new(directory.path().join("tasks.db")).unwrap();

        let first = store.migrate_legacy_json_if_present(&legacy_path).unwrap();
        std::fs::write(&legacy_path, &bytes).unwrap();
        let second = store.migrate_legacy_json_if_present(&legacy_path).unwrap();

        assert_eq!(first.source_digest, second.source_digest);
        assert_eq!(first.evidence_path, second.evidence_path);
        assert!(!legacy_path.exists());
        let conn = store.lock_connection().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM legacy_scheduled_task_sources",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM legacy_scheduled_task_migration_records",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn malformed_legacy_json_is_retained_as_unknown_source_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        std::fs::write(&legacy_path, b"{not-an-array-and-not-valid-json").unwrap();
        let store = TaskStore::new(directory.path().join("tasks.db")).unwrap();

        let report = store.migrate_legacy_json_if_present(&legacy_path).unwrap();

        assert!(report.source_malformed);
        assert_eq!(report.item_count, 0);
        assert_eq!(report.quarantined_count, 1);
        assert!(!legacy_path.exists());
        assert!(report.evidence_path.unwrap().exists());
        assert!(store.list_tasks(None).unwrap().is_empty());
        let conn = store.lock_connection().unwrap();
        let reason: String = conn
            .query_row(
                "SELECT source_reason_code FROM legacy_scheduled_task_sources",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reason, "legacy_source_malformed_unknown");
    }

    #[cfg(unix)]
    #[test]
    fn legacy_json_symlink_is_rejected_without_following_or_retiring_target() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("outside.json");
        let legacy_path = directory.path().join("scheduled_tasks.json");
        std::fs::write(&target, b"[]").unwrap();
        symlink(&target, &legacy_path).unwrap();
        let store = TaskStore::new(directory.path().join("tasks.db")).unwrap();

        assert!(store.migrate_legacy_json_if_present(&legacy_path).is_err());
        assert!(legacy_path.exists());
        assert_eq!(std::fs::read(&target).unwrap(), b"[]");
    }

    #[cfg(unix)]
    #[test]
    fn legacy_json_fifo_is_rejected_by_nonblocking_fd_validation() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        let path = CString::new(legacy_path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let store = TaskStore::new(directory.path().join("tasks.db")).unwrap();

        let error = store
            .migrate_legacy_json_if_present(&legacy_path)
            .unwrap_err()
            .to_string();
        assert!(error.contains("regular file"), "{error}");
        assert!(legacy_path.exists());
    }

    #[test]
    fn legacy_retirement_rejects_path_replacement_after_opened_fd_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        let displaced_path = directory.path().join("displaced.json");
        let evidence_path = directory.path().join("evidence.json");
        std::fs::write(&legacy_path, b"[]").unwrap();
        let opened = OpenedLegacyScheduledTaskSource::open(&legacy_path).unwrap();
        std::fs::rename(&legacy_path, &displaced_path).unwrap();
        std::fs::write(&legacy_path, b"[{\"status\":\"running\"}]").unwrap();

        let error = retire_legacy_source_file(&legacy_path, &evidence_path, &opened)
            .unwrap_err()
            .to_string();

        assert!(error.contains("no longer names the opened source file"));
        assert_eq!(
            std::fs::read(&legacy_path).unwrap(),
            b"[{\"status\":\"running\"}]"
        );
        assert_eq!(std::fs::read(&displaced_path).unwrap(), b"[]");
        assert!(!evidence_path.exists());
    }

    #[test]
    fn oversized_legacy_source_reads_only_the_bounded_fingerprint_prefix() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        let file = File::create(&legacy_path).unwrap();
        file.set_len(MAX_LEGACY_SCHEDULED_TASK_BYTES + 1024 * 1024)
            .unwrap();
        drop(file);

        let mut opened = OpenedLegacyScheduledTaskSource::open(&legacy_path).unwrap();

        assert!(opened.bytes.is_none());
        assert_eq!(
            opened.byte_len,
            MAX_LEGACY_SCHEDULED_TASK_BYTES + 1024 * 1024
        );
        assert_eq!(
            std::io::Seek::stream_position(&mut opened.file).unwrap(),
            MAX_OVERSIZED_LEGACY_FINGERPRINT_BYTES
        );
    }

    #[test]
    fn copied_task_store_cannot_rebind_pending_executing_or_terminal_truth_to_another_path() {
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x91; 32]).unwrap();
        for state in ["pending", "executing", "completed"] {
            let directory = tempfile::tempdir().unwrap();
            let source_path = directory.path().join(format!("source-{state}.db"));
            let copied_path = directory.path().join(format!("copy-{state}.db"));
            {
                let store =
                    TaskStore::new_with_authority_key(&source_path, &authority_key).unwrap();
                let task_id = format!("copy-transplant-{state}");
                store
                    .create_task_idempotent(&due_task(&task_id, "proposal-copy-transplant"))
                    .unwrap();
                if state == "executing" {
                    let claim = store
                        .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
                        .unwrap()
                        .unwrap();
                    assert!(store.begin_claim_execution(&claim).unwrap());
                } else if state == "completed" {
                    let claim = store
                        .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
                        .unwrap()
                        .unwrap();
                    assert!(store.begin_claim_execution(&claim).unwrap());
                    let started_at = chrono::Utc::now();
                    store
                        .record_provider_started(
                            &claim,
                            "copy-terminal-provider-request",
                            "ollama",
                            "local-model",
                            started_at,
                            &scheduled_provider_evidence(&claim),
                        )
                        .unwrap();
                    store
                        .record_provider_terminal(
                            &claim,
                            &completed_receipt(
                                &claim,
                                "copy-terminal-provider-request",
                                started_at,
                            ),
                        )
                        .unwrap();
                    let result_ref = format!(
                        "conversation://copy-transplant/message/{}",
                        claim.attempt_number
                    );
                    let result_digest = digest_ref("copy-transplant-terminal-result");
                    store
                        .stage_claim_result_delivery(&claim, &result_ref, &result_digest)
                        .unwrap();
                    assert!(store
                        .complete_claim(
                            &claim,
                            "copy-transplant-agent-run",
                            &result_ref,
                            &result_digest,
                        )
                        .unwrap());
                }
                store
                    .lock_connection()
                    .unwrap()
                    .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                    .unwrap();
            }

            // The same canonical path survives a normal process-style reopen.
            drop(TaskStore::new_with_authority_key(&source_path, &authority_key).unwrap());
            std::fs::copy(&source_path, &copied_path).unwrap();

            let copied = TaskStore::new_with_authority_key(&copied_path, &authority_key);
            assert!(
                copied.is_err(),
                "a {state} task-store database copy rebound to a new canonical path"
            );
            assert!(
                TaskStore::open_read_only_existing_with_authority_key(
                    &copied_path,
                    &authority_key,
                )
                .is_err(),
                "a {state} task-store copy became read-only canonical truth at another path"
            );
        }
    }

    #[test]
    fn same_path_live_writable_owner_is_rejected_before_reconciliation_can_contaminate_a_claim() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("same-slot.db");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x37; 32]).unwrap();
        let wrong_key = TaskStoreAuthorityKey::from_key_material(&[0x38; 32]).unwrap();
        let first = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        first
            .create_task_idempotent(&due_task("same-slot-claim", "proposal-same-slot-claim"))
            .unwrap();
        let claim = first
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        let first_store_id = first
            .runtime_authority()
            .unwrap()
            .canonical_store_identity
            .to_string();

        let second_error = match TaskStore::new_with_authority_key(&path, &authority_key) {
            Ok(second) => {
                let reconciled = second
                    .reconcile_previous_process_claims(chrono::Utc::now())
                    .unwrap();
                let contaminated = first
                    .list_tasks(None)
                    .unwrap()
                    .into_iter()
                    .find(|task| task.id == claim.task.id)
                    .unwrap();
                panic!(
                    "a second live writable owner reconciled the first owner's active claim: \
                     reconciled={reconciled}, status={}",
                    contaminated.status
                );
            }
            Err(error) => error.to_string(),
        };
        assert!(
            second_error.contains("scheduled_task_store_sqlite_slot_owner_lease_unavailable"),
            "{second_error}"
        );

        let read_only =
            TaskStore::open_read_only_existing_with_authority_key(&path, &authority_key).unwrap();
        assert_eq!(read_only.list_tasks(Some("running")).unwrap().len(), 1);
        let read_only_claim_error = read_only
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .expect_err("a read-only TaskStore must reject claim authority before SQLite mutation")
            .to_string();
        assert_eq!(
            read_only_claim_error,
            "scheduled_task_store_write_authority_required:claim_next_due"
        );
        let read_only_reconcile_error = read_only
            .reconcile_previous_process_claims(chrono::Utc::now())
            .expect_err(
                "a read-only TaskStore must reject reconciliation authority before SQLite mutation",
            )
            .to_string();
        assert_eq!(
            read_only_reconcile_error,
            "scheduled_task_store_write_authority_required:reconcile_previous_process_claims"
        );
        assert!(first.begin_claim_execution(&claim).unwrap());

        drop(read_only);
        drop(first);
        let reopened = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        assert_eq!(
            reopened
                .runtime_authority()
                .unwrap()
                .canonical_store_identity
                .as_ref(),
            first_store_id
        );
        drop(reopened);
        assert!(TaskStore::new_with_authority_key(&path, &wrong_key).is_err());
    }

    #[test]
    fn read_only_store_rejects_mutation_before_sqlite_filesystem_or_capability_side_effect() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        fn error_or_success<T>(result: Result<T>) -> String {
            match result {
                Ok(_) => "unexpected_success".to_string(),
                Err(error) => error.to_string(),
            }
        }

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("read-only-mutation-gate.sqlite");
        let missing_legacy = directory.path().join("scheduled_tasks.json");
        let legacy_with_review = directory.path().join("legacy_with_review.json");
        std::fs::write(
            &legacy_with_review,
            serde_json::to_vec(&serde_json::json!([{
                "id": "read-only-legacy-review",
                "title": "Read-only legacy review",
                "prompt": "must not chmod through an observation handle",
                "scheduled_at": (chrono::Utc::now() + chrono::Duration::days(2)).to_rfc3339(),
                "status": "pending"
            }]))
            .unwrap(),
        )
        .unwrap();
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x7a; 32]).unwrap();
        let (
            claim,
            reconciliation_admission,
            tool_attempt,
            tool_started,
            tool_terminal,
            legacy_evidence_path,
        ) = {
            let writable = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
            writable
                .create_task_idempotent(&due_task(
                    "read-only-lifecycle",
                    "read-only-lifecycle-proposal",
                ))
                .unwrap();
            let claim = writable
                .claim_next_due(chrono::Utc::now(), chrono::Duration::minutes(5))
                .unwrap()
                .unwrap();
            assert!(writable.begin_claim_execution(&claim).unwrap());
            let (tool_attempt, tool_started, tool_terminal) = remote_unknown_tool_receipt_fixture(
                "read-only-tool-manifest",
                "read-only-tool-run",
                "read-only-tool-input",
            );
            writable
                .record_tool_dispatch_started(&claim, &tool_attempt, &tool_started)
                .unwrap();
            writable
                .record_tool_terminal(&claim, &tool_terminal)
                .unwrap();
            assert_eq!(
                writable.settle_claim_after_timeout(&claim).unwrap(),
                ScheduledClaimSettlement::UnknownRequiresReconciliation
            );
            let reconciliation_admission = writable
                .issue_scheduled_reconciliation_test_admission(
                    &claim.task.id,
                    &claim.attempt_id,
                    ScheduledReconciliationTestResolution::RetrySafe,
                )
                .unwrap();
            let legacy_report = writable
                .migrate_legacy_json_if_present(&legacy_with_review)
                .unwrap();
            assert_eq!(legacy_report.review_required_count, 1);
            let legacy_evidence_path = legacy_report.evidence_path.unwrap();
            writable
                .lock_connection()
                .unwrap()
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                .unwrap();
            (
                claim,
                reconciliation_admission,
                tool_attempt,
                tool_started,
                tool_terminal,
                legacy_evidence_path,
            )
        };
        #[cfg(unix)]
        std::fs::set_permissions(
            &legacy_evidence_path,
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let read_only =
            TaskStore::open_read_only_existing_with_authority_key(&path, &authority_key).unwrap();
        let before_bytes = std::fs::read(&path).unwrap();
        let before_metadata = std::fs::metadata(&path).unwrap();
        let before_evidence_bytes = std::fs::read(&legacy_evidence_path).unwrap();
        let before_evidence_metadata = std::fs::metadata(&legacy_evidence_path).unwrap();
        let candidate = LegacyScheduledTaskReviewCandidate {
            source_digest: digest_ref("read-only-legacy-source"),
            source_ordinal: 0,
            item_digest: digest_ref("read-only-legacy-item"),
            title: "read-only".into(),
            description: "read-only".into(),
            due_at: chrono::Utc::now().to_rfc3339(),
            priority: "medium".into(),
            action_type: "scheduled_task".into(),
            source_run_id: None,
            source_proposal_id: None,
            review_created_at: chrono::Utc::now().to_rfc3339(),
        };
        let task = due_task("read-only-create", "read-only-create-proposal");
        let result_ref = "conversation://read-only/message/1";
        let result_digest = digest_ref("read-only-result");
        let provider_started = error_or_success(read_only.record_provider_started(
            &claim,
            "read-only-provider-request",
            "ollama",
            "local-model",
            chrono::Utc::now(),
            &scheduled_provider_evidence(&claim),
        ));
        let provider_terminal = error_or_success(read_only.record_provider_terminal(
            &claim,
            &failed_receipt(&claim, "read-only-provider-terminal", chrono::Utc::now()),
        ));

        let actual = vec![
            error_or_success(read_only.migrate_legacy_json_if_present(&missing_legacy)),
            error_or_success(read_only.pending_legacy_review_candidates(directory.path())),
            error_or_success(
                read_only.mark_legacy_review_proposal_staged(&candidate, "read-only-proposal"),
            ),
            error_or_success(read_only.create_task_idempotent(&task)),
            error_or_success(read_only.quarantine_unproven_reviewed_cloud_task("read-only-task")),
            error_or_success(
                read_only.claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30)),
            ),
            error_or_success(read_only.begin_claim_execution(&claim)),
            provider_started,
            provider_terminal,
            error_or_success(read_only.record_tool_dispatch_started(
                &claim,
                &tool_attempt,
                &tool_started,
            )),
            error_or_success(read_only.record_tool_terminal(&claim, &tool_terminal)),
            error_or_success(read_only.stage_claim_result_delivery(
                &claim,
                result_ref,
                &result_digest,
            )),
            error_or_success(read_only.complete_claim(
                &claim,
                "read-only-agent-run",
                result_ref,
                &result_digest,
            )),
            error_or_success(read_only.settle_claim_after_error(
                &claim,
                "read_only_probe",
                Some(&digest_ref("read-only-error")),
            )),
            error_or_success(read_only.settle_claim_after_timeout(&claim)),
            error_or_success(read_only.quarantine_claim_unknown(&claim, "read_only_probe")),
            error_or_success(read_only.reconcile_previous_process_claims(chrono::Utc::now())),
            error_or_success(read_only.reconcile_abandoned_writer_generation(chrono::Utc::now())),
            error_or_success(read_only.reconcile_expired_claims(chrono::Utc::now())),
            error_or_success(read_only.issue_tool_failed_reconciliation(
                &claim.task.id,
                &claim.attempt_id,
                tool_terminal,
            )),
            error_or_success(read_only.issue_scheduled_reconciliation_test_admission(
                &claim.task.id,
                &claim.attempt_id,
                ScheduledReconciliationTestResolution::RetrySafe,
            )),
            error_or_success(read_only.reconcile_unknown_attempt(reconciliation_admission)),
        ];
        let expected = vec![
            "scheduled_task_store_write_authority_required:migrate_legacy_json_if_present",
            "scheduled_task_store_write_authority_required:pending_legacy_review_candidates",
            "scheduled_task_store_write_authority_required:mark_legacy_review_proposal_staged",
            "scheduled_task_store_write_authority_required:create_task_idempotent",
            "scheduled_task_store_write_authority_required:quarantine_unproven_reviewed_cloud_task",
            "scheduled_task_store_write_authority_required:claim_next_due",
            "scheduled_task_store_write_authority_required:begin_claim_execution",
            "scheduled_task_store_write_authority_required:record_provider_truth",
            "scheduled_task_store_write_authority_required:record_provider_truth",
            "scheduled_task_store_write_authority_required:record_tool_dispatch_started",
            "scheduled_task_store_write_authority_required:record_tool_terminal",
            "scheduled_task_store_write_authority_required:stage_claim_result_delivery",
            "scheduled_task_store_write_authority_required:complete_claim",
            "scheduled_task_store_write_authority_required:settle_claim_after_error",
            "scheduled_task_store_write_authority_required:settle_claim_after_timeout",
            "scheduled_task_store_write_authority_required:quarantine_claim_unknown",
            "scheduled_task_store_write_authority_required:reconcile_previous_process_claims",
            "scheduled_task_store_write_authority_required:reconcile_abandoned_writer_generation",
            "scheduled_task_store_write_authority_required:reconcile_expired_claims",
            "scheduled_task_store_write_authority_required:issue_tool_failed_reconciliation",
            "scheduled_task_store_write_authority_required:issue_scheduled_reconciliation_test_admission",
            "scheduled_task_store_write_authority_required:reconcile_unknown_attempt",
        ];
        assert_eq!(actual, expected);
        assert!(!missing_legacy.exists());
        assert_eq!(std::fs::read(&path).unwrap(), before_bytes);
        assert_eq!(
            std::fs::read(&legacy_evidence_path).unwrap(),
            before_evidence_bytes
        );
        assert_eq!(
            std::fs::metadata(&legacy_evidence_path)
                .unwrap()
                .modified()
                .unwrap(),
            before_evidence_metadata.modified().unwrap()
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before_metadata.modified().unwrap()
        );
        #[cfg(unix)]
        {
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode(),
                before_metadata.permissions().mode()
            );
            assert_eq!(
                std::fs::metadata(&legacy_evidence_path)
                    .unwrap()
                    .permissions()
                    .mode(),
                before_evidence_metadata.permissions().mode()
            );
        }
    }

    // This test's temporary provider endpoint is process-global and must stay
    // isolated until the async reconciliation request is fully observed.
    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn provider_failed_reconciliation_read_only_gate_preserves_all_file_and_capability_state()
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("provider-reconciliation-read-only.sqlite");
        let lock_path = directory
            .path()
            .join("provider-reconciliation-read-only.sqlite.openlife-owner.lock");
        let legacy_path = directory.path().join("scheduled_tasks.json");
        std::fs::write(
            &legacy_path,
            serde_json::to_vec(&serde_json::json!([{
                "id": "provider-read-only-legacy",
                "title": "Provider read-only legacy evidence",
                "prompt": "must remain byte-for-byte unchanged",
                "scheduled_at": (chrono::Utc::now() + chrono::Duration::days(2)).to_rfc3339(),
                "status": "pending"
            }]))
            .unwrap(),
        )
        .unwrap();
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x84; 32]).unwrap();
        let store = Arc::new(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());
        let legacy_evidence = store
            .migrate_legacy_json_if_present(&legacy_path)
            .unwrap()
            .evidence_path
            .unwrap();
        let claim = Arc::new(claim_and_begin(
            &store,
            "provider-read-only-task",
            "provider-read-only-task-proposal",
        ));
        let task_id = claim.task.id.clone();
        let attempt_id = claim.attempt_id.clone();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", format!("http://{address}"));
        std::env::remove_var("OLLAMA_HOST");
        let server = tokio::spawn(async move {
            let (mut tags_socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 16 * 1024];
            let _ = tags_socket.read(&mut request).await.unwrap();
            let tags_body = r#"{"models":[{"name":"qwen-local:latest","size":1}]}"#;
            let tags_response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                tags_body.len(),
                tags_body
            );
            tags_socket
                .write_all(tags_response.as_bytes())
                .await
                .unwrap();

            let (mut chat_socket, _) = listener.accept().await.unwrap();
            let _ = chat_socket.read(&mut request).await.unwrap();
            let failure_body = r#"{"error":"confirmed provider failure"}"#;
            let failure_response = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                failure_body.len(),
                failure_body
            );
            chat_socket
                .write_all(failure_response.as_bytes())
                .await
                .unwrap();
        });
        let scheduler = crate::scheduler::InferenceScheduler::new(
            "qwen-local:latest".into(),
            true,
            "openai".into(),
            "http://127.0.0.1:9/v1".into(),
            "unused-cloud-key".into(),
            "unused-cloud-model".into(),
            "unused-embedding".into(),
            false,
        );
        let (scheduler, admission_handle) = scheduler
            .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&claim))
            .unwrap();
        let messages = vec![crate::llm::ChatMessage {
            role: "user".into(),
            content: claim.task.description.clone(),
        }];
        let authorization = ProviderPolicyAuthorization::from_scheduled_claim(&claim)
            .and_then(|authorization| {
                authorization.authorize_derived_payload(
                    ProviderPayloadPurpose::AgentLoopStep,
                    &claim.task.description,
                    &messages,
                    &[],
                )
            })
            .unwrap();
        let request = scheduler
            .prepare_scheduled_chat_request(
                messages,
                Vec::new(),
                crate::llm::ContextManifest {
                    request_id: "provider-read-only-failed-proof".into(),
                    privacy_decision_id: claim.provider_grant.policy_decision_digest.clone(),
                    selected_context_refs: Vec::new(),
                    included_context_categories: Vec::new(),
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::RuntimeCompiledMessages,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization,
                crate::config::NetworkPolicy {
                    default_decision: "allow".into(),
                    ..crate::config::NetworkPolicy::default()
                },
                false,
            )
            .await
            .unwrap();
        let outcome = scheduler.execute_scheduled_provider_request(request).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        server.await.unwrap();
        assert_eq!(
            outcome.receipt.as_ref().map(|receipt| receipt.status),
            Some(ProviderInvocationStatus::Failed)
        );
        let proof = outcome.terminal_proof.unwrap();
        drop(scheduler);
        drop(admission_handle);
        drop(claim);
        drop(store);

        let read_only =
            TaskStore::open_read_only_existing_with_authority_key(&path, &authority_key).unwrap();
        let mut before = sqlite_family_states(&path);
        before.push((legacy_evidence.clone(), exact_file_state(&legacy_evidence)));
        before.push((lock_path.clone(), exact_file_state(&lock_path)));
        let error = read_only
            .issue_provider_failed_reconciliation(&task_id, &attempt_id, proof)
            .expect_err("read-only provider reconciliation must reject before proof consumption")
            .to_string();
        assert_eq!(
            error,
            "scheduled_task_store_write_authority_required:issue_provider_failed_reconciliation"
        );
        assert_file_states_unchanged(&before);
    }

    #[test]
    fn same_process_reopen_is_not_a_previous_process_epoch() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("same-process-reopen.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x6a; 32]).unwrap();
        let first = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        first
            .create_task_idempotent(&due_task(
                "same-process-reopen-task",
                "same-process-reopen-proposal",
            ))
            .unwrap();
        let claim = first
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .unwrap()
            .unwrap();
        let first_process_epoch = first.runtime_authority().unwrap().process_epoch_id;
        let first_writer_generation = first
            .runtime_authority()
            .unwrap()
            .writer_owner_generation_id;
        let first_attempt_id = claim.attempt_id().to_string();
        drop(claim);
        drop(first);

        let reopened = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        let reopened_process_epoch = reopened.runtime_authority().unwrap().process_epoch_id;
        let reopened_writer_generation = reopened
            .runtime_authority()
            .unwrap()
            .writer_owner_generation_id;
        assert_eq!(
            first_process_epoch, reopened_process_epoch,
            "two owners created by the same OS process must share one process epoch"
        );
        assert_ne!(
            first_writer_generation, reopened_writer_generation,
            "each successful owner lease must have a distinct writer generation"
        );
        assert_eq!(
            reopened
                .reconcile_abandoned_runtime_claims(
                    chrono::Utc::now(),
                    AbandonedRuntimeScope::PreviousProcessEpoch,
                )
                .unwrap(),
            0,
            "same-process abandoned writer generation is not a previous process"
        );
        assert_eq!(
            reopened
                .reconcile_abandoned_writer_generation(chrono::Utc::now())
                .unwrap(),
            1
        );
        let attempt = reopened
            .latest_attempt_for_task("same-process-reopen-task")
            .unwrap()
            .unwrap();
        assert_eq!(attempt.attempt_id, first_attempt_id);
        assert_eq!(attempt.process_epoch_id, first_process_epoch.to_string());
        assert_eq!(
            attempt.writer_owner_generation_id,
            first_writer_generation.to_string()
        );
        assert_eq!(attempt.status, "expired_before_execution");
    }

    #[test]
    fn scheduler_attempt_and_receipts_bind_process_epoch_and_writer_generation() {
        let store = TaskStore::new_in_memory().unwrap();
        let conn = store.lock_connection().unwrap();
        let columns = |table: &str| -> std::collections::HashSet<String> {
            conn.prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        for table in [
            "scheduler_attempts",
            "scheduler_provider_receipts",
            "scheduler_tool_dispatches",
        ] {
            let columns = columns(table);
            assert!(
                columns.contains("process_epoch_id"),
                "{table} does not bind the OS process epoch"
            );
            assert!(
                columns.contains("writer_owner_generation_id"),
                "{table} does not bind the exact writable owner generation"
            );
        }
    }

    #[test]
    fn v14_rows_without_writer_generation_migrate_to_legacy_unknown_not_exact_truth() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("v14-missing-writer-generation.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x7e; 32]).unwrap();
        {
            let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
            let terminal =
                claim_and_begin(&store, "v14-terminal-task", "v14-terminal-task-proposal");
            let provider_started_at = chrono::Utc::now();
            store
                .record_provider_started(
                    &terminal,
                    "v14-terminal-provider",
                    "ollama",
                    "local-model",
                    provider_started_at,
                    &scheduled_provider_evidence(&terminal),
                )
                .unwrap();
            store
                .record_provider_terminal(
                    &terminal,
                    &completed_receipt(&terminal, "v14-terminal-provider", provider_started_at),
                )
                .unwrap();
            let (tool_attempt, tool_started, tool_terminal) = observed_tool_receipt_fixture(
                "v14-terminal-tool",
                "v14-terminal-tool-run",
                "v14-terminal-tool-input",
            );
            store
                .record_tool_dispatch_started(&terminal, &tool_attempt, &tool_started)
                .unwrap();
            store
                .record_tool_terminal(&terminal, &tool_terminal)
                .unwrap();
            let result_ref = "conversation://v14-terminal/message/1";
            let result_digest = digest_ref("v14-terminal-result");
            store
                .stage_claim_result_delivery(&terminal, result_ref, &result_digest)
                .unwrap();
            assert!(store
                .complete_claim(
                    &terminal,
                    "v14-terminal-agent-run",
                    result_ref,
                    &result_digest,
                )
                .unwrap());

            let active = claim_and_begin(&store, "v14-active-task", "v14-active-task-proposal");
            store
                .record_provider_started(
                    &active,
                    "v14-active-provider",
                    "ollama",
                    "local-model",
                    chrono::Utc::now(),
                    &scheduled_provider_evidence(&active),
                )
                .unwrap();
        }
        downgrade_current_task_store_to_v14_without_writer_generation(&path);

        let reopened = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        let tasks = reopened.list_tasks(None).unwrap();
        assert_eq!(tasks.len(), 2);
        assert!(tasks
            .iter()
            .all(|task| task.status == "unknown_legacy_execution_state"));
        let conn = reopened.lock_connection().unwrap();
        let attempts = conn
            .prepare(
                "SELECT status, provider_provenance_state, process_epoch_id,
                        writer_owner_generation_id
                 FROM scheduler_attempts ORDER BY task_id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(attempts.len(), 2);
        for (status, provenance, process_epoch, writer_generation) in attempts {
            assert_eq!(status, "unknown");
            assert_eq!(provenance, "legacy_unavailable");
            assert!(uuid::Uuid::parse_str(&process_epoch).is_ok());
            assert_eq!(writer_generation, "legacy_writer_owner_generation_unknown");
        }
        let provider_rows = conn
            .prepare(
                "SELECT status, policy_evidence_state, migration_associated_grant_id,
                        writer_owner_generation_id
                 FROM scheduler_provider_receipts ORDER BY request_id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(provider_rows.len(), 2);
        for (status, evidence_state, migration_grant, writer_generation) in provider_rows {
            assert_eq!(status, "remote_unknown");
            assert_eq!(evidence_state, "legacy_unavailable");
            assert!(migration_grant.is_some());
            assert_eq!(writer_generation, "legacy_writer_owner_generation_unknown");
        }
        let tool_row: (String, String, String, String, String) = conn
            .query_row(
                "SELECT status, identity_state, transport_status, effect_status,
                        writer_owner_generation_id
                 FROM scheduler_tool_dispatches",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(tool_row.0, "unknown");
        assert_eq!(tool_row.1, "legacy_unavailable");
        assert_eq!(tool_row.2, "remote_unknown");
        assert_eq!(tool_row.3, "unknown");
        assert_eq!(tool_row.4, "legacy_writer_owner_generation_unknown");
        assert_eq!(
            task_store_schema_version(&conn).unwrap(),
            Some(TASK_STORE_SCHEMA_VERSION)
        );
    }

    #[test]
    fn v14_writer_generation_migration_rolls_back_every_truth_change_on_fault() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("v14-writer-generation-rollback.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x7f; 32]).unwrap();
        {
            let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
            let claim = claim_and_begin(&store, "v14-rollback-task", "v14-rollback-task-proposal");
            store
                .record_provider_started(
                    &claim,
                    "v14-rollback-provider",
                    "ollama",
                    "local-model",
                    chrono::Utc::now(),
                    &scheduled_provider_evidence(&claim),
                )
                .unwrap();
        }
        downgrade_current_task_store_to_v14_without_writer_generation(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_v15_writer_generation_migration
                 BEFORE UPDATE OF provider_provenance_state ON scheduler_attempts
                 BEGIN
                    SELECT RAISE(ABORT, 'fault-injected v15 writer migration');
                 END;",
            )
            .unwrap();
        }

        let error = TaskStore::new_with_authority_key(&path, &authority_key)
            .err()
            .expect("fault-injected v15 migration must roll back")
            .to_string();
        assert!(
            error.contains("fault-injected v15 writer migration"),
            "{error}"
        );
        let conn = Connection::open(&path).unwrap();
        assert_eq!(task_store_schema_version(&conn).unwrap(), Some(14));
        for table in [
            "scheduler_attempts",
            "scheduler_provider_receipts",
            "scheduler_tool_dispatches",
        ] {
            assert!(!table_has_column(&conn, table, "writer_owner_generation_id").unwrap());
        }
        let truth: (String, String, String, String) = conn
            .query_row(
                "SELECT t.status, a.status, a.provider_provenance_state,
                        r.policy_evidence_state
                 FROM tasks t
                 JOIN scheduler_attempts a ON a.task_id = t.id
                 JOIN scheduler_provider_receipts r ON r.attempt_id = a.attempt_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(truth.0, "running");
        assert_eq!(truth.1, "executing");
        assert_eq!(truth.2, "exact");
        assert_eq!(truth.3, "exact");
        assert!(
            task_store_metadata_value(&conn, TASK_STORE_OWNER_LOCK_VERIFIER_METADATA_KEY)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn current_exact_runtime_identity_rejects_nil_non_v4_and_non_uuid_sentinels() {
        for invalid in [
            "00000000-0000-0000-0000-000000000000",
            "00000000-0000-1000-8000-000000000001",
            "shared-runtime-identity-sentinel",
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory
                .path()
                .join("invalid-exact-runtime-identity.sqlite");
            let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x83; 32]).unwrap();
            {
                let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
                let claim = claim_and_begin(
                    &store,
                    "invalid-exact-runtime-task",
                    "invalid-exact-runtime-proposal",
                );
                store
                    .record_provider_started(
                        &claim,
                        "invalid-exact-runtime-provider",
                        "ollama",
                        "local-model",
                        chrono::Utc::now(),
                        &scheduled_provider_evidence(&claim),
                    )
                    .unwrap();
            }
            {
                let conn = Connection::open(&path).unwrap();
                conn.execute(
                    "UPDATE scheduler_attempts
                     SET process_epoch_id = ?1, writer_owner_generation_id = ?1",
                    [invalid],
                )
                .unwrap();
                conn.execute(
                    "UPDATE scheduler_provider_receipts
                     SET process_epoch_id = ?1, writer_owner_generation_id = ?1",
                    [invalid],
                )
                .unwrap();
                conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                    .unwrap();
            }
            let error = TaskStore::new_with_authority_key(&path, &authority_key)
                .err()
                .expect("invalid exact runtime identity must fail closed")
                .to_string();
            assert!(
                error.contains("exact scheduler attempt runtime identity is invalid"),
                "invalid={invalid}: {error}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn hmac_failure_and_missing_current_binding_do_not_open_sqlite_writable_or_touch_files() {
        for mode in ["mismatch", "mismatch_with_wal", "missing"] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(format!("owner-hmac-{mode}.sqlite"));
            let lock_path = directory
                .path()
                .join(format!("owner-hmac-{mode}.sqlite.openlife-owner.lock"));
            let displaced_lock = directory
                .path()
                .join(format!("owner-hmac-{mode}.displaced"));
            let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x81; 32]).unwrap();
            drop(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());
            let wal_keeper = if mode == "mismatch_with_wal" {
                let conn = Connection::open(&path).unwrap();
                conn.execute_batch(
                    "CREATE TABLE writable_preflight_wal_probe(value INTEGER);
                     INSERT INTO writable_preflight_wal_probe VALUES (1);",
                )
                .unwrap();
                Some(conn)
            } else {
                None
            };
            if mode.starts_with("mismatch") {
                std::fs::rename(&lock_path, &displaced_lock).unwrap();
                std::fs::File::create(&lock_path).unwrap();
            } else {
                let conn = Connection::open(&path).unwrap();
                conn.execute_batch(
                    "DROP TRIGGER task_store_authority_metadata_immutable_update;
                     DROP TRIGGER task_store_authority_metadata_immutable_delete;
                     DELETE FROM task_store_metadata
                     WHERE key = 'canonical_task_store_owner_lock_verifier_v1';
                     PRAGMA wal_checkpoint(TRUNCATE);",
                )
                .unwrap();
            }
            let mut before = sqlite_family_states(&path);
            before.push((lock_path.clone(), exact_file_state(&lock_path)));
            let error = TaskStore::new_with_authority_key(&path, &authority_key)
                .err()
                .expect("an unauthenticated existing store must fail before writable SQLite open")
                .to_string();
            if mode.starts_with("mismatch") {
                assert!(
                    error.contains("task_store_owner_lock_authentication_failed"),
                    "{error}"
                );
            } else {
                assert!(
                    error.contains("task_store_owner_lock_authority_metadata_missing"),
                    "{error}"
                );
            }
            assert_file_states_unchanged(&before);
            if mode.starts_with("mismatch") {
                std::fs::remove_file(&lock_path).unwrap();
                std::fs::rename(&displaced_lock, &lock_path).unwrap();
            }
            drop(wal_keeper);
        }
    }

    #[cfg(unix)]
    #[test]
    fn wrong_root_key_with_live_wal_is_rejected_without_touching_canonical_family_or_temp_root() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wrong-key-live-wal.sqlite");
        let lock_path = directory
            .path()
            .join("wrong-key-live-wal.sqlite.openlife-owner.lock");
        let auth_temp_root = directory.path().join("auth-temp-root");
        std::fs::create_dir(&auth_temp_root).unwrap();
        let canary = auth_temp_root.join("must-remain-only-entry.canary");
        std::fs::write(&canary, b"canonical pre-auth must not stage temp copies").unwrap();
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x41; 32]).unwrap();
        let wrong_key = TaskStoreAuthorityKey::from_key_material(&[0x42; 32]).unwrap();
        {
            let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
            store
                .create_task_idempotent(&due_task("wrong-key-wal-task", "wrong-key-wal-proposal"))
                .unwrap();
        }
        let wal_keeper = Connection::open(&path).unwrap();
        wal_keeper
            .execute_batch(
                "CREATE TABLE wrong_key_live_wal_probe(value INTEGER);
                 INSERT INTO wrong_key_live_wal_probe VALUES (1);",
            )
            .unwrap();
        assert!(PathBuf::from(format!("{}-wal", path.display())).exists());
        let mut before = sqlite_family_states(&path);
        before.push((lock_path.clone(), exact_file_state(&lock_path)));
        let temp_before = std::fs::read_dir(&auth_temp_root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();

        let error = TaskStore::new_with_authority_key(&path, &wrong_key)
            .err()
            .expect("a wrong root key must fail from the external envelope")
            .to_string();
        assert!(
            error.contains("task_store_owner_lock_authentication_failed"),
            "{error}"
        );
        assert_file_states_unchanged(&before);
        let temp_after = std::fs::read_dir(&auth_temp_root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(temp_after, temp_before);
        assert_eq!(
            std::fs::read(&canary).unwrap(),
            b"canonical pre-auth must not stage temp copies"
        );
        drop(wal_keeper);
    }

    #[cfg(unix)]
    #[test]
    fn owner_envelope_transplant_and_database_inode_replacement_fail_before_sqlite_open() {
        let directory = tempfile::tempdir().unwrap();
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x43; 32]).unwrap();
        let first = directory.path().join("envelope-first.sqlite");
        let second = directory.path().join("envelope-second.sqlite");
        for path in [&first, &second] {
            drop(TaskStore::new_with_authority_key(path, &authority_key).unwrap());
        }
        let first_lock = directory
            .path()
            .join("envelope-first.sqlite.openlife-owner.lock");
        let second_lock = directory
            .path()
            .join("envelope-second.sqlite.openlife-owner.lock");
        std::fs::write(&second_lock, std::fs::read(&first_lock).unwrap()).unwrap();
        let mut transplant_before = sqlite_family_states(&second);
        transplant_before.push((second_lock.clone(), exact_file_state(&second_lock)));
        let transplant_error = TaskStore::new_with_authority_key(&second, &authority_key)
            .err()
            .expect("a signed envelope must not transplant to another lock/database slot")
            .to_string();
        assert!(
            transplant_error.contains("task_store_owner_lock_authentication_failed"),
            "{transplant_error}"
        );
        assert_file_states_unchanged(&transplant_before);

        let original = directory.path().join("inode-original.sqlite");
        let replacement = directory.path().join("inode-replacement.sqlite");
        let displaced = directory.path().join("inode-displaced.sqlite");
        drop(TaskStore::new_with_authority_key(&original, &authority_key).unwrap());
        std::fs::copy(&original, &replacement).unwrap();
        std::fs::rename(&original, &displaced).unwrap();
        std::fs::rename(&replacement, &original).unwrap();
        let original_lock = directory
            .path()
            .join("inode-original.sqlite.openlife-owner.lock");
        let mut inode_before = sqlite_family_states(&original);
        inode_before.push((original_lock.clone(), exact_file_state(&original_lock)));
        let inode_error = TaskStore::new_with_authority_key(&original, &authority_key)
            .err()
            .expect("a replacement database inode must not inherit canonical authority")
            .to_string();
        assert!(
            inode_error.contains("task_store_owner_lock_authentication_failed"),
            "{inode_error}"
        );
        assert_file_states_unchanged(&inode_before);
    }

    #[cfg(unix)]
    #[test]
    fn valid_owner_envelope_allows_normal_restart_with_live_wal_truth() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("valid-envelope-live-wal.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x44; 32]).unwrap();
        {
            let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
            store
                .create_task_idempotent(&due_task(
                    "live-wal-restart-task",
                    "live-wal-restart-proposal",
                ))
                .unwrap();
        }
        let wal_keeper = Connection::open(&path).unwrap();
        wal_keeper
            .execute_batch(
                "CREATE TABLE valid_envelope_live_wal_probe(value INTEGER);
                 INSERT INTO valid_envelope_live_wal_probe VALUES (1);",
            )
            .unwrap();
        assert!(PathBuf::from(format!("{}-wal", path.display())).exists());

        let reopened = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        let tasks = reopened.list_tasks(None).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "live-wal-restart-task");
        assert_eq!(
            reopened
                .lock_connection()
                .unwrap()
                .query_row(
                    "SELECT value FROM valid_envelope_live_wal_probe",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        drop(reopened);
        drop(wal_keeper);
    }

    #[cfg(unix)]
    #[test]
    fn checkpointed_pre_v15_store_migrates_once_but_current_envelope_clear_never_resigns() {
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("checkpointed-pre-envelope.sqlite");
        let lock_path = directory
            .path()
            .join("checkpointed-pre-envelope.sqlite.openlife-owner.lock");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x45; 32]).unwrap();
        let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        store
            .create_task_idempotent(&due_task(
                "legacy-envelope-task",
                "legacy-envelope-proposal",
            ))
            .unwrap();
        {
            let conn = store.lock_connection().unwrap();
            conn.execute_batch(
                "DROP TRIGGER task_store_authority_metadata_immutable_update;
                 DROP TRIGGER task_store_authority_metadata_immutable_delete;
                 DELETE FROM task_store_metadata
                 WHERE key = 'canonical_task_store_owner_lock_verifier_v1';
                 UPDATE openlife_schema_versions SET version = 14
                 WHERE component = 'task_store';
                 PRAGMA wal_checkpoint(TRUNCATE);
                 PRAGMA journal_mode=DELETE;",
            )
            .unwrap();
        }
        drop(store);
        for suffix in ["-wal", "-shm"] {
            assert!(!PathBuf::from(format!("{}{}", path.display(), suffix)).exists());
        }
        let lock_inode = std::fs::metadata(&lock_path).unwrap().ino();
        std::fs::File::create(&lock_path).unwrap();
        assert_eq!(std::fs::metadata(&lock_path).unwrap().ino(), lock_inode);
        assert_eq!(std::fs::metadata(&lock_path).unwrap().len(), 0);

        let reopened = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        assert_eq!(reopened.list_tasks(None).unwrap().len(), 1);
        let envelope_bytes = std::fs::read(&lock_path).unwrap();
        assert!(!envelope_bytes.is_empty());
        assert!(envelope_bytes.len() <= MAX_TASK_STORE_OWNER_ENVELOPE_BYTES);
        let envelope: TaskStoreOwnerEnvelopeV1 = serde_json::from_slice(&envelope_bytes).unwrap();
        assert_eq!(envelope.schema, TASK_STORE_OWNER_ENVELOPE_SCHEMA);

        {
            let conn = reopened.lock_connection().unwrap();
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
                .unwrap();
        }
        drop(reopened);
        std::fs::File::create(&lock_path).unwrap();
        assert_eq!(std::fs::metadata(&lock_path).unwrap().ino(), lock_inode);
        let mut before = sqlite_family_states(&path);
        before.push((lock_path.clone(), exact_file_state(&lock_path)));
        for attempt in 1..=2 {
            let error = TaskStore::new_with_authority_key(&path, &authority_key)
                .err()
                .expect("a current store with a cleared envelope must remain fail-closed")
                .to_string();
            assert!(
                error.contains("owner_envelope_missing_current_schema"),
                "attempt={attempt}: {error}"
            );
            assert_file_states_unchanged(&before);
            assert_eq!(std::fs::metadata(&lock_path).unwrap().len(), 0);
        }
    }

    #[cfg(unix)]
    #[test]
    fn pre_v15_store_with_internal_owner_verifier_cannot_recreate_a_missing_envelope() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pre-v15-already-owner-bound.sqlite");
        let lock_path = directory
            .path()
            .join("pre-v15-already-owner-bound.sqlite.openlife-owner.lock");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x48; 32]).unwrap();
        let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        {
            let conn = store.lock_connection().unwrap();
            conn.execute_batch(
                "UPDATE openlife_schema_versions SET version = 14
                 WHERE component = 'task_store';
                 PRAGMA wal_checkpoint(TRUNCATE);
                 PRAGMA journal_mode=DELETE;",
            )
            .unwrap();
        }
        drop(store);
        std::fs::File::create(&lock_path).unwrap();
        let mut before = sqlite_family_states(&path);
        before.push((lock_path.clone(), exact_file_state(&lock_path)));

        let error = TaskStore::new_with_authority_key(&path, &authority_key)
            .err()
            .expect("pre-v15 schema alone must not authorize envelope recreation")
            .to_string();
        assert!(
            error.contains("legacy_owner_verifier_already_bound"),
            "{error}"
        );
        assert_file_states_unchanged(&before);
        assert_eq!(std::fs::metadata(&lock_path).unwrap().len(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn same_inode_owner_envelope_tamper_poison_is_sticky() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("same-inode-envelope-tamper.sqlite");
        let lock_path = directory
            .path()
            .join("same-inode-envelope-tamper.sqlite.openlife-owner.lock");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x46; 32]).unwrap();
        let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        let original_envelope = std::fs::read(&lock_path).unwrap();
        std::fs::write(&lock_path, b"{\"partial\":true}").unwrap();

        let first_error = store.list_tasks(None).unwrap_err().to_string();
        assert!(
            first_error.contains("task_store_owner_envelope_poisoned"),
            "{first_error}"
        );
        std::fs::write(&lock_path, original_envelope).unwrap();
        let second_error = store.list_tasks(None).unwrap_err().to_string();
        assert!(
            second_error.contains("task_store_owner_envelope_poisoned"),
            "{second_error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn future_schema_fails_during_read_only_authentication_with_zero_file_changes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("future-schema-read-only-preflight.sqlite");
        let lock_path = directory
            .path()
            .join("future-schema-read-only-preflight.sqlite.openlife-owner.lock");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x82; 32]).unwrap();
        drop(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute(
                "UPDATE openlife_schema_versions SET version = ?1
                 WHERE component = 'task_store'",
                [TASK_STORE_SCHEMA_VERSION + 1],
            )
            .unwrap();
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .unwrap();
        }
        let mut before = sqlite_family_states(&path);
        before.push((lock_path.clone(), exact_file_state(&lock_path)));
        let error = TaskStore::new_with_authority_key(&path, &authority_key)
            .err()
            .expect("future schema must fail during read-only preflight")
            .to_string();
        assert!(
            error.contains("task store schema is newer than this OpenLife build"),
            "{error}"
        );
        assert_file_states_unchanged(&before);
    }

    const TASK_STORE_CROSS_PROCESS_HELPER_MODE_ENV: &str =
        "OPENLIFE_TASK_STORE_CROSS_PROCESS_HELPER_MODE";
    const TASK_STORE_CROSS_PROCESS_HELPER_PATH_ENV: &str =
        "OPENLIFE_TASK_STORE_CROSS_PROCESS_HELPER_PATH";
    const TASK_STORE_CROSS_PROCESS_HELPER_KEY_BYTE_ENV: &str =
        "OPENLIFE_TASK_STORE_CROSS_PROCESS_HELPER_KEY_BYTE";
    const TASK_STORE_CROSS_PROCESS_HELPER_STORE_ID_ENV: &str =
        "OPENLIFE_TASK_STORE_CROSS_PROCESS_HELPER_STORE_ID";
    const TASK_STORE_CROSS_PROCESS_HELPER_PARENT_EPOCH_ENV: &str =
        "OPENLIFE_TASK_STORE_CROSS_PROCESS_HELPER_PARENT_EPOCH";

    #[test]
    #[ignore = "invoked only as the exact child-process owner-lease probe"]
    fn scheduled_task_store_cross_process_writer_helper() {
        let Ok(mode) = std::env::var(TASK_STORE_CROSS_PROCESS_HELPER_MODE_ENV) else {
            return;
        };
        let path = PathBuf::from(
            std::env::var(TASK_STORE_CROSS_PROCESS_HELPER_PATH_ENV)
                .expect("cross-process helper database path"),
        );
        let key_byte = std::env::var(TASK_STORE_CROSS_PROCESS_HELPER_KEY_BYTE_ENV)
            .expect("cross-process helper key byte")
            .parse::<u8>()
            .expect("cross-process helper key byte must be u8");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[key_byte; 32]).unwrap();
        match mode.as_str() {
            "expect_unavailable" => {
                let error = match TaskStore::new_with_authority_key(&path, &authority_key) {
                    Ok(_) => panic!(
                        "a child process acquired the canonical TaskStore while the parent still owned it"
                    ),
                    Err(error) => error,
                };
                let failure = error
                    .downcast_ref::<crate::sqlite_migration::SqliteSlotOwnerLeaseUnavailable>()
                    .expect("owner-lease rejection must preserve its structured failure layer");
                assert_eq!(
                    failure.failure_layer(),
                    crate::sqlite_migration::SqliteSlotOwnerLeaseFailureLayer::OsOwnerLockWouldBlock
                );
                let os_error_kind = error.chain().find_map(|source| {
                    source
                        .downcast_ref::<std::io::Error>()
                        .map(std::io::Error::kind)
                });
                assert_eq!(os_error_kind, Some(std::io::ErrorKind::WouldBlock));
                assert!(
                    error
                        .to_string()
                        .contains("scheduled_task_store_sqlite_slot_owner_lease_unavailable"),
                    "{error}"
                );
                println!(
                    "task_store_cross_process_probe:lease_unavailable:\
                     failure_layer=os_owner_lock_would_block:source_kind=would_block"
                );
            }
            "expect_link_count_rejected" => {
                let error = match TaskStore::new_with_authority_key(&path, &authority_key) {
                    Ok(_) => panic!("a hardlinked uninitialized slot reached SQLite"),
                    Err(error) => error.to_string(),
                };
                assert!(
                    error.contains("scheduled_task_store_database_link_count_invalid"),
                    "{error}"
                );
                println!("task_store_cross_process_probe:link_count_rejected");
            }
            "expect_owner_lock_replacement_rejected" => {
                let error = match TaskStore::new_with_authority_key(&path, &authority_key) {
                    Ok(_) => panic!(
                        "a replaced owner-lock inode authenticated as the canonical TaskStore owner"
                    ),
                    Err(error) => error.to_string(),
                };
                assert!(
                    error.contains("task_store_owner_lock_authentication_failed"),
                    "{error}"
                );
                println!("task_store_cross_process_probe:owner_lock_replacement_rejected");
            }
            "expect_open" => {
                let expected_store_id = std::env::var(TASK_STORE_CROSS_PROCESS_HELPER_STORE_ID_ENV)
                    .expect("cross-process helper expected store id");
                let store = TaskStore::new_with_authority_key(&path, &authority_key)
                    .expect("child process opens after final parent owner drop");
                assert_eq!(
                    store
                        .runtime_authority()
                        .unwrap()
                        .canonical_store_identity
                        .as_ref(),
                    expected_store_id
                );
                let parent_epoch = uuid::Uuid::parse_str(
                    &std::env::var(TASK_STORE_CROSS_PROCESS_HELPER_PARENT_EPOCH_ENV)
                        .expect("cross-process helper parent epoch"),
                )
                .unwrap();
                assert_ne!(
                    store.runtime_authority().unwrap().process_epoch_id,
                    parent_epoch,
                    "an exec'd child must own a different OS-process epoch"
                );
                println!("task_store_cross_process_probe:opened_same_canonical_store");
            }
            "claim_start_and_exit" => {
                let parent_epoch = uuid::Uuid::parse_str(
                    &std::env::var(TASK_STORE_CROSS_PROCESS_HELPER_PARENT_EPOCH_ENV)
                        .expect("cross-process helper parent epoch"),
                )
                .unwrap();
                let store = TaskStore::new_with_authority_key(&path, &authority_key)
                    .expect("child process opens the released canonical store");
                assert_ne!(
                    store.runtime_authority().unwrap().process_epoch_id,
                    parent_epoch
                );
                let claim = store
                    .claim_next_due(chrono::Utc::now(), chrono::Duration::minutes(5))
                    .unwrap()
                    .expect("child process claims the prepared task");
                assert!(store.begin_claim_execution(&claim).unwrap());
                let evidence = scheduled_provider_evidence(&claim);
                assert!(store
                    .record_provider_started(
                        &claim,
                        "child-provider-start-request",
                        "ollama",
                        "child-local-model",
                        chrono::Utc::now(),
                        &evidence,
                    )
                    .unwrap());
                let attempt = store
                    .latest_attempt_for_task(claim.task().id.as_str())
                    .unwrap()
                    .unwrap();
                let receipt = store
                    .provider_receipts_for_attempt(claim.attempt_id())
                    .unwrap()
                    .into_iter()
                    .next()
                    .unwrap();
                assert_eq!(attempt.process_epoch_id, receipt.process_epoch_id);
                assert_eq!(
                    attempt.writer_owner_generation_id,
                    receipt.writer_owner_generation_id
                );
                println!("task_store_cross_process_probe:provider_started_then_exit");
            }
            other => panic!("unknown cross-process TaskStore helper mode: {other}"),
        }
    }

    fn run_scheduled_task_store_cross_process_writer_helper(
        path: &Path,
        key_byte: u8,
        expected_store_id: &str,
        parent_process_epoch: uuid::Uuid,
        mode: &str,
    ) -> std::process::Output {
        std::process::Command::new(std::env::current_exe().expect("current test executable"))
            .arg("--exact")
            .arg("tasks::tests::scheduled_task_store_cross_process_writer_helper")
            .arg("--ignored")
            .arg("--nocapture")
            .env(TASK_STORE_CROSS_PROCESS_HELPER_MODE_ENV, mode)
            .env(TASK_STORE_CROSS_PROCESS_HELPER_PATH_ENV, path)
            .env(
                TASK_STORE_CROSS_PROCESS_HELPER_KEY_BYTE_ENV,
                key_byte.to_string(),
            )
            .env(
                TASK_STORE_CROSS_PROCESS_HELPER_STORE_ID_ENV,
                expected_store_id,
            )
            .env(
                TASK_STORE_CROSS_PROCESS_HELPER_PARENT_EPOCH_ENV,
                parent_process_epoch.to_string(),
            )
            .output()
            .expect("spawn exact TaskStore cross-process helper")
    }

    #[test]
    fn scheduled_task_store_cross_process_writer_lease_rejects_then_reopens_after_drop() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scheduled-task-cross-process.sqlite");
        let key_byte = 0x6d;
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[key_byte; 32]).unwrap();
        let parent_owner = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        let canonical_store_id = parent_owner
            .runtime_authority()
            .unwrap()
            .canonical_store_identity
            .to_string();
        let parent_process_epoch = parent_owner.runtime_authority().unwrap().process_epoch_id;
        let same_process_error = match TaskStore::new_with_authority_key(&path, &authority_key) {
            Ok(_) => panic!("a same-process second owner acquired the canonical TaskStore"),
            Err(error) => error,
        };
        let same_process_failure = same_process_error
            .downcast_ref::<crate::sqlite_migration::SqliteSlotOwnerLeaseUnavailable>()
            .expect("same-process rejection must preserve its structured failure layer");
        assert_eq!(
            same_process_failure.failure_layer(),
            crate::sqlite_migration::SqliteSlotOwnerLeaseFailureLayer::ProcessRegistry
        );
        assert!(
            same_process_error
                .chain()
                .all(|source| source.downcast_ref::<std::io::Error>().is_none()),
            "process-registry rejection must not be reported as an OS lock error"
        );
        println!(
            "task_store_owner_lease_probe:same_process:\
             failure_layer=process_registry:source_kind=none"
        );

        let rejected = run_scheduled_task_store_cross_process_writer_helper(
            &path,
            key_byte,
            &canonical_store_id,
            parent_process_epoch,
            "expect_unavailable",
        );
        assert!(
            rejected.status.success(),
            "child process did not fail closed on the parent's OS owner lease\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&rejected.stdout),
            String::from_utf8_lossy(&rejected.stderr)
        );
        assert!(
            String::from_utf8_lossy(&rejected.stdout)
                .contains(
                    "task_store_cross_process_probe:lease_unavailable:\
                     failure_layer=os_owner_lock_would_block:source_kind=would_block"
                ),
            "child process exited without proving that it executed the lease-unavailable branch\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&rejected.stdout),
            String::from_utf8_lossy(&rejected.stderr)
        );
        println!(
            "task_store_cross_process_probe:lease_unavailable:\
             failure_layer=os_owner_lock_would_block:source_kind=would_block"
        );

        drop(parent_owner);
        let reopened = run_scheduled_task_store_cross_process_writer_helper(
            &path,
            key_byte,
            &canonical_store_id,
            parent_process_epoch,
            "expect_open",
        );
        assert!(
            reopened.status.success(),
            "child process did not acquire the same canonical slot after final parent drop\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&reopened.stdout),
            String::from_utf8_lossy(&reopened.stderr)
        );
        assert!(
            String::from_utf8_lossy(&reopened.stdout)
                .contains("task_store_cross_process_probe:opened_same_canonical_store"),
            "child process exited without proving that it reopened the canonical store\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&reopened.stdout),
            String::from_utf8_lossy(&reopened.stderr)
        );
        println!("task_store_cross_process_probe:opened_same_canonical_store");
    }

    #[test]
    fn child_process_provider_start_is_remote_unknown_even_when_parent_clock_rolls_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scheduled-task-child-exit.sqlite");
        let key_byte = 0x6e;
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[key_byte; 32]).unwrap();
        let parent_owner = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        parent_owner
            .create_task_idempotent(&due_task(
                "child-process-provider-start",
                "child-process-provider-start-proposal",
            ))
            .unwrap();
        let canonical_store_id = parent_owner
            .runtime_authority()
            .unwrap()
            .canonical_store_identity
            .to_string();
        let parent_process_epoch = parent_owner.runtime_authority().unwrap().process_epoch_id;
        drop(parent_owner);

        let child = run_scheduled_task_store_cross_process_writer_helper(
            &path,
            key_byte,
            &canonical_store_id,
            parent_process_epoch,
            "claim_start_and_exit",
        );
        assert!(
            child.status.success(),
            "child provider-start probe failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        assert!(
            String::from_utf8_lossy(&child.stdout)
                .contains("task_store_cross_process_probe:provider_started_then_exit"),
            "child exited without proving its durable provider start\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );

        let reopened = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        assert_eq!(
            reopened.runtime_authority().unwrap().process_epoch_id,
            parent_process_epoch
        );
        let before = reopened
            .latest_attempt_for_task("child-process-provider-start")
            .unwrap()
            .unwrap();
        assert_eq!(before.status, "executing");
        assert_ne!(before.process_epoch_id, parent_process_epoch.to_string());
        let receipts = reopened
            .provider_receipts_for_attempt(&before.attempt_id)
            .unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].status, "started");
        assert_eq!(receipts[0].process_epoch_id, before.process_epoch_id);
        assert_eq!(
            receipts[0].writer_owner_generation_id,
            before.writer_owner_generation_id
        );

        let rolled_back_clock = chrono::DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(
            reopened
                .reconcile_previous_process_claims(rolled_back_clock)
                .unwrap(),
            1
        );
        let after = reopened
            .latest_attempt_for_task("child-process-provider-start")
            .unwrap()
            .unwrap();
        assert_eq!(after.status, "unknown");
        assert_eq!(after.process_epoch_id, before.process_epoch_id);
        assert_eq!(
            after.writer_owner_generation_id,
            before.writer_owner_generation_id
        );
        let receipt = reopened
            .provider_receipts_for_attempt(&after.attempt_id)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(receipt.status, "remote_unknown");
        let expected_remote_unknown = digest_ref("scheduler_process_exit_remote_state_unknown");
        assert_eq!(
            receipt.error_digest.as_deref(),
            Some(expected_remote_unknown.as_str())
        );
    }

    #[cfg(unix)]
    #[test]
    fn scheduled_task_store_rejects_hardlinked_database_before_sqlite_writable_open() {
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("hardlink-canonical.sqlite");
        let alias = directory.path().join("hardlink-alias.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x6f; 32]).unwrap();
        drop(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());
        std::fs::hard_link(&path, &alias).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().nlink(), 2);
        let before_bytes = std::fs::read(&path).unwrap();
        let before_modified = std::fs::metadata(&path).unwrap().modified().unwrap();

        let error = match TaskStore::new_with_authority_key(&path, &authority_key) {
            Ok(_) => panic!("a hardlinked canonical database reached SQLite writable open"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains("scheduled_task_store_database_link_count_invalid"),
            "{error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before_bytes);
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before_modified
        );
    }

    #[cfg(unix)]
    #[test]
    fn running_task_store_hardlink_violation_permanently_poisons_the_owner() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("running-hardlink.sqlite");
        let alias = directory.path().join("running-hardlink-alias.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x70; 32]).unwrap();
        let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        store
            .lock_connection()
            .unwrap()
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        let before_bytes = std::fs::read(&path).unwrap();
        let before_metadata = std::fs::metadata(&path).unwrap();
        std::fs::hard_link(&path, &alias).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().nlink(), 2);

        let first_error = store
            .create_task_idempotent(&due_task(
                "hardlink-poison-first",
                "hardlink-poison-first-proposal",
            ))
            .unwrap_err()
            .to_string();
        assert!(
            first_error.contains("scheduled_task_store_database_link_count_invalid"),
            "{first_error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before_bytes);
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before_metadata.modified().unwrap()
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode(),
            before_metadata.permissions().mode()
        );

        std::fs::remove_file(&alias).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().nlink(), 1);
        let second_error = store
            .create_task_idempotent(&due_task(
                "hardlink-poison-second",
                "hardlink-poison-second-proposal",
            ))
            .unwrap_err()
            .to_string();
        assert!(
            second_error.contains("scheduled_task_store_sqlite_slot_owner_poisoned"),
            "{second_error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before_bytes);
    }

    #[cfg(unix)]
    #[test]
    fn hardlink_added_between_lease_and_sqlite_open_fails_before_pragma() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lease-open-race.sqlite");
        let alias = directory.path().join("lease-open-race-alias.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x71; 32]).unwrap();
        drop(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());
        let before_bytes = std::fs::read(&path).unwrap();
        let before_modified = std::fs::metadata(&path).unwrap().modified().unwrap();

        let error = open_task_store_database_with_stable_slot(
            &path,
            || std::fs::hard_link(&path, &alias).unwrap(),
            || {},
        )
        .expect_err("lease/open hardlink race must fail closed")
        .to_string();
        assert!(
            error.contains("scheduled_task_store_database_link_count_invalid"),
            "{error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before_bytes);
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before_modified
        );
    }

    #[cfg(unix)]
    #[test]
    fn sidecar_replacement_after_preflight_fails_before_configure_and_poison_is_sticky() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preflight-sidecar-race.sqlite");
        let lock_path = directory
            .path()
            .join("preflight-sidecar-race.sqlite.openlife-owner.lock");
        let displaced_lock = directory.path().join("preflight-sidecar-race.displaced");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x7b; 32]).unwrap();
        drop(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; PRAGMA secure_delete=OFF;")
                .unwrap();
        }

        let (conn, canonical_path, owner_lease) =
            open_task_store_database_with_stable_slot(&path, || {}, || {}).unwrap();
        let owner_lock_identity_material = owner_lease.owner_lock_identity_material();
        let database_slot =
            TaskStoreDatabaseSlot::for_canonical_path(&canonical_path, &authority_key).unwrap();
        preflight_existing_task_store_owner_lock_binding(
            &conn,
            &database_slot,
            &owner_lock_identity_material,
        )
        .unwrap();
        let connection =
            crate::sqlite_migration::IdentityBoundSqliteConnection::writable(conn, owner_lease);
        let before = sqlite_family_states(&path);
        let error = configure_authenticated_task_store_connection(&connection, || {
            std::fs::rename(&lock_path, &displaced_lock).unwrap();
            std::fs::File::create(&lock_path).unwrap();
        })
        .expect_err("sidecar replacement after preflight must fail before configure")
        .to_string();
        assert!(
            error.contains("scheduled_task_store_owner_lock_identity_changed"),
            "{error}"
        );
        std::fs::remove_file(&lock_path).unwrap();
        std::fs::rename(&displaced_lock, &lock_path).unwrap();
        let poisoned = connection
            .lock()
            .expect_err("restoring the sidecar pathname must not unpoison the handle")
            .to_string();
        assert!(
            poisoned.contains("scheduled_task_store_sqlite_slot_owner_poisoned"),
            "{poisoned}"
        );
        assert_file_states_unchanged(&before);
    }

    #[cfg(unix)]
    #[test]
    fn database_hardlink_after_preflight_fails_before_configure_and_poison_is_sticky() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preflight-hardlink-race.sqlite");
        let alias = directory
            .path()
            .join("preflight-hardlink-race.alias.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x7c; 32]).unwrap();
        drop(TaskStore::new_with_authority_key(&path, &authority_key).unwrap());
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; PRAGMA secure_delete=OFF;")
                .unwrap();
        }

        let (conn, canonical_path, owner_lease) =
            open_task_store_database_with_stable_slot(&path, || {}, || {}).unwrap();
        let owner_lock_identity_material = owner_lease.owner_lock_identity_material();
        let database_slot =
            TaskStoreDatabaseSlot::for_canonical_path(&canonical_path, &authority_key).unwrap();
        preflight_existing_task_store_owner_lock_binding(
            &conn,
            &database_slot,
            &owner_lock_identity_material,
        )
        .unwrap();
        let connection =
            crate::sqlite_migration::IdentityBoundSqliteConnection::writable(conn, owner_lease);
        let before = sqlite_family_states(&path);
        let error = configure_authenticated_task_store_connection(&connection, || {
            std::fs::hard_link(&path, &alias).unwrap()
        })
        .expect_err("database hardlink after preflight must fail before configure")
        .to_string();
        assert!(
            error.contains("scheduled_task_store_database_link_count_invalid"),
            "{error}"
        );
        std::fs::remove_file(&alias).unwrap();
        let poisoned = connection
            .lock()
            .expect_err("removing the hardlink must not unpoison the handle")
            .to_string();
        assert!(
            poisoned.contains("scheduled_task_store_sqlite_slot_owner_poisoned"),
            "{poisoned}"
        );
        assert_file_states_unchanged(&before);
    }

    #[cfg(unix)]
    #[test]
    fn two_child_processes_cannot_initialize_hardlink_aliases() {
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("uninitialized-first.sqlite");
        let second = directory.path().join("uninitialized-second.sqlite");
        std::fs::File::create(&first).unwrap();
        std::fs::hard_link(&first, &second).unwrap();
        assert_eq!(std::fs::metadata(&first).unwrap().nlink(), 2);
        let parent_epoch = task_store_process_epoch_id();
        let first_path = first.clone();
        let second_path = second.clone();
        let (first_output, second_output) = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                run_scheduled_task_store_cross_process_writer_helper(
                    &first_path,
                    0x72,
                    "uninitialized",
                    parent_epoch,
                    "expect_link_count_rejected",
                )
            });
            let second = scope.spawn(|| {
                run_scheduled_task_store_cross_process_writer_helper(
                    &second_path,
                    0x72,
                    "uninitialized",
                    parent_epoch,
                    "expect_link_count_rejected",
                )
            });
            (first.join().unwrap(), second.join().unwrap())
        });
        for output in [first_output, second_output] {
            assert!(
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .contains("task_store_cross_process_probe:link_count_rejected"),
                "hardlink child did not prove pre-SQLite rejection\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert_eq!(std::fs::metadata(&first).unwrap().len(), 0);
        assert_eq!(std::fs::metadata(&first).unwrap().nlink(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn preexisting_owner_lock_symlink_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("owner-lock-symlink.sqlite");
        let lock_path = directory
            .path()
            .join("owner-lock-symlink.sqlite.openlife-owner.lock");
        let target = directory.path().join("owner-lock-symlink-target");
        std::fs::write(&target, b"owner lock target must remain untouched").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
        symlink(&target, &lock_path).unwrap();
        let before_target = exact_file_state(&target);
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x7d; 32]).unwrap();

        let error = TaskStore::new_with_authority_key(&path, &authority_key)
            .err()
            .expect("a preexisting owner-lock symlink must fail closed")
            .to_string();
        assert!(
            error.contains("scheduled_task_store_owner_lock_symlink_rejected"),
            "{error}"
        );
        assert_eq!(exact_file_state(&target), before_target);
        assert!(std::fs::symlink_metadata(&lock_path)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn owner_lock_replacement_poison_is_permanent_and_persistent_binding_rejects_child() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lock-replacement.sqlite");
        let lock_path = directory
            .path()
            .join("lock-replacement.sqlite.openlife-owner.lock");
        let displaced_lock = directory.path().join("displaced-owner.lock");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x73; 32]).unwrap();
        let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        let store_id = store
            .runtime_authority()
            .unwrap()
            .canonical_store_identity
            .to_string();
        let parent_epoch = store.runtime_authority().unwrap().process_epoch_id;
        store
            .lock_connection()
            .unwrap()
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        let before_bytes = std::fs::read(&path).unwrap();
        let before_metadata = std::fs::metadata(&path).unwrap();
        std::fs::rename(&lock_path, &displaced_lock).unwrap();
        std::fs::File::create(&lock_path).unwrap();

        let first_error = store
            .create_task_idempotent(&due_task(
                "lock-replacement-first",
                "lock-replacement-first-proposal",
            ))
            .unwrap_err()
            .to_string();
        assert!(
            first_error.contains("scheduled_task_store_owner_lock_identity_changed"),
            "{first_error}"
        );
        let child = run_scheduled_task_store_cross_process_writer_helper(
            &path,
            0x73,
            &store_id,
            parent_epoch,
            "expect_owner_lock_replacement_rejected",
        );
        assert!(
            child.status.success()
                && String::from_utf8_lossy(&child.stdout)
                    .contains("task_store_cross_process_probe:owner_lock_replacement_rejected"),
            "replacement lockfile created a second process owner\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );

        std::fs::remove_file(&lock_path).unwrap();
        std::fs::rename(&displaced_lock, &lock_path).unwrap();
        let second_error = store
            .create_task_idempotent(&due_task(
                "lock-replacement-second",
                "lock-replacement-second-proposal",
            ))
            .unwrap_err()
            .to_string();
        assert!(
            second_error.contains("scheduled_task_store_sqlite_slot_owner_poisoned"),
            "{second_error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before_bytes);
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before_metadata.modified().unwrap()
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode(),
            before_metadata.permissions().mode()
        );
    }

    #[cfg(unix)]
    #[test]
    fn scheduled_task_store_symlink_swap_during_open_fails_closed() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("slot-first.sqlite");
        let second = directory.path().join("slot-second.sqlite");
        let slot = directory.path().join("slot-link.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x5e; 32]).unwrap();
        for path in [&first, &second] {
            drop(TaskStore::new_with_authority_key(path, &authority_key).unwrap());
        }
        symlink(&first, &slot).unwrap();

        let (writable_conn, writable_observed, writable_lease) =
            open_task_store_database_with_stable_slot(
                &slot,
                || {
                    std::fs::remove_file(&slot).unwrap();
                    symlink(&second, &slot).unwrap();
                },
                || {
                    std::fs::remove_file(&slot).unwrap();
                    symlink(&first, &slot).unwrap();
                },
            )
            .expect("writable open must stay on its pre-resolved canonical slot");
        assert_eq!(writable_observed, std::fs::canonicalize(&first).unwrap());
        drop(writable_conn);
        drop(writable_lease);

        std::fs::remove_file(&slot).unwrap();
        symlink(&first, &slot).unwrap();
        let read_only_error = open_task_store_database_read_only_with_stable_slot(
            &slot,
            || {
                std::fs::remove_file(&slot).unwrap();
                symlink(&second, &slot).unwrap();
            },
            || {
                std::fs::remove_file(&slot).unwrap();
                symlink(&first, &slot).unwrap();
            },
        )
        .expect_err("read-only TaskStore open must reject a changed symlink slot")
        .to_string();
        assert!(
            read_only_error
                .contains("scheduled_task_store_database_slot_changed_during_read_only_open")
                || read_only_error
                    .contains("scheduled_task_store_read_only_database_identity_changed"),
            "{read_only_error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn post_preauth_symlink_swap_cannot_redirect_writable_open_or_touch_live_wal_family() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("preauth-first.sqlite");
        let second = directory.path().join("preauth-second.sqlite");
        let alias = directory.path().join("preauth-slot.sqlite");
        let second_lock = directory
            .path()
            .join("preauth-second.sqlite.openlife-owner.lock");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x47; 32]).unwrap();
        {
            let store = TaskStore::new_with_authority_key(&first, &authority_key).unwrap();
            store
                .create_task_idempotent(&due_task("preauth-first-task", "preauth-first-proposal"))
                .unwrap();
        }
        drop(TaskStore::new_with_authority_key(&second, &authority_key).unwrap());
        let second_wal_keeper = Connection::open(&second).unwrap();
        second_wal_keeper
            .execute_batch(
                "CREATE TABLE preauth_second_live_wal(value INTEGER);
                 INSERT INTO preauth_second_live_wal VALUES (1);",
            )
            .unwrap();
        assert!(PathBuf::from(format!("{}-wal", second.display())).exists());
        assert!(PathBuf::from(format!("{}-shm", second.display())).exists());
        symlink(&first, &alias).unwrap();
        let mut second_before = sqlite_family_states(&second);
        second_before.push((second_lock.clone(), exact_file_state(&second_lock)));

        let reopened = TaskStore::new_with_authority_key_and_open_hooks(
            &alias,
            &authority_key,
            || {
                std::fs::remove_file(&alias).unwrap();
                symlink(&second, &alias).unwrap();
            },
            || {},
        )
        .unwrap();
        let tasks = reopened.list_tasks(None).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "preauth-first-task");
        assert_file_states_unchanged(&second_before);
        drop(reopened);
        drop(second_wal_keeper);
    }

    #[cfg(unix)]
    #[test]
    fn same_path_database_replacement_never_inherits_the_original_store_authority() {
        let directory = tempfile::tempdir().unwrap();
        let slot = directory.path().join("scheduled-tasks.sqlite");
        let displaced = directory.path().join("scheduled-tasks-old-inode.sqlite");
        let replacement = directory.path().join("scheduled-tasks-copy.sqlite");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x39; 32]).unwrap();
        let first = TaskStore::new_with_authority_key(&slot, &authority_key).unwrap();
        first
            .create_task_idempotent(&due_task(
                "task-slot-replacement",
                "proposal-slot-replacement",
            ))
            .unwrap();
        first
            .lock_connection()
            .unwrap()
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        std::fs::copy(&slot, &replacement).unwrap();
        std::fs::rename(&slot, &displaced).unwrap();
        std::fs::rename(&replacement, &slot).unwrap();

        let second_error = TaskStore::new_with_authority_key(&slot, &authority_key)
            .err()
            .expect("replacement pathname must not create a second live TaskStore owner")
            .to_string();
        assert!(
            second_error.contains("scheduled_task_store_sqlite_slot_owner_lease_unavailable"),
            "{second_error}"
        );
        let first_error = first.list_tasks(None).unwrap_err().to_string();
        assert!(
            first_error.contains("scheduled_task_store_database_identity_changed"),
            "{first_error}"
        );

        drop(first);
        for suffix in ["-wal", "-shm"] {
            let sidecar = PathBuf::from(format!("{}{}", slot.display(), suffix));
            let _ = std::fs::remove_file(sidecar);
        }
        let lock_path = directory
            .path()
            .join("scheduled-tasks.sqlite.openlife-owner.lock");
        let mut before = sqlite_family_states(&slot);
        let lock_state = exact_file_state(&lock_path);
        before.push((lock_path, lock_state));
        let replacement_error = TaskStore::new_with_authority_key(&slot, &authority_key)
            .err()
            .expect("a replacement inode must remain unauthenticated after the old lease drops")
            .to_string();
        assert!(
            replacement_error.contains("task_store_owner_lock_authentication_failed"),
            "{replacement_error}"
        );
        assert_file_states_unchanged(&before);
    }

    #[test]
    fn pre_authority_pending_state_is_metadata_quarantined_before_slot_binding() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pre-authority-v12.db");
        let authority_key = TaskStoreAuthorityKey::from_key_material(&[0x72; 32]).unwrap();
        {
            let store = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
            store
                .create_task_idempotent(&due_task(
                    "pre-authority-pending",
                    "proposal-pre-authority-pending",
                ))
                .unwrap();
        }
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "DROP TRIGGER task_store_authority_metadata_immutable_update;
                 DROP TRIGGER task_store_authority_metadata_immutable_delete;
                 DELETE FROM task_store_metadata;
                 UPDATE openlife_schema_versions SET version = 12
                 WHERE component = 'task_store';",
            )
            .unwrap();
        }

        let rebound = TaskStore::new_with_authority_key(&path, &authority_key).unwrap();
        assert!(rebound.list_tasks(None).unwrap().is_empty());
        assert!(rebound
            .claim_next_due(
                chrono::Utc::now() + chrono::Duration::days(1),
                chrono::Duration::seconds(30),
            )
            .unwrap()
            .is_none());
        let conn = rebound.lock_connection().unwrap();
        let quarantined: (i64, String, Option<String>, String) = conn
            .query_row(
                "SELECT source_schema_version, task_status, attempt_status,
                        terminal_truth_digest
                 FROM legacy_task_store_truth_quarantine",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(quarantined.0, 12);
        assert_eq!(quarantined.1, "pending");
        assert!(quarantined.2.is_none());
        assert!(quarantined.3.starts_with("sha256:"));
    }
}
