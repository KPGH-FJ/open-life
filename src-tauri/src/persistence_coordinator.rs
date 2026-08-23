use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

pub const PERSISTENCE_EFFECTS_BLOCKED: &str = "persistence_effects_blocked";
pub const PERSISTENCE_STORE_UNAVAILABLE: &str = "persistence_store_unavailable";
pub const PERSISTENCE_ADMISSION_INVALIDATED: &str = "persistence_admission_invalidated";

pub const EXPECTED_BOOTSTRAP_STORES: &[&str] = &[
    "ConfigStore",
    "LifeModelFileStore",
    "PrivacyPolicyStore",
    "McpAuditKeyReferenceStore",
    "MemoryStore",
    "ConversationStore",
    "FeedbackStore",
    "VectorStore",
    "CanonicalTaskRuntimeStore",
    "ProposalStore",
    "MemoryLifecycleStore",
    "McpAuditStore",
    "ToolPermissionStore",
];

/// Capability-local stores do not own the base Chat/Work lifecycle. Their
/// exact gateway still fails closed when that capability is selected, but an
/// unavailable optional capability must not disable unrelated provider,
/// filesystem, Web, or Artifact work.
const NON_BASE_AGENT_STORES: &[&str] = &[
    "LifeModelFileStore",
    "FeedbackStore",
    "VectorStore",
    "MemoryLifecycleStore",
    "McpAuditKeyReferenceStore",
    "McpAuditStore",
];

