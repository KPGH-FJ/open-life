use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zeroize::Zeroize;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use parking_lot::{ArcRwLockReadGuard, ArcRwLockWriteGuard, RawRwLock, RwLock};
use ring::{
    digest::{Context as DigestContext, SHA256},
    hkdf, hmac,
};

const MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION: i64 = 1;
const MCP_AUDIT_AUTHORITY_BINDING_VERSION: i64 = 2;
const MCP_AUDIT_AUTHORITY_VERIFIER_DOMAIN: &[u8] = b"openlife-mcp-audit-authority-verifier-v2";

#[cfg(test)]
std::thread_local! {
    static AUTHORITY_POST_BINDING_FAILURE_PATH: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
std::thread_local! {
    static REFERENCE_POST_READ_SWAP: std::cell::RefCell<Option<(PathBuf, PathBuf, PathBuf)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
std::thread_local! {
    static WRITE_AFTER_POISON_PRECHECK_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
struct WriteAfterPoisonPrecheckHookGuard;

#[cfg(test)]
impl Drop for WriteAfterPoisonPrecheckHookGuard {
    fn drop(&mut self) {
        WRITE_AFTER_POISON_PRECHECK_HOOK.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

#[cfg(test)]
fn inject_write_after_poison_precheck_hook(
    hook: impl FnOnce() + 'static,
) -> WriteAfterPoisonPrecheckHookGuard {
    WRITE_AFTER_POISON_PRECHECK_HOOK.with(|slot| {
        assert!(
            slot.borrow_mut().replace(Box::new(hook)).is_none(),
            "only one MCP audit write-precheck hook may be active on a test thread"
        );
    });
    WriteAfterPoisonPrecheckHookGuard
}

fn run_write_after_poison_precheck_hook() {
    #[cfg(test)]
    WRITE_AFTER_POISON_PRECHECK_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook();
        }
    });
}

#[cfg(test)]
struct ReferencePostReadSwapGuard;

#[cfg(test)]
impl Drop for ReferencePostReadSwapGuard {
    fn drop(&mut self) {
        REFERENCE_POST_READ_SWAP.with(|slot| {
            slot.replace(None);
        });
    }
}

#[cfg(test)]
fn inject_reference_post_read_swap(
    target: PathBuf,
    replacement: PathBuf,
    displaced: PathBuf,
) -> ReferencePostReadSwapGuard {
    REFERENCE_POST_READ_SWAP.with(|slot| {
        slot.replace(Some((target, replacement, displaced)));
    });
    ReferencePostReadSwapGuard
}

fn maybe_inject_reference_post_read_swap(path: &Path) -> Result<()> {
    #[cfg(test)]
    {
        let swap = REFERENCE_POST_READ_SWAP.with(|slot| slot.borrow().clone());
        if let Some((target, replacement, displaced)) = swap {
            if target == path {
                std::fs::rename(&target, &displaced)?;
                std::fs::rename(&replacement, &target)?;
            }
        }
    }
    #[cfg(not(test))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
struct AuthorityPostBindingFailureGuard {
    previous: Option<PathBuf>,
}

#[cfg(test)]
impl Drop for AuthorityPostBindingFailureGuard {
    fn drop(&mut self) {
        AUTHORITY_POST_BINDING_FAILURE_PATH.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

#[cfg(test)]
fn inject_authority_post_binding_failure(path: PathBuf) -> AuthorityPostBindingFailureGuard {
    let previous = AUTHORITY_POST_BINDING_FAILURE_PATH.with(|slot| slot.replace(Some(path)));
    AuthorityPostBindingFailureGuard { previous }
}

fn authority_post_binding_failure_injected(path: &Path) -> bool {
    #[cfg(test)]
    {
        return AUTHORITY_POST_BINDING_FAILURE_PATH.with(|slot| {
            slot.borrow()
                .as_deref()
                .is_some_and(|injected| injected == path)
        });
    }
    #[cfg(not(test))]
    {
        let _ = path;
        false
    }
}

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

impl Drop for AuditKeyMaterial {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

/// Non-secret inputs authenticated inside the exact retained MCP audit
/// database. The reference store owns the raw store identity; SQLite stores
/// only its digest plus a key-authenticated verifier.
#[derive(Debug, Clone, PartialEq, Eq)]
struct McpAuditAuthorityBinding {
    store_identity: String,
    canonical_slot_digest: String,
}

impl McpAuditAuthorityBinding {
    fn new(store_identity: impl Into<String>, canonical_slot_digest: impl Into<String>) -> Self {
        Self {
            store_identity: store_identity.into(),
            canonical_slot_digest: canonical_slot_digest.into(),
        }
    }
}

/// Startup state transition proven by the durable reference receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpAuditAuthorityTransition {
    VerifyActive {
        active_epoch: u64,
    },
    Prepare {
        transition_id: uuid::Uuid,
        previous_active_epoch: Option<u64>,
        pending_epoch: u64,
        origin: McpAuditDurableReferenceOrigin,
        secret_state: McpAuditDurableSecretState,
        pending_secret_digest: [u8; 32],
        database_state: McpAuditDurableDatabaseState,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpAuditDurableReferencePhase {
    Prepared,
    Active,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpAuditDurableDatabaseState {
    NotAttempted,
    Attempted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpAuditDurableReferenceOrigin {
    FreshCreate,
    ExistingStoreRotation,
    LegacyMigration,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpAuditDurableSecretState {
    Pending,
    Verified,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpAuditDurableReferenceDocument {
    version: u32,
    store_identity: String,
    canonical_slot_digest: String,
    phase: McpAuditDurableReferencePhase,
    origin: McpAuditDurableReferenceOrigin,
    transition_id: Option<String>,
    secret_state: McpAuditDurableSecretState,
    pending_secret_digest: Option<String>,
    database_state: McpAuditDurableDatabaseState,
    active_epoch: Option<u64>,
    pending_epoch: Option<u64>,
    keys: Vec<AuditKeyConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct McpAuditDurableReferenceWire {
    version: u32,
    store_identity: String,
    canonical_slot_digest: String,
    phase: McpAuditDurableReferencePhase,
    origin: McpAuditDurableReferenceOrigin,
    transition_id: Option<String>,
    secret_state: McpAuditDurableSecretState,
    pending_secret_digest: Option<String>,
    database_state: McpAuditDurableDatabaseState,
    active_epoch: Option<u64>,
    pending_epoch: Option<u64>,
    keys: Vec<AuditKeyConfig>,
}

impl<'de> Deserialize<'de> for McpAuditDurableReferenceDocument {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = McpAuditDurableReferenceWire::deserialize(deserializer)?;
        let document = Self {
            version: wire.version,
            store_identity: wire.store_identity,
            canonical_slot_digest: wire.canonical_slot_digest,
            phase: wire.phase,
            origin: wire.origin,
            transition_id: wire.transition_id,
            secret_state: wire.secret_state,
            pending_secret_digest: wire.pending_secret_digest,
            database_state: wire.database_state,
            active_epoch: wire.active_epoch,
            pending_epoch: wire.pending_epoch,
            keys: wire.keys,
        };
        document.validate().map_err(serde::de::Error::custom)?;
        Ok(document)
    }
}

fn valid_mcp_audit_secret_digest(value: &str) -> bool {
    parse_mcp_audit_secret_digest(value).is_some()
}

fn parse_mcp_audit_secret_digest(value: &str) -> Option<[u8; 32]> {
    let hex = value.strip_prefix("sha256:")?;
    if hex.len() != 64
        || !hex
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
    {
        return None;
    }
    let mut digest = [0u8; 32];
    for (index, output) in digest.iter_mut().enumerate() {
        let offset = index * 2;
        *output = u8::from_str_radix(&hex[offset..offset + 2], 16).ok()?;
    }
    Some(digest)
}

fn mcp_audit_secret_value_digest(key: &[u8; 32]) -> String {
    let encoded = zeroize::Zeroizing::new(general_purpose::STANDARD.encode(key));
    let mut digest = ring::digest::Context::new(&SHA256);
    digest.update(b"openlife-mcp-audit-secret-value-v1\0");
    digest.update(encoded.as_bytes());
    format!(
        "sha256:{}",
        digest
            .finish()
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

/// Opaque proof that a v2 reference document is presently visible at the
/// canonical reference path and is bound to the requested SQLite slot. Fields
/// are private, and the raw activation primitives are crate-private, so shipped
/// callers cannot authorize a writer from in-memory materials alone.
#[derive(Clone)]
pub struct McpAuditDurableReferenceReceipt {
    reference_path: PathBuf,
    file_identity: String,
    bytes_digest: [u8; 32],
    document: McpAuditDurableReferenceDocument,
    store_identity: String,
    canonical_slot_digest: String,
    pending_secret_digest: Option<String>,
    transition: McpAuditAuthorityTransition,
    keys: Vec<AuditKeyConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpAuditLegacyReferenceFormat {
    VersionedV1,
    UnversionedKeyring,
}

/// Exact bounded/no-follow receipt for a legacy predecessor.  Parsed configs
/// and the inode+bytes generation are sealed together by Core so a later
/// offline verifier cannot re-read a different file and call it the same
/// migration input.
#[derive(Clone)]
pub struct McpAuditLegacyReferenceReceipt {
    reference_path: PathBuf,
    file_identity: String,
    bytes_digest: [u8; 32],
    format: McpAuditLegacyReferenceFormat,
    store_identity: Option<String>,
    keys: Vec<AuditKeyConfig>,
}

impl std::fmt::Debug for McpAuditLegacyReferenceReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditLegacyReferenceReceipt")
            .field("reference_path", &self.reference_path)
            .field("format", &self.format)
            .field("store_identity", &self.store_identity)
            .field("key_count", &self.keys.len())
            .finish_non_exhaustive()
    }
}

impl McpAuditLegacyReferenceReceipt {
    pub fn format(&self) -> McpAuditLegacyReferenceFormat {
        self.format
    }

    pub fn store_identity(&self) -> Option<&str> {
        self.store_identity.as_deref()
    }

    pub fn keys(&self) -> &[AuditKeyConfig] {
        &self.keys
    }

    pub fn revalidate_visible(&self) -> Result<()> {
        let snapshot = read_bounded_reference_snapshot(&self.reference_path)?;
        let digest = ring::digest::digest(&SHA256, &snapshot.bytes);
        if snapshot.file_identity != self.file_identity || digest.as_ref() != self.bytes_digest {
            anyhow::bail!("mcp_audit_legacy_reference_changed_after_receipt");
        }
        Ok(())
    }
}

pub enum McpAuditLoadedReference {
    DurableV2(McpAuditDurableReferenceReceipt),
    Legacy(McpAuditLegacyReferenceReceipt),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct McpAuditLegacyVersionedReferenceWire {
    version: u32,
    store_identity: String,
    keys: Vec<AuditKeyConfig>,
}

impl std::fmt::Debug for McpAuditDurableReferenceReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditDurableReferenceReceipt")
            .field("reference_path", &self.reference_path)
            .field("store_identity", &self.store_identity)
            .field("canonical_slot_digest", &self.canonical_slot_digest)
            .field("transition", &self.transition)
            .field("key_count", &self.keys.len())
            .finish_non_exhaustive()
    }
}

/// One-shot authority to create the first canonical MCP audit database. It is
/// issued either from simultaneous reference+database absence, or from an
/// exact crash-recovery receipt whose key material matches the persisted
/// domain-separated secret digest while the DB transition is still
/// NotAttempted. `origin=fresh_create` by itself never grants this capability.
pub struct McpAuditFreshDatabaseCreationCapability {
    reservation: crate::sqlite_migration::SqliteSlotOwnerReservation,
    reference_path: PathBuf,
    provenance: McpAuditFreshDatabaseCreationProvenance,
}

/// One-shot authority for deleting only a crash-staged fresh reference whose
/// durable state proves that SQLite has not been attempted.  It owns the same
/// no-create slot reservation across the exact no-replace move to a unique
/// tombstone and can release that reservation only after canonical absence is
/// durable.
pub struct McpAuditFreshReferenceRollbackCapability {
    reservation: crate::sqlite_migration::SqliteSlotOwnerReservation,
    receipt: McpAuditDurableReferenceReceipt,
}

impl std::fmt::Debug for McpAuditFreshReferenceRollbackCapability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditFreshReferenceRollbackCapability")
            .field("reference_path", &self.receipt.reference_path)
            .finish_non_exhaustive()
    }
}

enum McpAuditFreshDatabaseCreationProvenance {
    InitialReferenceAbsence,
    PreparedRecovery {
        store_identity: String,
        canonical_slot_digest: String,
        transition_id: uuid::Uuid,
        pending_secret_digest: String,
        keys: Vec<AuditKeyConfig>,
    },
}

/// Private proof consumed only by authority-row authentication. It is minted
/// after the fresh capability and the currently visible sealed receipt have
/// both been validated, so a JSON origin claim cannot initialize SQLite.
struct McpAuditFreshAuthorityInitializationPermit {
    store_identity: String,
    canonical_slot_digest: String,
    transition_id: uuid::Uuid,
    pending_secret_digest: String,
    pending_epoch: u64,
}

impl std::fmt::Debug for McpAuditFreshDatabaseCreationCapability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditFreshDatabaseCreationCapability")
            .field("reference_path", &self.reference_path)
            .field(
                "provenance",
                &match &self.provenance {
                    McpAuditFreshDatabaseCreationProvenance::InitialReferenceAbsence => {
                        "initial_reference_absence"
                    }
                    McpAuditFreshDatabaseCreationProvenance::PreparedRecovery { .. } => {
                        "prepared_recovery"
                    }
                },
            )
            .finish_non_exhaustive()
    }
}

impl McpAuditFreshDatabaseCreationCapability {
    /// Canonical reference path covered by this one-shot capability. This is
    /// exposed only so the Tauri reference writer can type-check that an exact
    /// transition is guarded by the same retained owner capability.
    pub fn reference_path(&self) -> &Path {
        &self.reference_path
    }

    pub fn authorize_initial_reference_publish(
        &self,
        next: &McpAuditDurableReferenceDocument,
    ) -> Result<McpAuditReferenceMutationPermit<'_>> {
        if !matches!(
            self.provenance,
            McpAuditFreshDatabaseCreationProvenance::InitialReferenceAbsence
        ) {
            anyhow::bail!("mcp_audit_initial_publish_requires_absence_capability");
        }
        if next.phase() != McpAuditDurableReferencePhase::Prepared
            || next.origin() != McpAuditDurableReferenceOrigin::FreshCreate
            || next.secret_state() != McpAuditDurableSecretState::Pending
            || next.database_state() != McpAuditDurableDatabaseState::NotAttempted
            || next.active_epoch().is_some()
            || next.canonical_slot_digest() != self.reservation.canonical_slot_digest()?
        {
            anyhow::bail!("mcp_audit_initial_reference_document_not_authorized");
        }
        McpAuditReferenceMutationPermit::publish_absent(
            McpAuditReferenceMutationAuthority::Fresh(self),
            self.reference_path.clone(),
            next.clone(),
        )
    }

    pub fn authorize_reference_transition(
        &self,
        previous: &McpAuditDurableReferenceReceipt,
        next: &McpAuditDurableReferenceDocument,
    ) -> Result<McpAuditReferenceMutationPermit<'_>> {
        self.validate_reference_generation(previous)?;
        McpAuditReferenceMutationPermit::replace_exact(
            McpAuditReferenceMutationAuthority::Fresh(self),
            previous,
            next.clone(),
        )
    }

    pub fn authorize_pending_secret_effect(
        &self,
        receipt: &McpAuditDurableReferenceReceipt,
        config: &AuditKeyConfig,
        expected_digest: &str,
    ) -> Result<McpAuditPendingSecretEffectPermit<'_>> {
        self.validate_reference_generation(receipt)?;
        match receipt.transition {
            McpAuditAuthorityTransition::Prepare {
                pending_epoch,
                origin: McpAuditDurableReferenceOrigin::FreshCreate,
                secret_state: McpAuditDurableSecretState::Pending,
                database_state: McpAuditDurableDatabaseState::NotAttempted,
                ..
            } if pending_epoch == config.epoch => {}
            _ => anyhow::bail!("mcp_audit_fresh_pending_secret_effect_state_mismatch"),
        }
        let key_ref = config
            .key_ref
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_pending_secret_key_ref_missing"))?;
        if receipt
            .keys
            .last()
            .is_none_or(|durable| !same_key_config(durable, config))
            || receipt.pending_secret_digest.as_deref() != Some(expected_digest)
        {
            anyhow::bail!("mcp_audit_fresh_pending_secret_effect_plan_mismatch");
        }
        Ok(McpAuditPendingSecretEffectPermit {
            authority: McpAuditPendingSecretEffectAuthority::Fresh(self),
            receipt: receipt.clone(),
            epoch: config.epoch,
            key_ref: key_ref.to_string(),
            expected_digest: expected_digest.to_string(),
        })
    }

    fn validate_reference_generation(
        &self,
        receipt: &McpAuditDurableReferenceReceipt,
    ) -> Result<()> {
        if self.reservation.existing_database_len()?.is_some() {
            anyhow::bail!("mcp_audit_fresh_database_appeared_before_reference_effect");
        }
        if receipt.reference_path != self.reference_path
            || receipt.canonical_slot_digest != self.reservation.canonical_slot_digest()?
        {
            anyhow::bail!("mcp_audit_fresh_reference_capability_mismatch");
        }
        match &self.provenance {
            McpAuditFreshDatabaseCreationProvenance::InitialReferenceAbsence => {
                if receipt.document.origin() != McpAuditDurableReferenceOrigin::FreshCreate {
                    anyhow::bail!("mcp_audit_fresh_reference_origin_mismatch");
                }
            }
            McpAuditFreshDatabaseCreationProvenance::PreparedRecovery {
                store_identity,
                canonical_slot_digest,
                transition_id,
                pending_secret_digest,
                keys,
            } => {
                let receipt_transition_id = receipt
                    .document
                    .transition_id()
                    .and_then(|value| uuid::Uuid::parse_str(value).ok());
                if receipt.store_identity != *store_identity
                    || receipt.canonical_slot_digest != *canonical_slot_digest
                    || receipt_transition_id != Some(*transition_id)
                    || receipt.pending_secret_digest.as_deref()
                        != Some(pending_secret_digest.as_str())
                    || receipt.keys.len() != keys.len()
                    || receipt
                        .keys
                        .iter()
                        .zip(keys)
                        .any(|(left, right)| !same_key_config(left, right))
                {
                    anyhow::bail!("mcp_audit_fresh_recovery_generation_changed");
                }
            }
        }
        receipt.revalidate_visible()
    }

    fn validate_reference_effect(
        &self,
        permit: &McpAuditReferenceMutationPermit<'_>,
    ) -> Result<()> {
        if permit.reference_path != self.reference_path
            || self.reservation.existing_database_len()?.is_some()
        {
            anyhow::bail!("mcp_audit_fresh_reference_effect_authority_mismatch");
        }
        match permit.kind {
            McpAuditReferenceMutationKind::PublishAbsent => {
                if !matches!(
                    self.provenance,
                    McpAuditFreshDatabaseCreationProvenance::InitialReferenceAbsence
                ) {
                    anyhow::bail!("mcp_audit_initial_publish_requires_absence_capability");
                }
            }
            McpAuditReferenceMutationKind::ReplaceExact => {
                self.validate_reference_generation(
                    permit.previous.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("mcp_audit_reference_predecessor_missing")
                    })?,
                )?;
            }
            McpAuditReferenceMutationKind::DeleteExact => {
                anyhow::bail!("mcp_audit_fresh_creation_capability_cannot_delete_reference");
            }
        }
        Ok(())
    }
}

impl McpAuditFreshReferenceRollbackCapability {
    pub fn authorize_reference_delete(self) -> Result<McpAuditReferenceMutationPermit<'static>> {
        let receipt = self.receipt.clone();
        McpAuditReferenceMutationPermit::delete_exact(
            McpAuditReferenceMutationAuthority::FreshRollback(self),
            &receipt,
        )
    }

    fn validate_reference_effect(
        &self,
        permit: &McpAuditReferenceMutationPermit<'_>,
    ) -> Result<()> {
        if permit.kind != McpAuditReferenceMutationKind::DeleteExact
            || permit.reference_path != self.receipt.reference_path
            || self.reservation.existing_database_len()?.is_some()
        {
            anyhow::bail!("mcp_audit_fresh_rollback_authority_mismatch");
        }
        let previous = permit
            .previous
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_reference_predecessor_missing"))?;
        if previous.file_identity != self.receipt.file_identity
            || previous.bytes_digest != self.receipt.bytes_digest
        {
            anyhow::bail!("mcp_audit_fresh_rollback_generation_changed");
        }
        self.receipt.revalidate_visible()
    }
}

pub const MCP_AUDIT_DURABLE_REFERENCE_VERSION: u32 = 2;
const MCP_AUDIT_DURABLE_REFERENCE_MAX_BYTES: usize = 128 * 1024;
pub const MCP_AUDIT_STORE_KEY_REF_PREFIX: &str =
    "keychain://com.openlife.desktop/mcp-audit-key-store-";

fn same_key_config(left: &AuditKeyConfig, right: &AuditKeyConfig) -> bool {
    left.mode == right.mode
        && left.salt_b64 == right.salt_b64
        && left.env_var == right.env_var
        && left.key_ref == right.key_ref
        && left.epoch == right.epoch
        && left.created_at == right.created_at
}

impl std::fmt::Debug for McpAuditDurableReferenceDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditDurableReferenceDocument")
            .field("version", &self.version)
            .field("store_identity", &self.store_identity)
            .field("canonical_slot_digest", &self.canonical_slot_digest)
            .field("phase", &self.phase)
            .field("origin", &self.origin)
            .field("transition_id", &self.transition_id)
            .field("secret_state", &self.secret_state)
            .field("database_state", &self.database_state)
            .field("active_epoch", &self.active_epoch)
            .field("pending_epoch", &self.pending_epoch)
            .field("key_count", &self.keys.len())
            .finish_non_exhaustive()
    }
}

impl PartialEq for McpAuditDurableReferenceDocument {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.store_identity == other.store_identity
            && self.canonical_slot_digest == other.canonical_slot_digest
            && self.phase == other.phase
            && self.origin == other.origin
            && self.transition_id == other.transition_id
            && self.secret_state == other.secret_state
            && self.pending_secret_digest == other.pending_secret_digest
            && self.database_state == other.database_state
            && self.active_epoch == other.active_epoch
            && self.pending_epoch == other.pending_epoch
            && self.keys.len() == other.keys.len()
            && self
                .keys
                .iter()
                .zip(&other.keys)
                .all(|(left, right)| same_key_config(left, right))
    }
}

