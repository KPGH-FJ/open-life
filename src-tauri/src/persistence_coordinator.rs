use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

use openlife_core::persistence_outbox::{
    GovernedDataImportJournal, GovernedDataImportReceipt,
    GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON,
};

pub const PERSISTENCE_EFFECTS_BLOCKED: &str = "persistence_effects_blocked";
pub const PERSISTENCE_STORE_UNAVAILABLE: &str = "persistence_store_unavailable";
pub const PERSISTENCE_ADMISSION_INVALIDATED: &str = "persistence_admission_invalidated";

pub const EXPECTED_BOOTSTRAP_STORES: &[&str] = &[
    "ConfigStore",
    "LifeModelFileStore",
    "LifeModelFileJournal",
    "GovernedDataImportJournal",
    "PluginRegistry",
    "PrivacyPolicyStore",
    "McpAuditKeyReferenceStore",
    "MemoryStore",
    "FeedbackStore",
    "VectorStore",
    "AgentRunStore",
    "CanonicalTaskRuntimeStore",
    "EvidenceStore",
    "LifeEventStore",
    "ProposalStore",
    "MemoryLifecycleStore",
    "PlanExecuteSessionStore",
    "MainChatAgentSessionStore",
    "MainChatActionQueueStore",
    "MainChatAgentEventStore",
    "PatchStore",
    "McpAuditStore",
    "ToolPermissionStore",
    "TaskStore",
];

/// Personalization and learning stores enrich the Agent but do not own its
/// basic ability to answer or dispatch an otherwise authorized capability.
/// Their exact read/write gateways still fail closed when the corresponding
/// feature is used.
const OPTIONAL_PERSONALIZATION_STORES: &[&str] = &[
    "LifeModelFileStore",
    "LifeModelFileJournal",
    "FeedbackStore",
    "VectorStore",
    "EvidenceStore",
    "MemoryLifecycleStore",
    "PatchStore",
];

#[cfg(test)]
pub const EXPLICIT_NON_CANONICAL_BOOTSTRAP_SURFACES: &[(&str, &str)] = &[
    ("VersionManager", "derived rollback snapshots"),
    ("RolloutMetricsStore", "derived telemetry"),
    ("SkillRegistry", "built-in and manifest-derived registry"),
    ("HotMemoryCache", "rebuildable cache"),
    ("ProviderHealthCache", "ephemeral observation cache"),
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

/// The only canonical owners that a governed data-import recovery is allowed
/// to mutate. Keeping this as a closed enum prevents the recovery capability
/// from being repurposed for provider dispatch, tool dispatch, configuration,
/// or an unrelated canonical write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// Store suffixes are part of the persisted recovery-owner vocabulary.
#[expect(
    clippy::enum_variant_names,
    reason = "owner=backend-contracts; expires=2026-10-01; preserve serialized or recovery vocabulary"
)]
pub(crate) enum GovernedDataImportRecoveryOwner {
    LifeModelFileStore,
    MemoryStore,
    VectorStore,
    StateStore,
}

/// Exact local canonical owner identity carried by commit admissions. Import
/// recovery remains a closed subset; AgentRun lifecycle writes are normal-only
/// and cannot be smuggled through a governed-import capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CanonicalWriteOwner {
    GovernedDataImport(GovernedDataImportRecoveryOwner),
    AgentRunStore,
}

impl From<GovernedDataImportRecoveryOwner> for CanonicalWriteOwner {
    fn from(owner: GovernedDataImportRecoveryOwner) -> Self {
        Self::GovernedDataImport(owner)
    }
}

impl CanonicalWriteOwner {
    fn required_store_names(self) -> &'static [&'static str] {
        match self {
            Self::GovernedDataImport(GovernedDataImportRecoveryOwner::LifeModelFileStore) => {
                &["LifeModelFileStore", "LifeModelFileJournal", "PatchStore"]
            }
            Self::GovernedDataImport(GovernedDataImportRecoveryOwner::MemoryStore) => {
                &["MemoryStore"]
            }
            Self::GovernedDataImport(GovernedDataImportRecoveryOwner::VectorStore) => {
                &["VectorStore"]
            }
            // State imports already run through the coordinator's global
            // critical-store gate. Unlike optional personalization stores,
            // StateStore is not independently registered by every governed
            // import fixture/runtime owner, so requiring a second registry
            // entry here would reject an otherwise available canonical store.
            Self::GovernedDataImport(GovernedDataImportRecoveryOwner::StateStore) => &[],
            Self::AgentRunStore => &["AgentRunStore"],
        }
    }

    fn governed_data_import_store_name(self) -> Option<&'static str> {
        match self {
            Self::GovernedDataImport(owner) => Some(owner.store_name()),
            Self::AgentRunStore => None,
        }
    }
}

impl GovernedDataImportRecoveryOwner {
    fn store_name(self) -> &'static str {
        match self {
            Self::LifeModelFileStore => "LifeModelFileStore",
            Self::MemoryStore => "MemoryStore",
            Self::VectorStore => "VectorStore",
            Self::StateStore => "StateStore",
        }
    }
}

/// An intentionally non-serializable, non-cloneable, crate-internal recovery
/// capability. Its fields are module-private, so sibling modules can only
/// obtain one through `mint_governed_data_import_recovery_admission`, which
/// verifies the receipt against the durable journal before minting it.
pub(crate) struct GovernedDataImportRecoveryAdmission<'journal> {
    journal: &'journal GovernedDataImportJournal,
    operation_id: String,
    payload_digest: String,
    request_digest: String,
    allowed_stores: BTreeSet<String>,
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
pub(crate) struct CanonicalWriteAdmission<'journal> {
    generation: u64,
    owners: BTreeSet<CanonicalWriteOwner>,
    kind: CanonicalWriteAdmissionKind<'journal>,
}

/// Non-cloneable, non-serializable proof that this operation was admitted for
/// exactly the AgentRun canonical owner. It deliberately has no import-recovery
/// constructor or conversion from a generic admission.
#[must_use = "exchange the AgentRun admission after the task fence and before the owner transaction"]
pub(crate) struct AgentRunCanonicalWriteAdmission {
    inner: CanonicalWriteAdmission<'static>,
}

enum CanonicalWriteAdmissionKind<'journal> {
    Normal,
    StartupReconciliation,
    GovernedDataImportRecovery {
        journal: &'journal GovernedDataImportJournal,
        operation_id: String,
        payload_digest: String,
        request_digest: String,
        allowed_stores: BTreeSet<String>,
    },
}