#[cfg(test)]
pub const EXPLICIT_NON_CANONICAL_BOOTSTRAP_SURFACES: &[(&str, &str)] = &[
    ("VersionManager", "derived rollback snapshots"),
    ("SkillRegistry", "built-in and manifest-derived registry"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceStoreMode {
    ReadWriteCanonical,
    ReadOnlyCanonical,
    Unavailable,
    EphemeralDevelopment,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceStoreHealth {
    pub store: String,
    pub mode: PersistenceStoreMode,
    pub reason_code: Option<String>,
    pub error_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceRuntimeMode {
    Initializing,
    ReadWrite,
    ReadOnlyDegraded,
    UnavailableDegraded,
    EphemeralDevelopment,
    IsolatedEvaluation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceHealthSnapshot {
    pub mode: PersistenceRuntimeMode,
    pub canonical_writes_allowed: bool,
    pub provider_dispatch_allowed: bool,
    pub tool_dispatch_allowed: bool,
    pub live_or_canonical_credit_eligible: bool,
    pub sealed: bool,
    pub stores: Vec<PersistenceStoreHealth>,
    pub global_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistenceGateError {
    AdmissionInvalidated {
        mode: PersistenceRuntimeMode,
    },
    EffectsBlocked {
        mode: PersistenceRuntimeMode,
    },
    StoreUnavailable {
        store: String,
        mode: PersistenceStoreMode,
    },
}

impl std::fmt::Display for PersistenceGateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AdmissionInvalidated { mode } => write!(
                formatter,
                "{PERSISTENCE_ADMISSION_INVALIDATED}: canonical write admission was superseded before commit in mode {mode:?}"
            ),
            Self::EffectsBlocked { mode } => write!(
                formatter,
                "{PERSISTENCE_EFFECTS_BLOCKED}: persistence mode {mode:?} forbids provider, tool, and canonical-write effects"
            ),
            Self::StoreUnavailable { store, mode } => write!(
                formatter,
                "{PERSISTENCE_STORE_UNAVAILABLE}: canonical store {store} is {mode:?}; state is unknown"
            ),
        }
    }
}

impl std::error::Error for PersistenceGateError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[expect(
    clippy::enum_variant_names,
    reason = "owner=backend-contracts; expires=2026-10-01; variants match canonical store names"
)]
pub(crate) enum CanonicalWriteOwner {
    LifeModelFileStore,
    MemoryStore,
    VectorStore,
}

impl CanonicalWriteOwner {
    fn required_store_names(self) -> &'static [&'static str] {
        match self {
            Self::LifeModelFileStore => &["LifeModelFileStore"],
            Self::MemoryStore => &["MemoryStore"],
            Self::VectorStore => &["VectorStore"],
        }
    }
}

/// A synchronous, pre-commit decision. It is deliberately not sufficient to
/// authorize a canonical mutation: every owner gateway must exchange it for a
/// `CanonicalCommitPermit` immediately before taking its store mutex or
/// starting its database transaction.
///
/// The admission is crate-private, non-cloneable, and non-serializable. A
/// normal admission captures the coordinator generation observed when policy
/// admitted the write. A recovery admission additionally retains the exact
/// durable-journal binding so the asynchronous commit permit can revalidate it
/// after entering the shared canonical-write barrier.
#[must_use = "an admission is not a commit authorization; exchange it for a CanonicalCommitPermit"]
pub(crate) struct CanonicalWriteAdmission {
    generation: u64,
    owners: BTreeSet<CanonicalWriteOwner>,
    kind: CanonicalWriteAdmissionKind,
}

enum CanonicalWriteAdmissionKind {
    Normal,
    StartupReconciliation,
}

/// RAII proof that a canonical owner is inside the only local commit window.
/// Keep this guard only across the bounded local owner mutation; drop it before
/// projection, provider, tool, or network awaits.
#[derive(Debug)]
#[must_use = "hold the permit across the bounded canonical commit window"]
pub(crate) struct CanonicalCommitPermit<'coordinator> {
    _barrier: tokio::sync::RwLockReadGuard<'coordinator, ()>,
}

#[derive(Default)]
struct PersistenceCoordinatorState {
    expected_stores: BTreeSet<String>,
    stores: BTreeMap<String, PersistenceStoreHealth>,
    global_reason_codes: Vec<String>,
    sealed: bool,
    isolated_evaluation: bool,
    admission_generation: u64,
}

/// Single process-wide authority for persistence health and effect admission.
/// It owns no product data; canonical stores remain the fact owners.
pub struct PersistenceCoordinator {
    state: RwLock<PersistenceCoordinatorState>,
    /// Gateways must obtain this barrier before taking an owner-local mutex or
    /// transaction and must never hold it across an external await.
    canonical_write_barrier: tokio::sync::RwLock<()>,
    canonical_outbox_worker_started: AtomicBool,
    canonical_outbox_notify: tokio::sync::Notify,
    work_run_causal_locks: Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
}

impl Default for PersistenceCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistenceCoordinator {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(PersistenceCoordinatorState::default()),
            canonical_write_barrier: tokio::sync::RwLock::new(()),
            canonical_outbox_worker_started: AtomicBool::new(false),
            canonical_outbox_notify: tokio::sync::Notify::new(),
            work_run_causal_locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn for_release_bootstrap() -> Self {
        Self::with_expected_stores(EXPECTED_BOOTSTRAP_STORES.iter().copied())
    }

    fn with_expected_stores<'a>(stores: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            state: RwLock::new(PersistenceCoordinatorState {
                expected_stores: stores.into_iter().map(str::to_string).collect(),
                ..PersistenceCoordinatorState::default()
            }),
            canonical_write_barrier: tokio::sync::RwLock::new(()),
            canonical_outbox_worker_started: AtomicBool::new(false),
            canonical_outbox_notify: tokio::sync::Notify::new(),
            work_run_causal_locks: Mutex::new(HashMap::new()),
        }
    }

    /// Explicit fixture mode. It permits isolated mechanics to execute but is
    /// never eligible for durable, live-provider, or product-trial credit.
    #[cfg(test)]
    pub(crate) fn isolated_evaluation() -> Self {
        Self {
            state: RwLock::new(PersistenceCoordinatorState {
                sealed: true,
                isolated_evaluation: true,
                ..PersistenceCoordinatorState::default()
            }),
            canonical_write_barrier: tokio::sync::RwLock::new(()),
            canonical_outbox_worker_started: AtomicBool::new(false),
            canonical_outbox_notify: tokio::sync::Notify::new(),
            work_run_causal_locks: Mutex::new(HashMap::new()),
        }
    }

    /// Claim the single background outbox consumer. The durable outbox rows
    /// are the queue; this flag only prevents one worker per foreground event.
    pub fn claim_canonical_outbox_worker(&self) -> bool {
        self.canonical_outbox_worker_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Coalescing wake-up for the single consumer. No event body or authority
    /// is copied into RAM; a wake-up always re-reads the durable owner queues.
    pub fn notify_canonical_outbox_worker(&self) {
        self.canonical_outbox_notify.notify_one();
    }

    pub async fn wait_for_canonical_outbox_work(&self) {
        self.canonical_outbox_notify.notified().await;
    }

    /// Process-local serialization only. The durable canonical aggregate
    /// revision remains the source of truth across crashes/restarts; this lock
    /// merely closes the in-process read-head/apply-target TOCTOU window.
    pub fn work_run_causal_lock(&self, run_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .work_run_causal_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(run_id).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(run_id.to_string(), Arc::downgrade(&lock));
        lock
    }

    pub fn register_read_write(&self, store: impl Into<String>) {
        self.register(store, PersistenceStoreMode::ReadWriteCanonical, None, None);
    }

    pub fn register_read_only(
        &self,
        store: impl Into<String>,
        reason_code: impl Into<String>,
        error: &str,
    ) {
        self.register(
            store,
            PersistenceStoreMode::ReadOnlyCanonical,
            Some(reason_code.into()),
            Some(error_digest(error)),
        );
    }

    pub fn register_unavailable(
        &self,
        store: impl Into<String>,
        reason_code: impl Into<String>,
        error: &str,
    ) {
        self.register(
            store,
            PersistenceStoreMode::Unavailable,
            Some(reason_code.into()),
            Some(error_digest(error)),
        );
    }

    pub fn register_ephemeral_development(
        &self,
        store: impl Into<String>,
        reason_code: impl Into<String>,
        error: &str,
    ) {
        self.register(
            store,
            PersistenceStoreMode::EphemeralDevelopment,
            Some(reason_code.into()),
            Some(error_digest(error)),
        );
    }

    /// Records only failures that indicate the durable substrate itself is no
    /// longer trustworthy. User validation, not-found, authorization, and CAS
    /// conflicts must not let untrusted input force global safe mode.
    pub fn register_runtime_durable_failure(&self, store: &str, error: impl AsRef<str>) -> bool {
        let error = error.as_ref();
        if !is_durable_storage_failure(error) {
            return false;
        }
        self.register_unavailable(store, "runtime_durable_store_failure", error);
        true
    }

    pub fn degrade_globally(&self, reason_code: impl Into<String>) {
        let reason_code = reason_code.into();
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.global_reason_codes.contains(&reason_code) {
            invalidate_canonical_write_admissions(&mut state);
            state.global_reason_codes.push(reason_code);
            state.global_reason_codes.sort();
        }
    }

    pub fn seal(&self) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.expected_stores.is_empty() && !state.isolated_evaluation {
            state
                .global_reason_codes
                .push("expected_store_manifest_empty".into());
        }
        let registered = state.stores.keys().cloned().collect::<BTreeSet<_>>();
        let missing = state
            .expected_stores
            .difference(&registered)
            .cloned()
            .collect::<Vec<_>>();
        for store in missing {
            state.stores.insert(
                store.clone(),
                PersistenceStoreHealth {
                    store,
                    mode: PersistenceStoreMode::Unavailable,
                    reason_code: Some("expected_store_not_initialized".into()),
                    error_digest: None,
                },
            );
        }
        state.sealed = true;
    }

    pub fn snapshot(&self) -> PersistenceHealthSnapshot {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stores = state.stores.values().cloned().collect::<Vec<_>>();
        let mode = runtime_mode(
            &stores,
            &state.global_reason_codes,
            state.sealed,
            state.isolated_evaluation,
        );
        let effects_allowed = matches!(
            mode,
            PersistenceRuntimeMode::ReadWrite | PersistenceRuntimeMode::IsolatedEvaluation
        );
        PersistenceHealthSnapshot {
            mode,
            canonical_writes_allowed: mode == PersistenceRuntimeMode::ReadWrite,
            provider_dispatch_allowed: effects_allowed,
            tool_dispatch_allowed: effects_allowed,
            live_or_canonical_credit_eligible: mode == PersistenceRuntimeMode::ReadWrite,
            sealed: state.sealed,
            stores,
            global_reason_codes: state.global_reason_codes.clone(),
        }
    }

    pub fn require_effects_allowed(&self) -> Result<(), PersistenceGateError> {
        let mode = self.snapshot().mode;
        if matches!(
            mode,
            PersistenceRuntimeMode::ReadWrite | PersistenceRuntimeMode::IsolatedEvaluation
        ) {
            Ok(())
        } else {
            Err(PersistenceGateError::EffectsBlocked { mode })
        }
    }

    /// Admits one product path against only the canonical stores it will
    /// actually read or mutate. This is intentionally narrower than the
    /// process-wide diagnostic mode: an unavailable compatibility or unrelated
    /// domain store must not disable canonical Chat or Work.
    pub fn require_effects_for_stores(
        &self,
        required_stores: &[&str],
    ) -> Result<(), PersistenceGateError> {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.isolated_evaluation {
            return Ok(());
        }
        if !state.sealed {
            return Err(PersistenceGateError::EffectsBlocked {
                mode: PersistenceRuntimeMode::Initializing,
            });
        }
        if !state.global_reason_codes.is_empty() {
            let stores = state.stores.values().cloned().collect::<Vec<_>>();
            return Err(PersistenceGateError::EffectsBlocked {
                mode: runtime_mode(&stores, &state.global_reason_codes, true, false),
            });
        }
        for store in required_stores {
            let mode = state
                .stores
                .get(*store)
                .map(|health| health.mode)
                .unwrap_or(PersistenceStoreMode::Unavailable);
            if mode != PersistenceStoreMode::ReadWriteCanonical {
                return Err(PersistenceGateError::StoreUnavailable {
                    store: (*store).to_string(),
                    mode,
                });
            }
        }
        Ok(())
    }

    /// Admit one or more canonical owners against the current persistence
    /// state. The admission must still be exchanged for a commit permit.
    pub(crate) fn admit_canonical_writes(
        &self,
        owners: &[CanonicalWriteOwner],
    ) -> Result<CanonicalWriteAdmission, PersistenceGateError> {
        let requested_owner_count = owners.len();
        let owners = owners.iter().copied().collect::<BTreeSet<_>>();
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mode = runtime_mode(
            &state.stores.values().cloned().collect::<Vec<_>>(),
            &state.global_reason_codes,
            state.sealed,
            state.isolated_evaluation,
        );
        if owners.is_empty() || owners.len() != requested_owner_count {
            return Err(PersistenceGateError::EffectsBlocked { mode });
        }
        if !matches!(
            mode,
            PersistenceRuntimeMode::ReadWrite | PersistenceRuntimeMode::IsolatedEvaluation
        ) {
            return Err(PersistenceGateError::EffectsBlocked { mode });
        }
        if let Some(error) = canonical_owner_write_error(&state, &owners) {
            return Err(error);
        }
        Ok(CanonicalWriteAdmission {
            generation: state.admission_generation,
            owners,
            kind: CanonicalWriteAdmissionKind::Normal,
        })
    }

    /// Pre-seal startup reconciliation uses the same process-wide barrier as
    /// product writes, without claiming that ordinary effects are enabled.
    pub(crate) fn admit_startup_reconciliation_writes(
        &self,
        owners: &[CanonicalWriteOwner],
    ) -> Result<CanonicalWriteAdmission, PersistenceGateError> {
        let requested_owner_count = owners.len();
        let owners = owners.iter().copied().collect::<BTreeSet<_>>();
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mode = runtime_mode(
            &state.stores.values().cloned().collect::<Vec<_>>(),
            &state.global_reason_codes,
            state.sealed,
            state.isolated_evaluation,
        );
        if owners.is_empty()
            || owners.len() != requested_owner_count
            || !startup_reconciliation_state_is_safe(&state)
        {
            return Err(PersistenceGateError::EffectsBlocked { mode });
        }
        Ok(CanonicalWriteAdmission {
            generation: state.admission_generation,
            owners,
            kind: CanonicalWriteAdmissionKind::StartupReconciliation,
        })
    }

    /// Revalidates a synchronous admission inside the shared canonical-write
    /// barrier. This method must be called before taking an owner mutex or
    /// opening its transaction. The returned guard is intentionally opaque.
    pub(crate) async fn acquire_canonical_commit_permit<'coordinator>(
        &'coordinator self,
        admission: &CanonicalWriteAdmission,
    ) -> Result<CanonicalCommitPermit<'coordinator>, PersistenceGateError> {
        let barrier = self.canonical_write_barrier.read().await;
        let (mode, generation, startup_reconciliation_is_safe, unavailable_owner) = {
            let state = self
                .state
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (
                runtime_mode(
                    &state.stores.values().cloned().collect::<Vec<_>>(),
                    &state.global_reason_codes,
                    state.sealed,
                    state.isolated_evaluation,
                ),
                state.admission_generation,
                startup_reconciliation_state_is_safe(&state),
                canonical_owner_write_error(&state, &admission.owners),
            )
        };
        if admission.generation != generation {
            return Err(PersistenceGateError::AdmissionInvalidated { mode });
        }
        if admission.owners.is_empty() {
            return Err(PersistenceGateError::EffectsBlocked { mode });
        }

        match &admission.kind {
            CanonicalWriteAdmissionKind::Normal
                if matches!(
                    mode,
                    PersistenceRuntimeMode::ReadWrite | PersistenceRuntimeMode::IsolatedEvaluation
                ) =>
            {
                if let Some(error) = unavailable_owner {
                    return Err(error);
                }
            }
            CanonicalWriteAdmissionKind::StartupReconciliation
                if startup_reconciliation_is_safe => {}
            _ => return Err(PersistenceGateError::EffectsBlocked { mode }),
        }

        Ok(CanonicalCommitPermit { _barrier: barrier })
    }

    pub fn bootstrap_mutations_safe(&self) -> bool {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !state.sealed
            && state.global_reason_codes.is_empty()
            && state
                .stores
                .values()
                .all(|health| health.mode == PersistenceStoreMode::ReadWriteCanonical)
    }

    /// Final startup-reconciliation admission after every canonical store has
    /// been initialized but before `seal` enables product effects.
    pub fn startup_reconciliation_mutations_safe(&self) -> bool {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        startup_reconciliation_state_is_safe(&state)
    }

    pub fn require_trusted_read(&self, store: &str) -> Result<(), PersistenceGateError> {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.isolated_evaluation {
            return Ok(());
        }
        let Some(health) = state.stores.get(store) else {
            return Err(PersistenceGateError::StoreUnavailable {
                store: store.to_string(),
                mode: PersistenceStoreMode::Unavailable,
            });
        };
        match health.mode {
            PersistenceStoreMode::ReadWriteCanonical | PersistenceStoreMode::ReadOnlyCanonical => {
                Ok(())
            }
            mode => Err(PersistenceGateError::StoreUnavailable {
                store: store.to_string(),
                mode,
            }),
        }
    }

    fn register(
        &self,
        store: impl Into<String>,
        mode: PersistenceStoreMode,
        reason_code: Option<String>,
        error_digest: Option<String>,
    ) {
        let store = store.into();
        let health = PersistenceStoreHealth {
            store: store.clone(),
            mode,
            reason_code,
            error_digest,
        };
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.sealed {
            let Some(existing) = state.stores.get(&store) else {
                return;
            };
            let monotonic_degradation = matches!(
                (existing.mode, mode),
                (
                    PersistenceStoreMode::ReadWriteCanonical,
                    PersistenceStoreMode::ReadOnlyCanonical | PersistenceStoreMode::Unavailable
                ) | (
                    PersistenceStoreMode::ReadOnlyCanonical,
                    PersistenceStoreMode::Unavailable
                ) | (
                    PersistenceStoreMode::EphemeralDevelopment,
                    PersistenceStoreMode::Unavailable
                )
            );
            if !monotonic_degradation {
                return;
            }
        }
        state.stores.insert(store, health);
    }
}