impl Eq for McpAuditDurableReferenceDocument {}

impl McpAuditDurableReferenceDocument {
    pub fn store_identity(&self) -> &str {
        &self.store_identity
    }

    pub fn canonical_slot_digest(&self) -> &str {
        &self.canonical_slot_digest
    }

    pub fn phase(&self) -> McpAuditDurableReferencePhase {
        self.phase
    }

    pub fn origin(&self) -> McpAuditDurableReferenceOrigin {
        self.origin
    }

    pub fn transition_id(&self) -> Option<&str> {
        self.transition_id.as_deref()
    }

    pub fn secret_state(&self) -> McpAuditDurableSecretState {
        self.secret_state
    }

    pub fn pending_secret_digest(&self) -> Option<&str> {
        self.pending_secret_digest.as_deref()
    }

    pub fn database_state(&self) -> McpAuditDurableDatabaseState {
        self.database_state
    }

    pub fn active_epoch(&self) -> Option<u64> {
        self.active_epoch
    }

    pub fn pending_epoch(&self) -> Option<u64> {
        self.pending_epoch
    }

    pub fn keys(&self) -> &[AuditKeyConfig] {
        &self.keys
    }

    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>> {
        self.authority_transition(None)?;
        serde_json::to_vec_pretty(self).context("encode canonical MCP audit durable reference v2")
    }

    pub fn prepared(
        store_identity: String,
        canonical_slot_digest: String,
        keys: Vec<AuditKeyConfig>,
        previous_active_epoch: Option<u64>,
        pending_epoch: u64,
        origin: McpAuditDurableReferenceOrigin,
        pending_secret_digest: String,
    ) -> std::result::Result<Self, String> {
        let document = Self {
            version: MCP_AUDIT_DURABLE_REFERENCE_VERSION,
            store_identity,
            canonical_slot_digest,
            phase: McpAuditDurableReferencePhase::Prepared,
            origin,
            transition_id: Some(uuid::Uuid::new_v4().to_string()),
            secret_state: McpAuditDurableSecretState::Pending,
            pending_secret_digest: Some(pending_secret_digest),
            database_state: McpAuditDurableDatabaseState::NotAttempted,
            active_epoch: previous_active_epoch,
            pending_epoch: Some(pending_epoch),
            keys,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn mark_active(&mut self) -> std::result::Result<(), String> {
        if self.phase != McpAuditDurableReferencePhase::Prepared {
            return Err("mcp_audit_reference_not_prepared".into());
        }
        if self.secret_state != McpAuditDurableSecretState::Verified
            || self.database_state != McpAuditDurableDatabaseState::Attempted
        {
            return Err("mcp_audit_reference_transition_not_sealed".into());
        }
        let pending_epoch = self
            .pending_epoch
            .ok_or_else(|| "mcp_audit_pending_epoch_missing".to_string())?;
        self.phase = McpAuditDurableReferencePhase::Active;
        self.active_epoch = Some(pending_epoch);
        self.pending_epoch = None;
        self.pending_secret_digest = None;
        self.transition_id = None;
        self.validate()
    }

    pub fn prepare_rotation(
        &mut self,
        config: AuditKeyConfig,
        pending_secret_digest: String,
    ) -> std::result::Result<(), String> {
        if self.phase != McpAuditDurableReferencePhase::Active {
            return Err("mcp_audit_rotation_requires_active_reference".into());
        }
        let previous_epoch = self
            .active_epoch
            .ok_or_else(|| "mcp_audit_active_epoch_missing".to_string())?;
        if config.epoch <= previous_epoch {
            return Err("mcp_audit_rotation_epoch_not_monotonic".into());
        }
        self.keys.push(config);
        self.phase = McpAuditDurableReferencePhase::Prepared;
        self.origin = McpAuditDurableReferenceOrigin::ExistingStoreRotation;
        self.transition_id = Some(uuid::Uuid::new_v4().to_string());
        self.secret_state = McpAuditDurableSecretState::Pending;
        self.pending_secret_digest = Some(pending_secret_digest);
        self.database_state = McpAuditDurableDatabaseState::NotAttempted;
        self.pending_epoch = self.keys.last().map(|config| config.epoch);
        self.validate()
    }

    pub fn prepared_transition_id(&self) -> std::result::Result<&str, String> {
        if self.phase != McpAuditDurableReferencePhase::Prepared {
            return Err("mcp_audit_reference_not_prepared".into());
        }
        self.transition_id
            .as_deref()
            .ok_or_else(|| "mcp_audit_prepared_transition_id_missing".into())
    }

    pub fn mark_secret_verified(&mut self) -> std::result::Result<(), String> {
        if self.phase != McpAuditDurableReferencePhase::Prepared
            || self.secret_state != McpAuditDurableSecretState::Pending
            || self.pending_secret_digest.is_none()
        {
            return Err("mcp_audit_pending_secret_transition_invalid".into());
        }
        self.secret_state = McpAuditDurableSecretState::Verified;
        self.validate()
    }

    pub fn mark_database_attempted(&mut self) -> std::result::Result<(), String> {
        if self.phase != McpAuditDurableReferencePhase::Prepared
            || self.secret_state != McpAuditDurableSecretState::Verified
            || self.database_state != McpAuditDurableDatabaseState::NotAttempted
        {
            return Err("mcp_audit_database_attempt_transition_invalid".into());
        }
        self.database_state = McpAuditDurableDatabaseState::Attempted;
        self.validate()
    }

    /// Structural validation used by orchestration before a path is known.
    /// It cannot authorize SQLite; store-bound authorization always calls the
    /// path-aware validator below through a durable receipt.
    pub fn validate(&self) -> std::result::Result<(), String> {
        self.authority_transition(None)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn validate_for_store(&self, db_path: &Path) -> Result<()> {
        self.authority_transition_for_store(db_path)?;
        Ok(())
    }

    fn authority_transition_for_store(
        &self,
        db_path: &Path,
    ) -> Result<McpAuditAuthorityTransition> {
        let expected_slot_digest =
            crate::sqlite_migration::canonical_sqlite_slot_digest(db_path, "mcp_audit_store")?;
        self.authority_transition(Some(&expected_slot_digest))
    }

    pub fn decode_canonical_for_store(bytes: &[u8], db_path: &Path) -> Result<Self> {
        let document: Self =
            serde_json::from_slice(bytes).context("decode MCP audit durable reference v2")?;
        let canonical = serde_json::to_vec_pretty(&document)
            .context("canonicalize MCP audit durable reference v2")?;
        if canonical != bytes {
            anyhow::bail!("mcp_audit_reference_noncanonical_or_ambiguous_json");
        }
        document.validate_for_store(db_path)?;
        Ok(document)
    }

    pub fn allows_transition_to(&self, next: &Self) -> bool {
        if self.version != next.version
            || self.store_identity != next.store_identity
            || self.canonical_slot_digest != next.canonical_slot_digest
        {
            return false;
        }
        match (self.phase, next.phase) {
            (McpAuditDurableReferencePhase::Active, McpAuditDurableReferencePhase::Prepared) => {
                next.origin == McpAuditDurableReferenceOrigin::ExistingStoreRotation
                    && self.transition_id.is_none()
                    && next.transition_id.is_some()
                    && next.secret_state == McpAuditDurableSecretState::Pending
                    && next.database_state == McpAuditDurableDatabaseState::NotAttempted
                    && next.active_epoch == self.active_epoch
                    && next.pending_epoch == next.keys.last().map(|config| config.epoch)
                    && next.keys.len() == self.keys.len().saturating_add(1)
                    && self
                        .keys
                        .iter()
                        .zip(&next.keys)
                        .all(|(left, right)| same_key_config(left, right))
            }
            (McpAuditDurableReferencePhase::Prepared, McpAuditDurableReferencePhase::Prepared) => {
                self.origin == next.origin
                    && self.transition_id == next.transition_id
                    && self.pending_secret_digest == next.pending_secret_digest
                    && self.active_epoch == next.active_epoch
                    && self.pending_epoch == next.pending_epoch
                    && self.keys.len() == next.keys.len()
                    && self
                        .keys
                        .iter()
                        .zip(&next.keys)
                        .all(|(left, right)| same_key_config(left, right))
                    && matches!(
                        (
                            self.secret_state,
                            self.database_state,
                            next.secret_state,
                            next.database_state,
                        ),
                        (
                            McpAuditDurableSecretState::Pending,
                            McpAuditDurableDatabaseState::NotAttempted,
                            McpAuditDurableSecretState::Verified,
                            McpAuditDurableDatabaseState::NotAttempted,
                        ) | (
                            McpAuditDurableSecretState::Verified,
                            McpAuditDurableDatabaseState::NotAttempted,
                            McpAuditDurableSecretState::Verified,
                            McpAuditDurableDatabaseState::Attempted,
                        )
                    )
            }
            (McpAuditDurableReferencePhase::Prepared, McpAuditDurableReferencePhase::Active) => {
                self.origin == next.origin
                    && self.transition_id.is_some()
                    && next.transition_id.is_none()
                    && self.secret_state == McpAuditDurableSecretState::Verified
                    && self.database_state == McpAuditDurableDatabaseState::Attempted
                    && next.secret_state == McpAuditDurableSecretState::Verified
                    && next.database_state == McpAuditDurableDatabaseState::Attempted
                    && next.active_epoch == self.pending_epoch
                    && next.pending_epoch.is_none()
                    && next.pending_secret_digest.is_none()
                    && self.keys.len() == next.keys.len()
                    && self
                        .keys
                        .iter()
                        .zip(&next.keys)
                        .all(|(left, right)| same_key_config(left, right))
            }
            _ => false,
        }
    }

    fn authority_transition(
        &self,
        expected_slot_digest: Option<&str>,
    ) -> Result<McpAuditAuthorityTransition> {
        if self.version != MCP_AUDIT_DURABLE_REFERENCE_VERSION {
            anyhow::bail!("mcp_audit_reference_version_unsupported");
        }
        let identity = uuid::Uuid::parse_str(&self.store_identity)
            .context("parse MCP audit durable reference identity")?;
        if identity.is_nil() || identity.get_version_num() != 4 {
            anyhow::bail!("mcp_audit_store_identity_not_random_v4");
        }
        if self.canonical_slot_digest.len() != "sha256:".len() + 64
            || !self.canonical_slot_digest.starts_with("sha256:")
            || !self.canonical_slot_digest["sha256:".len()..]
                .chars()
                .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
        {
            anyhow::bail!("mcp_audit_canonical_slot_digest_invalid");
        }
        if expected_slot_digest.is_some_and(|expected| self.canonical_slot_digest != expected) {
            anyhow::bail!("mcp_audit_reference_canonical_slot_mismatch");
        }
        if self.keys.is_empty() {
            anyhow::bail!("mcp_audit_reference_keyring_empty");
        }
        for pair in self.keys.windows(2) {
            if pair[0].epoch >= pair[1].epoch {
                anyhow::bail!("mcp_audit_reference_epochs_not_strictly_increasing");
            }
        }
        for config in &self.keys {
            if config.mode == KeyMode::Keychain
                && config
                    .key_ref
                    .as_deref()
                    .map(str::trim)
                    .is_none_or(str::is_empty)
            {
                anyhow::bail!("mcp_audit_reference_keychain_ref_missing");
            }
        }
        let last_epoch = self.keys.last().expect("non-empty").epoch;
        let (bound_epoch, transition) = match self.phase {
            McpAuditDurableReferencePhase::Prepared => {
                let transition_id = self
                    .transition_id
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("mcp_audit_prepared_transition_id_missing"))?;
                let transition_id = uuid::Uuid::parse_str(transition_id)
                    .context("parse MCP audit prepared transition id")?;
                if transition_id.is_nil() || transition_id.get_version_num() != 4 {
                    anyhow::bail!("mcp_audit_prepared_transition_id_not_random_v4");
                }
                let pending_digest = self
                    .pending_secret_digest
                    .as_deref()
                    .and_then(parse_mcp_audit_secret_digest)
                    .ok_or_else(|| anyhow::anyhow!("mcp_audit_pending_secret_digest_invalid"))?;
                if self.secret_state == McpAuditDurableSecretState::Pending
                    && self.database_state != McpAuditDurableDatabaseState::NotAttempted
                {
                    anyhow::bail!("mcp_audit_pending_secret_database_state_invalid");
                }
                let pending_epoch = self
                    .pending_epoch
                    .ok_or_else(|| anyhow::anyhow!("mcp_audit_pending_epoch_missing"))?;
                if pending_epoch != last_epoch
                    || self.active_epoch.is_some_and(|active| {
                        active >= pending_epoch || !self.keys.iter().any(|key| key.epoch == active)
                    })
                {
                    anyhow::bail!("mcp_audit_prepared_epoch_state_invalid");
                }
                match self.origin {
                    McpAuditDurableReferenceOrigin::FreshCreate if self.active_epoch.is_some() => {
                        anyhow::bail!("mcp_audit_fresh_reference_has_previous_epoch");
                    }
                    McpAuditDurableReferenceOrigin::ExistingStoreRotation
                        if self.active_epoch.is_none() =>
                    {
                        anyhow::bail!("mcp_audit_rotation_reference_previous_epoch_missing");
                    }
                    McpAuditDurableReferenceOrigin::FreshCreate
                    | McpAuditDurableReferenceOrigin::ExistingStoreRotation
                    | McpAuditDurableReferenceOrigin::LegacyMigration => {}
                }
                (
                    pending_epoch,
                    McpAuditAuthorityTransition::Prepare {
                        transition_id,
                        previous_active_epoch: self.active_epoch,
                        pending_epoch,
                        origin: self.origin,
                        secret_state: self.secret_state,
                        pending_secret_digest: pending_digest,
                        database_state: self.database_state,
                    },
                )
            }
            McpAuditDurableReferencePhase::Active => {
                if self.transition_id.is_some()
                    || self.pending_epoch.is_some()
                    || self.active_epoch != Some(last_epoch)
                    || self.secret_state != McpAuditDurableSecretState::Verified
                    || self.pending_secret_digest.is_some()
                    || self.database_state != McpAuditDurableDatabaseState::Attempted
                {
                    anyhow::bail!("mcp_audit_active_epoch_state_invalid");
                }
                (
                    last_epoch,
                    McpAuditAuthorityTransition::VerifyActive {
                        active_epoch: last_epoch,
                    },
                )
            }
        };
        let expected_ref = format!(
            "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-{bound_epoch}",
            identity.simple()
        );
        let bound = self
            .keys
            .iter()
            .find(|key| key.epoch == bound_epoch)
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_bound_epoch_config_missing"))?;
        if bound.mode != KeyMode::Keychain
            || bound.key_ref.as_deref() != Some(expected_ref.as_str())
        {
            anyhow::bail!("mcp_audit_bound_epoch_reference_mismatch");
        }
        Ok(transition)
    }
}

fn canonical_reference_path(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("mcp_audit_reference_file_name_missing"))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok(std::fs::canonicalize(parent)
        .context("canonicalize MCP audit reference parent")?
        .join(file_name))
}

fn configure_reference_no_follow(options: &mut OpenOptions) {
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

fn reference_file_identity(file: &std::fs::File, path: &Path) -> Result<String> {
    let metadata = file
        .metadata()
        .with_context(|| format!("read MCP audit reference metadata at {}", path.display()))?;
    if !metadata.is_file() {
        anyhow::bail!("mcp_audit_reference_not_regular_file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            anyhow::bail!("mcp_audit_reference_link_count_invalid");
        }
        return Ok(format!(
            "unix:{}:{}:{}",
            metadata.dev(),
            metadata.ino(),
            metadata.nlink()
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            anyhow::bail!("mcp_audit_reference_reparse_point_rejected");
        }
        let information = winapi_util::file::information(file)?;
        if information.number_of_links() != 1 {
            anyhow::bail!("mcp_audit_reference_link_count_invalid");
        }
        return Ok(format!(
            "windows:{}:{}:{}",
            information.volume_serial_number(),
            information.file_index(),
            information.number_of_links()
        ));
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = metadata;
        anyhow::bail!("mcp_audit_reference_stable_identity_unsupported");
    }
}

#[derive(Debug)]
pub struct McpAuditReferenceReadSnapshot {
    bytes: Vec<u8>,
    file_identity: String,
}

impl McpAuditReferenceReadSnapshot {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn file_identity(&self) -> &str {
        &self.file_identity
    }
}

fn read_bounded_reference_snapshot(path: &Path) -> Result<McpAuditReferenceReadSnapshot> {
    let path_metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("inspect MCP audit reference at {}", path.display()))?;
    if path_metadata.file_type().is_symlink() {
        anyhow::bail!("mcp_audit_reference_symlink_rejected");
    }
    let mut options = OpenOptions::new();
    options.read(true);
    configure_reference_no_follow(&mut options);
    let file = options
        .open(path)
        .with_context(|| format!("open MCP audit reference at {}", path.display()))?;
    let opened_identity = reference_file_identity(&file, path)?;
    if file.metadata()?.len() > MCP_AUDIT_DURABLE_REFERENCE_MAX_BYTES as u64 {
        anyhow::bail!("mcp_audit_reference_too_large");
    }
    let mut bytes = Vec::new();
    file.try_clone()?
        .take(MCP_AUDIT_DURABLE_REFERENCE_MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MCP_AUDIT_DURABLE_REFERENCE_MAX_BYTES {
        anyhow::bail!("mcp_audit_reference_too_large");
    }
    maybe_inject_reference_post_read_swap(path)?;
    let mut reopened_options = OpenOptions::new();
    reopened_options.read(true);
    configure_reference_no_follow(&mut reopened_options);
    let reopened = reopened_options.open(path)?;
    if reference_file_identity(&file, path)? != opened_identity
        || reference_file_identity(&reopened, path)? != opened_identity
    {
        anyhow::bail!("mcp_audit_reference_identity_changed_during_read");
    }
    Ok(McpAuditReferenceReadSnapshot {
        bytes,
        file_identity: opened_identity,
    })
}

/// Shared no-follow, bounded, stable-identity reader for every MCP audit
/// reference generation (legacy, v1, and v2). Callers classify the bytes only
/// after this primitive succeeds, so oversized or retargeted inputs are never
/// decoded by a weaker legacy path.
pub fn read_bounded_mcp_audit_reference(path: &Path) -> Result<Vec<u8>> {
    Ok(read_bounded_mcp_audit_reference_snapshot(path)?.bytes)
}

pub fn read_bounded_mcp_audit_reference_snapshot(
    path: &Path,
) -> Result<McpAuditReferenceReadSnapshot> {
    let canonical = canonical_reference_path(path)?;
    read_bounded_reference_snapshot(&canonical)
}

fn validate_legacy_reference_keys(keys: &[AuditKeyConfig]) -> Result<()> {
    if keys.is_empty() {
        anyhow::bail!("mcp_audit_key_reference_set_empty");
    }
    for pair in keys.windows(2) {
        if pair[0].epoch >= pair[1].epoch {
            anyhow::bail!("mcp_audit_key_epoch_not_strictly_increasing");
        }
    }
    for key in keys {
        if key.mode == KeyMode::Keychain
            && key
                .key_ref
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty)
        {
            anyhow::bail!("mcp_audit_keychain_reference_missing");
        }
    }
    Ok(())
}

/// One bounded/no-follow read and one Core-owned classifier for v2 and both
/// legacy shapes.  The returned legacy receipt preserves the exact inode and
/// byte digest that were parsed; no later cutover stage may reconstruct that
/// proof from a second read.
pub fn load_mcp_audit_reference_for_store(
    reference_path: &Path,
    db_path: &Path,
) -> Result<McpAuditLoadedReference> {
    let reference_path = canonical_reference_path(reference_path)?;
    let canonical_database =
        crate::sqlite_migration::canonical_sqlite_slot(db_path, "mcp_audit_store")?;
    if reference_path != canonical_database.with_file_name("mcp_audit_keys.json") {
        anyhow::bail!("mcp_audit_reference_canonical_owner_path_mismatch");
    }
    let snapshot = read_bounded_reference_snapshot(&reference_path)?;
    if let Ok(document) =
        McpAuditDurableReferenceDocument::decode_canonical_for_store(&snapshot.bytes, db_path)
    {
        return Ok(McpAuditLoadedReference::DurableV2(
            McpAuditDurableReferenceReceipt::from_snapshot(
                reference_path,
                db_path,
                snapshot,
                document,
            )?,
        ));
    }

    let (format, store_identity, keys) = if let Ok(document) =
        serde_json::from_slice::<McpAuditLegacyVersionedReferenceWire>(&snapshot.bytes)
    {
        if document.version != 1 {
            anyhow::bail!("mcp_audit_legacy_reference_version_unsupported");
        }
        let identity = uuid::Uuid::parse_str(&document.store_identity)
            .context("parse MCP audit legacy store identity")?;
        if identity.is_nil() || identity.get_version_num() != 4 {
            anyhow::bail!("mcp_audit_legacy_store_identity_not_random_v4");
        }
        (
            McpAuditLegacyReferenceFormat::VersionedV1,
            Some(document.store_identity),
            document.keys,
        )
    } else {
        let keys = serde_json::from_slice::<Vec<AuditKeyConfig>>(&snapshot.bytes)
            .context("mcp_audit_reference_decode_failed")?;
        (
            McpAuditLegacyReferenceFormat::UnversionedKeyring,
            None,
            keys,
        )
    };
    validate_legacy_reference_keys(&keys)?;
    let digest = ring::digest::digest(&SHA256, &snapshot.bytes);
    let mut bytes_digest = [0u8; 32];
    bytes_digest.copy_from_slice(digest.as_ref());
    Ok(McpAuditLoadedReference::Legacy(
        McpAuditLegacyReferenceReceipt {
            reference_path,
            file_identity: snapshot.file_identity,
            bytes_digest,
            format,
            store_identity,
            keys,
        },
    ))
}

impl McpAuditDurableReferenceReceipt {
    pub fn load_for_store(reference_path: &Path, db_path: &Path) -> Result<Self> {
        let reference_path = canonical_reference_path(reference_path)?;
        let canonical_database =
            crate::sqlite_migration::canonical_sqlite_slot(db_path, "mcp_audit_store")?;
        let expected_reference = canonical_database.with_file_name("mcp_audit_keys.json");
        if reference_path != expected_reference {
            anyhow::bail!("mcp_audit_reference_canonical_owner_path_mismatch");
        }
        let snapshot = read_bounded_reference_snapshot(&reference_path)?;
        let document =
            McpAuditDurableReferenceDocument::decode_canonical_for_store(&snapshot.bytes, db_path)?;
        Self::from_snapshot(reference_path, db_path, snapshot, document)
    }

    fn from_snapshot(
        reference_path: PathBuf,
        db_path: &Path,
        snapshot: McpAuditReferenceReadSnapshot,
        document: McpAuditDurableReferenceDocument,
    ) -> Result<Self> {
        let transition = document.authority_transition_for_store(db_path)?;
        let digest = ring::digest::digest(&SHA256, &snapshot.bytes);
        let mut bytes_digest = [0u8; 32];
        bytes_digest.copy_from_slice(digest.as_ref());
        Ok(Self {
            reference_path,
            file_identity: snapshot.file_identity,
            bytes_digest,
            document: document.clone(),
            store_identity: document.store_identity.clone(),
            canonical_slot_digest: document.canonical_slot_digest.clone(),
            pending_secret_digest: document.pending_secret_digest.clone(),
            transition,
            keys: document.keys.clone(),
        })
    }

    pub fn document(&self) -> &McpAuditDurableReferenceDocument {
        &self.document
    }

    pub fn reference_path(&self) -> &Path {
        &self.reference_path
    }

    pub fn revalidate_visible(&self) -> Result<()> {
        let snapshot = read_bounded_reference_snapshot(&self.reference_path)?;
        let digest = ring::digest::digest(&SHA256, &snapshot.bytes);
        if snapshot.file_identity != self.file_identity || digest.as_ref() != self.bytes_digest {
            anyhow::bail!("mcp_audit_reference_changed_after_receipt");
        }
        Ok(())
    }

    fn validate_materials(&self, materials: &[AuditKeyMaterial]) -> Result<()> {
        if materials.len() != self.keys.len()
            || materials
                .iter()
                .zip(&self.keys)
                .any(|(material, durable)| !same_key_config(&material.config, durable))
        {
            anyhow::bail!("mcp_audit_reference_material_keyring_mismatch");
        }
        Ok(())
    }

    fn validate_pending_secret_material(&self, materials: &[AuditKeyMaterial]) -> Result<()> {
        if let McpAuditAuthorityTransition::Prepare {
            pending_secret_digest,
            ..
        } = self.transition
        {
            let pending = materials
                .last()
                .ok_or_else(|| anyhow::anyhow!("mcp_audit_pending_key_unavailable"))?;
            let observed = mcp_audit_secret_value_digest(&pending.key);
            if parse_mcp_audit_secret_digest(&observed) != Some(pending_secret_digest)
                || self.pending_secret_digest.as_deref() != Some(observed.as_str())
            {
                anyhow::bail!("mcp_audit_prepared_secret_material_digest_mismatch");
            }
        }
        Ok(())
    }

    fn require_database_transition_sealed(&self) -> Result<()> {
        match self.transition {
            McpAuditAuthorityTransition::VerifyActive { .. }
            | McpAuditAuthorityTransition::Prepare {
                secret_state: McpAuditDurableSecretState::Verified,
                database_state: McpAuditDurableDatabaseState::Attempted,
                ..
            } => Ok(()),
            McpAuditAuthorityTransition::Prepare { .. } => {
                anyhow::bail!("mcp_audit_prepared_reference_not_sealed_for_database_transition")
            }
        }
    }

    fn is_fresh_create_transition(&self) -> bool {
        matches!(
            self.transition,
            McpAuditAuthorityTransition::Prepare {
                origin: McpAuditDurableReferenceOrigin::FreshCreate,
                secret_state: McpAuditDurableSecretState::Verified,
                database_state: McpAuditDurableDatabaseState::Attempted,
                previous_active_epoch: None,
                ..
            }
        )
    }

    fn into_proof(self) -> McpAuditDurableReferenceProof {
        McpAuditDurableReferenceProof {
            reference_path: self.reference_path,
            file_identity: self.file_identity,
            bytes_digest: self.bytes_digest,
            store_identity: self.store_identity,
            canonical_slot_digest: self.canonical_slot_digest,
            document: self.document,
            transition: self.transition,
            keys: self.keys,
        }
    }
}

/// Commit-aware failure from the only shipped MCP-audit reference mutation
/// edge. `VisibleDurabilityUnknown` is intentionally sticky: callers must
/// reconcile the canonical file instead of retrying a possibly committed
/// transition.
#[derive(Debug)]
pub struct McpAuditReferenceMutationError {
    commit_state: crate::atomic_file::AtomicWriteCommitState,
    detail: String,
}

impl McpAuditReferenceMutationError {
    pub fn commit_state(&self) -> crate::atomic_file::AtomicWriteCommitState {
        self.commit_state
    }

    pub fn precommit_rejected(detail: impl Into<String>) -> Self {
        Self {
            commit_state: crate::atomic_file::AtomicWriteCommitState::NotCommitted,
            detail: detail.into(),
        }
    }

    fn outcome_unknown(detail: impl Into<String>) -> Self {
        Self {
            commit_state: crate::atomic_file::AtomicWriteCommitState::VisibleDurabilityUnknown,
            detail: detail.into(),
        }
    }
}

impl From<crate::atomic_file::AtomicWriteError> for McpAuditReferenceMutationError {
    fn from(error: crate::atomic_file::AtomicWriteError) -> Self {
        Self {
            commit_state: error.commit_state(),
            detail: error.to_string(),
        }
    }
}

impl std::fmt::Display for McpAuditReferenceMutationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "mcp_audit_reference_{:?}:{}",
            self.commit_state, self.detail
        )
    }
}