/// RAII proof that a canonical owner is inside the only local commit window.
/// Keep this guard only across the bounded local owner mutation; drop it before
/// projection, provider, tool, or network awaits.
#[derive(Debug)]
#[must_use = "hold the permit across the bounded canonical commit window"]
pub(crate) struct CanonicalCommitPermit<'coordinator> {
    _barrier: tokio::sync::RwLockReadGuard<'coordinator, ()>,
}

/// Exclusive governed-import terminalization fence. While this value is alive
/// no admitted canonical owner can enter a commit window. The acquisition
/// method decides whether this is a healthy completion fence or a recovery
/// resolution fence; the opaque guard itself carries no policy authority.
#[derive(Debug)]
#[must_use = "hold the fence from owner observation through terminal journal commit"]
pub(crate) struct GovernedDataImportTerminalizationFence<'coordinator> {
    _barrier: tokio::sync::RwLockWriteGuard<'coordinator, ()>,
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
    /// Lock order is strict: canonical barrier -> durable recovery journal ->
    /// owner-local mutex/transaction. Gateways must never request this barrier
    /// while already holding an owner lock, and must never hold it across an
    /// external await. The exclusive recovery fence follows the same prefix and
    /// then observes owners sequentially without mutating them.
    canonical_write_barrier: tokio::sync::RwLock<()>,
    canonical_outbox_worker_started: AtomicBool,
    canonical_outbox_notify: tokio::sync::Notify,
    agent_run_causal_locks: Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
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
            agent_run_causal_locks: Mutex::new(HashMap::new()),
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
            agent_run_causal_locks: Mutex::new(HashMap::new()),
        }
    }

    /// Explicit fixture mode. It permits isolated mechanics to execute but is
    /// never eligible for durable, live-provider, or product-trial credit.
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
            agent_run_causal_locks: Mutex::new(HashMap::new()),
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
    pub fn agent_run_causal_lock(&self, run_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .agent_run_causal_locks
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

    /// Mints the narrow recovery capability only from the durable journal's
    /// current non-terminal receipt. A caller-created receipt is insufficient:
    /// every field must match the journal-owned receipt exactly.
    ///
    /// This method deliberately does not remove the global degradation reason.
    /// The process remains fail-closed for every ordinary effect throughout
    /// recovery and requires a clean bootstrap after terminalization.
    pub(crate) fn mint_governed_data_import_recovery_admission<'journal>(
        &self,
        journal: &'journal GovernedDataImportJournal,
        receipt: &GovernedDataImportReceipt,
        operation_id: &str,
        payload_digest: &str,
        request_digest: &str,
    ) -> anyhow::Result<GovernedDataImportRecoveryAdmission<'journal>> {
        let recovery_state_is_safe = {
            let state = self
                .state
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            governed_data_import_recovery_state_is_safe(&state)
        };
        if !recovery_state_is_safe {
            anyhow::bail!("governed data-import recovery admission is not available");
        }

        if receipt.stage.is_terminal()
            || receipt.operation_id != operation_id
            || receipt.payload_digest != payload_digest
            || receipt.request_digest != request_digest
            || !is_canonical_uuid_v4(operation_id)
            || !is_sha256_digest(payload_digest)
            || !is_sha256_digest(request_digest)
        {
            anyhow::bail!("governed data-import recovery receipt binding mismatch");
        }

        let durable_receipt = journal
            .recovery_requirement()?
            .ok_or_else(|| anyhow::anyhow!("no governed data-import recovery is pending"))?;
        if durable_receipt != *receipt {
            anyhow::bail!("governed data-import recovery receipt is not current durable truth");
        }

        let allowed_stores = governed_data_import_receipt_stores(receipt)?;
        Ok(GovernedDataImportRecoveryAdmission {
            journal,
            operation_id: operation_id.to_string(),
            payload_digest: payload_digest.to_string(),
            request_digest: request_digest.to_string(),
            allowed_stores,
        })
    }

    /// Synchronous admission for one or more canonical import owners. The
    /// returned value is only a pre-commit decision; callers must still obtain
    /// `acquire_canonical_commit_permit` before touching any owner lock or
    /// transaction. A single multi-owner admission/permit must cover an atomic
    /// Memory+Vector replacement so Tokio's writer preference cannot deadlock
    /// on recursive read-lock acquisition.
    pub(crate) fn admit_normal_or_governed_data_import_writes<'journal>(
        &self,
        owners: &[GovernedDataImportRecoveryOwner],
        recovery: Option<&GovernedDataImportRecoveryAdmission<'journal>>,
        operation_id: &str,
        payload_digest: &str,
        request_digest: &str,
    ) -> Result<CanonicalWriteAdmission<'journal>, PersistenceGateError> {
        let requested_owner_count = owners.len();
        let owners = owners
            .iter()
            .copied()
            .map(CanonicalWriteOwner::from)
            .collect::<BTreeSet<_>>();
        let (mode, generation, recovery_state_is_safe, unavailable_owner) = {
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
            (
                mode,
                state.admission_generation,
                governed_data_import_recovery_state_is_safe(&state),
                canonical_owner_write_error(&state, &owners),
            )
        };
        if owners.is_empty() || owners.len() != requested_owner_count {
            return Err(PersistenceGateError::EffectsBlocked { mode });
        }
        if matches!(
            mode,
            PersistenceRuntimeMode::ReadWrite | PersistenceRuntimeMode::IsolatedEvaluation
        ) {
            // A caller that explicitly chose the recovery lane must never be
            // silently downgraded to ordinary write authority. In particular,
            // a token minted by another coordinator/journal cannot authorize a
            // healthy AppState merely because normal writes are enabled there.
            if recovery.is_some() {
                return Err(PersistenceGateError::EffectsBlocked { mode });
            }
            if let Some(error) = unavailable_owner {
                return Err(error);
            }
            return Ok(CanonicalWriteAdmission {
                generation,
                owners,
                kind: CanonicalWriteAdmissionKind::Normal,
            });
        }
        if !recovery_state_is_safe {
            return Err(PersistenceGateError::EffectsBlocked { mode });
        }

        let admitted = if let Some(token) = recovery {
            let token_binding_matches = token.operation_id == operation_id
                && token.payload_digest == payload_digest
                && token.request_digest == request_digest
                && owners.iter().all(|owner| {
                    owner
                        .governed_data_import_store_name()
                        .is_some_and(|store| token.allowed_stores.contains(store))
                });
            if !token_binding_matches {
                false
            } else {
                match token.journal.recovery_requirement() {
                    Ok(Some(current)) => {
                        !current.stage.is_terminal()
                            && current.operation_id == operation_id
                            && current.payload_digest == payload_digest
                            && current.request_digest == request_digest
                            && governed_data_import_receipt_stores(&current)
                                .is_ok_and(|stores| stores == token.allowed_stores)
                    }
                    Ok(None) => false,
                    Err(error) => {
                        self.register_unavailable(
                            "GovernedDataImportJournal",
                            "data_import_journal_read_failed",
                            &error.to_string(),
                        );
                        false
                    }
                }
            }
        } else {
            false
        };
        if admitted {
            let token = recovery.expect("admitted recovery requires a token");
            Ok(CanonicalWriteAdmission {
                generation,
                owners,
                kind: CanonicalWriteAdmissionKind::GovernedDataImportRecovery {
                    journal: token.journal,
                    operation_id: token.operation_id.clone(),
                    payload_digest: token.payload_digest.clone(),
                    request_digest: token.request_digest.clone(),
                    allowed_stores: token.allowed_stores.clone(),
                },
            })
        } else {
            Err(PersistenceGateError::EffectsBlocked { mode })
        }
    }

    /// Backward-compatible single-owner shape. Unlike the previous unit gate,
    /// it now returns a generation-bound admission that must be exchanged for
    /// a commit permit by the gateway.
    pub(crate) fn require_normal_or_governed_data_import_write<'journal>(
        &self,
        owner: GovernedDataImportRecoveryOwner,
        recovery: Option<&GovernedDataImportRecoveryAdmission<'journal>>,
        operation_id: &str,
        payload_digest: &str,
        request_digest: &str,
    ) -> Result<CanonicalWriteAdmission<'journal>, PersistenceGateError> {
        self.admit_normal_or_governed_data_import_writes(
            &[owner],
            recovery,
            operation_id,
            payload_digest,
            request_digest,
        )
    }

    /// Pre-seal startup reconciliation uses the same process-wide barrier as
    /// product writes, but it cannot pretend that ordinary effects are already
    /// enabled. Admission is available only after every expected canonical
    /// store is registered read-write and before `seal`; the generation is
    /// rechecked again inside the shared commit window.
    pub(crate) fn admit_startup_reconciliation_writes(
        &self,
        owners: &[GovernedDataImportRecoveryOwner],
    ) -> Result<CanonicalWriteAdmission<'static>, PersistenceGateError> {
        let requested_owner_count = owners.len();
        let owners = owners
            .iter()
            .copied()
            .map(CanonicalWriteOwner::from)
            .collect::<BTreeSet<_>>();
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

    /// Normal product admission for exactly one AgentRun canonical mutation.
    /// This is intentionally separate from governed-import recovery ownership.
    pub(crate) fn admit_agent_run_write(
        &self,
    ) -> Result<AgentRunCanonicalWriteAdmission, PersistenceGateError> {
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
        if !matches!(
            mode,
            PersistenceRuntimeMode::ReadWrite | PersistenceRuntimeMode::IsolatedEvaluation
        ) {
            return Err(PersistenceGateError::EffectsBlocked { mode });
        }
        if let Some(error) = canonical_owner_write_error(
            &state,
            &[CanonicalWriteOwner::AgentRunStore].into_iter().collect(),
        ) {
            return Err(error);
        }
        Ok(AgentRunCanonicalWriteAdmission {
            inner: CanonicalWriteAdmission {
                generation: state.admission_generation,
                owners: [CanonicalWriteOwner::AgentRunStore].into_iter().collect(),
                kind: CanonicalWriteAdmissionKind::Normal,
            },
        })
    }

    /// Pre-seal startup reconciliation admission for exactly one AgentRun
    /// canonical mutation. AgentRun intentionally remains outside
    /// `GovernedDataImportRecoveryOwner`: startup repair is a bounded lifecycle
    /// projection, not a data-import recovery capability.
    pub(crate) fn admit_startup_agent_run_write(
        &self,
    ) -> Result<AgentRunCanonicalWriteAdmission, PersistenceGateError> {
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
        if !startup_reconciliation_state_is_safe(&state) {
            return Err(PersistenceGateError::EffectsBlocked { mode });
        }
        Ok(AgentRunCanonicalWriteAdmission {
            inner: CanonicalWriteAdmission {
                generation: state.admission_generation,
                owners: [CanonicalWriteOwner::AgentRunStore].into_iter().collect(),
                kind: CanonicalWriteAdmissionKind::StartupReconciliation,
            },
        })
    }

    /// Revalidates a synchronous admission inside the shared canonical-write
    /// barrier. This method must be called before taking an owner mutex or
    /// opening its transaction. The returned guard is intentionally opaque.
    pub(crate) async fn acquire_canonical_commit_permit<'coordinator>(
        &'coordinator self,
        admission: &CanonicalWriteAdmission<'_>,
    ) -> Result<CanonicalCommitPermit<'coordinator>, PersistenceGateError> {
        let barrier = self.canonical_write_barrier.read().await;
        let (
            mode,
            generation,
            recovery_state_is_safe,
            startup_reconciliation_is_safe,
            unavailable_owner,
        ) = {
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
                governed_data_import_recovery_state_is_safe(&state),
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
            CanonicalWriteAdmissionKind::GovernedDataImportRecovery {
                journal,
                operation_id,
                payload_digest,
                request_digest,
                allowed_stores,
            } if recovery_state_is_safe
                && admission.owners.iter().all(|owner| {
                    owner
                        .governed_data_import_store_name()
                        .is_some_and(|store| allowed_stores.contains(store))
                }) =>
            {
                let current = match journal.recovery_requirement() {
                    Ok(Some(current)) => current,
                    Ok(None) => return Err(PersistenceGateError::EffectsBlocked { mode }),
                    Err(error) => {
                        self.register_unavailable(
                            "GovernedDataImportJournal",
                            "data_import_journal_read_failed",
                            &error.to_string(),
                        );
                        return Err(PersistenceGateError::EffectsBlocked {
                            mode: self.snapshot().mode,
                        });
                    }
                };
                let binding_matches = !current.stage.is_terminal()
                    && current.operation_id == *operation_id
                    && current.payload_digest == *payload_digest
                    && current.request_digest == *request_digest
                    && governed_data_import_receipt_stores(&current)
                        .is_ok_and(|stores| stores == *allowed_stores);
                if !binding_matches {
                    return Err(PersistenceGateError::EffectsBlocked { mode });
                }
            }
            _ => return Err(PersistenceGateError::EffectsBlocked { mode }),
        }

        Ok(CanonicalCommitPermit { _barrier: barrier })
    }

    /// Revalidates an AgentRun-only admission at the lifecycle operation's
    /// linearization point. The caller must already hold the per-task fence and
    /// must keep the returned permit across the complete AgentRun transaction,
    /// including its canonical outbox mutation.
    pub(crate) async fn acquire_agent_run_commit_permit<'coordinator>(
        &'coordinator self,
        admission: &AgentRunCanonicalWriteAdmission,
    ) -> Result<CanonicalCommitPermit<'coordinator>, PersistenceGateError> {
        debug_assert_eq!(
            admission.inner.owners,
            [CanonicalWriteOwner::AgentRunStore].into_iter().collect()
        );
        self.acquire_canonical_commit_permit(&admission.inner).await
    }

    /// Enters the exclusive cross-owner recovery fence. It may transition a
    /// healthy sealed runtime into the one permitted recovery degradation, or
    /// re-enter an already recovery-degraded runtime. Any unrelated degradation
    /// fails closed. The exact durable receipt is checked while the barrier is
    /// held, before old admissions are invalidated and again before returning.
    pub(crate) async fn acquire_governed_data_import_resolution_fence<'coordinator>(
        &'coordinator self,
        journal: &GovernedDataImportJournal,
        expected_receipt: &GovernedDataImportReceipt,
    ) -> anyhow::Result<GovernedDataImportTerminalizationFence<'coordinator>> {
        validate_governed_data_import_resolution_receipt(expected_receipt)?;
        let barrier = self.canonical_write_barrier.write().await;

        {
            let state = self
                .state
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !governed_data_import_resolution_state_can_enter(&state) {
                anyhow::bail!("governed data-import resolution fence is not available");
            }
        }
        require_exact_durable_recovery_receipt(journal, expected_receipt)?;

        {
            let mut state = self
                .state
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !governed_data_import_resolution_state_can_enter(&state) {
                anyhow::bail!("governed data-import resolution fence state changed");
            }
            invalidate_canonical_write_admissions(&mut state);
            if state.global_reason_codes.is_empty() {
                state
                    .global_reason_codes
                    .push(GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON.to_string());
            }
        }

        require_exact_durable_recovery_receipt(journal, expected_receipt)?;
        {
            let state = self
                .state
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !governed_data_import_recovery_state_is_safe(&state) {
                anyhow::bail!(
                    "governed data-import resolution fence lost exclusive recovery state"
                );
            }
        }

        Ok(GovernedDataImportTerminalizationFence { _barrier: barrier })
    }

    /// Linearizes a successful governed import without turning an otherwise
    /// healthy runtime into recovery mode. The write side drains every active
    /// canonical owner permit, invalidates admissions minted before the
    /// terminal observation, and remains held through the caller's exact owner
    /// verification and durable `Completed` transition.
    ///
    /// A restarted exact-recovery import may also complete through this fence,
    /// but unrelated degradation, initialization and isolated evaluation all
    /// fail closed. Unlike the resolution fence, this method never adds or
    /// removes a runtime reason code.
    pub(crate) async fn acquire_governed_data_import_completion_fence<'coordinator>(
        &'coordinator self,
        journal: &GovernedDataImportJournal,
        expected_receipt: &GovernedDataImportReceipt,
    ) -> anyhow::Result<GovernedDataImportTerminalizationFence<'coordinator>> {
        validate_governed_data_import_resolution_receipt(expected_receipt)?;
        let barrier = self.canonical_write_barrier.write().await;

        {
            let state = self
                .state
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !governed_data_import_completion_state_can_enter(&state) {
                anyhow::bail!("governed data-import completion fence is not available");
            }
        }
        require_exact_durable_recovery_receipt(journal, expected_receipt)?;

        {
            let mut state = self
                .state
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !governed_data_import_completion_state_can_enter(&state) {
                anyhow::bail!("governed data-import completion fence state changed");
            }
            invalidate_canonical_write_admissions(&mut state);
        }

        require_exact_durable_recovery_receipt(journal, expected_receipt)?;
        {
            let state = self
                .state
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !governed_data_import_completion_state_can_enter(&state) {
                anyhow::bail!("governed data-import completion fence lost terminalization state");
            }
        }

        Ok(GovernedDataImportTerminalizationFence { _barrier: barrier })
    }

    /// Bootstrap-only admission for internal migration/reconciliation. Product
    /// effects still remain blocked until `seal` completes the full manifest.
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
    // A process cannot practically consume the u64 space. Wrapping is still
    // preferable to saturation here: saturation could make a later recovery
    // fence accept an admission minted at MAX instead of invalidating it.
    state.admission_generation = state.admission_generation.wrapping_add(1);
}