fn invalidate_canonical_write_admissions(state: &mut PersistenceCoordinatorState) {
    // A process cannot practically consume the u64 space. Wrapping still ensures a later admission cannot reuse a saturated generation.
    state.admission_generation = state.admission_generation.wrapping_add(1);
}

fn startup_reconciliation_state_is_safe(state: &PersistenceCoordinatorState) -> bool {
    let registered = state.stores.keys().cloned().collect::<BTreeSet<_>>();
    !state.sealed
        && !state.isolated_evaluation
        && !state.expected_stores.is_empty()
        && state.global_reason_codes.is_empty()
        && state.expected_stores.is_subset(&registered)
        && state
            .stores
            .values()
            .all(|health| health.mode == PersistenceStoreMode::ReadWriteCanonical)
}

impl openlife_core::agent::ToolAuditPersistenceObserver for PersistenceCoordinator {
    fn audit_persistence_failed(&self, receipt: &openlife_core::agent::ToolExecutionReceipt) {
        if receipt.audit_persistence_status
            != openlife_core::agent::ToolAuditPersistenceStatus::Failed
        {
            return;
        }
        self.register_unavailable(
            "McpAuditStore",
            "runtime_audit_commit_failed",
            &format!("tool_audit_commit_failed:receipt_id={}", receipt.receipt_id),
        );
    }
}