impl std::error::Error for McpAuditReferenceMutationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpAuditReferenceMutationKind {
    PublishAbsent,
    ReplaceExact,
    DeleteExact,
}

/// Core-minted capability for one exact reference effect.  Its fields and
/// constructors are private.  The retained authority guard lives inside (or
/// is borrowed by) the permit for the whole filesystem operation, so shipped
/// Tauri code cannot separate a policy check from the effect edge.
pub struct McpAuditReferenceMutationPermit<'a> {
    authority: McpAuditReferenceMutationAuthority<'a>,
    kind: McpAuditReferenceMutationKind,
    reference_path: PathBuf,
    database_path: PathBuf,
    previous: Option<McpAuditDurableReferenceReceipt>,
    next: Option<McpAuditDurableReferenceDocument>,
    next_bytes: Option<Vec<u8>>,
}

enum McpAuditReferenceMutationAuthority<'a> {
    Fresh(&'a McpAuditFreshDatabaseCreationCapability),
    StableStore {
        store: &'a McpAuditStore,
        _guard: ArcRwLockReadGuard<RawRwLock, McpAuditDurableReferenceAuthority>,
    },
    Rotation(&'a McpAuditRotationTransition),
    FreshRollback(McpAuditFreshReferenceRollbackCapability),
    #[cfg(any(test, feature = "test-utils"))]
    TestOnly,
}

impl std::fmt::Debug for McpAuditReferenceMutationPermit<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditReferenceMutationPermit")
            .field("kind", &self.kind)
            .field("reference_path", &self.reference_path)
            .field("has_previous", &self.previous.is_some())
            .field("has_next", &self.next.is_some())
            .finish_non_exhaustive()
    }
}

impl<'a> McpAuditReferenceMutationPermit<'a> {
    fn publish_absent(
        authority: McpAuditReferenceMutationAuthority<'a>,
        reference_path: PathBuf,
        next: McpAuditDurableReferenceDocument,
    ) -> Result<Self> {
        let next_bytes = next.to_canonical_bytes()?;
        Ok(Self {
            database_path: reference_path.with_file_name("mcp_audit.db"),
            authority,
            kind: McpAuditReferenceMutationKind::PublishAbsent,
            reference_path,
            previous: None,
            next: Some(next),
            next_bytes: Some(next_bytes),
        })
    }

    fn replace_exact(
        authority: McpAuditReferenceMutationAuthority<'a>,
        previous: &McpAuditDurableReferenceReceipt,
        next: McpAuditDurableReferenceDocument,
    ) -> Result<Self> {
        previous.revalidate_visible()?;
        if !previous.document.allows_transition_to(&next) {
            anyhow::bail!("mcp_audit_reference_transition_not_monotonic");
        }
        let next_bytes = next.to_canonical_bytes()?;
        Ok(Self {
            database_path: previous.reference_path.with_file_name("mcp_audit.db"),
            authority,
            kind: McpAuditReferenceMutationKind::ReplaceExact,
            reference_path: previous.reference_path.clone(),
            previous: Some(previous.clone()),
            next: Some(next),
            next_bytes: Some(next_bytes),
        })
    }

    fn delete_exact(
        authority: McpAuditReferenceMutationAuthority<'a>,
        previous: &McpAuditDurableReferenceReceipt,
    ) -> Result<Self> {
        previous.revalidate_visible()?;
        Ok(Self {
            database_path: previous.reference_path.with_file_name("mcp_audit.db"),
            authority,
            kind: McpAuditReferenceMutationKind::DeleteExact,
            reference_path: previous.reference_path.clone(),
            previous: Some(previous.clone()),
            next: None,
            next_bytes: None,
        })
    }

    fn validate_authority_at_effect_edge(&self) -> Result<()> {
        match &self.authority {
            McpAuditReferenceMutationAuthority::Fresh(capability) => {
                capability.validate_reference_effect(self)?
            }
            McpAuditReferenceMutationAuthority::StableStore { store, _guard } => {
                store.validate_reference_effect(self)?
            }
            McpAuditReferenceMutationAuthority::Rotation(transition) => {
                transition.validate_reference_effect(self)?
            }
            McpAuditReferenceMutationAuthority::FreshRollback(capability) => {
                capability.validate_reference_effect(self)?
            }
            #[cfg(any(test, feature = "test-utils"))]
            McpAuditReferenceMutationAuthority::TestOnly => {}
        }
        Ok(())
    }

    fn validate_common_at_effect_edge(&self) -> Result<()> {
        let canonical = canonical_reference_path(&self.reference_path)?;
        if canonical != self.reference_path
            || self.database_path != self.reference_path.with_file_name("mcp_audit.db")
        {
            anyhow::bail!("mcp_audit_reference_mutation_path_changed");
        }
        match self.kind {
            McpAuditReferenceMutationKind::PublishAbsent => {
                if self.previous.is_some() || self.next.is_none() || self.next_bytes.is_none() {
                    anyhow::bail!("mcp_audit_reference_publish_permit_invalid");
                }
                match std::fs::symlink_metadata(&self.reference_path) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Ok(_) => anyhow::bail!("mcp_audit_fresh_reference_slot_not_absent"),
                    Err(error) => return Err(error.into()),
                }
            }
            McpAuditReferenceMutationKind::ReplaceExact => {
                let previous = self
                    .previous
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("mcp_audit_reference_predecessor_missing"))?;
                let next = self
                    .next
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("mcp_audit_reference_successor_missing"))?;
                previous.revalidate_visible()?;
                if !previous.document.allows_transition_to(next) {
                    anyhow::bail!("mcp_audit_reference_transition_not_monotonic");
                }
            }
            McpAuditReferenceMutationKind::DeleteExact => {
                self.previous
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("mcp_audit_reference_predecessor_missing"))?
                    .revalidate_visible()?;
                if self.next.is_some() || self.next_bytes.is_some() {
                    anyhow::bail!("mcp_audit_reference_delete_permit_invalid");
                }
            }
        }
        self.validate_authority_at_effect_edge()
    }

    pub fn commit_write(
        self,
    ) -> std::result::Result<McpAuditDurableReferenceReceipt, McpAuditReferenceMutationError> {
        self.validate_common_at_effect_edge().map_err(|error| {
            McpAuditReferenceMutationError::precommit_rejected(error.to_string())
        })?;
        let bytes = self.next_bytes.as_deref().ok_or_else(|| {
            McpAuditReferenceMutationError::precommit_rejected("write bytes missing")
        })?;
        match self.kind {
            McpAuditReferenceMutationKind::PublishAbsent => {
                crate::atomic_file::write_atomic_create_new_commit_aware(
                    &self.reference_path,
                    bytes,
                )?;
            }
            McpAuditReferenceMutationKind::ReplaceExact => {
                crate::atomic_file::write_atomic_commit_aware(&self.reference_path, bytes)?;
            }
            McpAuditReferenceMutationKind::DeleteExact => {
                return Err(McpAuditReferenceMutationError::precommit_rejected(
                    "delete permit used for write",
                ));
            }
        }
        let receipt = McpAuditDurableReferenceReceipt::load_for_store(
            &self.reference_path,
            &self.database_path,
        )
        .map_err(|error| {
            McpAuditReferenceMutationError::outcome_unknown(format!(
                "mcp_audit_reference_post_write_recheck_failed:{error}"
            ))
        })?;
        if receipt.document() != self.next.as_ref().expect("write permit has next document") {
            return Err(McpAuditReferenceMutationError::outcome_unknown(
                "mcp_audit_reference_post_write_document_mismatch",
            ));
        }
        Ok(receipt)
    }

    pub fn commit_delete(
        self,
    ) -> std::result::Result<
        crate::sqlite_migration::SqliteSlotOwnerReservation,
        McpAuditReferenceMutationError,
    > {
        self.validate_common_at_effect_edge().map_err(|error| {
            McpAuditReferenceMutationError::precommit_rejected(error.to_string())
        })?;
        if self.kind != McpAuditReferenceMutationKind::DeleteExact {
            return Err(McpAuditReferenceMutationError::precommit_rejected(
                "write permit used for delete",
            ));
        }
        let previous = self
            .previous
            .as_ref()
            .expect("delete permit has predecessor");
        let parent = self
            .reference_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let file_name = self
            .reference_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("mcp_audit_keys.json");
        let displaced = self
            .reference_path
            .with_file_name(format!(".{file_name}.rollback-{}", uuid::Uuid::new_v4()));
        crate::atomic_file::rename_no_replace(&self.reference_path, &displaced).map_err(
            |error| {
                McpAuditReferenceMutationError::precommit_rejected(format!(
                    "mcp_audit_reference_exact_rollback_rename_failed:{error}"
                ))
            },
        )?;
        let snapshot = read_bounded_reference_snapshot(&displaced).map_err(|error| {
            McpAuditReferenceMutationError::outcome_unknown(format!(
                "rollback_receipt_recheck_failed:{error}"
            ))
        })?;
        let digest = ring::digest::digest(&SHA256, &snapshot.bytes);
        if snapshot.file_identity != previous.file_identity
            || digest.as_ref() != previous.bytes_digest
        {
            return Err(McpAuditReferenceMutationError::outcome_unknown(
                "rollback_receipt_generation_changed",
            ));
        }
        #[cfg(unix)]
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                McpAuditReferenceMutationError::outcome_unknown(format!(
                    "rollback_parent_sync_failed:{error}"
                ))
            })?;
        // The unique tombstone is intentionally retained. The durable commit
        // point is absence of the canonical reference name, not deletion of
        // the displaced evidence. Removing it in the same operation would
        // add a second durability window and could make restart generation
        // ownership ambiguous after a power loss.
        match self.authority {
            McpAuditReferenceMutationAuthority::FreshRollback(capability) => {
                Ok(capability.reservation)
            }
            _ => Err(McpAuditReferenceMutationError::outcome_unknown(
                "delete completed without fresh rollback owner",
            )),
        }
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_only_publish(
        reference_path: &Path,
        next: McpAuditDurableReferenceDocument,
    ) -> Result<Self> {
        let reference_path = canonical_reference_path(reference_path)?;
        Self::publish_absent(
            McpAuditReferenceMutationAuthority::TestOnly,
            reference_path,
            next,
        )
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_only_replace(
        previous: &McpAuditDurableReferenceReceipt,
        next: McpAuditDurableReferenceDocument,
    ) -> Result<Self> {
        Self::replace_exact(McpAuditReferenceMutationAuthority::TestOnly, previous, next)
    }
}

/// Core-minted authority retained across one credential-store create effect.
/// The caller may inspect neither the authority nor the sealed generation; it
/// can only ask the permit to validate the exact planned epoch/reference and
/// digest immediately before invoking the OS credential store.
pub struct McpAuditPendingSecretEffectPermit<'a> {
    authority: McpAuditPendingSecretEffectAuthority<'a>,
    receipt: McpAuditDurableReferenceReceipt,
    epoch: u64,
    key_ref: String,
    expected_digest: String,
}

enum McpAuditPendingSecretEffectAuthority<'a> {
    Fresh(&'a McpAuditFreshDatabaseCreationCapability),
    Rotation(&'a McpAuditRotationTransition),
    #[cfg(any(test, feature = "test-utils"))]
    TestOnly,
}

impl std::fmt::Debug for McpAuditPendingSecretEffectPermit<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditPendingSecretEffectPermit")
            .field("reference_path", &self.receipt.reference_path)
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

impl McpAuditPendingSecretEffectPermit<'_> {
    pub fn validate_at_effect_edge(
        &self,
        epoch: u64,
        key_ref: &str,
        expected_digest: &str,
    ) -> Result<()> {
        if epoch != self.epoch || key_ref != self.key_ref || expected_digest != self.expected_digest
        {
            anyhow::bail!("mcp_audit_pending_secret_effect_plan_mismatch");
        }
        self.receipt.revalidate_visible()?;
        match &self.authority {
            McpAuditPendingSecretEffectAuthority::Fresh(capability) => {
                capability.validate_reference_generation(&self.receipt)?
            }
            McpAuditPendingSecretEffectAuthority::Rotation(transition) => {
                transition.validate_visible_rotation_generation(&self.receipt)?
            }
            #[cfg(any(test, feature = "test-utils"))]
            McpAuditPendingSecretEffectAuthority::TestOnly => {}
        }
        match self.receipt.transition {
            McpAuditAuthorityTransition::Prepare {
                pending_epoch,
                secret_state: McpAuditDurableSecretState::Pending,
                database_state: McpAuditDurableDatabaseState::NotAttempted,
                ..
            } if pending_epoch == epoch => {}
            _ => anyhow::bail!("mcp_audit_pending_secret_effect_state_mismatch"),
        }
        let pending_config = self
            .receipt
            .keys
            .last()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_pending_key_config_missing"))?;
        if pending_config.epoch != epoch
            || pending_config.key_ref.as_deref() != Some(key_ref)
            || self.receipt.pending_secret_digest.as_deref() != Some(expected_digest)
        {
            anyhow::bail!("mcp_audit_pending_secret_effect_reference_mismatch");
        }
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_only(
        receipt: &McpAuditDurableReferenceReceipt,
        epoch: u64,
        key_ref: String,
        expected_digest: String,
    ) -> Result<Self> {
        receipt.revalidate_visible()?;
        Ok(Self {
            authority: McpAuditPendingSecretEffectAuthority::TestOnly,
            receipt: receipt.clone(),
            epoch,
            key_ref,
            expected_digest,
        })
    }
}

#[derive(Debug, Clone)]
struct McpAuditDurableReferenceProof {
    reference_path: PathBuf,
    file_identity: String,
    bytes_digest: [u8; 32],
    store_identity: String,
    canonical_slot_digest: String,
    document: McpAuditDurableReferenceDocument,
    transition: McpAuditAuthorityTransition,
    keys: Vec<AuditKeyConfig>,
}

impl McpAuditDurableReferenceProof {
    fn revalidate_visible(&self) -> Result<()> {
        let snapshot = read_bounded_reference_snapshot(&self.reference_path)?;
        let digest = ring::digest::digest(&SHA256, &snapshot.bytes);
        if snapshot.file_identity != self.file_identity || digest.as_ref() != self.bytes_digest {
            anyhow::bail!("mcp_audit_reference_changed_after_receipt");
        }
        Ok(())
    }

    fn bound_epoch(&self) -> u64 {
        match self.transition {
            McpAuditAuthorityTransition::VerifyActive { active_epoch } => active_epoch,
            McpAuditAuthorityTransition::Prepare { pending_epoch, .. } => pending_epoch,
        }
    }

    fn is_active(&self) -> bool {
        matches!(
            self.transition,
            McpAuditAuthorityTransition::VerifyActive { .. }
        )
    }

    fn validate_materials(&self, materials: &[AuditKeyMaterial]) -> Result<()> {
        if materials.len() != self.keys.len()
            || materials
                .iter()
                .zip(&self.keys)
                .any(|(material, durable)| !same_key_config(&material.config, durable))
        {
            anyhow::bail!("mcp_audit_reference_material_keyring_mismatch");
        }
        Ok(())
    }
}

fn receipt_matches_reference_proof(
    receipt: &McpAuditDurableReferenceReceipt,
    proof: &McpAuditDurableReferenceProof,
) -> bool {
    receipt.reference_path == proof.reference_path
        && receipt.file_identity == proof.file_identity
        && receipt.bytes_digest == proof.bytes_digest
        && receipt.store_identity == proof.store_identity
        && receipt.canonical_slot_digest == proof.canonical_slot_digest
        && receipt.document == proof.document
        && receipt.transition == proof.transition
        && receipt.keys.len() == proof.keys.len()
        && receipt
            .keys
            .iter()
            .zip(&proof.keys)
            .all(|(left, right)| same_key_config(left, right))
}

#[derive(Debug, Clone)]
enum McpAuditDurableReferenceAuthority {
    UnboundFixture,
    Stable {
        generation_id: uuid::Uuid,
        proof: McpAuditDurableReferenceProof,
    },
    Transitioning {
        transition_id: uuid::Uuid,
        previous_generation_id: uuid::Uuid,
        previous_proof: McpAuditDurableReferenceProof,
        pending_epoch: u64,
    },
}

/// Opaque process-local rotation capability. Beginning a transition blocks
/// every retained product-store clone before the prepared reference is made
/// visible. Dropping an unfinished transition permanently poisons writes; only
/// an exact not-committed abort or a sealed active receipt can complete it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpAuditRotationDatabaseOutcome {
    NotAttempted,
    AttemptedUnknown,
    Committed,
}

pub struct McpAuditRotationTransition {
    transition_id: uuid::Uuid,
    reference_path: PathBuf,
    authority_guard: Option<ArcRwLockWriteGuard<RawRwLock, McpAuditDurableReferenceAuthority>>,
    write_poison: Arc<Mutex<Option<String>>>,
    pending_epoch: u64,
    database_outcome: McpAuditRotationDatabaseOutcome,
    completed: bool,
}