fn governed_data_import_resolution_state_can_enter(state: &PersistenceCoordinatorState) -> bool {
    if !state.sealed
        || state.isolated_evaluation
        || state.expected_stores.is_empty()
        || !(state.global_reason_codes.is_empty()
            || state.global_reason_codes.as_slice()
                == [GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON])
    {
        return false;
    }

    state.expected_stores.iter().all(|store| {
        state
            .stores
            .get(store)
            .is_some_and(|health| health.mode == PersistenceStoreMode::ReadWriteCanonical)
    }) && state
        .stores
        .values()
        .all(|health| health.mode == PersistenceStoreMode::ReadWriteCanonical)
}

fn validate_governed_data_import_resolution_receipt(
    receipt: &GovernedDataImportReceipt,
) -> anyhow::Result<()> {
    if receipt.stage.is_terminal()
        || !is_canonical_uuid_v4(&receipt.operation_id)
        || !is_sha256_digest(&receipt.payload_digest)
        || !is_sha256_digest(&receipt.request_digest)
    {
        anyhow::bail!("governed data-import resolution receipt binding mismatch");
    }
    governed_data_import_receipt_stores(receipt)?;
    Ok(())
}

fn require_exact_durable_recovery_receipt(
    journal: &GovernedDataImportJournal,
    expected_receipt: &GovernedDataImportReceipt,
) -> anyhow::Result<()> {
    let durable_receipt = journal
        .recovery_requirement()?
        .ok_or_else(|| anyhow::anyhow!("no governed data-import recovery is pending"))?;
    if durable_receipt != *expected_receipt {
        anyhow::bail!("governed data-import resolution receipt is not current durable truth");
    }
    Ok(())
}