impl openlife_core::agent::DurableStoreFailureObserver for PersistenceCoordinator {
    fn durable_store_failed(&self, store_kind: &'static str, raw_error: &str) {
        // Reuse the sole process-wide durable-failure classifier. The Core
        // callback carries no payload and cannot mutate product state; it only
        // prevents a raw store failure from being disguised by a later typed
        // blocker/action result.
        self.register_runtime_durable_failure(store_kind, raw_error);
    }
}

fn runtime_mode(
    stores: &[PersistenceStoreHealth],
    global_reason_codes: &[String],
    sealed: bool,
    isolated_evaluation: bool,
) -> PersistenceRuntimeMode {
    if isolated_evaluation {
        PersistenceRuntimeMode::IsolatedEvaluation
    } else if !sealed {
        PersistenceRuntimeMode::Initializing
    } else if stores
        .iter()
        .filter(|health| store_blocks_base_agent_effects(&health.store))
        .any(|health| health.mode == PersistenceStoreMode::Unavailable)
    {
        PersistenceRuntimeMode::UnavailableDegraded
    } else if !global_reason_codes.is_empty()
        || stores
            .iter()
            .filter(|health| store_blocks_base_agent_effects(&health.store))
            .any(|health| health.mode == PersistenceStoreMode::ReadOnlyCanonical)
    {
        PersistenceRuntimeMode::ReadOnlyDegraded
    } else if stores
        .iter()
        .filter(|health| store_blocks_base_agent_effects(&health.store))
        .any(|health| health.mode == PersistenceStoreMode::EphemeralDevelopment)
    {
        PersistenceRuntimeMode::EphemeralDevelopment
    } else {
        PersistenceRuntimeMode::ReadWrite
    }
}