impl std::fmt::Debug for McpAuditRotationTransition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpAuditRotationTransition")
            .field("transition_id", &self.transition_id)
            .field("reference_path", &self.reference_path)
            .field("pending_epoch", &self.pending_epoch)
            .field("database_outcome", &self.database_outcome)
            .field("completed", &self.completed)
            .finish_non_exhaustive()
    }
}

impl McpAuditRotationTransition {
    /// Canonical reference path guarded by the retained exclusive transition.
    pub fn reference_path(&self) -> &Path {
        &self.reference_path
    }

    pub fn transition_id(&self) -> uuid::Uuid {
        self.transition_id
    }

    pub fn authorize_reference_transition(
        &self,
        previous: &McpAuditDurableReferenceReceipt,
        next: &McpAuditDurableReferenceDocument,
    ) -> Result<McpAuditReferenceMutationPermit<'_>> {
        self.validate_visible_rotation_generation(previous)?;
        if !previous.document.allows_transition_to(next) {
            anyhow::bail!("mcp_audit_reference_transition_not_monotonic");
        }
        let next_transition_id = next
            .transition_id()
            .and_then(|value| uuid::Uuid::parse_str(value).ok());
        if next.phase() == McpAuditDurableReferencePhase::Prepared
            && next_transition_id != Some(self.transition_id)
        {
            anyhow::bail!("mcp_audit_rotation_reference_transition_id_mismatch");
        }
        McpAuditReferenceMutationPermit::replace_exact(
            McpAuditReferenceMutationAuthority::Rotation(self),
            previous,
            next.clone(),
        )
    }

    pub fn authorize_pending_secret_effect(
        &self,
        receipt: &McpAuditDurableReferenceReceipt,
        config: &AuditKeyConfig,
        expected_digest: &str,
    ) -> Result<McpAuditPendingSecretEffectPermit<'_>> {
        self.validate_visible_rotation_generation(receipt)?;
        match receipt.transition {
            McpAuditAuthorityTransition::Prepare {
                transition_id,
                pending_epoch,
                origin: McpAuditDurableReferenceOrigin::ExistingStoreRotation,
                secret_state: McpAuditDurableSecretState::Pending,
                database_state: McpAuditDurableDatabaseState::NotAttempted,
                ..
            } if transition_id == self.transition_id && pending_epoch == config.epoch => {}
            _ => anyhow::bail!("mcp_audit_rotation_pending_secret_effect_state_mismatch"),
        }
        let key_ref = config
            .key_ref
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_pending_secret_key_ref_missing"))?;
        if receipt
            .keys
            .last()
            .is_none_or(|durable| !same_key_config(durable, config))
            || receipt.pending_secret_digest.as_deref() != Some(expected_digest)
        {
            anyhow::bail!("mcp_audit_rotation_pending_secret_effect_plan_mismatch");
        }
        Ok(McpAuditPendingSecretEffectPermit {
            authority: McpAuditPendingSecretEffectAuthority::Rotation(self),
            receipt: receipt.clone(),
            epoch: config.epoch,
            key_ref: key_ref.to_string(),
            expected_digest: expected_digest.to_string(),
        })
    }

    fn validate_visible_rotation_generation(
        &self,
        receipt: &McpAuditDurableReferenceReceipt,
    ) -> Result<()> {
        if self.completed || receipt.reference_path != self.reference_path {
            anyhow::bail!("mcp_audit_rotation_reference_owner_mismatch");
        }
        let guard = self
            .authority_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_rotation_transition_guard_missing"))?;
        let previous_proof = match &**guard {
            McpAuditDurableReferenceAuthority::Transitioning {
                transition_id,
                previous_proof,
                pending_epoch,
                ..
            } if *transition_id == self.transition_id && *pending_epoch == self.pending_epoch => {
                previous_proof
            }
            _ => anyhow::bail!("mcp_audit_rotation_transition_state_mismatch"),
        };
        match receipt.transition {
            McpAuditAuthorityTransition::VerifyActive { .. }
                if receipt_matches_reference_proof(receipt, previous_proof) => {}
            McpAuditAuthorityTransition::Prepare {
                transition_id,
                pending_epoch,
                origin: McpAuditDurableReferenceOrigin::ExistingStoreRotation,
                ..
            } if transition_id == self.transition_id && pending_epoch == self.pending_epoch => {}
            _ => anyhow::bail!("mcp_audit_rotation_visible_generation_mismatch"),
        }
        receipt.revalidate_visible()
    }

    fn validate_reference_effect(
        &self,
        permit: &McpAuditReferenceMutationPermit<'_>,
    ) -> Result<()> {
        if permit.kind != McpAuditReferenceMutationKind::ReplaceExact {
            anyhow::bail!("mcp_audit_rotation_reference_effect_kind_mismatch");
        }
        self.validate_visible_rotation_generation(
            permit
                .previous
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("mcp_audit_reference_predecessor_missing"))?,
        )
    }
}