fn governed_data_import_recovery_state_is_safe(state: &PersistenceCoordinatorState) -> bool {
    if !state.sealed
        || state.isolated_evaluation
        || state.expected_stores.is_empty()
        || state.global_reason_codes.as_slice() != [GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON]
    {
        return false;
    }

    state.expected_stores.iter().all(|store| {
        state
            .stores
            .get(store)
            .is_some_and(|health| health.mode == PersistenceStoreMode::ReadWriteCanonical)
    }) && state
        .stores
        .values()
        .all(|health| health.mode == PersistenceStoreMode::ReadWriteCanonical)
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

fn governed_data_import_completion_state_can_enter(state: &PersistenceCoordinatorState) -> bool {
    if !state.sealed || state.isolated_evaluation || state.expected_stores.is_empty() {
        return false;
    }
    let every_store_is_canonical = state.expected_stores.iter().all(|store| {
        state
            .stores
            .get(store)
            .is_some_and(|health| health.mode == PersistenceStoreMode::ReadWriteCanonical)
    }) && state
        .stores
        .values()
        .all(|health| health.mode == PersistenceStoreMode::ReadWriteCanonical);
    if !every_store_is_canonical {
        return false;
    }
    state.global_reason_codes.is_empty() || governed_data_import_recovery_state_is_safe(state)
}

fn governed_data_import_receipt_stores(
    receipt: &GovernedDataImportReceipt,
) -> anyhow::Result<BTreeSet<String>> {
    if receipt.owners.is_empty() || receipt.target_count as usize != receipt.owners.len() {
        anyhow::bail!("governed data-import recovery receipt owner count mismatch");
    }

    let mut stores = BTreeSet::new();
    for owner in &receipt.owners {
        let supported = matches!(
            (owner.owner.as_str(), owner.import_target.as_str()),
            ("LifeModelFileStore", "life_model")
                | ("MemoryStore", "messages")
                | ("VectorStore", "vectors")
                | ("StateStore", "state_store")
        );
        if !supported || !stores.insert(owner.owner.clone()) {
            anyhow::bail!("invalid governed data-import recovery owner mapping");
        }
    }
    if !stores.contains("LifeModelFileStore") {
        anyhow::bail!("governed data-import recovery requires LifeModelFileStore");
    }
    Ok(stores)
}

fn is_sha256_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn is_canonical_uuid_v4(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|parsed| {
        parsed.get_version() == Some(uuid::Version::Random)
            && parsed.hyphenated().to_string() == value
    })
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
    !OPTIONAL_PERSONALIZATION_STORES.contains(&store)
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
    use openlife_core::persistence_outbox::{
        metadata_digest, GovernedDataImportOwnerPlan, GovernedDataImportOwnerStatus,
        GovernedDataImportOwnerUpdate, GovernedDataImportPrepare, GovernedDataImportStage,
    };

    fn governed_import_digest(marker: char) -> String {
        metadata_digest(&format!("governed-import-test-{marker}"))
    }

    fn governed_import_prepare(
        operation_id: String,
        include_all_owners: bool,
    ) -> GovernedDataImportPrepare {
        let mut owners = vec![GovernedDataImportOwnerPlan {
            owner: "LifeModelFileStore".into(),
            import_target: "life_model".into(),
            before_digest: governed_import_digest('1'),
            target_digest: governed_import_digest('2'),
            item_count: 1,
        }];
        if include_all_owners {
            owners.extend([
                GovernedDataImportOwnerPlan {
                    owner: "MemoryStore".into(),
                    import_target: "messages".into(),
                    before_digest: governed_import_digest('3'),
                    target_digest: governed_import_digest('4'),
                    item_count: 2,
                },
                GovernedDataImportOwnerPlan {
                    owner: "VectorStore".into(),
                    import_target: "vectors".into(),
                    before_digest: governed_import_digest('5'),
                    target_digest: governed_import_digest('6'),
                    item_count: 3,
                },
                GovernedDataImportOwnerPlan {
                    owner: "StateStore".into(),
                    import_target: "state_store".into(),
                    before_digest: governed_import_digest('7'),
                    target_digest: governed_import_digest('8'),
                    item_count: 4,
                },
            ]);
        }
        GovernedDataImportPrepare {
            operation_id,
            payload_digest: governed_import_digest('a'),
            request_digest: governed_import_digest('b'),
            owners,
        }
    }

    fn governed_import_recovery_coordinator() -> PersistenceCoordinator {
        let stores = [
            "GovernedDataImportJournal",
            "LifeModelFileStore",
            "MemoryStore",
            "VectorStore",
            "StateStore",
        ];
        let coordinator = PersistenceCoordinator::with_expected_stores(stores);
        for store in stores {
            coordinator.register_read_write(store);
        }
        coordinator.degrade_globally(GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON);
        coordinator.seal();
        coordinator
    }

    fn governed_import_normal_coordinator() -> PersistenceCoordinator {
        let stores = [
            "GovernedDataImportJournal",
            "LifeModelFileStore",
            "MemoryStore",
            "VectorStore",
            "StateStore",
        ];
        let coordinator = PersistenceCoordinator::with_expected_stores(stores);
        for store in stores {
            coordinator.register_read_write(store);
        }
        coordinator.seal();
        coordinator
    }

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
        let coordinator =
            PersistenceCoordinator::with_expected_stores(["AgentRunStore", "LifeModelFileStore"]);
        coordinator.register_read_write("AgentRunStore");
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
            coordinator.require_normal_or_governed_data_import_write(
                GovernedDataImportRecoveryOwner::LifeModelFileStore,
                None,
                "",
                "",
                "",
            ),
            Err(PersistenceGateError::StoreUnavailable { store, .. })
                if store == "LifeModelFileStore"
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
            health.store == "TaskStore"
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
                GovernedDataImportRecoveryOwner::MemoryStore,
                GovernedDataImportRecoveryOwner::VectorStore,
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
            .admit_startup_reconciliation_writes(&[GovernedDataImportRecoveryOwner::MemoryStore,])
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
        for required in [
            "LifeModelFileStore",
            "LifeModelFileJournal",
            "GovernedDataImportJournal",
            "PluginRegistry",
        ] {
            assert!(expected.contains(required));
        }
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
            .admit_normal_or_governed_data_import_writes(
                &[GovernedDataImportRecoveryOwner::MemoryStore],
                None,
                "",
                "",
                "",
            )
            .expect("isolated mechanics may enter the local commit barrier");
        let _permit = coordinator
            .acquire_canonical_commit_permit(&admission)
            .await
            .expect("isolated mechanics retain no canonical/live credit");
    }

    #[tokio::test]
    async fn canonical_barrier_rejects_empty_duplicate_and_stale_normal_admissions() {
        let coordinator = governed_import_normal_coordinator();
        assert!(coordinator
            .admit_normal_or_governed_data_import_writes(&[], None, "", "", "")
            .is_err());
        assert!(coordinator
            .admit_normal_or_governed_data_import_writes(
                &[
                    GovernedDataImportRecoveryOwner::MemoryStore,
                    GovernedDataImportRecoveryOwner::MemoryStore,
                ],
                None,
                "",
                "",
                "",
            )
            .is_err());

        let stale = coordinator
            .admit_normal_or_governed_data_import_writes(
                &[
                    GovernedDataImportRecoveryOwner::MemoryStore,
                    GovernedDataImportRecoveryOwner::VectorStore,
                ],
                None,
                "",
                "",
                "",
            )
            .expect("healthy runtime admits a bounded multi-owner write");
        coordinator.degrade_globally("injected_canonical_write_stop");
        assert!(coordinator
            .acquire_canonical_commit_permit(&stale)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn agent_run_admission_is_exact_normal_only_and_generation_bound() {
        let coordinator = PersistenceCoordinator::for_release_bootstrap();
        for store in EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        coordinator.seal();
        let admission = coordinator
            .admit_agent_run_write()
            .expect("healthy sealed runtime admits exactly AgentRunStore");

        coordinator.degrade_globally("agent_run_admission_counterfactual");
        assert!(matches!(
            coordinator
                .acquire_agent_run_commit_permit(&admission)
                .await,
            Err(PersistenceGateError::AdmissionInvalidated { .. })
        ));
        assert!(coordinator.admit_agent_run_write().is_err());

        let import_recovery = governed_import_recovery_coordinator();
        assert!(
            import_recovery.admit_agent_run_write().is_err(),
            "governed import recovery must never grant AgentRun owner authority"
        );
    }

    #[test]
    fn healthy_runtime_never_downgrades_an_explicit_recovery_token_to_normal_authority() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let prepare = governed_import_prepare(uuid::Uuid::new_v4().to_string(), true);
        let receipt = journal.prepare(prepare.clone()).unwrap().receipt;
        let recovery_coordinator = governed_import_recovery_coordinator();
        let recovery = recovery_coordinator
            .mint_governed_data_import_recovery_admission(
                &journal,
                &receipt,
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .unwrap();

        let unrelated_healthy_coordinator = governed_import_normal_coordinator();
        assert!(unrelated_healthy_coordinator
            .admit_normal_or_governed_data_import_writes(
                &[GovernedDataImportRecoveryOwner::MemoryStore],
                Some(&recovery),
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
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

    #[test]
    fn governed_data_import_recovery_token_is_durable_bound_and_owner_scoped() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let prepare = governed_import_prepare(uuid::Uuid::new_v4().to_string(), true);
        let receipt = journal.prepare(prepare.clone()).unwrap().receipt;
        let coordinator = governed_import_recovery_coordinator();
        let token = coordinator
            .mint_governed_data_import_recovery_admission(
                &journal,
                &receipt,
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .expect("durable current receipt mints the narrow recovery token");

        for owner in [
            GovernedDataImportRecoveryOwner::LifeModelFileStore,
            GovernedDataImportRecoveryOwner::MemoryStore,
            GovernedDataImportRecoveryOwner::VectorStore,
            GovernedDataImportRecoveryOwner::StateStore,
        ] {
            let _admission = coordinator
                .require_normal_or_governed_data_import_write(
                    owner,
                    Some(&token),
                    &prepare.operation_id,
                    &prepare.payload_digest,
                    &prepare.request_digest,
                )
                .expect("receipt owner is admitted for this exact recovery");
        }

        assert!(coordinator.require_effects_allowed().is_err());
        let snapshot = coordinator.snapshot();
        assert_eq!(snapshot.mode, PersistenceRuntimeMode::ReadOnlyDegraded);
        assert!(!snapshot.provider_dispatch_allowed);
        assert!(!snapshot.tool_dispatch_allowed);
        assert!(!snapshot.canonical_writes_allowed);
        assert_eq!(
            snapshot.global_reason_codes,
            vec![GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON.to_string()],
            "minting and consuming the token must not clear degradation"
        );
    }

    #[test]
    fn governed_data_import_recovery_rejects_forged_terminal_or_mismatched_receipts() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let prepare = governed_import_prepare(uuid::Uuid::new_v4().to_string(), true);
        let receipt = journal.prepare(prepare.clone()).unwrap().receipt;
        let coordinator = governed_import_recovery_coordinator();
        let token = coordinator
            .mint_governed_data_import_recovery_admission(
                &journal,
                &receipt,
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .unwrap();

        let mut forged = receipt.clone();
        forged.request_digest = governed_import_digest('c');
        assert!(coordinator
            .mint_governed_data_import_recovery_admission(
                &journal,
                &forged,
                &prepare.operation_id,
                &prepare.payload_digest,
                &forged.request_digest,
            )
            .is_err());
        assert!(coordinator
            .mint_governed_data_import_recovery_admission(
                &journal,
                &receipt,
                &prepare.operation_id,
                &governed_import_digest('d'),
                &prepare.request_digest,
            )
            .is_err());

        let steps = [
            (
                GovernedDataImportStage::LifeModelApplied,
                "LifeModelFileStore",
            ),
            (GovernedDataImportStage::MemoryApplied, "MemoryStore"),
            (GovernedDataImportStage::VectorApplied, "VectorStore"),
            (GovernedDataImportStage::StateCommitted, "StateStore"),
        ];
        for (stage, owner) in steps {
            journal
                .transition(
                    &prepare.operation_id,
                    stage,
                    &[GovernedDataImportOwnerUpdate {
                        owner: owner.into(),
                        status: GovernedDataImportOwnerStatus::Applied,
                    }],
                    None,
                )
                .unwrap();
        }
        let terminal = journal
            .transition(
                &prepare.operation_id,
                GovernedDataImportStage::Completed,
                &[],
                Some(&governed_import_digest('e')),
            )
            .unwrap();
        assert!(terminal.stage.is_terminal());
        assert!(coordinator
            .mint_governed_data_import_recovery_admission(
                &journal,
                &terminal,
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .is_err());
        assert!(
            coordinator
                .require_normal_or_governed_data_import_write(
                    GovernedDataImportRecoveryOwner::LifeModelFileStore,
                    Some(&token),
                    &prepare.operation_id,
                    &prepare.payload_digest,
                    &prepare.request_digest,
                )
                .is_err(),
            "a token must stop admitting writes once its durable receipt terminalizes"
        );
    }

    #[test]
    fn governed_data_import_recovery_fails_closed_on_state_or_binding_drift() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let prepare = governed_import_prepare(uuid::Uuid::new_v4().to_string(), false);
        let receipt = journal.prepare(prepare.clone()).unwrap().receipt;
        let coordinator = governed_import_recovery_coordinator();
        let token = coordinator
            .mint_governed_data_import_recovery_admission(
                &journal,
                &receipt,
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .unwrap();

        assert!(coordinator
            .require_normal_or_governed_data_import_write(
                GovernedDataImportRecoveryOwner::MemoryStore,
                Some(&token),
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .is_err());
        assert!(coordinator
            .require_normal_or_governed_data_import_write(
                GovernedDataImportRecoveryOwner::LifeModelFileStore,
                Some(&token),
                &uuid::Uuid::new_v4().to_string(),
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .is_err());

        coordinator.degrade_globally("unrelated_runtime_failure");
        assert!(coordinator
            .require_normal_or_governed_data_import_write(
                GovernedDataImportRecoveryOwner::LifeModelFileStore,
                Some(&token),
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .is_err());

        let unsealed = PersistenceCoordinator::with_expected_stores(["LifeModelFileStore"]);
        unsealed.register_read_write("LifeModelFileStore");
        unsealed.degrade_globally(GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON);
        assert!(unsealed
            .mint_governed_data_import_recovery_admission(
                &journal,
                &receipt,
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .is_err());

        let unsafe_store = governed_import_recovery_coordinator();
        unsafe_store.register_read_only("MemoryStore", "runtime_io_failure", "read-only disk");
        assert!(unsafe_store
            .mint_governed_data_import_recovery_admission(
                &journal,
                &receipt,
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .is_err());
    }

    #[tokio::test]
    async fn resolution_fence_rejects_wrong_receipt_and_invalidates_old_admission() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let prepare = governed_import_prepare(uuid::Uuid::new_v4().to_string(), true);
        let receipt = journal.prepare(prepare).unwrap().receipt;
        let coordinator = governed_import_normal_coordinator();
        let old_admission = coordinator
            .admit_normal_or_governed_data_import_writes(
                &[GovernedDataImportRecoveryOwner::MemoryStore],
                None,
                "",
                "",
                "",
            )
            .unwrap();

        let mut wrong_receipt = receipt.clone();
        wrong_receipt.request_digest = governed_import_digest('f');
        assert!(coordinator
            .acquire_governed_data_import_resolution_fence(&journal, &wrong_receipt)
            .await
            .is_err());
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::ReadWrite,
            "a rejected receipt must not mutate coordinator health"
        );

        let fence = coordinator
            .acquire_governed_data_import_resolution_fence(&journal, &receipt)
            .await
            .expect("exact durable receipt acquires the exclusive fence");
        assert_eq!(
            coordinator.snapshot().global_reason_codes,
            vec![GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON.to_string()]
        );
        drop(fence);
        assert!(coordinator
            .acquire_canonical_commit_permit(&old_admission)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn completion_fence_invalidates_old_admission_without_degrading_healthy_runtime() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let prepare = governed_import_prepare(uuid::Uuid::new_v4().to_string(), true);
        let receipt = journal.prepare(prepare).unwrap().receipt;
        let coordinator = governed_import_normal_coordinator();
        let old_admission = coordinator
            .admit_normal_or_governed_data_import_writes(
                &[GovernedDataImportRecoveryOwner::MemoryStore],
                None,
                "",
                "",
                "",
            )
            .unwrap();

        let fence = coordinator
            .acquire_governed_data_import_completion_fence(&journal, &receipt)
            .await
            .expect("healthy import may enter its exact completion fence");
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::ReadWrite
        );
        assert!(coordinator.snapshot().global_reason_codes.is_empty());
        drop(fence);
        assert!(coordinator
            .acquire_canonical_commit_permit(&old_admission)
            .await
            .is_err());

        let recovery = governed_import_recovery_coordinator();
        let recovery_fence = recovery
            .acquire_governed_data_import_completion_fence(&journal, &receipt)
            .await
            .expect("the exact restarted import may complete without a second fence type");
        assert_eq!(
            recovery.snapshot().global_reason_codes,
            vec![GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON.to_string()]
        );
        drop(recovery_fence);

        let unrelated = governed_import_normal_coordinator();
        unrelated.degrade_globally("unrelated_runtime_failure");
        assert!(unrelated
            .acquire_governed_data_import_completion_fence(&journal, &receipt)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn shared_commit_window_drains_before_exclusive_fence_and_drop_releases_barrier() {
        let directory = tempfile::tempdir().unwrap();
        let journal =
            Arc::new(GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap());
        let prepare = governed_import_prepare(uuid::Uuid::new_v4().to_string(), true);
        let receipt = journal.prepare(prepare.clone()).unwrap().receipt;
        let coordinator = Arc::new(governed_import_normal_coordinator());
        let admission = coordinator
            .admit_normal_or_governed_data_import_writes(
                &[
                    GovernedDataImportRecoveryOwner::MemoryStore,
                    GovernedDataImportRecoveryOwner::VectorStore,
                ],
                None,
                "",
                "",
                "",
            )
            .unwrap();
        let shared_permit = coordinator
            .acquire_canonical_commit_permit(&admission)
            .await
            .unwrap();

        let (writer_queued_tx, writer_queued_rx) = tokio::sync::oneshot::channel();
        let (writer_acquired_tx, mut writer_acquired_rx) = tokio::sync::oneshot::channel();
        let (release_writer_tx, release_writer_rx) = tokio::sync::oneshot::channel();
        let (writer_released_tx, writer_released_rx) = tokio::sync::oneshot::channel();
        let fence_coordinator = Arc::clone(&coordinator);
        let fence_journal = Arc::clone(&journal);
        let fence_receipt = receipt.clone();
        let fence_task = tokio::spawn(async move {
            let mut fence_future = Box::pin(
                fence_coordinator
                    .acquire_governed_data_import_resolution_fence(&fence_journal, &fence_receipt),
            );
            // `biased` polls the write-lock future first. Because the shared
            // permit is live, that poll deterministically queues the writer;
            // only then does the ready branch notify the test.
            tokio::select! {
                biased;
                _ = &mut fence_future => panic!("exclusive fence bypassed a live shared permit"),
                _ = async { let _ = writer_queued_tx.send(()); } => {}
            }
            let fence = fence_future.await.expect("writer acquires after drain");
            let _ = writer_acquired_tx.send(());
            let _ = release_writer_rx.await;
            drop(fence);
            let _ = writer_released_tx.send(());
        });

        writer_queued_rx.await.unwrap();
        assert!(matches!(
            writer_acquired_rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        drop(shared_permit);
        writer_acquired_rx.await.unwrap();
        release_writer_tx.send(()).unwrap();
        writer_released_rx.await.unwrap();
        fence_task.await.unwrap();

        let recovery = coordinator
            .mint_governed_data_import_recovery_admission(
                &journal,
                &receipt,
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .unwrap();
        let recovery_admission = coordinator
            .admit_normal_or_governed_data_import_writes(
                &[
                    GovernedDataImportRecoveryOwner::MemoryStore,
                    GovernedDataImportRecoveryOwner::VectorStore,
                ],
                Some(&recovery),
                &prepare.operation_id,
                &prepare.payload_digest,
                &prepare.request_digest,
            )
            .unwrap();
        let _recovery_permit = coordinator
            .acquire_canonical_commit_permit(&recovery_admission)
            .await
            .expect("dropping the exclusive fence releases the shared barrier");
    }

    #[tokio::test]
    async fn d067_audit_commit_failure_degrades_and_blocks_later_effects() {
        use openlife_core::agent::ToolAuditPersistenceObserver;

        let coordinator = PersistenceCoordinator::with_expected_stores(["McpAuditStore"]);
        coordinator.register_read_write("McpAuditStore");
        coordinator.seal();
        assert!(coordinator.require_effects_allowed().is_ok());

        let mut registry = openlife_core::mcp::McpRegistry::new();
        registry.register_builtin(
            openlife_core::tool_manifest::ToolManifest {
                id: "d067.read".into(),
                name: "d067.read".into(),
                description: "D067 persistence failure fixture.".into(),
                parameters: serde_json::json!({"type": "object"}),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                idempotency_contract:
                    openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
                tags: vec![],
            },
            Box::new(|_| Ok(serde_json::json!({"ok": true}).to_string())),
        );
        let permission_store =
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit_store = openlife_core::mcp_audit::McpAuditStore::unavailable_sentinel(
            "d067_injected_audit_failure",
        );
        let privacy_engine = openlife_core::privacy::PrivacyEngine::new();
        let owner_store = openlife_core::agent::AgentRunStore::new_in_memory().unwrap();
        let owner_run = openlife_core::agent::AgentRun::new_tool_execution_run("d067.read");
        owner_store.create_run(&owner_run).unwrap();
        let context = openlife_core::agent::ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_agent_run_store(&owner_store)
        .with_tool_audit_persistence_observer(&coordinator);

        let result = openlife_core::agent::ToolGateway::from_executor_config(Default::default())
            .execute(
                openlife_core::agent::AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "d067.read".into(),
                    input: serde_json::json!({}),
                    source_run_id: Some(owner_run.id),
                    step_index: 0,
                },
                &context,
            )
            .await
            .expect("tool outcome remains available after audit failure");

        assert_eq!(
            result.status,
            openlife_core::agent::ActionExecutionStatus::Succeeded
        );
        assert_eq!(
            result.execution_receipt.audit_persistence_status,
            openlife_core::agent::ToolAuditPersistenceStatus::Failed
        );
        assert_eq!(
            coordinator.snapshot().mode,
            PersistenceRuntimeMode::UnavailableDegraded
        );
        assert!(coordinator.require_effects_allowed().is_err());
        assert!(coordinator.snapshot().stores.iter().any(|health| {
            health.store == "McpAuditStore"
                && health.reason_code.as_deref() == Some("runtime_audit_commit_failed")
        }));

        let forged_non_failure =
            openlife_core::agent::ToolExecutionReceipt::test_gateway_failed_before_dispatch(
                None,
                None,
                "d067-non-failure-callback".into(),
                openlife_core::agent::ToolActionEffect::ReadOnly,
                openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
            );
        let fresh = PersistenceCoordinator::with_expected_stores(["McpAuditStore"]);
        fresh.register_read_write("McpAuditStore");
        fresh.seal();
        fresh.audit_persistence_failed(&forged_non_failure);
        assert_eq!(fresh.snapshot().mode, PersistenceRuntimeMode::ReadWrite);
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

    #[test]
    fn shipped_effect_entrypoints_consult_the_same_persistence_gate() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for relative in [
            "main_chat_send.rs",
            "main_chat_streaming.rs",
            "scheduler_runner.rs",
            "memory_gateway.rs",
            "life_model_write_gateway.rs",
            "commands/proposal.rs",
            "commands/execution.rs",
        ] {
            let source = std::fs::read_to_string(root.join(relative)).unwrap();
            assert!(
                source.contains("require_effects_allowed"),
                "{relative} must fail closed through PersistenceCoordinator"
            );
        }
        let lib = std::fs::read_to_string(root.join("lib.rs")).unwrap();
        let direct_tool = lib
            .split("async fn execute_tool_call")
            .nth(1)
            .expect("dev tool entrypoint")
            .split("async fn inspect_mcp_call")
            .next()
            .unwrap();
        assert!(direct_tool.contains("require_effects_allowed"));
        let bootstrap = std::fs::read_to_string(root.join("bootstrap.rs")).unwrap();
        assert!(!bootstrap.contains("process::exit"));
        assert!(bootstrap.contains("required_store_or_unavailable"));
        assert!(bootstrap.contains("unavailable_sentinel"));
    }
}
