use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

pub const PERSISTENCE_EFFECTS_BLOCKED: &str = "persistence_effects_blocked";
pub const PERSISTENCE_STORE_UNAVAILABLE: &str = "persistence_store_unavailable";

pub const EXPECTED_BOOTSTRAP_STORES: &[&str] = &[
    "ConfigStore",
    "LifeModelFileStore",
    "LifeModelFileJournal",
    "HSAssetAuthorityRegistry",
    "BuilderSessionStore",
    "PluginRegistry",
    "PrivacyPolicyStore",
    "McpAuditKeyReferenceStore",
    "MemoryStore",
    "FeedbackStore",
    "VectorStore",
    "AgentRunStore",
    "EvidenceStore",
    "LifeEventStore",
    "HeuristicStore",
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

#[derive(Default)]
struct PersistenceCoordinatorState {
    expected_stores: BTreeSet<String>,
    stores: BTreeMap<String, PersistenceStoreHealth>,
    global_reason_codes: Vec<String>,
    sealed: bool,
    isolated_evaluation: bool,
}

/// Single process-wide authority for persistence health and effect admission.
/// It owns no product data; canonical stores remain the fact owners.
pub struct PersistenceCoordinator {
    state: RwLock<PersistenceCoordinatorState>,
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
        let registered = state.stores.keys().cloned().collect::<BTreeSet<_>>();
        !state.sealed
            && state.global_reason_codes.is_empty()
            && state.expected_stores.is_subset(&registered)
            && state
                .stores
                .values()
                .all(|health| health.mode == PersistenceStoreMode::ReadWriteCanonical)
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
        .any(|health| health.mode == PersistenceStoreMode::Unavailable)
    {
        PersistenceRuntimeMode::UnavailableDegraded
    } else if !global_reason_codes.is_empty()
        || stores
            .iter()
            .any(|health| health.mode == PersistenceStoreMode::ReadOnlyCanonical)
    {
        PersistenceRuntimeMode::ReadOnlyDegraded
    } else if stores
        .iter()
        .any(|health| health.mode == PersistenceStoreMode::EphemeralDevelopment)
    {
        PersistenceRuntimeMode::EphemeralDevelopment
    } else {
        PersistenceRuntimeMode::ReadWrite
    }
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
            "HSAssetAuthorityRegistry",
            "BuilderSessionStore",
            "PluginRegistry",
        ] {
            assert!(expected.contains(required));
        }
    }

    #[test]
    fn isolated_evaluation_executes_without_claiming_canonical_or_live_credit() {
        let coordinator = PersistenceCoordinator::isolated_evaluation();
        let snapshot = coordinator.snapshot();
        assert_eq!(snapshot.mode, PersistenceRuntimeMode::IsolatedEvaluation);
        assert!(coordinator.require_effects_allowed().is_ok());
        assert!(!snapshot.canonical_writes_allowed);
        assert!(!snapshot.live_or_canonical_credit_eligible);
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