impl Drop for McpAuditRotationTransition {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        if let Ok(mut poison) = self.write_poison.lock() {
            if poison.is_none() {
                *poison = Some("mcp_audit_rotation_transition_abandoned".into());
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct McpAuditAuthorityState {
    store_identity_digest: String,
    canonical_slot_digest: String,
    database_identity_digest: String,
    key_epoch: u64,
    epoch_set_digest: String,
    epoch_count: u64,
    reference_digest: String,
    successor_active_reference_digest: Option<String>,
    transition_generation: String,
    reference_phase: McpAuditDurableReferencePhase,
    reference_origin: McpAuditDurableReferenceOrigin,
    migration_id: Option<String>,
}

#[derive(Debug)]
struct McpAuditAuthorityRow {
    version: i64,
    store_identity_digest: String,
    canonical_slot_digest: String,
    database_identity_digest: String,
    key_epoch: u64,
    epoch_set_digest: String,
    epoch_count: u64,
    reference_digest: String,
    successor_active_reference_digest: Option<String>,
    transition_generation: String,
    reference_phase: McpAuditDurableReferencePhase,
    reference_origin: McpAuditDurableReferenceOrigin,
    migration_id: Option<String>,
    verifier: String,
}

/// Encrypted SQLite-backed store for MCP call logs with configurable key management.
pub trait McpAuditRuntimeFailureObserver: Send + Sync {
    fn mark_mcp_audit_store_unavailable(&self, reason_code: &'static str, detail: &str);
}

/// Durable MCP-audit commit authority used by ToolGateway execution.
///
/// Product implementations resolve the canonical store only at the short,
/// synchronous commit edge. This keeps replaceable `AppState` guards out of
/// provider/tool/network awaits while preventing a resource snapshot from
/// freezing an obsolete key/authority generation across rotation.
pub trait McpAuditDurableWriter: Send + Sync {
    fn clone_owned_writer(&self) -> Arc<dyn McpAuditDurableWriter>;

    fn insert_log_durably(
        &self,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> Result<i64>;

    fn report_runtime_failure(&self, reason_code: &'static str, detail: &str);
}

#[derive(Clone)]
pub struct McpAuditStore {
    db_path: PathBuf,
    connection: Option<Arc<crate::sqlite_migration::IdentityBoundSqliteConnection>>,
    read_only: bool,
    unavailable_reason: Option<String>,
    key: [u8; 32],
    key_config: AuditKeyConfig,
    keyring: HashMap<u64, [u8; 32]>,
    key_configs: Vec<AuditKeyConfig>,
    authority: Option<McpAuditAuthorityState>,
    durable_reference_authority: Arc<RwLock<McpAuditDurableReferenceAuthority>>,
    write_poison: Arc<Mutex<Option<String>>>,
    runtime_failure_observer: Option<Arc<dyn McpAuditRuntimeFailureObserver>>,
}

impl Drop for McpAuditStore {
    fn drop(&mut self) {
        self.key.zeroize();
        for key in self.keyring.values_mut() {
            key.zeroize();
        }
    }
}

impl McpAuditStore {
    /// Test/dev evaluator-only constructor. Ordinary release builds do not
    /// expose this writable path. The caller must pre-create the parent, which
    /// is then canonicalized and proven to remain under the canonical OS temp
    /// root before any owner reservation or SQLite open occurs.
    #[cfg(any(test, feature = "test-utils", debug_assertions))]
    pub fn isolated_runtime_evaluation(db_path: impl Into<PathBuf>) -> Self {
        let db_path = db_path.into();
        let canonical_temp_root = match std::fs::canonicalize(std::env::temp_dir()) {
            Ok(path) => path,
            Err(error) => {
                return Self::unavailable_sentinel(format!(
                    "isolated MCP audit evaluation temp root unavailable: {error}"
                ));
            }
        };
        let file_name = match db_path.file_name() {
            Some(file_name) => file_name.to_os_string(),
            None => {
                return Self::unavailable_sentinel(
                    "isolated MCP audit evaluation database file name missing",
                );
            }
        };
        let parent = db_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let canonical_parent = match std::fs::canonicalize(parent) {
            Ok(path) => path,
            Err(error) => {
                return Self::unavailable_sentinel(format!(
                    "isolated MCP audit evaluation parent is not pre-created: {error}"
                ));
            }
        };
        if !canonical_parent.starts_with(&canonical_temp_root) {
            return Self::unavailable_sentinel(
                "isolated MCP audit evaluation canonical path is outside the OS temp directory",
            );
        }
        let db_path = canonical_parent.join(file_name);
        let config = AuditKeyConfig::default();
        Self::with_legacy_keyring_unchecked(db_path, vec![config])
    }

    /// Test/fixture-only constructor for the historical deterministic key.
    /// Product code must hydrate a random keychain epoch and call
    /// `with_key_materials`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let config = AuditKeyConfig::default();
        Self::with_config(db_path, config)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_config(db_path: impl Into<PathBuf>, config: AuditKeyConfig) -> Self {
        Self::with_keyring(db_path, vec![config])
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_keyring(db_path: impl Into<PathBuf>, configs: Vec<AuditKeyConfig>) -> Self {
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
                None,
                None,
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

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_key_materials(
        db_path: impl Into<PathBuf>,
        materials: Vec<AuditKeyMaterial>,
    ) -> Result<Self> {
        let db_path = db_path.into();
        let reservation = Self::reserve_writable_owner(&db_path)?;
        Self::activate_with_reservation(db_path, materials, reservation, None, None)
    }

    /// Reserve the canonical writer slot without creating or opening SQLite.
    /// Bootstrap must call this before creating key material or persisting key
    /// references.
    pub fn reserve_writable_owner(
        db_path: impl AsRef<Path>,
    ) -> Result<crate::sqlite_migration::SqliteSlotOwnerReservation> {
        let canonical_slot =
            crate::sqlite_migration::canonical_sqlite_slot(db_path.as_ref(), "mcp_audit_store")?;
        crate::sqlite_migration::SqliteSlotOwnerLease::reserve_no_create(
            &canonical_slot,
            "mcp_audit_store",
        )
    }

    /// Convert an already-held no-create reservation into the sole capability
    /// allowed to create a fresh database.  This check is deliberately before
    /// secret creation and before the first reference write.
    pub fn authorize_fresh_database_creation(
        reservation: crate::sqlite_migration::SqliteSlotOwnerReservation,
        reference_path: &Path,
    ) -> Result<McpAuditFreshDatabaseCreationCapability> {
        if reservation.existing_database_len()?.is_some() {
            anyhow::bail!("mcp_audit_fresh_database_slot_not_absent");
        }
        let expected_reference = reservation
            .canonical_slot()
            .with_file_name("mcp_audit_keys.json");
        let canonical_reference = canonical_reference_path(reference_path)?;
        if canonical_reference != expected_reference {
            anyhow::bail!("mcp_audit_reference_canonical_owner_path_mismatch");
        }
        match std::fs::symlink_metadata(&canonical_reference) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => anyhow::bail!("mcp_audit_fresh_reference_slot_not_absent"),
            Err(error) => return Err(error).context("inspect fresh MCP audit reference slot"),
        }
        Ok(McpAuditFreshDatabaseCreationCapability {
            reservation,
            reference_path: canonical_reference,
            provenance: McpAuditFreshDatabaseCreationProvenance::InitialReferenceAbsence,
        })
    }

    /// Recover a crash-staged fresh reference only while its exact secret is
    /// available and the durable DB transition still says NotAttempted.  This
    /// is intentionally distinct from trusting `origin=fresh_create` alone.
    pub fn authorize_fresh_database_recovery(
        reservation: crate::sqlite_migration::SqliteSlotOwnerReservation,
        receipt: &McpAuditDurableReferenceReceipt,
        materials: &[AuditKeyMaterial],
    ) -> Result<McpAuditFreshDatabaseCreationCapability> {
        if reservation.existing_database_len()?.is_some() {
            anyhow::bail!("mcp_audit_fresh_recovery_database_slot_not_absent");
        }
        receipt.revalidate_visible()?;
        receipt.validate_materials(materials)?;
        let transition_id = match receipt.transition {
            McpAuditAuthorityTransition::Prepare {
                transition_id,
                previous_active_epoch: None,
                origin: McpAuditDurableReferenceOrigin::FreshCreate,
                database_state: McpAuditDurableDatabaseState::NotAttempted,
                ..
            } => transition_id,
            _ => anyhow::bail!("mcp_audit_fresh_recovery_reference_state_invalid"),
        };
        let pending_key = materials
            .last()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_pending_key_unavailable"))?;
        let expected_digest = mcp_audit_secret_value_digest(&pending_key.key);
        if receipt.pending_secret_digest.as_deref() != Some(expected_digest.as_str()) {
            anyhow::bail!("mcp_audit_fresh_recovery_secret_digest_mismatch");
        }
        let expected_reference = reservation
            .canonical_slot()
            .with_file_name("mcp_audit_keys.json");
        if receipt.reference_path != expected_reference {
            anyhow::bail!("mcp_audit_reference_canonical_owner_path_mismatch");
        }
        Ok(McpAuditFreshDatabaseCreationCapability {
            reservation,
            reference_path: receipt.reference_path.clone(),
            provenance: McpAuditFreshDatabaseCreationProvenance::PreparedRecovery {
                store_identity: receipt.store_identity.clone(),
                canonical_slot_digest: receipt.canonical_slot_digest.clone(),
                transition_id,
                pending_secret_digest: expected_digest,
                keys: receipt.keys.clone(),
            },
        })
    }

    /// Authorize rollback only for the exact fresh Prepared generation whose
    /// durable database marker is still NotAttempted and whose retained slot
    /// reservation proves SQLite remains absent.  No legacy or rotation
    /// receipt can mint this capability.
    pub fn authorize_fresh_reference_rollback(
        reservation: crate::sqlite_migration::SqliteSlotOwnerReservation,
        receipt: &McpAuditDurableReferenceReceipt,
    ) -> Result<McpAuditFreshReferenceRollbackCapability> {
        if reservation.existing_database_len()?.is_some() {
            anyhow::bail!("mcp_audit_fresh_rollback_database_slot_not_absent");
        }
        receipt.revalidate_visible()?;
        match receipt.transition {
            McpAuditAuthorityTransition::Prepare {
                previous_active_epoch: None,
                origin: McpAuditDurableReferenceOrigin::FreshCreate,
                database_state: McpAuditDurableDatabaseState::NotAttempted,
                ..
            } => {}
            _ => anyhow::bail!("mcp_audit_fresh_rollback_reference_state_invalid"),
        }
        let expected_reference = reservation
            .canonical_slot()
            .with_file_name("mcp_audit_keys.json");
        if receipt.reference_path != expected_reference
            || receipt.canonical_slot_digest != reservation.canonical_slot_digest()?
        {
            anyhow::bail!("mcp_audit_fresh_rollback_owner_mismatch");
        }
        Ok(McpAuditFreshReferenceRollbackCapability {
            reservation,
            receipt: receipt.clone(),
        })
    }

    /// Product activation from an already-held no-create reservation and a
    /// durable reference-store transition. Direct material-only construction
    /// remains fixture-only; this path authenticates store, slot, database
    /// identity, and key epoch before any product table migration or write.
    pub fn activate_store_bound_authority(
        db_path: impl Into<PathBuf>,
        materials: Vec<AuditKeyMaterial>,
        reservation: crate::sqlite_migration::SqliteSlotOwnerReservation,
        receipt: McpAuditDurableReferenceReceipt,
    ) -> Result<Self> {
        receipt.revalidate_visible()?;
        receipt.validate_materials(&materials)?;
        receipt.validate_pending_secret_material(&materials)?;
        receipt.require_database_transition_sealed()?;
        match reservation.existing_database_len()? {
            Some(length) if length > 0 => {}
            Some(_) => anyhow::bail!("mcp_audit_existing_database_empty"),
            None => anyhow::bail!("mcp_audit_existing_database_missing"),
        }
        Self::activate_with_reservation(db_path, materials, reservation, Some(receipt), None)
    }

    /// Consume the one-shot absence capability after Pending -> Verified ->
    /// Attempted has been durably published.  No ordinary restart receipt can
    /// take this path, even when its JSON claims `origin=fresh_create`.
    pub fn activate_fresh_store_bound_authority(
        db_path: impl Into<PathBuf>,
        materials: Vec<AuditKeyMaterial>,
        capability: McpAuditFreshDatabaseCreationCapability,
        receipt: McpAuditDurableReferenceReceipt,
    ) -> Result<Self> {
        receipt.revalidate_visible()?;
        receipt.validate_materials(&materials)?;
        receipt.validate_pending_secret_material(&materials)?;
        receipt.require_database_transition_sealed()?;
        let (transition_id, pending_epoch, transition_pending_digest) = match receipt.transition {
            McpAuditAuthorityTransition::Prepare {
                transition_id,
                previous_active_epoch: None,
                pending_epoch,
                origin: McpAuditDurableReferenceOrigin::FreshCreate,
                secret_state: McpAuditDurableSecretState::Verified,
                pending_secret_digest,
                database_state: McpAuditDurableDatabaseState::Attempted,
            } => (transition_id, pending_epoch, pending_secret_digest),
            _ => anyhow::bail!("mcp_audit_fresh_database_capability_transition_mismatch"),
        };
        if receipt.reference_path != capability.reference_path {
            anyhow::bail!("mcp_audit_fresh_database_capability_reference_mismatch");
        }
        let pending_material = materials
            .last()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_pending_key_unavailable"))?;
        let observed_pending_digest = mcp_audit_secret_value_digest(&pending_material.key);
        let observed_pending_digest_bytes = parse_mcp_audit_secret_digest(&observed_pending_digest)
            .expect("internally generated MCP audit digest");
        if transition_pending_digest != observed_pending_digest_bytes
            || receipt.pending_secret_digest.as_deref() != Some(observed_pending_digest.as_str())
        {
            anyhow::bail!("mcp_audit_fresh_activation_secret_digest_mismatch");
        }
        if let McpAuditFreshDatabaseCreationProvenance::PreparedRecovery {
            store_identity,
            canonical_slot_digest,
            transition_id: authorized_transition_id,
            pending_secret_digest: authorized_pending_digest,
            keys,
        } = &capability.provenance
        {
            if receipt.store_identity != *store_identity
                || receipt.canonical_slot_digest != *canonical_slot_digest
                || transition_id != *authorized_transition_id
                || observed_pending_digest != *authorized_pending_digest
                || receipt.pending_secret_digest.as_deref()
                    != Some(authorized_pending_digest.as_str())
                || receipt.keys.len() != keys.len()
                || receipt
                    .keys
                    .iter()
                    .zip(keys)
                    .any(|(current, expected)| !same_key_config(current, expected))
            {
                anyhow::bail!("mcp_audit_fresh_recovery_generation_changed");
            }
        }
        if capability.reservation.existing_database_len()?.is_some() {
            anyhow::bail!("mcp_audit_fresh_database_appeared_before_activation");
        }
        let initialization_permit = McpAuditFreshAuthorityInitializationPermit {
            store_identity: receipt.store_identity.clone(),
            canonical_slot_digest: receipt.canonical_slot_digest.clone(),
            transition_id,
            pending_secret_digest: observed_pending_digest,
            pending_epoch,
        };
        Self::activate_with_reservation(
            db_path,
            materials,
            capability.reservation,
            Some(receipt),
            Some(initialization_permit),
        )
    }

    fn activate_with_reservation(
        db_path: impl Into<PathBuf>,
        mut materials: Vec<AuditKeyMaterial>,
        reservation: crate::sqlite_migration::SqliteSlotOwnerReservation,
        authority_receipt: Option<McpAuditDurableReferenceReceipt>,
        fresh_initialization_permit: Option<McpAuditFreshAuthorityInitializationPermit>,
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
            active.config.clone(),
            keyring,
            key_configs,
            reservation,
            authority_receipt,
            fresh_initialization_permit,
        )
    }

    fn build_writable(
        db_path: &Path,
        key: [u8; 32],
        key_config: AuditKeyConfig,
        keyring: HashMap<u64, [u8; 32]>,
        key_configs: Vec<AuditKeyConfig>,
        reservation: crate::sqlite_migration::SqliteSlotOwnerReservation,
        authority_receipt: Option<McpAuditDurableReferenceReceipt>,
        fresh_initialization_permit: Option<McpAuditFreshAuthorityInitializationPermit>,
    ) -> Result<Self> {
        if let Some(receipt) = authority_receipt.as_ref() {
            receipt.revalidate_visible()?;
            receipt.require_database_transition_sealed()?;
        }
        let expected_slot =
            crate::sqlite_migration::canonical_sqlite_slot(db_path, "mcp_audit_store")?;
        if reservation.canonical_slot() != expected_slot {
            anyhow::bail!(
                "mcp_audit_store_owner_reservation_path_mismatch:{}!={}",
                reservation.canonical_slot().display(),
                expected_slot.display()
            );
        }
        let owner_lease = reservation.activate_exact_database()?;
        let database_identity_material = owner_lease.database_identity_material()?;
        let conn = Connection::open_with_flags(
            &expected_slot,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .with_context(|| {
            format!(
                "open owned MCP audit database at {}",
                expected_slot.display()
            )
        })?;
        let observed =
            crate::sqlite_migration::canonical_opened_main_database_path(&conn, "mcp_audit_store")?
                .ok_or_else(|| {
                    anyhow::anyhow!("mcp_audit_store_persistent_database_path_missing")
                })?;
        if observed != expected_slot {
            anyhow::bail!(
                "mcp_audit_store_database_slot_changed_during_open:{}!={}",
                expected_slot.display(),
                observed.display()
            );
        }
        owner_lease.bind_opened_database_identity(&conn)?;
        let connection = Arc::new(
            crate::sqlite_migration::IdentityBoundSqliteConnection::writable(conn, owner_lease)?,
        );
        let authority = authority_receipt.map(|receipt| {
            let binding = McpAuditAuthorityBinding::new(
                receipt.store_identity.clone(),
                receipt.canonical_slot_digest.clone(),
            );
            let transition = receipt.transition;
            let proof = receipt.into_proof();
            (binding, transition, proof)
        });
        let durable_reference_authority = match authority.as_ref() {
            Some((_, _, proof)) => McpAuditDurableReferenceAuthority::Stable {
                generation_id: uuid::Uuid::new_v4(),
                proof: proof.clone(),
            },
            None => McpAuditDurableReferenceAuthority::UnboundFixture,
        };
        let mut store = Self {
            db_path: expected_slot,
            connection: Some(connection),
            read_only: false,
            unavailable_reason: None,
            key,
            key_config,
            keyring,
            key_configs,
            authority: None,
            durable_reference_authority: Arc::new(RwLock::new(durable_reference_authority)),
            write_poison: Arc::new(Mutex::new(None)),
            runtime_failure_observer: None,
        };
        if let Some((binding, transition, proof)) = authority {
            store.authenticate_authority_binding(
                binding,
                transition,
                &proof,
                &database_identity_material,
                fresh_initialization_permit,
            )?;
            if authority_post_binding_failure_injected(&store.db_path) {
                anyhow::bail!("injected_mcp_audit_post_authority_binding_failure");
            }
        }
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
            .map(|material| material.config.clone())
            .collect();
        Self::build_read_only(
            &db_path.into(),
            active.key,
            active.config.clone(),
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
        let canonical_slot =
            crate::sqlite_migration::canonical_sqlite_slot(db_path, "mcp_audit_store")?;
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
        let connection =
            crate::sqlite_migration::IdentityBoundSqliteConnection::read_only(conn, identity)?;
        Ok(Self {
            db_path: canonical_slot,
            connection: Some(Arc::new(connection)),
            read_only: true,
            unavailable_reason: None,
            key,
            key_config,
            keyring,
            key_configs,
            authority: None,
            durable_reference_authority: Arc::new(RwLock::new(
                McpAuditDurableReferenceAuthority::UnboundFixture,
            )),
            write_poison: Arc::new(Mutex::new(None)),
            runtime_failure_observer: None,
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
            authority: None,
            durable_reference_authority: Arc::new(RwLock::new(
                McpAuditDurableReferenceAuthority::UnboundFixture,
            )),
            write_poison: Arc::new(Mutex::new(Some("mcp_audit_store_unavailable".into()))),
            runtime_failure_observer: None,
        }
    }

    pub fn install_runtime_failure_observer(
        &mut self,
        observer: Arc<dyn McpAuditRuntimeFailureObserver>,
    ) {
        self.runtime_failure_observer = Some(observer);
    }

    pub fn report_runtime_failure(&self, reason_code: &'static str, detail: &str) {
        if let Some(observer) = self.runtime_failure_observer.as_ref() {
            // A diagnostic observer must never replace the canonical failure
            // that triggered it. The shipped observer is synchronous and
            // infallible; containment also protects the durable-write worker
            // from a poisoned/custom observer implementation.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                observer.mark_mcp_audit_store_unavailable(reason_code, detail);
            }));
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

    fn tagged_digest(domain: &[u8], value: &[u8]) -> String {
        let mut context = DigestContext::new(&SHA256);
        context.update(domain);
        context.update(&[0]);
        context.update(value);
        let digest = context.finish();
        format!(
            "sha256:{}",
            general_purpose::STANDARD_NO_PAD.encode(digest.as_ref())
        )
    }

    fn authority_verifier_message(state: &McpAuditAuthorityState) -> Vec<u8> {
        let phase = match state.reference_phase {
            McpAuditDurableReferencePhase::Prepared => "prepared",
            McpAuditDurableReferencePhase::Active => "active",
        };
        let origin = match state.reference_origin {
            McpAuditDurableReferenceOrigin::FreshCreate => "fresh_create",
            McpAuditDurableReferenceOrigin::ExistingStoreRotation => "existing_store_rotation",
            McpAuditDurableReferenceOrigin::LegacyMigration => "legacy_migration",
        };
        format!(
            "openlife-mcp-audit-authority-v{}\nstore={}\nslot={}\ndatabase={}\nepoch={}\nepoch_set={}\nepoch_count={}\nreference={}\nsuccessor_active_reference={}\ngeneration={}\nphase={}\norigin={}\nmigration={}",
            MCP_AUDIT_AUTHORITY_BINDING_VERSION,
            state.store_identity_digest,
            state.canonical_slot_digest,
            state.database_identity_digest,
            state.key_epoch,
            state.epoch_set_digest,
            state.epoch_count,
            state.reference_digest,
            state
                .successor_active_reference_digest
                .as_deref()
                .unwrap_or("-"),
            state.transition_generation,
            phase,
            origin,
            state.migration_id.as_deref().unwrap_or("-"),
        )
        .into_bytes()
    }

    fn authority_mac_key(payload_key: &[u8; 32]) -> Result<[u8; 32]> {
        struct AuthorityMacKeyLength;
        impl hkdf::KeyType for AuthorityMacKeyLength {
            fn len(&self) -> usize {
                32
            }
        }
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, MCP_AUDIT_AUTHORITY_VERIFIER_DOMAIN);
        let prk = salt.extract(payload_key);
        let info = [MCP_AUDIT_AUTHORITY_VERIFIER_DOMAIN];
        let okm = prk
            .expand(&info, AuthorityMacKeyLength)
            .map_err(|_| anyhow::anyhow!("mcp_audit_authority_hkdf_expand_failed"))?;
        let mut key = [0u8; 32];
        okm.fill(&mut key)
            .map_err(|_| anyhow::anyhow!("mcp_audit_authority_hkdf_fill_failed"))?;
        Ok(key)
    }

    fn authority_verifier(
        state: &McpAuditAuthorityState,
        payload_key: &[u8; 32],
    ) -> Result<String> {
        let mac_key = Self::authority_mac_key(payload_key)?;
        let signing_key = hmac::Key::new(hmac::HMAC_SHA256, &mac_key);
        let tag = hmac::sign(&signing_key, &Self::authority_verifier_message(state));
        Ok(format!(
            "hmac-sha256:{}",
            general_purpose::STANDARD_NO_PAD.encode(tag.as_ref())
        ))
    }

    fn encode_sha256_digest(digest: &[u8; 32]) -> String {
        format!(
            "sha256:{}",
            digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    }

    fn sha256_digest(bytes: &[u8]) -> String {
        let digest = ring::digest::digest(&SHA256, bytes);
        let mut value = [0u8; 32];
        value.copy_from_slice(digest.as_ref());
        Self::encode_sha256_digest(&value)
    }

    fn epoch_set_digest(keys: &[AuditKeyConfig]) -> Result<String> {
        let encoded = serde_json::to_vec(keys).context("encode MCP audit epoch manifest")?;
        Ok(Self::tagged_digest(
            b"openlife-mcp-audit-epoch-set-v1",
            &encoded,
        ))
    }

    fn transition_generation(store_identity: &str, key_epoch: u64) -> String {
        Self::tagged_digest(
            b"openlife-mcp-audit-transition-generation-v1",
            format!("{store_identity}\0{key_epoch}").as_bytes(),
        )
    }

    fn authority_state_for_reference(
        store_identity_digest: &str,
        canonical_slot_digest: &str,
        database_identity_digest: &str,
        proof: &McpAuditDurableReferenceProof,
        migration_id: Option<String>,
    ) -> Result<McpAuditAuthorityState> {
        let successor_active_reference_digest = if proof.is_active() {
            None
        } else {
            let mut successor = proof.document.clone();
            successor.mark_active().map_err(anyhow::Error::msg)?;
            Some(Self::sha256_digest(&successor.to_canonical_bytes()?))
        };
        Ok(McpAuditAuthorityState {
            store_identity_digest: store_identity_digest.to_string(),
            canonical_slot_digest: canonical_slot_digest.to_string(),
            database_identity_digest: database_identity_digest.to_string(),
            key_epoch: proof.bound_epoch(),
            epoch_set_digest: Self::epoch_set_digest(&proof.keys)?,
            epoch_count: proof.keys.len() as u64,
            reference_digest: Self::encode_sha256_digest(&proof.bytes_digest),
            successor_active_reference_digest,
            transition_generation: Self::transition_generation(
                &proof.store_identity,
                proof.bound_epoch(),
            ),
            reference_phase: if proof.is_active() {
                McpAuditDurableReferencePhase::Active
            } else {
                McpAuditDurableReferencePhase::Prepared
            },
            reference_origin: proof.document.origin(),
            migration_id,
        })
    }

    fn authority_state_from_row(row: &McpAuditAuthorityRow) -> McpAuditAuthorityState {
        McpAuditAuthorityState {
            store_identity_digest: row.store_identity_digest.clone(),
            canonical_slot_digest: row.canonical_slot_digest.clone(),
            database_identity_digest: row.database_identity_digest.clone(),
            key_epoch: row.key_epoch,
            epoch_set_digest: row.epoch_set_digest.clone(),
            epoch_count: row.epoch_count,
            reference_digest: row.reference_digest.clone(),
            successor_active_reference_digest: row.successor_active_reference_digest.clone(),
            transition_generation: row.transition_generation.clone(),
            reference_phase: row.reference_phase,
            reference_origin: row.reference_origin,
            migration_id: row.migration_id.clone(),
        }
    }

    fn prepared_manifest_allows_active_successor(
        prepared: &McpAuditAuthorityState,
        active: &McpAuditAuthorityState,
    ) -> bool {
        prepared.store_identity_digest == active.store_identity_digest
            && prepared.canonical_slot_digest == active.canonical_slot_digest
            && prepared.database_identity_digest == active.database_identity_digest
            && prepared.key_epoch == active.key_epoch
            && prepared.epoch_set_digest == active.epoch_set_digest
            && prepared.epoch_count == active.epoch_count
            && prepared.transition_generation == active.transition_generation
            && prepared.reference_phase == McpAuditDurableReferencePhase::Prepared
            && active.reference_phase == McpAuditDurableReferencePhase::Active
            && prepared.reference_origin == active.reference_origin
            && prepared.migration_id == active.migration_id
            && prepared.successor_active_reference_digest.as_deref()
                == Some(active.reference_digest.as_str())
            && active.successor_active_reference_digest.is_none()
    }

    fn authority_table_exists(tx: &Transaction<'_>) -> Result<bool> {
        tx.query_row(
            "SELECT COUNT(*) = 1 FROM sqlite_master
             WHERE type = 'table' AND name = 'mcp_audit_authority_binding'",
            [],
            |row| row.get(0),
        )
        .context("inspect MCP audit authority binding table")
    }

    fn create_authority_table(tx: &Transaction<'_>) -> Result<()> {
        tx.execute(
            "CREATE TABLE IF NOT EXISTS mcp_audit_authority_binding (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                version INTEGER NOT NULL,
                store_identity_digest TEXT NOT NULL,
                canonical_slot_digest TEXT NOT NULL,
                database_identity_digest TEXT NOT NULL,
                key_epoch INTEGER NOT NULL CHECK (key_epoch >= 0),
                epoch_set_digest TEXT NOT NULL,
                epoch_count INTEGER NOT NULL CHECK (epoch_count > 0),
                reference_digest TEXT NOT NULL,
                successor_active_reference_digest TEXT,
                transition_generation TEXT NOT NULL,
                reference_phase TEXT NOT NULL CHECK (reference_phase IN ('prepared', 'active')),
                reference_origin TEXT NOT NULL CHECK (reference_origin IN ('fresh_create', 'existing_store_rotation', 'legacy_migration')),
                migration_id TEXT,
                verifier TEXT NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    fn read_authority_row(tx: &Transaction<'_>) -> Result<Option<McpAuditAuthorityRow>> {
        tx.query_row(
            "SELECT version, store_identity_digest, canonical_slot_digest,
                    database_identity_digest, key_epoch, epoch_set_digest,
                    epoch_count, reference_digest, successor_active_reference_digest,
                    transition_generation, reference_phase, reference_origin,
                    migration_id, verifier
             FROM mcp_audit_authority_binding WHERE singleton = 1",
            [],
            |row| {
                let epoch = row.get::<_, i64>(4)?;
                if epoch < 0 {
                    return Err(rusqlite::Error::IntegralValueOutOfRange(4, epoch));
                }
                let epoch_count = row.get::<_, i64>(6)?;
                if epoch_count <= 0 {
                    return Err(rusqlite::Error::IntegralValueOutOfRange(6, epoch_count));
                }
                let phase = match row.get::<_, String>(10)?.as_str() {
                    "prepared" => McpAuditDurableReferencePhase::Prepared,
                    "active" => McpAuditDurableReferencePhase::Active,
                    _ => {
                        return Err(rusqlite::Error::InvalidColumnType(
                            10,
                            "reference_phase".into(),
                            rusqlite::types::Type::Text,
                        ))
                    }
                };
                let origin = match row.get::<_, String>(11)?.as_str() {
                    "fresh_create" => McpAuditDurableReferenceOrigin::FreshCreate,
                    "existing_store_rotation" => {
                        McpAuditDurableReferenceOrigin::ExistingStoreRotation
                    }
                    "legacy_migration" => McpAuditDurableReferenceOrigin::LegacyMigration,
                    _ => {
                        return Err(rusqlite::Error::InvalidColumnType(
                            11,
                            "reference_origin".into(),
                            rusqlite::types::Type::Text,
                        ))
                    }
                };
                Ok(McpAuditAuthorityRow {
                    version: row.get(0)?,
                    store_identity_digest: row.get(1)?,
                    canonical_slot_digest: row.get(2)?,
                    database_identity_digest: row.get(3)?,
                    key_epoch: epoch as u64,
                    epoch_set_digest: row.get(5)?,
                    epoch_count: epoch_count as u64,
                    reference_digest: row.get(7)?,
                    successor_active_reference_digest: row.get(8)?,
                    transition_generation: row.get(9)?,
                    reference_phase: phase,
                    reference_origin: origin,
                    migration_id: row.get(12)?,
                    verifier: row.get(13)?,
                })
            },
        )
        .optional()
        .context("read MCP audit authority binding")
    }

    fn verify_authority_row(
        row: &McpAuditAuthorityRow,
        expected: &McpAuditAuthorityState,
        keyring: &HashMap<u64, [u8; 32]>,
    ) -> Result<()> {
        let row_state = Self::authenticate_authority_row(row, keyring)?;
        if &row_state != expected {
            anyhow::bail!("mcp_audit_authority_manifest_mismatch");
        }
        Ok(())
    }

    fn authenticate_authority_row(
        row: &McpAuditAuthorityRow,
        keyring: &HashMap<u64, [u8; 32]>,
    ) -> Result<McpAuditAuthorityState> {
        if row.version != MCP_AUDIT_AUTHORITY_BINDING_VERSION {
            anyhow::bail!("mcp_audit_authority_binding_version_mismatch");
        }
        let key = keyring
            .get(&row.key_epoch)
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_authority_key_epoch_unavailable"))?;
        let row_state = Self::authority_state_from_row(row);
        let encoded = row
            .verifier
            .strip_prefix("hmac-sha256:")
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_authority_verifier_format_invalid"))?;
        let tag = general_purpose::STANDARD_NO_PAD
            .decode(encoded)
            .context("decode MCP audit authority verifier")?;
        let mac_key = Self::authority_mac_key(key)?;
        let verification_key = hmac::Key::new(hmac::HMAC_SHA256, &mac_key);
        hmac::verify(
            &verification_key,
            &Self::authority_verifier_message(&row_state),
            &tag,
        )
        .map_err(|_| anyhow::anyhow!("mcp_audit_authority_verifier_invalid"))?;
        Ok(row_state)
    }

    fn write_authority_row(
        tx: &Transaction<'_>,
        state: &McpAuditAuthorityState,
        key: &[u8; 32],
    ) -> Result<()> {
        tx.execute(
            "INSERT INTO mcp_audit_authority_binding (
                singleton, version, store_identity_digest, canonical_slot_digest,
                database_identity_digest, key_epoch, epoch_set_digest, epoch_count,
                reference_digest, successor_active_reference_digest,
                transition_generation, reference_phase, reference_origin,
                migration_id, verifier
             ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(singleton) DO UPDATE SET
                version = excluded.version,
                store_identity_digest = excluded.store_identity_digest,
                canonical_slot_digest = excluded.canonical_slot_digest,
                database_identity_digest = excluded.database_identity_digest,
                key_epoch = excluded.key_epoch,
                epoch_set_digest = excluded.epoch_set_digest,
                epoch_count = excluded.epoch_count,
                reference_digest = excluded.reference_digest,
                successor_active_reference_digest = excluded.successor_active_reference_digest,
                transition_generation = excluded.transition_generation,
                reference_phase = excluded.reference_phase,
                reference_origin = excluded.reference_origin,
                migration_id = excluded.migration_id,
                verifier = excluded.verifier",
            params![
                MCP_AUDIT_AUTHORITY_BINDING_VERSION,
                state.store_identity_digest,
                state.canonical_slot_digest,
                state.database_identity_digest,
                state.key_epoch as i64,
                state.epoch_set_digest,
                state.epoch_count as i64,
                state.reference_digest,
                state.successor_active_reference_digest,
                state.transition_generation,
                match state.reference_phase {
                    McpAuditDurableReferencePhase::Prepared => "prepared",
                    McpAuditDurableReferencePhase::Active => "active",
                },
                match state.reference_origin {
                    McpAuditDurableReferenceOrigin::FreshCreate => "fresh_create",
                    McpAuditDurableReferenceOrigin::ExistingStoreRotation => {
                        "existing_store_rotation"
                    }
                    McpAuditDurableReferenceOrigin::LegacyMigration => "legacy_migration",
                },
                state.migration_id,
                Self::authority_verifier(state, key)?,
            ],
        )?;
        Ok(())
    }

    fn authenticate_authority_binding(
        &mut self,
        binding: McpAuditAuthorityBinding,
        transition: McpAuditAuthorityTransition,
        reference_proof: &McpAuditDurableReferenceProof,
        database_identity_material: &str,
        fresh_initialization_permit: Option<McpAuditFreshAuthorityInitializationPermit>,
    ) -> Result<()> {
        let identity = uuid::Uuid::parse_str(&binding.store_identity)
            .context("parse MCP audit store identity")?;
        if identity.is_nil() || identity.get_version_num() != 4 {
            anyhow::bail!("mcp_audit_store_identity_not_random_v4");
        }
        let expected_slot_digest = crate::sqlite_migration::canonical_sqlite_slot_digest(
            &self.db_path,
            "mcp_audit_store",
        )?;
        if binding.canonical_slot_digest != expected_slot_digest {
            anyhow::bail!("mcp_audit_reference_canonical_slot_mismatch");
        }
        if matches!(
            transition,
            McpAuditAuthorityTransition::Prepare {
                secret_state: McpAuditDurableSecretState::Pending,
                ..
            } | McpAuditAuthorityTransition::Prepare {
                database_state: McpAuditDurableDatabaseState::NotAttempted,
                ..
            }
        ) {
            anyhow::bail!("mcp_audit_prepared_reference_not_sealed_for_database_transition");
        }
        let target_epoch = match transition {
            McpAuditAuthorityTransition::VerifyActive { active_epoch } => active_epoch,
            McpAuditAuthorityTransition::Prepare { pending_epoch, .. } => pending_epoch,
        };
        if target_epoch != self.key_config.epoch || !self.keyring.contains_key(&target_epoch) {
            anyhow::bail!("mcp_audit_authority_target_epoch_material_mismatch");
        }

        let store_identity_digest = Self::tagged_digest(
            b"openlife-mcp-audit-store-identity-v1",
            identity.simple().to_string().as_bytes(),
        );
        let database_identity_digest = Self::tagged_digest(
            b"openlife-mcp-audit-database-identity-v1",
            database_identity_material.as_bytes(),
        );
        let target_state = Self::authority_state_for_reference(
            &store_identity_digest,
            &expected_slot_digest,
            &database_identity_digest,
            reference_proof,
            None,
        )?;

        self.with_initialization_write(|conn| {
            let tx = conn.transaction()?;
            match transition {
                McpAuditAuthorityTransition::VerifyActive { active_epoch } => {
                    if !Self::authority_table_exists(&tx)? {
                        anyhow::bail!("mcp_audit_active_reference_has_no_database_verifier");
                    }
                    let row = Self::read_authority_row(&tx)?
                        .ok_or_else(|| anyhow::anyhow!("mcp_audit_authority_binding_missing"))?;
                    let authenticated = Self::authenticate_authority_row(&row, &self.keyring)?;
                    if authenticated.key_epoch != active_epoch {
                        anyhow::bail!("mcp_audit_active_reference_epoch_mismatch");
                    }
                    if authenticated == target_state {
                        // Normal restart: the authenticated manifest already
                        // owns this exact Active generation.
                    } else if Self::prepared_manifest_allows_active_successor(
                        &authenticated,
                        &target_state,
                    ) {
                        // Crash reconciliation: Active sidecar is exact and
                        // the HMAC-authenticated Prepared manifest commits the
                        // same epoch set/generation. Neither surface alone is
                        // sufficient; together they authorize only this phase
                        // and reference-digest advance.
                        let active_key = self.keyring.get(&active_epoch).ok_or_else(|| {
                            anyhow::anyhow!("mcp_audit_authority_key_epoch_unavailable")
                        })?;
                        Self::write_authority_row(&tx, &target_state, active_key)?;
                    } else {
                        anyhow::bail!("mcp_audit_active_reference_manifest_mismatch");
                    }
                }
                McpAuditAuthorityTransition::Prepare {
                    transition_id,
                    previous_active_epoch,
                    pending_epoch,
                    origin,
                    pending_secret_digest,
                    ..
                } => {
                    match origin {
                        McpAuditDurableReferenceOrigin::FreshCreate => {
                            let table_exists = Self::authority_table_exists(&tx)?;
                            let row = if table_exists {
                                Self::read_authority_row(&tx)?
                            } else {
                                None
                            };
                            match row {
                                None => {
                                    let permit = fresh_initialization_permit.as_ref().ok_or_else(
                                        || {
                                            anyhow::anyhow!(
                                                "mcp_audit_fresh_authority_initialization_permit_missing"
                                            )
                                        },
                                    )?;
                                    if permit.store_identity != binding.store_identity
                                        || permit.canonical_slot_digest != expected_slot_digest
                                        || permit.transition_id != transition_id
                                        || permit.pending_epoch != pending_epoch
                                        || parse_mcp_audit_secret_digest(
                                            &permit.pending_secret_digest,
                                        ) != Some(pending_secret_digest)
                                    {
                                        anyhow::bail!(
                                            "mcp_audit_fresh_authority_initialization_permit_mismatch"
                                        );
                                    }
                                    if !table_exists {
                                        Self::create_authority_table(&tx)?;
                                    }
                                    let pending_key = self.keyring.get(&pending_epoch).ok_or_else(
                                        || anyhow::anyhow!("mcp_audit_pending_key_unavailable"),
                                    )?;
                                    Self::write_authority_row(&tx, &target_state, pending_key)?;
                                }
                                Some(row) => {
                                    if row.key_epoch != pending_epoch {
                                        anyhow::bail!(
                                            "mcp_audit_fresh_prepared_authority_epoch_mismatch"
                                        );
                                    }
                                    Self::verify_authority_row(
                                        &row,
                                        &target_state,
                                        &self.keyring,
                                    )?;
                                }
                            }
                        }
                        McpAuditDurableReferenceOrigin::ExistingStoreRotation => {
                            if fresh_initialization_permit.is_some() {
                                anyhow::bail!(
                                    "mcp_audit_rotation_rejected_fresh_initialization_permit"
                                );
                            }
                            let previous_epoch = previous_active_epoch.ok_or_else(|| {
                                anyhow::anyhow!(
                                    "mcp_audit_rotation_reference_previous_epoch_missing"
                                )
                            })?;
                            if !Self::authority_table_exists(&tx)? {
                                anyhow::bail!("mcp_audit_rotation_authority_binding_missing");
                            }
                            let row = Self::read_authority_row(&tx)?.ok_or_else(|| {
                                anyhow::anyhow!("mcp_audit_rotation_authority_binding_missing")
                            })?;
                            if row.key_epoch == pending_epoch {
                                Self::verify_authority_row(
                                    &row,
                                    &target_state,
                                    &self.keyring,
                                )?;
                            } else if row.key_epoch == previous_epoch {
                                let previous_state =
                                    Self::authenticate_authority_row(&row, &self.keyring)?;
                                let previous_keys = &reference_proof.keys
                                    [..reference_proof.keys.len().saturating_sub(1)];
                                if previous_state.store_identity_digest != store_identity_digest
                                    || previous_state.canonical_slot_digest != expected_slot_digest
                                    || previous_state.database_identity_digest
                                        != database_identity_digest
                                    || previous_state.key_epoch != previous_epoch
                                    || previous_state.epoch_set_digest
                                        != Self::epoch_set_digest(previous_keys)?
                                    || previous_state.epoch_count != previous_keys.len() as u64
                                    || previous_state.transition_generation
                                        != Self::transition_generation(
                                            &reference_proof.store_identity,
                                            previous_epoch,
                                        )
                                    || previous_state.reference_phase
                                        != McpAuditDurableReferencePhase::Active
                                    || previous_state
                                        .successor_active_reference_digest
                                        .is_some()
                                    || previous_state.migration_id.is_some()
                                    || !valid_mcp_audit_secret_digest(
                                        &previous_state.reference_digest,
                                    )
                                {
                                    anyhow::bail!(
                                        "mcp_audit_rotation_predecessor_manifest_mismatch"
                                    );
                                }
                                let pending_key = self.keyring.get(&pending_epoch).ok_or_else(
                                    || anyhow::anyhow!("mcp_audit_pending_key_unavailable"),
                                )?;
                                Self::write_authority_row(&tx, &target_state, pending_key)?;
                            } else {
                                anyhow::bail!("mcp_audit_rotation_previous_epoch_mismatch");
                            }
                        }
                        McpAuditDurableReferenceOrigin::LegacyMigration => {
                            anyhow::bail!(
                                "mcp_audit_legacy_authority_cutover_proof_required"
                            );
                        }
                    }
                }
            }
            tx.commit()?;
            Ok(())
        })?;
        self.authority = Some(target_state);
        Ok(())
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

    /// Mint one exact reference replacement while retaining the current
    /// stable authority generation read guard across the filesystem commit.
    /// This path is used only after bootstrap has activated the exact sealed
    /// Prepared receipt and needs to publish its Active successor.
    pub fn authorize_reference_transition(
        &self,
        previous: &McpAuditDurableReferenceReceipt,
        next: &McpAuditDurableReferenceDocument,
    ) -> Result<McpAuditReferenceMutationPermit<'_>> {
        self.require_writes_enabled()?;
        previous.revalidate_visible()?;
        let guard = self.durable_reference_authority.read_arc();
        match &*guard {
            McpAuditDurableReferenceAuthority::Stable { proof, .. }
                if receipt_matches_reference_proof(previous, proof) => {}
            _ => anyhow::bail!("mcp_audit_reference_stable_generation_mismatch"),
        }
        if !previous.document.allows_transition_to(next) {
            anyhow::bail!("mcp_audit_reference_transition_not_monotonic");
        }
        McpAuditReferenceMutationPermit::replace_exact(
            McpAuditReferenceMutationAuthority::StableStore {
                store: self,
                _guard: guard,
            },
            previous,
            next.clone(),
        )
    }

    fn validate_reference_effect(
        &self,
        permit: &McpAuditReferenceMutationPermit<'_>,
    ) -> Result<()> {
        self.require_writes_enabled()?;
        if permit.kind != McpAuditReferenceMutationKind::ReplaceExact
            || permit.reference_path != self.db_path.with_file_name("mcp_audit_keys.json")
        {
            anyhow::bail!("mcp_audit_store_reference_effect_mismatch");
        }
        let previous = permit
            .previous
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_reference_predecessor_missing"))?;
        let guard = match &permit.authority {
            McpAuditReferenceMutationAuthority::StableStore { _guard, .. } => _guard,
            _ => anyhow::bail!("mcp_audit_store_reference_guard_missing"),
        };
        match &**guard {
            McpAuditDurableReferenceAuthority::Stable { proof, .. }
                if receipt_matches_reference_proof(previous, proof) =>
            {
                Ok(())
            }
            _ => anyhow::bail!("mcp_audit_reference_stable_generation_changed"),
        }
    }

    pub fn begin_store_bound_key_rotation(
        &self,
        material: &AuditKeyMaterial,
        prepared_transition_id: &str,
    ) -> Result<McpAuditRotationTransition> {
        self.require_writes_enabled()?;
        if self.read_only || self.unavailable_reason.is_some() {
            anyhow::bail!("MCP audit key rotation requires the canonical writable owner");
        }
        if material.config.epoch <= self.key_config.epoch {
            anyhow::bail!("MCP audit key epoch must increase monotonically");
        }
        if material.config.mode != KeyMode::Keychain || material.config.key_ref.is_none() {
            anyhow::bail!("MCP audit rotation requires a keychain reference");
        }
        let transition_id = uuid::Uuid::parse_str(prepared_transition_id)
            .context("parse MCP audit prepared rotation transition id")?;
        if transition_id.is_nil() || transition_id.get_version_num() != 4 {
            anyhow::bail!("mcp_audit_rotation_transition_id_not_random_v4");
        }
        let mut authority_guard = self.durable_reference_authority.write_arc();
        // A writer may have poisoned this generation while begin was waiting
        // for the exclusive authority gate. Recheck only after owning the gate
        // so no transition can start from an already-invalid generation.
        self.require_writes_enabled()?;
        let (previous_generation_id, previous_proof) = match &*authority_guard {
            McpAuditDurableReferenceAuthority::Stable {
                generation_id,
                proof,
            } => {
                self.validate_local_writer_against_proof(proof, true)?;
                self.validate_reference_proof_locked(proof)?;
                (*generation_id, proof.clone())
            }
            McpAuditDurableReferenceAuthority::UnboundFixture => {
                anyhow::bail!("mcp_audit_product_authority_binding_missing");
            }
            McpAuditDurableReferenceAuthority::Transitioning { .. } => {
                anyhow::bail!("mcp_audit_reference_transition_in_progress");
            }
        };
        let reference_path = previous_proof.reference_path.clone();
        *authority_guard = McpAuditDurableReferenceAuthority::Transitioning {
            transition_id,
            previous_generation_id,
            previous_proof,
            pending_epoch: material.config.epoch,
        };
        Ok(McpAuditRotationTransition {
            transition_id,
            reference_path,
            authority_guard: Some(authority_guard),
            write_poison: self.write_poison.clone(),
            pending_epoch: material.config.epoch,
            database_outcome: McpAuditRotationDatabaseOutcome::NotAttempted,
            completed: false,
        })
    }

    fn rotation_guard<'a>(
        &self,
        transition: &'a mut McpAuditRotationTransition,
    ) -> Result<&'a mut McpAuditDurableReferenceAuthority> {
        // Every transition operation already owns the authority write guard.
        // Read poison only within that lock order; no caller may retain the
        // poison mutex while attempting to acquire the authority lock.
        self.require_writes_enabled()?;
        if transition.completed || !Arc::ptr_eq(&transition.write_poison, &self.write_poison) {
            anyhow::bail!("mcp_audit_rotation_transition_owner_mismatch");
        }
        let guard = transition
            .authority_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_rotation_transition_guard_missing"))?;
        if !Arc::ptr_eq(
            ArcRwLockWriteGuard::rwlock(guard),
            &self.durable_reference_authority,
        ) {
            anyhow::bail!("mcp_audit_rotation_transition_owner_mismatch");
        }
        Ok(&mut **guard)
    }

    fn poison_rotation_failure(&self, reason: &str, error: anyhow::Error) -> anyhow::Error {
        let detail = error.to_string();
        match self.set_write_poison_locked(format!("{reason}:{detail}")) {
            Ok(()) => anyhow::anyhow!("{reason}:{detail}"),
            Err(poison) => {
                anyhow::anyhow!("{reason}:{detail}; mcp_audit_write_poison_failed:{poison}")
            }
        }
    }

    pub fn abort_store_bound_key_rotation_not_committed(
        &self,
        transition: &mut McpAuditRotationTransition,
    ) -> Result<()> {
        if transition.database_outcome != McpAuditRotationDatabaseOutcome::NotAttempted {
            anyhow::bail!("mcp_audit_rotation_database_already_attempted");
        }
        let transition_id = transition.transition_id;
        let (previous_generation_id, previous_proof) = {
            let authority = self.rotation_guard(transition)?;
            match authority {
                McpAuditDurableReferenceAuthority::Transitioning {
                    transition_id: current,
                    previous_generation_id,
                    previous_proof,
                    ..
                } if *current == transition_id => (*previous_generation_id, previous_proof.clone()),
                _ => anyhow::bail!("mcp_audit_rotation_transition_state_mismatch"),
            }
        };
        self.validate_reference_proof_locked(&previous_proof)?;
        *self.rotation_guard(transition)? = McpAuditDurableReferenceAuthority::Stable {
            generation_id: previous_generation_id,
            proof: previous_proof,
        };
        transition.completed = true;
        transition.authority_guard.take();
        Ok(())
    }

    /// Linearize a durable Prepared reference into the authenticated database
    /// while the opaque transition owns the exclusive reference/write gate.
    pub fn commit_store_bound_key_rotation(
        &mut self,
        material: AuditKeyMaterial,
        receipt: McpAuditDurableReferenceReceipt,
        transition: &mut McpAuditRotationTransition,
    ) -> Result<()> {
        if transition.database_outcome != McpAuditRotationDatabaseOutcome::NotAttempted
            || transition.pending_epoch != material.config.epoch
        {
            anyhow::bail!("mcp_audit_rotation_transition_epoch_mismatch");
        }
        let transition_id = transition.transition_id;
        let previous_proof = match self.rotation_guard(transition)? {
            McpAuditDurableReferenceAuthority::Transitioning {
                transition_id: current,
                previous_proof,
                pending_epoch,
                ..
            } if *current == transition_id && *pending_epoch == material.config.epoch => {
                previous_proof.clone()
            }
            _ => anyhow::bail!("mcp_audit_rotation_transition_state_mismatch"),
        };
        receipt.revalidate_visible()?;
        let prepared_proof = receipt.into_proof();
        let current_authority = self
            .authority
            .clone()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_product_authority_binding_missing"))?;
        if current_authority.key_epoch != self.key_config.epoch {
            anyhow::bail!("mcp_audit_in_memory_authority_epoch_mismatch");
        }
        match prepared_proof.transition {
            McpAuditAuthorityTransition::Prepare {
                transition_id: prepared_transition_id,
                previous_active_epoch,
                pending_epoch,
                origin: McpAuditDurableReferenceOrigin::ExistingStoreRotation,
                secret_state: McpAuditDurableSecretState::Verified,
                pending_secret_digest,
                database_state: McpAuditDurableDatabaseState::Attempted,
            } if previous_active_epoch == Some(self.key_config.epoch)
                && prepared_transition_id == transition_id
                && parse_mcp_audit_secret_digest(&mcp_audit_secret_value_digest(&material.key))
                    == Some(pending_secret_digest)
                && pending_epoch == material.config.epoch => {}
            _ => anyhow::bail!("mcp_audit_rotation_reference_transition_mismatch"),
        }
        let expected_store_digest = Self::tagged_digest(
            b"openlife-mcp-audit-store-identity-v1",
            uuid::Uuid::parse_str(&prepared_proof.store_identity)?
                .simple()
                .to_string()
                .as_bytes(),
        );
        if prepared_proof.store_identity != previous_proof.store_identity
            || prepared_proof.canonical_slot_digest != previous_proof.canonical_slot_digest
            || prepared_proof.canonical_slot_digest != current_authority.canonical_slot_digest
            || expected_store_digest != current_authority.store_identity_digest
        {
            anyhow::bail!("mcp_audit_rotation_reference_authority_mismatch");
        }
        let mut expected_configs = self.key_configs.clone();
        expected_configs.push(material.config.clone());
        if prepared_proof.keys.len() != expected_configs.len()
            || prepared_proof
                .keys
                .iter()
                .zip(&expected_configs)
                .any(|(durable, expected)| !same_key_config(durable, expected))
        {
            anyhow::bail!("mcp_audit_rotation_reference_keyring_mismatch");
        }
        let next_authority = Self::authority_state_for_reference(
            &current_authority.store_identity_digest,
            &current_authority.canonical_slot_digest,
            &current_authority.database_identity_digest,
            &prepared_proof,
            None,
        )?;
        self.validate_reference_proof_locked(&prepared_proof)?;
        // From this point on, abort-to-A is permanently forbidden. A failed
        // checked operation may have committed B before losing postflight
        // identity proof, so its outcome is unknown and the generation must be
        // poisoned rather than "rolled back" by reference manipulation.
        transition.database_outcome = McpAuditRotationDatabaseOutcome::AttemptedUnknown;
        let database_result = self.with_checked_database_operation(|conn| {
            let tx = conn.transaction()?;
            if !Self::authority_table_exists(&tx)? {
                anyhow::bail!("mcp_audit_authority_binding_missing");
            }
            let row = Self::read_authority_row(&tx)?
                .ok_or_else(|| anyhow::anyhow!("mcp_audit_authority_binding_missing"))?;
            Self::verify_authority_row(&row, &current_authority, &self.keyring)?;
            if row.key_epoch != current_authority.key_epoch {
                anyhow::bail!("mcp_audit_authority_rotation_epoch_mismatch");
            }
            Self::write_authority_row(&tx, &next_authority, &material.key)?;
            tx.commit()?;
            Ok(())
        });
        if let Err(error) = database_result {
            return Err(
                self.poison_rotation_failure("mcp_audit_rotation_database_outcome_unknown", error)
            );
        }
        transition.database_outcome = McpAuditRotationDatabaseOutcome::Committed;
        if let Err(error) = self.validate_reference_proof_locked(&prepared_proof) {
            return Err(self.poison_rotation_failure(
                "mcp_audit_rotation_prepared_reference_postcommit_failed",
                error,
            ));
        }

        self.key = material.key;
        self.key_config = material.config.clone();
        self.keyring.insert(material.config.epoch, material.key);
        self.key_configs.push(material.config.clone());
        self.key_configs.sort_by_key(|config| config.epoch);
        self.authority = Some(next_authority);
        Ok(())
    }

    fn validate_active_proof_for_current_store(
        &self,
        proof: &McpAuditDurableReferenceProof,
    ) -> Result<McpAuditAuthorityState> {
        if !proof.is_active() || proof.bound_epoch() != self.key_config.epoch {
            anyhow::bail!("mcp_audit_active_reference_epoch_mismatch");
        }
        let current_authority = self
            .authority
            .clone()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_product_authority_binding_missing"))?;
        let expected_store_digest = Self::tagged_digest(
            b"openlife-mcp-audit-store-identity-v1",
            uuid::Uuid::parse_str(&proof.store_identity)?
                .simple()
                .to_string()
                .as_bytes(),
        );
        if proof.canonical_slot_digest != current_authority.canonical_slot_digest
            || expected_store_digest != current_authority.store_identity_digest
            || current_authority.key_epoch != proof.bound_epoch()
            || proof.keys.len() != self.key_configs.len()
            || proof
                .keys
                .iter()
                .zip(&self.key_configs)
                .any(|(durable, local)| !same_key_config(durable, local))
        {
            anyhow::bail!("mcp_audit_active_reference_authority_mismatch");
        }
        let active_authority = Self::authority_state_for_reference(
            &current_authority.store_identity_digest,
            &current_authority.canonical_slot_digest,
            &current_authority.database_identity_digest,
            proof,
            current_authority.migration_id.clone(),
        )?;
        if !Self::prepared_manifest_allows_active_successor(&current_authority, &active_authority) {
            anyhow::bail!("mcp_audit_active_reference_manifest_transition_mismatch");
        }
        Ok(active_authority)
    }

    fn verify_database_authority(
        &self,
        conn: &mut Connection,
        expected: &McpAuditAuthorityState,
    ) -> Result<()> {
        let tx = conn.transaction()?;
        if !Self::authority_table_exists(&tx)? {
            anyhow::bail!("mcp_audit_authority_binding_missing");
        }
        let row = Self::read_authority_row(&tx)?
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_authority_binding_missing"))?;
        Self::verify_authority_row(&row, expected, &self.keyring)?;
        if row.key_epoch != expected.key_epoch {
            anyhow::bail!("mcp_audit_active_reference_epoch_mismatch");
        }
        tx.commit()?;
        Ok(())
    }

    fn transition_database_authority(
        &self,
        conn: &mut Connection,
        expected: &McpAuditAuthorityState,
        next: &McpAuditAuthorityState,
    ) -> Result<()> {
        let tx = conn.transaction()?;
        if !Self::authority_table_exists(&tx)? {
            anyhow::bail!("mcp_audit_authority_binding_missing");
        }
        let row = Self::read_authority_row(&tx)?
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_authority_binding_missing"))?;
        Self::verify_authority_row(&row, expected, &self.keyring)?;
        if !Self::prepared_manifest_allows_active_successor(expected, next) {
            anyhow::bail!("mcp_audit_authority_phase_transition_invalid");
        }
        let active_key = self
            .keyring
            .get(&next.key_epoch)
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_authority_key_epoch_unavailable"))?;
        Self::write_authority_row(&tx, next, active_key)?;
        tx.commit()?;
        Ok(())
    }

    pub fn install_active_reference_after_rotation(
        &mut self,
        receipt: McpAuditDurableReferenceReceipt,
        transition: &mut McpAuditRotationTransition,
    ) -> Result<()> {
        if transition.database_outcome != McpAuditRotationDatabaseOutcome::Committed
            || transition.pending_epoch != self.key_config.epoch
        {
            anyhow::bail!("mcp_audit_rotation_database_not_committed");
        }
        let transition_id = transition.transition_id;
        match self.rotation_guard(transition)? {
            McpAuditDurableReferenceAuthority::Transitioning {
                transition_id: current,
                pending_epoch,
                ..
            } if *current == transition_id && *pending_epoch == self.key_config.epoch => {}
            _ => anyhow::bail!("mcp_audit_rotation_transition_state_mismatch"),
        }
        if let Err(error) = receipt.revalidate_visible() {
            return Err(self.poison_rotation_failure(
                "mcp_audit_rotation_active_receipt_revalidation_failed",
                error,
            ));
        }
        let active_proof = receipt.into_proof();
        let active_authority = self
            .validate_active_proof_for_current_store(&active_proof)
            .map_err(|error| {
                self.poison_rotation_failure("mcp_audit_rotation_active_proof_invalid", error)
            })?;
        self.validate_reference_proof_locked(&active_proof)?;
        let prepared_authority = self
            .authority
            .clone()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_product_authority_binding_missing"))?;
        if let Err(error) = self.with_checked_database_operation(|conn| {
            self.transition_database_authority(conn, &prepared_authority, &active_authority)
        }) {
            return Err(self.poison_rotation_failure(
                "mcp_audit_rotation_active_database_verification_failed",
                error,
            ));
        }
        self.validate_reference_proof_locked(&active_proof)?;
        self.authority = Some(active_authority);
        *self.rotation_guard(transition)? = McpAuditDurableReferenceAuthority::Stable {
            generation_id: uuid::Uuid::new_v4(),
            proof: active_proof,
        };
        transition.completed = true;
        transition.authority_guard.take();
        Ok(())
    }

    pub fn install_active_reference_after_bootstrap(
        &mut self,
        receipt: McpAuditDurableReferenceReceipt,
    ) -> Result<()> {
        receipt.revalidate_visible()?;
        let active_proof = receipt.into_proof();
        let active_authority = self.validate_active_proof_for_current_store(&active_proof)?;
        let prepared_authority = self
            .authority
            .clone()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_product_authority_binding_missing"))?;
        let mut authority_guard = self.durable_reference_authority.write_arc();
        self.require_writes_enabled()?;
        match &*authority_guard {
            McpAuditDurableReferenceAuthority::Stable { proof, .. }
                if proof.reference_path == active_proof.reference_path
                    && proof.store_identity == active_proof.store_identity
                    && proof.canonical_slot_digest == active_proof.canonical_slot_digest
                    && matches!(
                        (proof.transition, active_proof.transition),
                        (
                            McpAuditAuthorityTransition::Prepare {
                                pending_epoch,
                                ..
                            },
                            McpAuditAuthorityTransition::VerifyActive { active_epoch }
                        ) if pending_epoch == active_epoch
                    )
                    && proof.keys.len() == active_proof.keys.len()
                    && proof
                        .keys
                        .iter()
                        .zip(&active_proof.keys)
                        .all(|(left, right)| same_key_config(left, right)) => {}
            _ => anyhow::bail!("mcp_audit_bootstrap_reference_transition_mismatch"),
        }
        self.validate_reference_proof_locked(&active_proof)?;
        self.with_checked_database_operation(|conn| {
            self.transition_database_authority(conn, &prepared_authority, &active_authority)
        })?;
        self.validate_reference_proof_locked(&active_proof)?;
        self.authority = Some(active_authority);
        *authority_guard = McpAuditDurableReferenceAuthority::Stable {
            generation_id: uuid::Uuid::new_v4(),
            proof: active_proof,
        };
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn rotate_key_material(&mut self, material: AuditKeyMaterial) -> Result<()> {
        if self.authority.is_some() {
            anyhow::bail!("product authority rotation requires a durable reference receipt");
        }
        self.validate_writable_owner()?;
        if material.config.epoch <= self.key_config.epoch {
            anyhow::bail!("MCP audit key epoch must increase monotonically");
        }
        if material.config.mode != KeyMode::Keychain || material.config.key_ref.is_none() {
            anyhow::bail!("MCP audit rotation requires a keychain reference");
        }
        self.key = material.key;
        self.key_config = material.config.clone();
        self.keyring.insert(material.config.epoch, material.key);
        self.key_configs.push(material.config.clone());
        self.key_configs.sort_by_key(|config| config.epoch);
        Ok(())
    }

    pub fn validate_writable_owner(&self) -> Result<()> {
        if self.read_only || self.unavailable_reason.is_some() {
            anyhow::bail!("MCP audit key rotation requires the canonical writable owner");
        }
        self.with_product_write(|_| Ok(()))
    }

    fn validate_reference_proof_locked(&self, proof: &McpAuditDurableReferenceProof) -> Result<()> {
        if let Err(error) = proof.revalidate_visible() {
            let detail = error.to_string();
            let poison_error = self
                .set_write_poison_locked(format!(
                    "mcp_audit_reference_revalidation_failed:{detail}"
                ))
                .err();
            return match poison_error {
                Some(poison) => Err(anyhow::anyhow!(
                    "mcp_audit_reference_revalidation_failed:{detail}; write_poison_failed:{poison}"
                )),
                None => Err(anyhow::anyhow!(
                    "mcp_audit_reference_revalidation_failed:{detail}"
                )),
            };
        }
        Ok(())
    }

    fn validate_local_writer_against_proof(
        &self,
        proof: &McpAuditDurableReferenceProof,
        require_active: bool,
    ) -> Result<()> {
        if require_active && !proof.is_active() {
            anyhow::bail!("mcp_audit_reference_not_active");
        }
        if proof.bound_epoch() != self.key_config.epoch {
            anyhow::bail!("mcp_audit_store_clone_key_epoch_stale");
        }
        if self
            .authority
            .as_ref()
            .is_some_and(|authority| authority.key_epoch != proof.bound_epoch())
        {
            anyhow::bail!("mcp_audit_in_memory_authority_epoch_mismatch");
        }
        if proof.keys.len() != self.key_configs.len()
            || proof
                .keys
                .iter()
                .zip(&self.key_configs)
                .any(|(durable, local)| !same_key_config(durable, local))
        {
            anyhow::bail!("mcp_audit_store_clone_keyring_stale");
        }
        Ok(())
    }

    fn with_checked_database_operation<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        if let Some(reason) = &self.unavailable_reason {
            anyhow::bail!("mcp_audit_store_unavailable:{reason}");
        }
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_store_connection_unavailable"))?;
        match connection.with_checked_operation(operation) {
            Ok(value) => Ok(value),
            Err(error) => Err(anyhow::Error::new(error)),
        }
    }

    fn with_reference_checked_write<T>(
        &self,
        require_active: bool,
        poison_operation_failure: Option<(&std::cell::Cell<bool>, &'static str)>,
        operation: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        self.require_writes_enabled()?;
        run_write_after_poison_precheck_hook();
        // Product mutations are serialized by the same exclusive gate used by
        // rotation and external poison publication. SQLite already serializes
        // the retained connection, so this closes late-write races without
        // reducing actual store throughput.
        let authority = self.durable_reference_authority.write();
        // The write may have passed the first poison check and then blocked on
        // a transition. Transition failure poisons while holding the exclusive
        // authority guard, so recheck only after acquiring our read guard and
        // before touching reference state or SQLite.
        self.require_writes_enabled()?;
        let execute = |proof: Option<&McpAuditDurableReferenceProof>| -> Result<T> {
            if let Some(proof) = proof {
                self.validate_local_writer_against_proof(proof, require_active)?;
                self.validate_reference_proof_locked(proof)?;
            }
            let operation_result = self.with_checked_database_operation(operation);
            let identity_invalid = operation_result.as_ref().err().is_some_and(|error| {
                error
                    .downcast_ref::<crate::sqlite_migration::SqliteCheckedOperationError>()
                    .is_some_and(|checked| checked.invalidates_identity())
            });
            if identity_invalid {
                self.set_write_poison_locked("mcp_audit_database_identity_failed")?;
            }
            if let Some((failed, reason)) = poison_operation_failure {
                if failed.get() {
                    self.set_write_poison_locked(reason)?;
                }
            }
            let postflight = proof
                .map(|proof| self.validate_reference_proof_locked(proof))
                .unwrap_or(Ok(()));
            match (operation_result, postflight) {
                (_, Err(error)) => Err(error),
                (Err(error), Ok(())) => Err(error),
                (Ok(value), Ok(())) => Ok(value),
            }
        };
        match &*authority {
            McpAuditDurableReferenceAuthority::UnboundFixture => execute(None),
            McpAuditDurableReferenceAuthority::Stable { proof, .. } => execute(Some(proof)),
            McpAuditDurableReferenceAuthority::Transitioning { .. } => {
                anyhow::bail!("mcp_audit_reference_transition_in_progress");
            }
        }
    }

    fn with_product_write<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        let authority_verification_failed = std::cell::Cell::new(false);
        self.with_reference_checked_write(
            true,
            Some((
                &authority_verification_failed,
                "mcp_audit_product_authority_unknown",
            )),
            |conn| {
                let transaction = conn.transaction()?;
                if let Err(error) = self.verify_current_authority_in_transaction(&transaction) {
                    authority_verification_failed.set(true);
                    return Err(error
                        .context("mcp_audit_product_transaction_authority_verification_failed"));
                }
                let value = operation(&transaction)?;
                transaction.commit()?;
                Ok(value)
            },
        )
    }

    fn verify_current_authority_in_transaction(&self, transaction: &Transaction<'_>) -> Result<()> {
        let Some(expected) = self.authority.as_ref() else {
            return Ok(());
        };
        if !Self::authority_table_exists(transaction)? {
            anyhow::bail!("mcp_audit_authority_binding_missing");
        }
        let row = Self::read_authority_row(transaction)?
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_authority_binding_missing"))?;
        Self::verify_authority_row(&row, expected, &self.keyring)?;
        if row.key_epoch != expected.key_epoch {
            anyhow::bail!("mcp_audit_authority_binding_epoch_mismatch");
        }
        Ok(())
    }

    fn with_initialization_write<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        self.with_reference_checked_write(false, None, operation)
    }

    fn require_writes_enabled(&self) -> Result<()> {
        if let Some(reason) = self
            .write_poison
            .lock()
            .map_err(|error| anyhow::anyhow!("mcp_audit_write_poison_mutex:{error}"))?
            .clone()
        {
            anyhow::bail!("mcp_audit_store_writes_disabled:{reason}");
        }
        Ok(())
    }

    /// Permanently disable writes for this process generation. The poison is
    /// shared by every clone so an already-issued tool-resource snapshot cannot
    /// write after reference/DB authority becomes unknown. Recovery requires a
    /// fresh authenticated startup.
    fn set_write_poison_locked(&self, reason: impl Into<String>) -> Result<()> {
        let mut poison = self
            .write_poison
            .lock()
            .map_err(|error| anyhow::anyhow!("mcp_audit_write_poison_mutex:{error}"))?;
        if poison.is_none() {
            *poison = Some(reason.into());
        }
        Ok(())
    }

    pub fn poison_writes(&self, reason: impl Into<String>) -> Result<()> {
        // External poison publication shares the exact exclusive gate used by
        // product mutations. Therefore poison returning is a linearization
        // barrier: no earlier mutation remains in flight and no later mutation
        // can pass the under-gate poison check.
        let _authority_gate = self.durable_reference_authority.write();
        self.set_write_poison_locked(reason)
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

    fn init_tables(&self) -> Result<()> {
        self.with_initialization_write(|conn| {
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
            Ok(())
        })?;
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
        let rows = self.with_initialization_write(|conn| {
            let mut statement = conn.prepare(
                "SELECT id, arguments_encrypted, result_encrypted, key_epoch
                 FROM mcp_log
                 WHERE payload_minimized_version < ?1",
            )?;
            let collected = statement
                .query_map([MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?.max(0) as u64,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(anyhow::Error::from);
            collected
        })?;
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

        self.with_initialization_write(|conn| {
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
        })
    }

    pub fn insert_log(
        &self,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> Result<i64> {
        let args_enc = self.encrypt(&audit_arguments_receipt(arguments)?)?;
        let res_enc = self.encrypt(&audit_result_receipt(result))?;
        let created_at = chrono::Utc::now().to_rfc3339();
        self.with_product_write(|conn| {
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
        })
    }

    pub fn list_logs(&self, limit: usize) -> Result<Vec<McpLogEntry>> {
        let rows = self.with_checked_database_operation(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, tool_name, arguments_encrypted, result_encrypted, success, pii_found, created_at, key_epoch
                 FROM mcp_log
                 ORDER BY id DESC
                 LIMIT ?1",
            )?;
            let collected = stmt
                .query_map([limit], |row| {
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
                })?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(anyhow::Error::from);
            collected
        })?;
        let mut out = Vec::new();
        for (id, tool_name, args_enc, res_enc, success, pii_found, created_at, key_epoch) in rows {
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
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        self.with_product_write(|conn| {
            conn.execute(
                "DELETE FROM mcp_log WHERE created_at < ?1",
                [cutoff.to_rfc3339()],
            )
            .map_err(anyhow::Error::from)
        })
    }
}

impl McpAuditDurableWriter for McpAuditStore {
    fn clone_owned_writer(&self) -> Arc<dyn McpAuditDurableWriter> {
        // Direct stores are retained for core fixtures and embedded callers.
        // Tauri product execution uses its canonical resolver implementation,
        // so product resource snapshots never clone key material here.
        Arc::new(self.clone())
    }

    fn insert_log_durably(
        &self,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> Result<i64> {
        self.insert_log(tool_name, arguments, result, success, pii_found)
    }

    fn report_runtime_failure(&self, reason_code: &'static str, detail: &str) {
        McpAuditStore::report_runtime_failure(self, reason_code, detail);
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

    fn store_bound_material(identity: uuid::Uuid, epoch: u64, key: [u8; 32]) -> AuditKeyMaterial {
        AuditKeyMaterial {
            config: AuditKeyConfig {
                mode: KeyMode::Keychain,
                salt_b64: None,
                env_var: None,
                key_ref: Some(format!(
                    "keychain://com.openlife.desktop/mcp-audit-key-store-{}-epoch-{epoch}",
                    identity.simple()
                )),
                epoch,
                created_at: "2026-07-13T00:00:00Z".into(),
            },
            key,
        }
    }

    fn durable_reference_document(
        database_path: &Path,
        identity: uuid::Uuid,
        keys: &[AuditKeyMaterial],
        phase: &str,
        active_epoch: Option<u64>,
        pending_epoch: Option<u64>,
    ) -> McpAuditDurableReferenceDocument {
        McpAuditDurableReferenceDocument {
            version: 2,
            store_identity: identity.to_string(),
            canonical_slot_digest: crate::sqlite_migration::canonical_sqlite_slot_digest(
                database_path,
                "mcp_audit_store",
            )
            .unwrap(),
            phase: match phase {
                "prepared" => McpAuditDurableReferencePhase::Prepared,
                "active" => McpAuditDurableReferencePhase::Active,
                other => panic!("unsupported test phase: {other}"),
            },
            origin: if active_epoch.is_some() {
                McpAuditDurableReferenceOrigin::ExistingStoreRotation
            } else {
                McpAuditDurableReferenceOrigin::FreshCreate
            },
            transition_id: (phase == "prepared").then(|| uuid::Uuid::new_v4().to_string()),
            secret_state: McpAuditDurableSecretState::Verified,
            pending_secret_digest: (phase == "prepared").then(|| {
                mcp_audit_secret_value_digest(&keys.last().expect("prepared reference key").key)
            }),
            database_state: McpAuditDurableDatabaseState::Attempted,
            active_epoch,
            pending_epoch,
            keys: keys
                .iter()
                .map(|material| material.config.clone())
                .collect(),
        }
    }

    fn write_durable_reference(
        reference_path: &Path,
        database_path: &Path,
        identity: uuid::Uuid,
        keys: &[AuditKeyMaterial],
        phase: &str,
        active_epoch: Option<u64>,
        pending_epoch: Option<u64>,
    ) -> McpAuditDurableReferenceReceipt {
        let document = durable_reference_document(
            database_path,
            identity,
            keys,
            phase,
            active_epoch,
            pending_epoch,
        );
        crate::atomic_file::write_atomic(
            reference_path,
            &serde_json::to_vec_pretty(&document).unwrap(),
        )
        .unwrap();
        McpAuditDurableReferenceReceipt::load_for_store(reference_path, database_path).unwrap()
    }

    #[test]
    fn fresh_reference_rollback_commits_canonical_absence_and_retains_exact_tombstone() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("mcp_audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let identity = uuid::Uuid::new_v4();
        let material = store_bound_material(identity, 1, [19u8; 32]);
        let canonical_slot_digest = crate::sqlite_migration::canonical_sqlite_slot_digest(
            &database_path,
            "mcp_audit_store",
        )
        .unwrap();
        let pending_secret_digest = mcp_audit_secret_value_digest(&material.key);

        let reservation = McpAuditStore::reserve_writable_owner(&database_path).unwrap();
        let capability =
            McpAuditStore::authorize_fresh_database_creation(reservation, &reference_path).unwrap();
        let document = McpAuditDurableReferenceDocument::prepared(
            identity.to_string(),
            canonical_slot_digest,
            vec![material.config.clone()],
            None,
            material.config.epoch,
            McpAuditDurableReferenceOrigin::FreshCreate,
            pending_secret_digest,
        )
        .unwrap();
        let publish = capability
            .authorize_initial_reference_publish(&document)
            .unwrap();
        let receipt = publish.commit_write().unwrap();
        let expected_bytes = std::fs::read(&reference_path).unwrap();
        drop(capability);

        let restart_reservation = McpAuditStore::reserve_writable_owner(&database_path).unwrap();
        let rollback =
            McpAuditStore::authorize_fresh_reference_rollback(restart_reservation, &receipt)
                .unwrap();
        let returned_reservation = rollback
            .authorize_reference_delete()
            .unwrap()
            .commit_delete()
            .unwrap();

        assert!(!reference_path.exists());
        let tombstones = std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".mcp_audit_keys.json.rollback-"))
            })
            .collect::<Vec<_>>();
        assert_eq!(tombstones.len(), 1);
        assert_eq!(std::fs::read(&tombstones[0]).unwrap(), expected_bytes);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(std::fs::metadata(&tombstones[0]).unwrap().nlink(), 1);
        }

        McpAuditStore::authorize_fresh_database_creation(returned_reservation, &reference_path)
            .unwrap();
    }

    fn write_fresh_prepared_reference(
        reference_path: &Path,
        database_path: &Path,
        identity: uuid::Uuid,
        transition_id: uuid::Uuid,
        material: &AuditKeyMaterial,
        pending_secret_digest: String,
        database_state: McpAuditDurableDatabaseState,
    ) -> McpAuditDurableReferenceReceipt {
        let document = McpAuditDurableReferenceDocument {
            version: MCP_AUDIT_DURABLE_REFERENCE_VERSION,
            store_identity: identity.to_string(),
            canonical_slot_digest: crate::sqlite_migration::canonical_sqlite_slot_digest(
                database_path,
                "mcp_audit_store",
            )
            .unwrap(),
            phase: McpAuditDurableReferencePhase::Prepared,
            origin: McpAuditDurableReferenceOrigin::FreshCreate,
            transition_id: Some(transition_id.to_string()),
            secret_state: McpAuditDurableSecretState::Verified,
            pending_secret_digest: Some(pending_secret_digest),
            database_state,
            active_epoch: None,
            pending_epoch: Some(material.config.epoch),
            keys: vec![material.config.clone()],
        };
        crate::atomic_file::write_atomic(
            reference_path,
            &serde_json::to_vec_pretty(&document).unwrap(),
        )
        .unwrap();
        McpAuditDurableReferenceReceipt::load_for_store(reference_path, database_path).unwrap()
    }

    fn activate_fresh_product_store(
        database_path: &Path,
        reference_path: &Path,
        identity: uuid::Uuid,
        material: &AuditKeyMaterial,
    ) -> McpAuditStore {
        let reservation = McpAuditStore::reserve_writable_owner(database_path).unwrap();
        let capability =
            McpAuditStore::authorize_fresh_database_creation(reservation, reference_path).unwrap();
        let prepared = write_durable_reference(
            reference_path,
            database_path,
            identity,
            std::slice::from_ref(material),
            "prepared",
            None,
            Some(material.config.epoch),
        );
        let mut store = McpAuditStore::activate_fresh_store_bound_authority(
            database_path,
            vec![material.clone()],
            capability,
            prepared,
        )
        .unwrap();
        let active = write_durable_reference(
            reference_path,
            database_path,
            identity,
            std::slice::from_ref(material),
            "active",
            Some(material.config.epoch),
            None,
        );
        store
            .install_active_reference_after_bootstrap(active)
            .unwrap();
        store
    }

    #[test]
    fn prepared_manifest_reconciliation_rejects_forged_active_for_fresh_and_rotation() {
        for origin in [
            McpAuditDurableReferenceOrigin::FreshCreate,
            McpAuditDurableReferenceOrigin::ExistingStoreRotation,
        ] {
            let committed_active_digest = format!("sha256:{}", "ab".repeat(32));
            let prepared = McpAuditAuthorityState {
                store_identity_digest: "sha256:store".into(),
                canonical_slot_digest: "sha256:slot".into(),
                database_identity_digest: "sha256:database".into(),
                key_epoch: 9,
                epoch_set_digest: "sha256:epoch-set".into(),
                epoch_count: 2,
                reference_digest: format!("sha256:{}", "cd".repeat(32)),
                successor_active_reference_digest: Some(committed_active_digest.clone()),
                transition_generation: "sha256:generation".into(),
                reference_phase: McpAuditDurableReferencePhase::Prepared,
                reference_origin: origin,
                migration_id: None,
            };
            let mut active = prepared.clone();
            active.reference_digest = committed_active_digest;
            active.successor_active_reference_digest = None;
            active.reference_phase = McpAuditDurableReferencePhase::Active;
            assert!(McpAuditStore::prepared_manifest_allows_active_successor(
                &prepared, &active
            ));

            let mut forged_digest = active.clone();
            forged_digest.reference_digest = format!("sha256:{}", "ef".repeat(32));
            assert!(!McpAuditStore::prepared_manifest_allows_active_successor(
                &prepared,
                &forged_digest,
            ));

            let mut forged_origin = active;
            forged_origin.reference_origin = match origin {
                McpAuditDurableReferenceOrigin::FreshCreate => {
                    McpAuditDurableReferenceOrigin::ExistingStoreRotation
                }
                McpAuditDurableReferenceOrigin::ExistingStoreRotation => {
                    McpAuditDurableReferenceOrigin::FreshCreate
                }
                McpAuditDurableReferenceOrigin::LegacyMigration => unreachable!(),
            };
            assert!(!McpAuditStore::prepared_manifest_allows_active_successor(
                &prepared,
                &forged_origin,
            ));
        }
    }

    #[test]
    fn authority_hmac_uses_hkdf_domain_separation_from_payload_key() {
        struct TestKeyLength;
        impl hkdf::KeyType for TestKeyLength {
            fn len(&self) -> usize {
                32
            }
        }
        let payload_key = [0x5a; 32];
        let state = McpAuditAuthorityState {
            store_identity_digest: "sha256:store".into(),
            canonical_slot_digest: "sha256:slot".into(),
            database_identity_digest: "sha256:database".into(),
            key_epoch: 9,
            epoch_set_digest: "sha256:epoch-set".into(),
            epoch_count: 2,
            reference_digest: format!("sha256:{}", "ab".repeat(32)),
            successor_active_reference_digest: Some(format!("sha256:{}", "cd".repeat(32))),
            transition_generation: "sha256:generation".into(),
            reference_phase: McpAuditDurableReferencePhase::Prepared,
            reference_origin: McpAuditDurableReferenceOrigin::FreshCreate,
            migration_id: None,
        };
        let message = McpAuditStore::authority_verifier_message(&state);
        let separated = McpAuditStore::authority_verifier(&state, &payload_key).unwrap();
        let direct = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &payload_key), &message);
        assert_ne!(
            separated,
            format!(
                "hmac-sha256:{}",
                general_purpose::STANDARD_NO_PAD.encode(direct.as_ref())
            )
        );

        let wrong_salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"wrong-authority-domain");
        let wrong_prk = wrong_salt.extract(&payload_key);
        let wrong_info = [b"wrong-authority-domain".as_slice()];
        let wrong_okm = wrong_prk.expand(&wrong_info, TestKeyLength).unwrap();
        let mut wrong_key = [0u8; 32];
        wrong_okm.fill(&mut wrong_key).unwrap();
        let wrong_tag = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &wrong_key), &message);
        let correct_key = McpAuditStore::authority_mac_key(&payload_key).unwrap();
        assert!(hmac::verify(
            &hmac::Key::new(hmac::HMAC_SHA256, &correct_key),
            &message,
            wrong_tag.as_ref(),
        )
        .is_err());
    }

    #[test]
    fn durable_receipt_rejects_noncanonical_and_duplicate_v2_json() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let identity = uuid::Uuid::new_v4();
        let epoch = 300;
        let material = store_bound_material(identity, epoch, [0x30; 32]);
        let document = McpAuditDurableReferenceDocument {
            version: MCP_AUDIT_DURABLE_REFERENCE_VERSION,
            store_identity: identity.to_string(),
            canonical_slot_digest: crate::sqlite_migration::canonical_sqlite_slot_digest(
                &database_path,
                "mcp_audit_store",
            )
            .unwrap(),
            phase: McpAuditDurableReferencePhase::Prepared,
            origin: McpAuditDurableReferenceOrigin::FreshCreate,
            transition_id: Some(uuid::Uuid::new_v4().to_string()),
            secret_state: McpAuditDurableSecretState::Verified,
            pending_secret_digest: Some(format!("sha256:{}", "ab".repeat(32))),
            database_state: McpAuditDurableDatabaseState::Attempted,
            active_epoch: None,
            pending_epoch: Some(epoch),
            keys: vec![material.config.clone()],
        };

        std::fs::write(&reference_path, serde_json::to_vec(&document).unwrap()).unwrap();
        let noncanonical =
            McpAuditDurableReferenceReceipt::load_for_store(&reference_path, &database_path)
                .unwrap_err()
                .to_string();
        assert!(
            noncanonical.contains("mcp_audit_reference_noncanonical_or_ambiguous_json"),
            "{noncanonical}"
        );

        let canonical = serde_json::to_string_pretty(&document).unwrap();
        let duplicate = canonical.replacen(
            "  \"version\": 2,",
            "  \"version\": 2,\n  \"version\": 2,",
            1,
        );
        std::fs::write(&reference_path, duplicate).unwrap();
        let ambiguous = format!(
            "{:#}",
            McpAuditDurableReferenceReceipt::load_for_store(&reference_path, &database_path)
                .unwrap_err()
        );
        assert!(ambiguous.contains("duplicate field"), "{ambiguous}");
    }

