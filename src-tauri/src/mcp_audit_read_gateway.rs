use crate::{
    errors::AppError,
    persistence_coordinator::{
        PersistenceHealthSnapshot, PersistenceStoreHealth, PersistenceStoreMode,
        PERSISTENCE_STORE_UNAVAILABLE,
    },
    AppState,
};
use openlife_core::mcp_audit::{AuditExport, McpAuditExportDays, McpAuditStore, McpLogEntry};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

const MCP_AUDIT_KEY_REFERENCE_STORE: &str = "McpAuditKeyReferenceStore";
const MCP_AUDIT_STORE: &str = "McpAuditStore";

pub const MCP_AUDIT_LIST_LIMIT_MIN: usize = 1;
pub const MCP_AUDIT_LIST_LIMIT_MAX: usize = 200;

/// Public product truth for every MCP audit read. The gateway owns this
/// contract; command and diagnostic surfaces only re-export or embed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAuditReadStatus {
    Available,
    Degraded,
    Unavailable,
    Unknown,
}

/// Stable, metadata-only reason codes. They intentionally contain no store
/// error text, path, key reference, payload, or decrypted audit value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAuditReadReasonCode {
    KeyReferenceStoreUnavailable,
    AuditStoreUnavailable,
    KeyReferenceStoreEphemeral,
    AuditStoreEphemeral,
    KeyReferenceStoreReadOnly,
    AuditStoreReadOnly,
    BothOwnersReadOnly,
    CompositeAuthorityChanged,
    AuditReadFailed,
}

impl McpAuditReadReasonCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::KeyReferenceStoreUnavailable => "key_reference_store_unavailable",
            Self::AuditStoreUnavailable => "audit_store_unavailable",
            Self::KeyReferenceStoreEphemeral => "key_reference_store_ephemeral",
            Self::AuditStoreEphemeral => "audit_store_ephemeral",
            Self::KeyReferenceStoreReadOnly => "key_reference_store_read_only",
            Self::AuditStoreReadOnly => "audit_store_read_only",
            Self::BothOwnersReadOnly => "both_owners_read_only",
            Self::CompositeAuthorityChanged => "composite_authority_changed",
            Self::AuditReadFailed => "audit_read_failed",
        }
    }
}

/// One discriminated contract is shared by list and diagnostics. Success
/// variants alone can carry facts; unavailable/unknown variants cannot carry
/// entries or counts, so a trusted zero cannot be confused with missing truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum McpAuditReadProjection<T> {
    Available {
        #[serde(flatten)]
        facts: T,
    },
    Degraded {
        #[serde(rename = "reasonCode")]
        reason_code: McpAuditReadReasonCode,
        #[serde(flatten)]
        facts: T,
    },
    Unavailable {
        #[serde(rename = "reasonCode")]
        reason_code: McpAuditReadReasonCode,
    },
    Unknown {
        #[serde(rename = "reasonCode")]
        reason_code: McpAuditReadReasonCode,
    },
}