fn store_blocks_base_agent_effects(store: &str) -> bool {
    !NON_BASE_AGENT_STORES.contains(&store)
}

fn canonical_owner_write_error(
    state: &PersistenceCoordinatorState,
    owners: &BTreeSet<CanonicalWriteOwner>,
) -> Option<PersistenceGateError> {
    if state.isolated_evaluation {
        return None;
    }
    owners.iter().find_map(|owner| {
        owner.required_store_names().iter().find_map(|store| {
            let mode = state
                .stores
                .get(*store)
                .map(|health| health.mode)
                .unwrap_or(PersistenceStoreMode::Unavailable);
            (mode != PersistenceStoreMode::ReadWriteCanonical).then(|| {
                PersistenceGateError::StoreUnavailable {
                    store: (*store).to_string(),
                    mode,
                }
            })
        })
    })
}

fn error_digest(error: &str) -> String {
    format!("{:x}", Sha256::digest(error.as_bytes()))
}

fn is_durable_storage_failure(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    [
        "database is locked",
        "database is busy",
        "attempt to write a readonly database",
        "read-only file system",
        "readonly database",
        "disk i/o error",
        "database disk image is malformed",
        "database or disk is full",
        "no space left on device",
        "unable to open database file",
        "failed to open",
        "no such table",
        "mutex poison",
        "permission denied (os error",
        "os error 13",
        "integrity check failed",
        "schema is missing",
        "file is not a database",
        "sqlite_corrupt",
        "sqlite_ioerr",
        "sqlite_full",
        "sqlite_readonly",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_or_unavailable_canonical_store_blocks_every_effect_class() {
        let coordinator = PersistenceCoordinator::with_expected_stores(["memory"]);
        coordinator.register_read_write("memory");
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::Initializing
        );
        assert!(coordinator.require_effects_allowed().is_err());
        coordinator.seal();
        assert!(coordinator.require_effects_allowed().is_ok());

        coordinator.register_read_only("memory", "migration_failed", "disk is read-only");
        let snapshot = coordinator.snapshot();
        assert_eq!(snapshot.mode, PersistenceRuntimeMode::ReadOnlyDegraded);
        assert!(!snapshot.canonical_writes_allowed);
        assert!(!snapshot.provider_dispatch_allowed);
        assert!(!snapshot.tool_dispatch_allowed);
        assert!(coordinator.require_trusted_read("memory").is_ok());
        assert!(coordinator.require_effects_allowed().is_err());

        coordinator.register_unavailable("memory", "open_failed", "corrupt database");
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::UnavailableDegraded
        );
        assert!(matches!(
            coordinator.require_trusted_read("memory"),
            Err(PersistenceGateError::StoreUnavailable { .. })
        ));
    }

    #[test]
    fn optional_personalization_store_failure_does_not_disable_base_agent_effects() {
        let coordinator = PersistenceCoordinator::with_expected_stores([
            "CanonicalTaskRuntimeStore",
            "LifeModelFileStore",
        ]);
        coordinator.register_read_write("CanonicalTaskRuntimeStore");
        coordinator.register_read_write("LifeModelFileStore");
        coordinator.seal();

        coordinator.register_unavailable(
            "LifeModelFileStore",
            "lifemodel_open_failed",
            "injected optional personalization failure",
        );

        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::ReadWrite
        );
        coordinator
            .require_effects_allowed()
            .expect("base Agent remains available without LifeModel personalization");
        assert!(matches!(
            coordinator.admit_canonical_writes(&[CanonicalWriteOwner::LifeModelFileStore]),
            Err(PersistenceGateError::StoreUnavailable { store, .. })
                if store == "LifeModelFileStore"
        ));
    }

    #[test]
    fn unavailable_mcp_audit_disables_only_the_mcp_capability() {
        let coordinator = PersistenceCoordinator::with_expected_stores([
            "ConversationStore",
            "CanonicalTaskRuntimeStore",
            "McpAuditKeyReferenceStore",
            "McpAuditStore",
        ]);
        coordinator.register_read_write("ConversationStore");
        coordinator.register_read_write("CanonicalTaskRuntimeStore");
        coordinator.register_unavailable(
            "McpAuditKeyReferenceStore",
            "mcp_audit_key_hydration_failed",
            "MCP audit credential is not initialized",
        );
        coordinator.register_unavailable(
            "McpAuditStore",
            "mcp_audit_credential_unavailable",
            "MCP audit store is unavailable",
        );
        coordinator.seal();

        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::ReadWrite
        );
        coordinator
            .require_effects_for_stores(&["ConversationStore", "CanonicalTaskRuntimeStore"])
            .expect("ordinary Chat and Work must not pay an MCP credential tax");
        assert!(matches!(
            coordinator.require_effects_for_stores(&["McpAuditStore"]),
            Err(PersistenceGateError::StoreUnavailable { store, .. })
                if store == "McpAuditStore"
        ));
    }

    #[test]
    fn scoped_agent_admission_ignores_unrelated_optional_store_health() {
        let coordinator = PersistenceCoordinator::with_expected_stores([
            "ConversationStore",
            "CanonicalTaskRuntimeStore",
            "LifeModelFileStore",
        ]);
        coordinator.register_read_write("ConversationStore");
        coordinator.register_read_write("CanonicalTaskRuntimeStore");
        coordinator.register_unavailable(
            "LifeModelFileStore",
            "optional_store_unavailable",
            "optional personalization store is unavailable",
        );
        coordinator.seal();

        coordinator
            .require_effects_allowed()
            .expect("optional personalization health does not block canonical Chat or Work");
        coordinator
            .require_effects_for_stores(&["ConversationStore"])
            .expect("canonical Chat ignores unrelated retired stores");
        coordinator
            .require_effects_for_stores(&["ConversationStore", "CanonicalTaskRuntimeStore"])
            .expect("canonical Work ignores unrelated retired stores");
    }

    #[test]
    fn scoped_agent_admission_requires_every_named_canonical_store() {
        let coordinator = PersistenceCoordinator::with_expected_stores([
            "ConversationStore",
            "CanonicalTaskRuntimeStore",
        ]);
        coordinator.register_read_write("ConversationStore");
        coordinator.register_unavailable(
            "CanonicalTaskRuntimeStore",
            "open_failed",
            "task runtime unavailable",
        );
        coordinator.seal();

        assert!(matches!(
            coordinator.require_effects_for_stores(&[
                "ConversationStore",
                "CanonicalTaskRuntimeStore",
            ]),
            Err(PersistenceGateError::StoreUnavailable { store, .. })
                if store == "CanonicalTaskRuntimeStore"
        ));
    }

    #[test]
    fn development_fallback_is_explicit_and_never_claimed_as_canonical() {
        let coordinator = PersistenceCoordinator::with_expected_stores(["memory"]);
        coordinator.register_ephemeral_development(
            "memory",
            "dev_ephemeral_fallback",
            "primary failed",
        );
        coordinator.seal();
        let snapshot = coordinator.snapshot();
        assert_eq!(snapshot.mode, PersistenceRuntimeMode::EphemeralDevelopment);
        assert!(!snapshot.canonical_writes_allowed);
        assert!(coordinator.require_trusted_read("memory").is_err());
    }

    #[test]
    fn empty_or_missing_expected_store_registry_can_never_become_read_write() {
        let empty = PersistenceCoordinator::new();
        assert_eq!(empty.snapshot().mode, PersistenceRuntimeMode::Initializing);
        assert!(empty.require_effects_allowed().is_err());
        empty.seal();
        assert_eq!(
            empty.snapshot().mode,
            PersistenceRuntimeMode::ReadOnlyDegraded
        );
        assert!(empty.require_effects_allowed().is_err());

        let coordinator = PersistenceCoordinator::for_release_bootstrap();
        coordinator.register_read_write("MemoryStore");
        coordinator.seal();
        let snapshot = coordinator.snapshot();
        assert_eq!(snapshot.mode, PersistenceRuntimeMode::UnavailableDegraded);
        assert!(snapshot.stores.iter().any(|health| {
            health.store == "ProposalStore"
                && health.reason_code.as_deref() == Some("expected_store_not_initialized")
        }));
        assert!(coordinator.require_effects_allowed().is_err());
    }

    #[test]
    fn complete_manifest_still_blocks_effects_until_startup_reconciliation_seals() {
        let coordinator = PersistenceCoordinator::for_release_bootstrap();
        for store in EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        assert!(coordinator.startup_reconciliation_mutations_safe());
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::Initializing
        );
        assert!(coordinator.require_effects_allowed().is_err());

        coordinator.seal();
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::ReadWrite
        );
        coordinator
            .require_effects_allowed()
            .expect("seal enables effects only after startup reconciliation");
    }

    #[tokio::test]
    async fn startup_reconciliation_admission_uses_shared_barrier_and_expires_at_seal() {
        let coordinator = PersistenceCoordinator::for_release_bootstrap();
        for store in EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        let admission = coordinator
            .admit_startup_reconciliation_writes(&[
                CanonicalWriteOwner::MemoryStore,
                CanonicalWriteOwner::VectorStore,
            ])
            .expect("complete pre-seal manifest admits only startup reconciliation");
        let permit = coordinator
            .acquire_canonical_commit_permit(&admission)
            .await
            .expect("startup reconciliation enters the ordinary shared barrier");
        drop(permit);

        coordinator.seal();
        assert!(coordinator
            .acquire_canonical_commit_permit(&admission)
            .await
            .is_err());
        assert!(coordinator
            .admit_startup_reconciliation_writes(&[CanonicalWriteOwner::MemoryStore,])
            .is_err());
    }

    #[test]
    fn canonical_manifest_and_noncanonical_exclusions_are_explicit_and_disjoint() {
        let expected = EXPECTED_BOOTSTRAP_STORES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(expected.len(), EXPECTED_BOOTSTRAP_STORES.len());
        for (surface, reason) in EXPLICIT_NON_CANONICAL_BOOTSTRAP_SURFACES {
            assert!(!reason.trim().is_empty());
            assert!(!expected.contains(surface));
        }
        assert!(!expected.contains("TaskStore"));
        assert!(!expected.contains("StateStore"));
        assert!(expected.contains("LifeModelFileStore"));
    }

    #[tokio::test]
    async fn isolated_evaluation_executes_without_claiming_canonical_or_live_credit() {
        let coordinator = PersistenceCoordinator::isolated_evaluation();
        let snapshot = coordinator.snapshot();
        assert_eq!(snapshot.mode, PersistenceRuntimeMode::IsolatedEvaluation);
        assert!(coordinator.require_effects_allowed().is_ok());
        assert!(!snapshot.canonical_writes_allowed);
        assert!(!snapshot.live_or_canonical_credit_eligible);

        let admission = coordinator
            .admit_canonical_writes(&[CanonicalWriteOwner::MemoryStore])
            .expect("isolated mechanics may enter the local commit barrier");
        let _permit = coordinator
            .acquire_canonical_commit_permit(&admission)
            .await
            .expect("isolated mechanics retain no canonical/live credit");
    }

    #[tokio::test]
    async fn canonical_barrier_rejects_empty_duplicate_and_stale_normal_admissions() {
        let coordinator =
            PersistenceCoordinator::with_expected_stores(["MemoryStore", "VectorStore"]);
        coordinator.register_read_write("MemoryStore");
        coordinator.register_read_write("VectorStore");
        coordinator.seal();
        assert!(coordinator.admit_canonical_writes(&[]).is_err());
        assert!(coordinator
            .admit_canonical_writes(&[
                CanonicalWriteOwner::MemoryStore,
                CanonicalWriteOwner::MemoryStore,
            ])
            .is_err());

        let stale = coordinator
            .admit_canonical_writes(&[
                CanonicalWriteOwner::MemoryStore,
                CanonicalWriteOwner::VectorStore,
            ])
            .expect("healthy runtime admits a bounded multi-owner write");
        coordinator.degrade_globally("injected_canonical_write_stop");
        assert!(coordinator
            .acquire_canonical_commit_permit(&stale)
            .await
            .is_err());
    }

    #[test]
    fn sealed_runtime_allows_only_monotonic_degradation_and_rejects_upgrade() {
        let coordinator = PersistenceCoordinator::with_expected_stores(["memory"]);
        coordinator.register_read_write("memory");
        coordinator.seal();
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::ReadWrite
        );

        coordinator.register_read_only("memory", "runtime_io_failure", "disk read-only");
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::ReadOnlyDegraded
        );
        coordinator.register_read_write("memory");
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::ReadOnlyDegraded,
            "runtime health cannot upgrade without a full restart and bootstrap validation"
        );
        coordinator.register_unavailable("memory", "runtime_corruption", "quick_check failed");
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::UnavailableDegraded
        );
        coordinator.register_read_only("memory", "attempted_upgrade", "ignored");
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::UnavailableDegraded
        );
        coordinator.register_unavailable("unknown_store", "unknown", "ignored");
        assert!(!coordinator
            .snapshot()
            .stores
            .iter()
            .any(|health| health.store == "unknown_store"));
    }

    #[test]
    fn user_validation_and_cas_errors_cannot_trigger_runtime_degradation() {
        let coordinator = PersistenceCoordinator::with_expected_stores(["permissions"]);
        coordinator.register_read_write("permissions");
        coordinator.seal();
        for untrusted_error in [
            "invalid policy value supplied by user",
            "record not found",
            "compare-and-swap conflict",
            "UNIQUE constraint failed: tool_permissions.id",
            "permission denied by product policy",
        ] {
            assert!(!coordinator.register_runtime_durable_failure("permissions", untrusted_error));
            assert_eq!(
                coordinator.snapshot().mode,
                PersistenceRuntimeMode::ReadWrite
            );
        }

        assert!(coordinator.register_runtime_durable_failure(
            "permissions",
            "database is locked after busy timeout"
        ));
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::UnavailableDegraded
        );
    }

    #[test]
    fn degraded_gate_prevents_provider_tool_and_canonical_write_closures() {
        let coordinator = PersistenceCoordinator::with_expected_stores(["memory"]);
        coordinator.register_read_only("memory", "migration_failed", "read-only filesystem");
        coordinator.seal();

        let provider_dispatches = std::cell::Cell::new(0usize);
        let tool_dispatches = std::cell::Cell::new(0usize);
        let canonical_writes = std::cell::Cell::new(0usize);
        for effect in [&provider_dispatches, &tool_dispatches, &canonical_writes] {
            if coordinator.require_effects_allowed().is_ok() {
                effect.set(effect.get() + 1);
            }
        }

        assert_eq!(provider_dispatches.get(), 0);
        assert_eq!(tool_dispatches.get(), 0);
        assert_eq!(canonical_writes.get(), 0);
        let snapshot = coordinator.snapshot();
        assert_eq!(snapshot.mode, PersistenceRuntimeMode::ReadOnlyDegraded);
        assert!(!snapshot.provider_dispatch_allowed);
        assert!(!snapshot.tool_dispatch_allowed);
        assert!(!snapshot.canonical_writes_allowed);
    }

    #[tokio::test]
    async fn canonical_outbox_worker_is_single_owner_and_notifications_coalesce() {
        let coordinator = PersistenceCoordinator::isolated_evaluation();
        assert!(coordinator.claim_canonical_outbox_worker());
        assert!(!coordinator.claim_canonical_outbox_worker());

        coordinator.notify_canonical_outbox_worker();
        coordinator.notify_canonical_outbox_worker();
        tokio::time::timeout(
            std::time::Duration::from_millis(50),
            coordinator.wait_for_canonical_outbox_work(),
        )
        .await
        .expect("one coalesced durable-queue wake-up");
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(20),
            coordinator.wait_for_canonical_outbox_work(),
        )
        .await
        .is_err());
    }
}