    #[test]
    fn fresh_recovery_capability_requires_exact_secret_digest_not_origin_claim() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let identity = uuid::Uuid::new_v4();
        let material = store_bound_material(identity, 304, [0x34; 32]);
        let encoded = general_purpose::STANDARD.encode(material.key);
        let mut digest = ring::digest::Context::new(&SHA256);
        digest.update(b"openlife-mcp-audit-secret-value-v1\0");
        digest.update(encoded.as_bytes());
        let expected_digest = format!(
            "sha256:{}",
            digest
                .finish()
                .as_ref()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let document = McpAuditDurableReferenceDocument {
            version: MCP_AUDIT_DURABLE_REFERENCE_VERSION,
            store_identity: identity.to_string(),
            canonical_slot_digest: crate::sqlite_migration::canonical_sqlite_slot_digest(
                &database_path,
                "mcp_audit_store",
            )
            .unwrap(),
            phase: McpAuditDurableReferencePhase::Prepared,
            origin: McpAuditDurableReferenceOrigin::FreshCreate,
            transition_id: Some(uuid::Uuid::new_v4().to_string()),
            secret_state: McpAuditDurableSecretState::Verified,
            pending_secret_digest: Some(expected_digest),
            database_state: McpAuditDurableDatabaseState::NotAttempted,
            active_epoch: None,
            pending_epoch: Some(material.config.epoch),
            keys: vec![material.config.clone()],
        };
        crate::atomic_file::write_atomic(
            &reference_path,
            &serde_json::to_vec_pretty(&document).unwrap(),
        )
        .unwrap();
        let receipt =
            McpAuditDurableReferenceReceipt::load_for_store(&reference_path, &database_path)
                .unwrap();
        let forged_material = AuditKeyMaterial {
            config: material.config.clone(),
            key: [0xff; 32],
        };

        let error = McpAuditStore::authorize_fresh_database_recovery(
            McpAuditStore::reserve_writable_owner(&database_path).unwrap(),
            &receipt,
            &[forged_material],
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("mcp_audit_fresh_recovery_secret_digest_mismatch"));

        McpAuditStore::authorize_fresh_database_recovery(
            McpAuditStore::reserve_writable_owner(&database_path).unwrap(),
            &receipt,
            &[material],
        )
        .unwrap();
    }