impl<T> McpAuditReadProjection<T> {
    fn map<U>(self, map: impl FnOnce(T) -> U) -> McpAuditReadProjection<U> {
        match self {
            Self::Available { facts } => McpAuditReadProjection::Available { facts: map(facts) },
            Self::Degraded { reason_code, facts } => McpAuditReadProjection::Degraded {
                reason_code,
                facts: map(facts),
            },
            Self::Unavailable { reason_code } => {
                McpAuditReadProjection::Unavailable { reason_code }
            }
            Self::Unknown { reason_code } => McpAuditReadProjection::Unknown { reason_code },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAuditDiagnosticFacts {
    pub recent_audit_count: usize,
    pub recent_pii_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAuditLogListFacts {
    pub entries: Vec<McpLogEntry>,
}

/// A validated WebView list bound. Internal callers cannot accidentally bypass
/// the same hard ceiling with an untyped `usize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct McpAuditListLimit(usize);

impl McpAuditListLimit {
    fn get(self) -> usize {
        self.0
    }
}

impl TryFrom<usize> for McpAuditListLimit {
    type Error = AppError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if (MCP_AUDIT_LIST_LIMIT_MIN..=MCP_AUDIT_LIST_LIMIT_MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(AppError::Config {
                message: "mcp_audit_list_limit_out_of_range".into(),
                hint: Some(format!(
                    "limit must be between {MCP_AUDIT_LIST_LIMIT_MIN} and {MCP_AUDIT_LIST_LIMIT_MAX}"
                )),
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuditOwnerSnapshot {
    mode: PersistenceStoreMode,
    reason_code: Option<String>,
    error_digest: Option<String>,
}

impl From<&PersistenceStoreHealth> for AuditOwnerSnapshot {
    fn from(health: &PersistenceStoreHealth) -> Self {
        Self {
            mode: health.mode,
            reason_code: health.reason_code.clone(),
            error_digest: health.error_digest.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositeAuditReadAuthority {
    key_reference: AuditOwnerSnapshot,
    audit_store: AuditOwnerSnapshot,
    status: McpAuditReadStatus,
    degraded_reason: Option<McpAuditReadReasonCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompositeAuditReadFailure {
    owner: &'static str,
    mode: PersistenceStoreMode,
    reason_code: McpAuditReadReasonCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockingAuditReadFailure {
    Operation,
    Worker,
    GateClosed,
}

/// The canonical audit store is mutex-serialized, so admitting more than one
/// blocking worker would only consume blocking-pool threads while they wait on
/// the same guard. The semaphore queues callers asynchronously before spawn.
const MCP_AUDIT_BLOCKING_WORKER_LIMIT: usize = 1;

#[derive(Default)]
struct BlockingAuditWorkerProbe {
    #[cfg(test)]
    active: AtomicUsize,
    #[cfg(test)]
    peak: AtomicUsize,
    #[cfg(test)]
    started: AtomicUsize,
}

struct BlockingAuditWorkerGuard {
    #[cfg(test)]
    probe: Arc<BlockingAuditWorkerProbe>,
}

impl BlockingAuditWorkerProbe {
    fn enter(probe: Arc<Self>) -> BlockingAuditWorkerGuard {
        #[cfg(test)]
        {
            probe.started.fetch_add(1, Ordering::SeqCst);
            let active = probe.active.fetch_add(1, Ordering::SeqCst) + 1;
            probe.peak.fetch_max(active, Ordering::SeqCst);
            BlockingAuditWorkerGuard { probe }
        }
        #[cfg(not(test))]
        {
            drop(probe);
            BlockingAuditWorkerGuard {}
        }
    }
}

impl Drop for BlockingAuditWorkerGuard {
    fn drop(&mut self) {
        #[cfg(test)]
        self.probe.active.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn run_blocking_audit_read<T, Operation>(
    store: Arc<Mutex<McpAuditStore>>,
    worker_gate: Arc<Semaphore>,
    worker_probe: Arc<BlockingAuditWorkerProbe>,
    operation: Operation,
) -> Result<T, BlockingAuditReadFailure>
where
    T: Send + 'static,
    Operation: FnOnce(&McpAuditStore) -> anyhow::Result<T> + Send + 'static,
{
    let permit = worker_gate
        .acquire_owned()
        .await
        .map_err(|_| BlockingAuditReadFailure::GateClosed)?;
    tokio::task::spawn_blocking(move || {
        // The owned permit lives in the blocking worker. Dropping or aborting
        // the async caller cannot admit a replacement worker while this one is
        // still reading the canonical store.
        let _permit = permit;
        let _worker = BlockingAuditWorkerProbe::enter(worker_probe);
        let store = store.blocking_lock();
        operation(&store).map_err(|_| BlockingAuditReadFailure::Operation)
    })
    .await
    .map_err(|_| BlockingAuditReadFailure::Worker)?
}

impl CompositeAuditReadFailure {
    fn new(owner: &'static str, mode: PersistenceStoreMode) -> Self {
        Self {
            owner,
            mode,
            reason_code: McpAuditReadGateway::unavailable_reason(owner, mode),
        }
    }

    fn export_error(self) -> AppError {
        AppError::db(format!(
            "{PERSISTENCE_STORE_UNAVAILABLE}: owner={}; mode={:?}; reason_code={}",
            self.owner,
            self.mode,
            self.reason_code.as_str()
        ))
    }
}

/// Single product seam for composite MCP audit reads. Both the durable
/// key-reference owner and the SQLite owner must remain independently
/// trustworthy; unrelated degraded stores do not lend or remove audit-read
/// authority.
pub(crate) struct McpAuditReadGateway {
    blocking_worker_gate: Arc<Semaphore>,
    blocking_worker_probe: Arc<BlockingAuditWorkerProbe>,
    #[cfg(test)]
    diagnostics_calls: AtomicUsize,
    #[cfg(test)]
    list_calls: AtomicUsize,
    #[cfg(test)]
    export_calls: AtomicUsize,
}

impl Default for McpAuditReadGateway {
    fn default() -> Self {
        Self {
            blocking_worker_gate: Arc::new(Semaphore::new(MCP_AUDIT_BLOCKING_WORKER_LIMIT)),
            blocking_worker_probe: Arc::new(BlockingAuditWorkerProbe::default()),
            #[cfg(test)]
            diagnostics_calls: AtomicUsize::new(0),
            #[cfg(test)]
            list_calls: AtomicUsize::new(0),
            #[cfg(test)]
            export_calls: AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct McpAuditReadGatewayCallCounts {
    pub diagnostics: usize,
    pub list: usize,
    pub export: usize,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct McpAuditBlockingWorkerStats {
    pub active: usize,
    pub peak: usize,
    pub started: usize,
    pub available_permits: usize,
}

impl McpAuditReadGateway {
    fn owner_snapshot<'a>(
        health: &'a PersistenceHealthSnapshot,
        owner: &str,
    ) -> Option<&'a PersistenceStoreHealth> {
        health
            .stores
            .iter()
            .find(|candidate| candidate.store == owner)
    }

    fn unavailable_reason(owner: &str, mode: PersistenceStoreMode) -> McpAuditReadReasonCode {
        match (owner, mode) {
            (MCP_AUDIT_KEY_REFERENCE_STORE, PersistenceStoreMode::EphemeralDevelopment) => {
                McpAuditReadReasonCode::KeyReferenceStoreEphemeral
            }
            (MCP_AUDIT_STORE, PersistenceStoreMode::EphemeralDevelopment) => {
                McpAuditReadReasonCode::AuditStoreEphemeral
            }
            (MCP_AUDIT_KEY_REFERENCE_STORE, _) => {
                McpAuditReadReasonCode::KeyReferenceStoreUnavailable
            }
            _ => McpAuditReadReasonCode::AuditStoreUnavailable,
        }
    }

    fn composite_read_authority(
        &self,
        state: &AppState,
    ) -> Result<CompositeAuditReadAuthority, CompositeAuditReadFailure> {
        for owner in [MCP_AUDIT_KEY_REFERENCE_STORE, MCP_AUDIT_STORE] {
            if let Err(error) = state.persistence_coordinator.require_trusted_read(owner) {
                let mode = match error {
                    crate::persistence_coordinator::PersistenceGateError::StoreUnavailable {
                        mode,
                        ..
                    } => mode,
                    crate::persistence_coordinator::PersistenceGateError::EffectsBlocked {
                        ..
                    } => PersistenceStoreMode::Unavailable,
                };
                return Err(CompositeAuditReadFailure::new(owner, mode));
            }
        }

        let health = state.persistence_coordinator.snapshot();
        let key_reference_health = Self::owner_snapshot(&health, MCP_AUDIT_KEY_REFERENCE_STORE)
            .ok_or_else(|| {
                CompositeAuditReadFailure::new(
                    MCP_AUDIT_KEY_REFERENCE_STORE,
                    PersistenceStoreMode::Unavailable,
                )
            })?;
        let audit_store_health =
            Self::owner_snapshot(&health, MCP_AUDIT_STORE).ok_or_else(|| {
                CompositeAuditReadFailure::new(MCP_AUDIT_STORE, PersistenceStoreMode::Unavailable)
            })?;

        for (owner, owner_health) in [
            (MCP_AUDIT_KEY_REFERENCE_STORE, key_reference_health),
            (MCP_AUDIT_STORE, audit_store_health),
        ] {
            if !matches!(
                owner_health.mode,
                PersistenceStoreMode::ReadWriteCanonical | PersistenceStoreMode::ReadOnlyCanonical
            ) {
                return Err(CompositeAuditReadFailure::new(owner, owner_health.mode));
            }
        }

        let key_read_only = key_reference_health.mode == PersistenceStoreMode::ReadOnlyCanonical;
        let audit_read_only = audit_store_health.mode == PersistenceStoreMode::ReadOnlyCanonical;
        let degraded_reason = match (key_read_only, audit_read_only) {
            (false, false) => None,
            (true, false) => Some(McpAuditReadReasonCode::KeyReferenceStoreReadOnly),
            (false, true) => Some(McpAuditReadReasonCode::AuditStoreReadOnly),
            (true, true) => Some(McpAuditReadReasonCode::BothOwnersReadOnly),
        };

        Ok(CompositeAuditReadAuthority {
            key_reference: key_reference_health.into(),
            audit_store: audit_store_health.into(),
            status: if degraded_reason.is_some() {
                McpAuditReadStatus::Degraded
            } else {
                McpAuditReadStatus::Available
            },
            degraded_reason,
        })
    }

    fn success_projection<T>(
        authority: CompositeAuditReadAuthority,
        facts: T,
    ) -> McpAuditReadProjection<T> {
        match authority.status {
            McpAuditReadStatus::Available => McpAuditReadProjection::Available { facts },
            McpAuditReadStatus::Degraded => McpAuditReadProjection::Degraded {
                reason_code: authority
                    .degraded_reason
                    .expect("degraded authority always has a typed reason"),
                facts,
            },
            McpAuditReadStatus::Unavailable | McpAuditReadStatus::Unknown => {
                unreachable!("only trusted success authority reaches success projection")
            }
        }
    }

    async fn read_log_facts_with_operation<Operation>(
        &self,
        state: &AppState,
        operation: Operation,
    ) -> McpAuditReadProjection<McpAuditLogListFacts>
    where
        Operation: FnOnce(&McpAuditStore) -> anyhow::Result<Vec<McpLogEntry>> + Send + 'static,
    {
        // D057/D064 must replace this coordinator-only comparison with the
        // authenticated manifest generation plus retained DB-identity receipt.
        // Until then, any observed coordinator transition discards all facts;
        // this check earns no exact-generation or DB-identity closure credit.
        let before = match self.composite_read_authority(state) {
            Ok(authority) => authority,
            Err(failure) => {
                return McpAuditReadProjection::Unavailable {
                    reason_code: failure.reason_code,
                }
            }
        };

        let entries = match run_blocking_audit_read(
            Arc::clone(&state.mcp_audit_store),
            Arc::clone(&self.blocking_worker_gate),
            Arc::clone(&self.blocking_worker_probe),
            operation,
        )
        .await
        {
            Ok(entries) => entries,
            Err(
                BlockingAuditReadFailure::Operation
                | BlockingAuditReadFailure::Worker
                | BlockingAuditReadFailure::GateClosed,
            ) => {
                return McpAuditReadProjection::Unknown {
                    reason_code: McpAuditReadReasonCode::AuditReadFailed,
                }
            }
        };

        let after = match self.composite_read_authority(state) {
            Ok(authority) => authority,
            Err(failure) => {
                return McpAuditReadProjection::Unavailable {
                    reason_code: failure.reason_code,
                }
            }
        };
        if before != after {
            return McpAuditReadProjection::Unknown {
                reason_code: McpAuditReadReasonCode::CompositeAuthorityChanged,
            };
        }

        Self::success_projection(before, McpAuditLogListFacts { entries })
    }

    async fn read_log_facts(
        &self,
        state: &AppState,
        limit: usize,
    ) -> McpAuditReadProjection<McpAuditLogListFacts> {
        self.read_log_facts_with_operation(state, move |audit| audit.list_logs(limit))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn worker_panic_projection_for_test(
        &self,
        state: &AppState,
    ) -> McpAuditReadProjection<McpAuditLogListFacts> {
        self.read_log_facts_with_operation(state, |_audit| -> anyhow::Result<Vec<McpLogEntry>> {
            panic!("d065_injected_blocking_audit_worker_panic")
        })
        .await
    }

    #[cfg(test)]
    pub(crate) async fn projection_with_operation_for_test<Operation>(
        &self,
        state: &AppState,
        operation: Operation,
    ) -> McpAuditReadProjection<McpAuditLogListFacts>
    where
        Operation: FnOnce(&McpAuditStore) -> anyhow::Result<Vec<McpLogEntry>> + Send + 'static,
    {
        self.read_log_facts_with_operation(state, operation).await
    }

    pub(crate) async fn diagnostic_counts(
        &self,
        state: &AppState,
    ) -> McpAuditReadProjection<McpAuditDiagnosticFacts> {
        #[cfg(test)]
        self.diagnostics_calls.fetch_add(1, Ordering::Relaxed);

        self.read_log_facts(state, 50)
            .await
            .map(|facts| McpAuditDiagnosticFacts {
                recent_audit_count: facts.entries.len(),
                recent_pii_count: facts.entries.iter().filter(|log| log.pii_found).count(),
            })
    }

    pub(crate) async fn list_logs(
        &self,
        state: &AppState,
        limit: McpAuditListLimit,
    ) -> McpAuditReadProjection<McpAuditLogListFacts> {
        #[cfg(test)]
        self.list_calls.fetch_add(1, Ordering::Relaxed);

        self.read_log_facts(state, limit.get()).await
    }

    pub(crate) async fn export_logs(
        &self,
        state: &AppState,
        window: McpAuditExportDays,
    ) -> Result<AuditExport, AppError> {
        #[cfg(test)]
        self.export_calls.fetch_add(1, Ordering::Relaxed);

        let before = self
            .composite_read_authority(state)
            .map_err(CompositeAuditReadFailure::export_error)?;
        let export = run_blocking_audit_read(
            Arc::clone(&state.mcp_audit_store),
            Arc::clone(&self.blocking_worker_gate),
            Arc::clone(&self.blocking_worker_probe),
            move |store| store.export_logs(window),
        )
        .await
        .map_err(|_| AppError::db(McpAuditReadReasonCode::AuditReadFailed.as_str()))?;
        let after = self
            .composite_read_authority(state)
            .map_err(CompositeAuditReadFailure::export_error)?;
        if before != after {
            return Err(AppError::db(
                McpAuditReadReasonCode::CompositeAuthorityChanged.as_str(),
            ));
        }
        Ok(export)
    }

    #[cfg(test)]
    pub(crate) fn call_counts(&self) -> McpAuditReadGatewayCallCounts {
        McpAuditReadGatewayCallCounts {
            diagnostics: self.diagnostics_calls.load(Ordering::Relaxed),
            list: self.list_calls.load(Ordering::Relaxed),
            export: self.export_calls.load(Ordering::Relaxed),
        }
    }

    #[cfg(test)]
    pub(crate) fn blocking_worker_stats(&self) -> McpAuditBlockingWorkerStats {
        McpAuditBlockingWorkerStats {
            active: self.blocking_worker_probe.active.load(Ordering::SeqCst),
            peak: self.blocking_worker_probe.peak.load(Ordering::SeqCst),
            started: self.blocking_worker_probe.started.load(Ordering::SeqCst),
            available_permits: self.blocking_worker_gate.available_permits(),
        }
    }

    #[cfg(test)]
    pub(crate) fn close_blocking_worker_gate_for_test(&self) {
        self.blocking_worker_gate.close();
    }
}