    #[test]
    fn fresh_recovery_capability_rejects_changed_transition_and_changed_digest_at_activation() {
        // The persisted transition id must survive Pending -> Verified -> Attempted.
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let identity = uuid::Uuid::new_v4();
        let transition_id = uuid::Uuid::new_v4();
        let material = store_bound_material(identity, 307, [0x37; 32]);
        let digest = mcp_audit_secret_value_digest(&material.key);
        let receipt = write_fresh_prepared_reference(
            &reference_path,
            &database_path,
            identity,
            transition_id,
            &material,
            digest.clone(),
            McpAuditDurableDatabaseState::NotAttempted,
        );
        let capability = McpAuditStore::authorize_fresh_database_recovery(
            McpAuditStore::reserve_writable_owner(&database_path).unwrap(),
            &receipt,
            std::slice::from_ref(&material),
        )
        .unwrap();
        let changed_receipt = write_fresh_prepared_reference(
            &reference_path,
            &database_path,
            identity,
            uuid::Uuid::new_v4(),
            &material,
            digest,
            McpAuditDurableDatabaseState::Attempted,
        );
        let error = match McpAuditStore::activate_fresh_store_bound_authority(
            &database_path,
            vec![material],
            capability,
            changed_receipt,
        ) {
            Ok(_) => panic!("changed recovery transition must not activate"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("mcp_audit_fresh_recovery_generation_changed"));
        assert!(!database_path.exists());

        // A new raw key cannot be substituted by rewriting the current digest:
        // it may match the current receipt, but it cannot match the digest bound
        // into the capability issued from the earlier exact receipt.
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let identity = uuid::Uuid::new_v4();
        let transition_id = uuid::Uuid::new_v4();
        let authorized = store_bound_material(identity, 308, [0x38; 32]);
        let substituted = store_bound_material(identity, 308, [0x39; 32]);
        let receipt = write_fresh_prepared_reference(
            &reference_path,
            &database_path,
            identity,
            transition_id,
            &authorized,
            mcp_audit_secret_value_digest(&authorized.key),
            McpAuditDurableDatabaseState::NotAttempted,
        );
        let capability = McpAuditStore::authorize_fresh_database_recovery(
            McpAuditStore::reserve_writable_owner(&database_path).unwrap(),
            &receipt,
            std::slice::from_ref(&authorized),
        )
        .unwrap();
        let substituted_receipt = write_fresh_prepared_reference(
            &reference_path,
            &database_path,
            identity,
            transition_id,
            &substituted,
            mcp_audit_secret_value_digest(&substituted.key),
            McpAuditDurableDatabaseState::Attempted,
        );
        let error = match McpAuditStore::activate_fresh_store_bound_authority(
            &database_path,
            vec![substituted],
            capability,
            substituted_receipt,
        ) {
            Ok(_) => panic!("changed recovery secret digest must not activate"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("mcp_audit_fresh_recovery_generation_changed"));
        assert!(!database_path.exists());
    }

    #[test]
    fn authority_row_initialization_is_origin_and_permit_specific() {
        fn table_exists(path: &Path, name: &str) -> bool {
            Connection::open(path)
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) = 1 FROM sqlite_master WHERE type='table' AND name=?1",
                    [name],
                    |row| row.get(0),
                )
                .unwrap()
        }

        // A Fresh receipt over an already-existing DB cannot manufacture the
        // first authority row without the consumed fresh capability.
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("fresh-existing.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        Connection::open(&database_path)
            .unwrap()
            .execute("CREATE TABLE unrelated(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        let identity = uuid::Uuid::new_v4();
        let material = store_bound_material(identity, 309, [0x40; 32]);
        let receipt = write_fresh_prepared_reference(
            &reference_path,
            &database_path,
            identity,
            uuid::Uuid::new_v4(),
            &material,
            mcp_audit_secret_value_digest(&material.key),
            McpAuditDurableDatabaseState::Attempted,
        );
        let error = match McpAuditStore::activate_store_bound_authority(
            &database_path,
            vec![material],
            McpAuditStore::reserve_writable_owner(&database_path).unwrap(),
            receipt,
        ) {
            Ok(_) => panic!("fresh existing database without permit must fail"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("mcp_audit_fresh_authority_initialization_permit_missing"));
        assert!(!table_exists(&database_path, "mcp_audit_authority_binding"));
        assert!(!table_exists(&database_path, "mcp_log"));

        // Rotation must authenticate an exact predecessor row; a bare existing
        // database and a prepared JSON claim are insufficient.
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("rotation-missing-row.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        Connection::open(&database_path)
            .unwrap()
            .execute("CREATE TABLE unrelated(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        let identity = uuid::Uuid::new_v4();
        let previous = store_bound_material(identity, 310, [0x41; 32]);
        let pending = store_bound_material(identity, 311, [0x42; 32]);
        let receipt = write_durable_reference(
            &reference_path,
            &database_path,
            identity,
            &[previous.clone(), pending.clone()],
            "prepared",
            Some(previous.config.epoch),
            Some(pending.config.epoch),
        );
        let error = match McpAuditStore::activate_store_bound_authority(
            &database_path,
            vec![previous, pending],
            McpAuditStore::reserve_writable_owner(&database_path).unwrap(),
            receipt,
        ) {
            Ok(_) => panic!("rotation without predecessor authority row must fail"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("mcp_audit_rotation_authority_binding_missing"));
        assert!(!table_exists(&database_path, "mcp_audit_authority_binding"));
        assert!(!table_exists(&database_path, "mcp_log"));

        // Legacy cutover remains a named D057/D064-B RED until an exact
        // predecessor and offline cutover proof exist; it must not initialize
        // either the authority or product schema in the meantime.
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("legacy-proof-required.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        Connection::open(&database_path)
            .unwrap()
            .execute("CREATE TABLE unrelated(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        let identity = uuid::Uuid::new_v4();
        let previous = store_bound_material(identity, 312, [0x43; 32]);
        let pending = store_bound_material(identity, 313, [0x44; 32]);
        let mut document = durable_reference_document(
            &database_path,
            identity,
            &[previous.clone(), pending.clone()],
            "prepared",
            Some(previous.config.epoch),
            Some(pending.config.epoch),
        );
        document.origin = McpAuditDurableReferenceOrigin::LegacyMigration;
        crate::atomic_file::write_atomic(
            &reference_path,
            &serde_json::to_vec_pretty(&document).unwrap(),
        )
        .unwrap();
        let receipt =
            McpAuditDurableReferenceReceipt::load_for_store(&reference_path, &database_path)
                .unwrap();
        let error = match McpAuditStore::activate_store_bound_authority(
            &database_path,
            vec![previous, pending],
            McpAuditStore::reserve_writable_owner(&database_path).unwrap(),
            receipt,
        ) {
            Ok(_) => panic!("legacy cutover without D057 proof must fail"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("mcp_audit_legacy_authority_cutover_proof_required"));
        assert!(!table_exists(&database_path, "mcp_audit_authority_binding"));
        assert!(!table_exists(&database_path, "mcp_log"));
    }

    #[test]
    fn unsealed_prepared_reference_is_rejected_before_database_creation() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let identity = uuid::Uuid::new_v4();
        let material = store_bound_material(identity, 305, [0x35; 32]);
        let capability = McpAuditStore::authorize_fresh_database_creation(
            McpAuditStore::reserve_writable_owner(&database_path).unwrap(),
            &reference_path,
        )
        .unwrap();
        let mut document = durable_reference_document(
            &database_path,
            identity,
            std::slice::from_ref(&material),
            "prepared",
            None,
            Some(material.config.epoch),
        );
        document.secret_state = McpAuditDurableSecretState::Pending;
        document.database_state = McpAuditDurableDatabaseState::NotAttempted;
        crate::atomic_file::write_atomic(
            &reference_path,
            &serde_json::to_vec_pretty(&document).unwrap(),
        )
        .unwrap();
        let receipt =
            McpAuditDurableReferenceReceipt::load_for_store(&reference_path, &database_path)
                .unwrap();

        let error = match McpAuditStore::activate_fresh_store_bound_authority(
            &database_path,
            vec![material],
            capability,
            receipt,
        ) {
            Ok(_) => panic!("unsealed reference must not create a database"),
            Err(error) => error,
        };

        assert!(error
            .to_string()
            .contains("mcp_audit_prepared_reference_not_sealed_for_database_transition"));
        assert!(!database_path.exists());
    }

    #[test]
    fn ordinary_activation_never_recreates_a_missing_database() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let identity = uuid::Uuid::new_v4();
        let material = store_bound_material(identity, 306, [0x36; 32]);
        let receipt = write_durable_reference(
            &reference_path,
            &database_path,
            identity,
            std::slice::from_ref(&material),
            "prepared",
            None,
            Some(material.config.epoch),
        );

        let error = match McpAuditStore::activate_store_bound_authority(
            &database_path,
            vec![material],
            McpAuditStore::reserve_writable_owner(&database_path).unwrap(),
            receipt,
        ) {
            Ok(_) => panic!("ordinary activation must not create a missing database"),
            Err(error) => error,
        };

        assert!(error
            .to_string()
            .contains("mcp_audit_existing_database_missing"));
        assert!(!database_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reference_reader_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.json");
        let link = directory.path().join("reference.json");
        std::fs::write(&target, b"{}").unwrap();
        symlink(&target, &link).unwrap();

        let error = read_bounded_mcp_audit_reference(&link)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("symlink") || error.contains("Too many levels"),
            "{error}"
        );
    }

    #[test]
    fn bounded_reference_reader_rejects_oversize_before_decode() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("reference.json");
        std::fs::write(&path, vec![b'x'; MCP_AUDIT_DURABLE_REFERENCE_MAX_BYTES + 1]).unwrap();

        let error = read_bounded_mcp_audit_reference(&path)
            .unwrap_err()
            .to_string();
        assert!(error.contains("mcp_audit_reference_too_large"), "{error}");
    }

    #[test]
    fn bounded_reference_reader_rejects_same_path_swap_during_read() {
        let directory = tempfile::tempdir().unwrap();
        let target = canonical_reference_path(&directory.path().join("reference.json")).unwrap();
        let replacement = target.with_file_name("replacement.json");
        let displaced = target.with_file_name("displaced.json");
        std::fs::write(&target, b"{\"first\":true}").unwrap();
        std::fs::write(&replacement, b"{\"second\":true}").unwrap();
        let _swap = inject_reference_post_read_swap(target.clone(), replacement, displaced);

        let error = read_bounded_mcp_audit_reference(&target)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("mcp_audit_reference_identity_changed_during_read"),
            "{error}"
        );
    }

    #[test]
    fn product_activation_exposes_only_the_opaque_durable_reference_receipt() {
        let source = include_str!("mcp_audit.rs");
        for forbidden in [
            ["pub struct ", "McpAuditAuthorityBinding"].concat(),
            ["pub enum ", "McpAuditAuthorityTransition"].concat(),
            ["pub fn ", "with_key_materials_and_reservation"].concat(),
        ] {
            assert!(
                !source.contains(&forbidden),
                "forbidden raw authority API: {forbidden}"
            );
        }
        assert!(source.contains("receipt: McpAuditDurableReferenceReceipt"));
        assert!(source.contains("pub struct McpAuditDurableReferenceReceipt"));
    }

    #[test]
    fn prepared_database_verifier_recovers_after_crash_before_product_schema() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let identity = uuid::Uuid::new_v4();
        let epoch = 301;
        let material = store_bound_material(identity, epoch, [0x31; 32]);
        let reservation = McpAuditStore::reserve_writable_owner(&path).unwrap();
        let capability =
            McpAuditStore::authorize_fresh_database_creation(reservation, &reference_path).unwrap();
        let receipt = write_durable_reference(
            &reference_path,
            &path,
            identity,
            std::slice::from_ref(&material),
            "prepared",
            None,
            Some(epoch),
        );
        let mut active_document = receipt.document().clone();
        let prepared_receipt = receipt.clone();
        let prepared_origin = active_document.origin();
        let prepared_transition_id = active_document
            .transition_id()
            .expect("prepared recovery fixture has one transition id")
            .to_string();
        let _fault = inject_authority_post_binding_failure(
            crate::sqlite_migration::canonical_sqlite_slot(&path, "mcp_audit_store").unwrap(),
        );
        let error = match McpAuditStore::activate_fresh_store_bound_authority(
            &path,
            vec![material.clone()],
            capability,
            receipt,
        ) {
            Ok(_) => panic!("post-binding failure must fail activation"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("injected_mcp_audit_post_authority_binding_failure"));
        drop(_fault);

        let connection = Connection::open(&path).unwrap();
        let authority_exists: bool = connection
            .query_row(
                "SELECT COUNT(*) = 1 FROM sqlite_master WHERE type='table' AND name='mcp_audit_authority_binding'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let product_table_exists: bool = connection
            .query_row(
                "SELECT COUNT(*) = 1 FROM sqlite_master WHERE type='table' AND name='mcp_log'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(authority_exists);
        assert!(!product_table_exists);
        drop(connection);

        let mut recovered = McpAuditStore::activate_store_bound_authority(
            &path,
            vec![material.clone()],
            McpAuditStore::reserve_writable_owner(&path).unwrap(),
            McpAuditDurableReferenceReceipt::load_for_store(&reference_path, &path).unwrap(),
        )
        .unwrap();
        active_document.mark_active().unwrap();
        assert_eq!(active_document.origin(), prepared_origin);
        assert_eq!(prepared_origin, McpAuditDurableReferenceOrigin::FreshCreate);
        assert!(!prepared_transition_id.is_empty());
        assert_eq!(active_document.transition_id(), None);
        let active_receipt = recovered
            .authorize_reference_transition(&prepared_receipt, &active_document)
            .unwrap()
            .commit_write()
            .unwrap();
        recovered
            .install_active_reference_after_bootstrap(active_receipt)
            .unwrap();
        recovered
            .insert_log("recovered", &serde_json::json!({}), "ok", true, false)
            .unwrap();
        drop(recovered);

        let active = McpAuditStore::activate_store_bound_authority(
            &path,
            vec![material],
            McpAuditStore::reserve_writable_owner(&path).unwrap(),
            McpAuditDurableReferenceReceipt::load_for_store(&reference_path, &path).unwrap(),
        )
        .unwrap();
        assert_eq!(active.list_logs(10).unwrap().len(), 1);
    }

    #[test]
    fn active_reference_never_bootstraps_a_database_without_authority_verifier() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        Connection::open(&path)
            .unwrap()
            .execute("CREATE TABLE unrelated(id INTEGER PRIMARY KEY)", [])
            .unwrap();
        let identity = uuid::Uuid::new_v4();
        let epoch = 302;
        let material = store_bound_material(identity, epoch, [0x32; 32]);
        let active_receipt = write_durable_reference(
            &reference_path,
            &path,
            identity,
            std::slice::from_ref(&material),
            "active",
            Some(epoch),
            None,
        );

        let error = match McpAuditStore::activate_store_bound_authority(
            &path,
            vec![material],
            McpAuditStore::reserve_writable_owner(&path).unwrap(),
            active_receipt,
        ) {
            Ok(_) => panic!("active reference without database verifier must fail"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("mcp_audit_active_reference_has_no_database_verifier"));
        let connection = Connection::open(&path).unwrap();
        for table in ["mcp_audit_authority_binding", "mcp_log"] {
            let exists: bool = connection
                .query_row(
                    "SELECT COUNT(*) = 1 FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(
                !exists,
                "{table} must not be created by active verification"
            );
        }
    }

    #[test]
    fn copied_database_cannot_reuse_another_canonical_slot_binding() {
        let directory = tempfile::tempdir().unwrap();
        let first_path = directory.path().join("first.db");
        let second_path = directory.path().join("second.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let identity = uuid::Uuid::new_v4();
        let epoch = 303;
        let material = store_bound_material(identity, epoch, [0x33; 32]);
        let reservation = McpAuditStore::reserve_writable_owner(&first_path).unwrap();
        let capability =
            McpAuditStore::authorize_fresh_database_creation(reservation, &reference_path).unwrap();
        let first_receipt = write_durable_reference(
            &reference_path,
            &first_path,
            identity,
            std::slice::from_ref(&material),
            "prepared",
            None,
            Some(epoch),
        );
        let first = McpAuditStore::activate_fresh_store_bound_authority(
            &first_path,
            vec![material.clone()],
            capability,
            first_receipt,
        )
        .unwrap();
        drop(first);
        std::fs::copy(&first_path, &second_path).unwrap();

        let error = McpAuditDurableReferenceReceipt::load_for_store(&reference_path, &second_path)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("mcp_audit_reference_canonical_slot_mismatch"));
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
    fn authority_poison_disables_late_writes_across_all_store_clones() {
        let directory = tempfile::tempdir().unwrap();
        let store = McpAuditStore::with_key_materials(
            directory.path().join("audit.db"),
            vec![keychain_material(911, [0x91; 32])],
        )
        .unwrap();
        let retained_clone = store.clone();
        store
            .poison_writes("reference_persistence_unknown")
            .unwrap();

        for writer in [&store, &retained_clone] {
            let error = writer
                .insert_log("late", &serde_json::json!({}), "forbidden", false, false)
                .unwrap_err();
            assert!(error
                .to_string()
                .contains("mcp_audit_store_writes_disabled:reference_persistence_unknown"));
        }
        assert!(store.list_logs(10).unwrap().is_empty());
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
            .with_checked_database_operation(|conn| {
                conn.execute(
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
                )?;
                Ok(())
            })
            .unwrap();
        drop(store);

        let restarted = McpAuditStore::with_keyring(&path, configs);
        let serialized = serde_json::to_string(&restarted.list_logs(10).unwrap()).unwrap();

        assert!(!serialized.contains(LEGACY_ARGUMENT));
        assert!(!serialized.contains(LEGACY_RESULT));
        assert!(serialized.contains("payloadStored"));
        assert!(serialized.contains("sha256:"));
        let version: i64 = restarted
            .with_checked_database_operation(|conn| {
                conn.query_row(
                    "SELECT payload_minimized_version FROM mcp_log WHERE tool_name = 'legacy_tool'",
                    [],
                    |row| row.get(0),
                )
                .map_err(anyhow::Error::from)
            })
            .unwrap();
        assert_eq!(version, MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION);
    }
}

/// Shareable handle.
pub type SharedMcpAuditStore = Arc<Mutex<McpAuditStore>>;
