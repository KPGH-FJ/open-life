use crate::errors::AppError;
use crate::memory_retrieval_filter::{
    filter_canonical_retrievable_memory_results, merge_memory_hits,
};
use crate::persistence_coordinator::{
    CanonicalCommitPermit, CanonicalWriteAdmission, CanonicalWriteOwner, PersistenceGateError,
};
use crate::AppState;
use once_cell::sync::Lazy as LazyLock;
use openlife_core::agent::ExplicitMemoryAdmissionProof;
use openlife_core::agent::{
    AgentProposal, CanonicalMemoryFactDescriptor, ExplicitMemoryWriteInput,
    ExplicitMemoryWriteReceipt, MemoryCandidate, MemoryLifecycleAcceptanceInput,
    MemoryLifecycleAcceptanceReport, MemoryLifecycleCategory, MemoryPrivacyEraseReport,
    MemoryRollbackReport,
};
use openlife_core::embedding::{
    execute_embedding, prepare_embedding_request_recorded, EmbeddingInvocationReceipt,
    EmbeddingOutcome, EmbeddingProfile, EmbeddingRouteConfig, EmbeddingRouteKind,
    PreparedEmbeddingRequestOutcome, UNKNOWN_EMBEDDING_PROFILE_ID,
};
use openlife_core::memory::{
    CanonicalMemoryRetrievalState, MemoryRetrievalDisposition, MemorySearchHit,
};
use openlife_core::persistence_outbox::{
    CanonicalProjectionHeadAdvanced, ProjectionDelivery, ProjectionDeliveryState,
};
use openlife_core::vectors::{
    plan_embedding_privacy, CanonicalVectorOwnerRef, MemoryChunk, TierStats, VectorRebuildEvidence,
    VectorSearchOutcome,
};
#[cfg(test)]
use openlife_core::vectors::{
    ExportedVectorChunk, VectorRebuildBatchItem, VectorRebuildJob, VectorRebuildJobStatus,
    VECTOR_REBUILD_BATCH_LIMIT,
};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashSet;
#[cfg(test)]
use std::collections::VecDeque;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex as StdMutex;

const BACKGROUND_CANONICAL_OUTBOX_BATCH: usize = 32;
const BACKGROUND_CANONICAL_OUTBOX_MAX_PASSES: usize = 4;
const BACKGROUND_CANONICAL_OUTBOX_RETRY_DELAY: std::time::Duration =
    std::time::Duration::from_secs(30);
static MEMORY_RETRIEVAL_PROJECTION_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

#[cfg(test)]
struct CanonicalCommitAdmissionBarrier {
    remaining_skips: usize,
    admitted: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}

#[cfg(test)]
static CANONICAL_COMMIT_ADMISSION_BARRIER: LazyLock<
    StdMutex<HashMap<usize, CanonicalCommitAdmissionBarrier>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));

fn require_persistence_write(state: &Arc<AppState>) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))
}

fn require_persistence_write_string(state: &Arc<AppState>) -> Result<(), String> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())
}

/// Enter the process-wide canonical commit window before taking any Memory or
/// Vector owner lock.
fn admit_memory_vector_writes(
    state: &Arc<AppState>,
    owners: &[CanonicalWriteOwner],
) -> Result<CanonicalWriteAdmission, AppError> {
    state
        .persistence_coordinator
        .admit_canonical_writes(owners)
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))
}

async fn exchange_memory_vector_commit_admission<'state>(
    state: &'state Arc<AppState>,
    admission: &CanonicalWriteAdmission,
) -> Result<CanonicalCommitPermit<'state>, AppError> {
    #[cfg(test)]
    wait_at_canonical_commit_admission_barrier(state).await;
    state
        .persistence_coordinator
        .acquire_canonical_commit_permit(admission)
        .await
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))
}

async fn acquire_memory_vector_commit_permit<'state>(
    state: &'state Arc<AppState>,
    owners: &[CanonicalWriteOwner],
) -> Result<CanonicalCommitPermit<'state>, AppError> {
    let admission = admit_memory_vector_writes(state, owners)?;
    exchange_memory_vector_commit_admission(state, &admission).await
}

#[cfg(test)]
async fn wait_at_canonical_commit_admission_barrier(state: &Arc<AppState>) {
    let coordinator_key = Arc::as_ptr(&state.persistence_coordinator) as usize;
    let barrier = {
        let mut barriers = CANONICAL_COMMIT_ADMISSION_BARRIER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut barrier = barriers.remove(&coordinator_key);
        if barrier
            .as_ref()
            .is_some_and(|barrier| barrier.remaining_skips > 0)
        {
            barrier
                .as_mut()
                .expect("checked test barrier")
                .remaining_skips -= 1;
            barriers.insert(
                coordinator_key,
                barrier.take().expect("checked test barrier"),
            );
            return;
        }
        barrier
    };
    if let Some(barrier) = barrier {
        let _ = barrier.admitted.send(());
        let _ = barrier.release.await;
    }
}

/// Run one synchronous VectorStore transaction under a short-lived shared
/// canonical permit. Rebuild provider and embedding awaits stay outside this helper.
#[cfg(test)]
async fn commit_vector_store_mutation<T>(
    state: &Arc<AppState>,
    mutation: impl FnOnce() -> anyhow::Result<T>,
) -> Result<T, AppError> {
    let _commit_permit =
        acquire_memory_vector_commit_permit(state, &[CanonicalWriteOwner::VectorStore]).await?;
    mutation().map_err(AppError::from)
}

/// One-shot, generation-bound proof minted immediately before a VectorStore
/// search. It carries no lock and is safe to retain while the pure search runs;
/// the later telemetry commit must exchange this exact admission, so a generation change between read and telemetry cannot authorize an old row id.
#[must_use = "mint immediately before the vector read and consume when recording telemetry"]
pub(crate) struct VectorSearchAccessTelemetryTicket {
    admission: Result<CanonicalWriteAdmission, MemoryVectorDegradedEvidence>,
}

/// MemoryStore counterpart to [`VectorSearchAccessTelemetryTicket`]. Distinct
/// opaque types prevent accidentally using a VectorStore admission for a
/// MemoryStore telemetry mutation.
#[must_use = "mint immediately before the memory read and consume when recording telemetry"]
pub(crate) struct MemorySearchAccessTelemetryTicket {
    admission: Result<CanonicalWriteAdmission, MemoryVectorDegradedEvidence>,
}

fn telemetry_admission_error(reason_code: &str, error: &AppError) -> MemoryVectorDegradedEvidence {
    MemoryVectorDegradedEvidence {
        reason_code: reason_code.into(),
        error_digest: Some(openlife_core::persistence_outbox::metadata_digest(
            error.message(),
        )),
    }
}

pub(crate) fn prepare_vector_search_access_telemetry(
    state: &Arc<AppState>,
) -> VectorSearchAccessTelemetryTicket {
    let admission: Result<CanonicalWriteAdmission, AppError> =
        admit_memory_vector_writes(state, &[CanonicalWriteOwner::VectorStore]);
    VectorSearchAccessTelemetryTicket {
        admission: admission
            .map_err(|error| telemetry_admission_error("vector_access_telemetry_skipped", &error)),
    }
}

pub(crate) fn prepare_memory_search_access_telemetry(
    state: &Arc<AppState>,
) -> MemorySearchAccessTelemetryTicket {
    let admission: Result<CanonicalWriteAdmission, AppError> =
        admit_memory_vector_writes(state, &[CanonicalWriteOwner::MemoryStore]);
    MemorySearchAccessTelemetryTicket {
        admission: admission
            .map_err(|error| telemetry_admission_error("memory_access_telemetry_skipped", &error)),
    }
}

/// Best-effort telemetry is deliberately separate from vector retrieval. The
/// admission was minted before the read and is revalidated only at commit; a
/// refused mutation is telemetry evidence and never invalidates valid search
/// results.
pub(crate) async fn record_vector_search_access_telemetry_with_state(
    matches: &[(MemoryChunk, f32)],
    state: &Arc<AppState>,
    ticket: VectorSearchAccessTelemetryTicket,
) -> Option<MemoryVectorDegradedEvidence> {
    let chunk_ids = matches
        .iter()
        .map(|(chunk, _)| chunk.id)
        .collect::<Vec<_>>();
    if chunk_ids.is_empty() {
        return None;
    }
    let admission = match ticket.admission {
        Ok(admission) => admission,
        Err(evidence) => return Some(evidence),
    };
    let store = state.vector_store.lock().await.clone();
    let result = async {
        let _commit_permit = exchange_memory_vector_commit_admission(state, &admission).await?;
        store
            .record_access_telemetry(&chunk_ids)
            .map_err(AppError::from)
    }
    .await;
    result
        .err()
        .map(|error| telemetry_admission_error("vector_access_telemetry_skipped", &error))
}

/// Best-effort MemoryStore access telemetry follows the same boundary as
/// vector telemetry: the search result is already valid, while this optional
/// mutation must obtain a fresh, short-lived MemoryStore commit permit. A
/// recovery fence can therefore skip telemetry without erasing the result.
pub(crate) async fn record_text_search_access_telemetry_with_state(
    hits: &[MemorySearchHit],
    state: &Arc<AppState>,
    ticket: MemorySearchAccessTelemetryTicket,
) -> Option<MemoryVectorDegradedEvidence> {
    let memory_ids = hits
        .iter()
        .map(|hit| hit.chunk.id)
        .filter(|memory_id| *memory_id > 0)
        .collect::<Vec<_>>();
    if memory_ids.is_empty() {
        return None;
    }
    let admission = match ticket.admission {
        Ok(admission) => admission,
        Err(evidence) => return Some(evidence),
    };
    let store = state.memory_store.lock().await.clone();
    let result = async {
        let _commit_permit = exchange_memory_vector_commit_admission(state, &admission).await?;
        store
            .record_text_search_access_telemetry(&memory_ids)
            .map_err(AppError::from)
    }
    .await;
    result
        .err()
        .map(|error| telemetry_admission_error("memory_access_telemetry_skipped", &error))
}

async fn acquire_canonical_projection_commit_permit<'state>(
    state: &'state Arc<AppState>,
    owner: CanonicalWriteOwner,
    lane: CanonicalProjectionCommitLane,
) -> Result<CanonicalCommitPermit<'state>, ProjectionReconciliationError> {
    let admission = match lane {
        CanonicalProjectionCommitLane::Normal => state
            .persistence_coordinator
            .admit_canonical_writes(&[owner]),
        CanonicalProjectionCommitLane::StartupReconciliation => state
            .persistence_coordinator
            .admit_startup_reconciliation_writes(&[owner]),
    }
    .map_err(ProjectionReconciliationError::from_gate)?;
    #[cfg(test)]
    wait_at_canonical_commit_admission_barrier(state).await;
    state
        .persistence_coordinator
        .acquire_canonical_commit_permit(&admission)
        .await
        .map_err(ProjectionReconciliationError::from_gate)
}

fn require_persistence_read(state: &Arc<AppState>, store: &str) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_trusted_read(store)
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))
}

fn notify_canonical_outbox_background_worker(state: &Arc<AppState>) {
    state
        .persistence_coordinator
        .notify_canonical_outbox_worker();
}

/// Start one coalescing background consumer. SQLite outbox rows remain the
/// only queue and authority; foreground commits merely wake this worker, so a
/// provider backlog cannot create one unbounded task per event.
pub(crate) fn start_canonical_outbox_background_worker(state: Arc<AppState>) {
    if !state
        .persistence_coordinator
        .claim_canonical_outbox_worker()
    {
        return;
    }
    notify_canonical_outbox_background_worker(&state);
    std::mem::drop(tauri::async_runtime::spawn(async move {
        let mut delayed_retry = false;
        loop {
            if delayed_retry {
                tokio::select! {
                    _ = state.persistence_coordinator.wait_for_canonical_outbox_work() => {}
                    _ = tokio::time::sleep(BACKGROUND_CANONICAL_OUTBOX_RETRY_DELAY) => {}
                }
            } else {
                state
                    .persistence_coordinator
                    .wait_for_canonical_outbox_work()
                    .await;
            }
            delayed_retry = false;
            for pass in 0..BACKGROUND_CANONICAL_OUTBOX_MAX_PASSES {
                match reconcile_canonical_outboxes_with_state(
                    &state,
                    BACKGROUND_CANONICAL_OUTBOX_BATCH,
                )
                .await
                {
                    Ok(report) if !report.backlog_may_remain => break,
                    Ok(report) if report.degraded > 0 => {
                        delayed_retry = true;
                        break;
                    }
                    Ok(_) if pass + 1 < BACKGROUND_CANONICAL_OUTBOX_MAX_PASSES => {
                        tokio::task::yield_now().await;
                    }
                    Ok(_) | Err(_) => {
                        delayed_retry = true;
                        break;
                    }
                }
            }
        }
    }));
}

fn runtime_store_error(state: &Arc<AppState>, store: &str, error: impl ToString) -> AppError {
    let error = error.to_string();
    state
        .persistence_coordinator
        .register_runtime_durable_failure(store, &error);
    AppError::db_with_hint(error, "persistence_runtime_degraded")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchResult {
    pub hits: Vec<(MemoryChunk, f32)>,
    pub embedding_profile: EmbeddingProfile,
    pub embedding_receipt: EmbeddingInvocationReceipt,
    pub vector_status: String,
    pub route_quality: EmbeddingRouteQuality,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebuild: Option<VectorRebuildEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_evidence: Option<MemoryVectorDegradedEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalMemoryOwnerInput {
    pub owner_kind: String,
    pub owner_id: String,
}

impl CanonicalMemoryOwnerInput {
    pub fn owner(&self) -> Result<CanonicalVectorOwnerRef, AppError> {
        CanonicalVectorOwnerRef::new(&self.owner_kind, &self.owner_id)
            .map_err(|error| AppError::permission(error.to_string()))
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedCanonicalMemoryView {
    pub owner: CanonicalMemoryOwnerInput,
    pub revision: u64,
    pub last_event_id: String,
    pub changed_at: String,
    pub canonical_disposition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRetrievalMutationResult {
    pub owner: CanonicalMemoryOwnerInput,
    pub disposition: String,
    pub changed: bool,
    pub canonical_committed: bool,
    pub revision: Option<u64>,
    pub outbox_event_id: Option<String>,
    pub projection_state: ProjectionDeliveryState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection_error_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCorrectionResult {
    pub memory_id: String,
    pub replaced_memory_id: String,
    pub canonical_committed: bool,
    pub projection_state: ProjectionDeliveryState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection_error_digest: Option<String>,
    pub undo_available: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingRouteQuality {
    SemanticModelVerified,
    DeterministicHashApproximation,
    IdentityUnknown,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryVectorDegradedEvidence {
    pub reason_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_digest: Option<String>,
}

#[cfg(test)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeNoteWriteResult {
    pub operation_id: String,
    pub replayed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_profile: Option<EmbeddingProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_receipt: Option<EmbeddingInvocationReceipt>,
    pub knowledge_note_id: i64,
    pub outbox_event_id: String,
    pub canonical_committed: bool,
    pub projection_state: ProjectionDeliveryState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection_error_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalOutboxReconciliationReport {
    pub examined: usize,
    pub applied: usize,
    pub degraded: usize,
    pub backlog_may_remain: bool,
    /// Tombstone/restore cleanup is privacy and truth critical. Creation/index
    /// projection degradation remains observable but must not make the whole
    /// product unusable when an embedding provider is unavailable.
    pub blocking_degraded: usize,
    pub blocking_backlog_may_remain: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// The suffix distinguishes canonical store ownership from similarly named
// projections and compatibility views.
enum CanonicalOutboxOwner {
    MemoryStore,
    MemoryLifecycleStore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CanonicalOutboxSelection {
    All {
        limit: usize,
    },
    BlockingOnly {
        limit: usize,
    },
    ForegroundExact {
        owner: CanonicalOutboxOwner,
        event_id: String,
    },
    BlockingExact {
        owner: CanonicalOutboxOwner,
        event_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CanonicalProjectionCommitLane {
    Normal,
    StartupReconciliation,
}

#[derive(Debug)]
enum ProjectionReconciliationError {
    /// The target or marker obtained an admission before an exclusive
    /// terminalization fence invalidated that generation. No owner mutation
    /// was attempted, so the durable outbox row must remain replayable rather
    /// than being mislabeled degraded.
    Deferred(PersistenceGateError),
    HeadAdvanced(CanonicalProjectionHeadAdvanced),
    Failed(String),
}

impl ProjectionReconciliationError {
    fn from_gate(error: PersistenceGateError) -> Self {
        match error {
            PersistenceGateError::AdmissionInvalidated { .. } => Self::Deferred(error),
            other => Self::Failed(other.to_string()),
        }
    }

    fn failed(error: impl ToString) -> Self {
        Self::Failed(error.to_string())
    }

    fn from_anyhow(error: anyhow::Error) -> Self {
        error
            .downcast_ref::<CanonicalProjectionHeadAdvanced>()
            .cloned()
            .map(Self::HeadAdvanced)
            .unwrap_or_else(|| Self::Failed(error.to_string()))
    }
}

impl std::fmt::Display for ProjectionReconciliationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deferred(error) => write!(formatter, "{error}"),
            Self::HeadAdvanced(error) => write!(formatter, "{error}"),
            Self::Failed(error) => formatter.write_str(error),
        }
    }
}

impl From<String> for ProjectionReconciliationError {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}

impl From<anyhow::Error> for ProjectionReconciliationError {
    fn from(error: anyhow::Error) -> Self {
        Self::from_anyhow(error)
    }
}

#[derive(Debug, Clone)]
struct OwnedProjectionDelivery {
    owner: CanonicalOutboxOwner,
    delivery: ProjectionDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProjectionApplicationOutcome {
    Applied,
    CompensatedToCanonicalHead {
        head_event_id: String,
        head_revision: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectionCandidateOutcome {
    Applied,
    Degraded,
    Deferred,
}

#[derive(Debug, Clone)]
struct SelectedProjectionDeliveries {
    candidates: Vec<OwnedProjectionDelivery>,
    backlog_may_remain: bool,
    blocking_backlog_may_remain: bool,
}

fn projection_delivery_is_blocking(delivery: &ProjectionDelivery) -> bool {
    delivery.tombstone_id.is_some()
        || delivery.mutation_kind == "restored"
        || delivery.aggregate_kind == "memory_retrieval"
}

fn projection_delivery_is_foreground_local(delivery: &ProjectionDelivery) -> bool {
    delivery.aggregate_kind == "memory_retrieval" && delivery.projection_target == "vector_store"
}

#[derive(Clone)]
struct EmbeddingPrivacyContext {
    provider: String,
    openai_base: String,
    openai_key: String,
    embedding_model: String,
    embedding_enabled: bool,
    credential_version: u64,
    privacy_engine: openlife_core::privacy::PrivacyEngine,
    network_policy: openlife_core::config::NetworkPolicy,
}

async fn embedding_privacy_context(state: &Arc<AppState>) -> EmbeddingPrivacyContext {
    let (
        provider,
        openai_base,
        openai_key,
        embedding_model,
        embedding_enabled,
        credential_version,
        network_policy,
    ) = {
        let cfg = state.config.lock().await;
        (
            cfg.llm.provider.clone(),
            cfg.llm.openai_base.clone(),
            cfg.llm.openai_key.clone(),
            cfg.llm.embedding_model.clone(),
            cfg.llm.embedding_enabled,
            cfg.llm.credential_version,
            cfg.system.network_policy.clone(),
        )
    };
    let privacy_engine = {
        let engine = state.privacy_engine.lock().await;
        engine.clone()
    };

    EmbeddingPrivacyContext {
        provider,
        openai_base,
        openai_key,
        embedding_model,
        embedding_enabled,
        credential_version,
        privacy_engine,
        network_policy,
    }
}

async fn embed_memory_text_with_privacy(
    text: &str,
    state: &Arc<AppState>,
) -> Result<EmbeddingOutcome, AppError> {
    let ctx = embedding_privacy_context(state).await;
    // One canonical Memory index must use one privacy-safe embedding space.
    // Per-record cloud/local routing creates incompatible profiles that cannot
    // be rebuilt or queried as one truthful index. Ollama remains a local
    // semantic route; cloud-configured products use the stable built-in local
    // profile for canonical Memory and its queries.
    let privacy_plan = plan_embedding_privacy(text, &ctx.privacy_engine, true);
    let prepared = prepare_embedding_request_recorded(
        text,
        EmbeddingRouteConfig::from_product_config(
            ctx.provider,
            ctx.openai_base,
            ctx.embedding_model,
            ctx.embedding_enabled,
            &ctx.openai_key,
            ctx.credential_version,
            ctx.network_policy,
        ),
        privacy_plan,
    );
    Ok(match prepared {
        PreparedEmbeddingRequestOutcome::Prepared(prepared) => execute_embedding(prepared).await,
        PreparedEmbeddingRequestOutcome::Rejected(outcome) => outcome,
    })
}

fn embedding_route_quality(
    profile: &EmbeddingProfile,
    embedding_succeeded: bool,
) -> EmbeddingRouteQuality {
    if !embedding_succeeded {
        return EmbeddingRouteQuality::Unavailable;
    }
    match profile.route {
        EmbeddingRouteKind::Cloud | EmbeddingRouteKind::Ollama
            if profile.id != UNKNOWN_EMBEDDING_PROFILE_ID =>
        {
            EmbeddingRouteQuality::SemanticModelVerified
        }
        EmbeddingRouteKind::DeterministicHash if profile.id != UNKNOWN_EMBEDDING_PROFILE_ID => {
            EmbeddingRouteQuality::DeterministicHashApproximation
        }
        _ => EmbeddingRouteQuality::IdentityUnknown,
    }
}

fn embedding_receipt_evidence(
    operation: &str,
    profile: &EmbeddingProfile,
    receipt: &EmbeddingInvocationReceipt,
) -> String {
    serde_json::json!({
        "operation": operation,
        "profileId": profile.id,
        "profileRoute": profile.route,
        "profileDimension": profile.dimension,
        "receiptStatus": receipt.status,
        "receiptSource": receipt.source,
        "routeReasonCode": receipt.route_reason_code,
        "cacheHit": receipt.cache_hit,
        "providerDispatches": receipt.provider_dispatches,
        "errorDigest": receipt.error_digest,
    })
    .to_string()
}

fn require_embedding_success(
    operation: &str,
    outcome: EmbeddingOutcome,
) -> Result<(Vec<f32>, EmbeddingProfile, EmbeddingInvocationReceipt), AppError> {
    let EmbeddingOutcome {
        profile,
        receipt,
        result,
    } = outcome;
    match result {
        Ok(embedding) if profile.id != UNKNOWN_EMBEDDING_PROFILE_ID => {
            Ok((embedding, profile, receipt))
        }
        Ok(_) => Err(AppError::external(
            serde_json::json!({
                "operation": operation,
                "status": "embedding_profile_identity_unknown",
                "receipt": embedding_receipt_evidence(operation, &profile, &receipt),
            })
            .to_string(),
        )),
        Err(_) => Err(AppError::external(embedding_receipt_evidence(
            operation, &profile, &receipt,
        ))),
    }
}

pub(crate) async fn reconcile_canonical_outboxes_with_state(
    state: &Arc<AppState>,
    limit: usize,
) -> Result<CanonicalOutboxReconciliationReport, String> {
    reconcile_selected_canonical_outboxes_with_state(state, CanonicalOutboxSelection::All { limit })
        .await
}

pub(crate) async fn reconcile_blocking_canonical_outboxes_with_state(
    state: &Arc<AppState>,
    limit: usize,
) -> Result<CanonicalOutboxReconciliationReport, String> {
    reconcile_selected_canonical_outboxes_with_state(
        state,
        CanonicalOutboxSelection::BlockingOnly { limit },
    )
    .await
}

async fn reconcile_foreground_canonical_outbox_event_with_state(
    state: &Arc<AppState>,
    owner: CanonicalOutboxOwner,
    event_id: &str,
) -> Result<CanonicalOutboxReconciliationReport, String> {
    reconcile_selected_canonical_outboxes_with_state(
        state,
        CanonicalOutboxSelection::ForegroundExact {
            owner,
            event_id: event_id.to_string(),
        },
    )
    .await
}

async fn reconcile_blocking_canonical_outbox_event_with_state(
    state: &Arc<AppState>,
    owner: CanonicalOutboxOwner,
    event_id: &str,
) -> Result<CanonicalOutboxReconciliationReport, String> {
    reconcile_selected_canonical_outboxes_with_state(
        state,
        CanonicalOutboxSelection::BlockingExact {
            owner,
            event_id: event_id.to_string(),
        },
    )
    .await
}

async fn reconcile_selected_canonical_outboxes_with_state(
    state: &Arc<AppState>,
    selection: CanonicalOutboxSelection,
) -> Result<CanonicalOutboxReconciliationReport, String> {
    // Startup reconciliation runs while the coordinator is deliberately still
    // sealed in `Initializing`. Permit only the bounded blocking projection
    // drain in that narrow window; foreground writes and the background `All`
    // consumer must still pass the normal effects-admission gate.
    let startup_blocking_reconciliation =
        matches!(&selection, CanonicalOutboxSelection::BlockingOnly { .. })
            && state
                .persistence_coordinator
                .startup_reconciliation_mutations_safe();
    if !startup_blocking_reconciliation {
        require_persistence_write_string(state)?;
    }
    let commit_lane = if startup_blocking_reconciliation {
        CanonicalProjectionCommitLane::StartupReconciliation
    } else {
        CanonicalProjectionCommitLane::Normal
    };
    let result = reconcile_canonical_outboxes_inner(state, selection, commit_lane).await;
    if let Err(error) = &result {
        state
            .persistence_coordinator
            .degrade_globally("runtime_canonical_outbox_reconciliation_failure");
        log::error!("canonical outbox reconciliation degraded: {error}");
    }
    result
}

async fn reconcile_canonical_outboxes_inner(
    state: &Arc<AppState>,
    selection: CanonicalOutboxSelection,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<CanonicalOutboxReconciliationReport, String> {
    let selected = select_projection_deliveries(state, selection).await?;
    let examined = selected.candidates.len();
    let mut report = CanonicalOutboxReconciliationReport {
        examined,
        applied: 0,
        degraded: 0,
        backlog_may_remain: selected.backlog_may_remain,
        blocking_degraded: 0,
        blocking_backlog_may_remain: selected.blocking_backlog_may_remain,
    };
    for candidate in selected.candidates {
        let blocking = projection_delivery_is_blocking(&candidate.delivery);
        match reconcile_projection_candidate(state, &candidate, commit_lane).await? {
            ProjectionCandidateOutcome::Degraded => {
                report.degraded += 1;
                report.blocking_degraded += usize::from(blocking);
            }
            ProjectionCandidateOutcome::Applied => {
                report.applied += 1;
            }
            ProjectionCandidateOutcome::Deferred => {
                // An exclusive terminalization fence invalidated a previously
                // admitted target or marker. The durable delivery is still
                // pending/replayable; do not turn contention into degradation.
                report.backlog_may_remain = true;
                report.blocking_backlog_may_remain |= blocking;
                notify_canonical_outbox_background_worker(state);
                break;
            }
        }
    }
    report.backlog_may_remain |= report.degraded > 0;
    report.blocking_backlog_may_remain |= report.blocking_degraded > 0;
    Ok(report)
}

async fn reconcile_projection_candidate(
    state: &Arc<AppState>,
    candidate: &OwnedProjectionDelivery,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<ProjectionCandidateOutcome, String> {
    if candidate.delivery.aggregate_kind == "memory_retrieval" {
        return reconcile_memory_retrieval_projection_candidate(state, candidate, commit_lane)
            .await;
    }
    match apply_projection_delivery(state, &candidate.delivery, commit_lane).await {
        Ok(ProjectionApplicationOutcome::Applied) => match mark_owned_projection_applied(
            state,
            candidate.owner,
            &candidate.delivery,
            commit_lane,
        )
        .await
        {
            Ok(()) => Ok(ProjectionCandidateOutcome::Applied),
            Err(ProjectionReconciliationError::Deferred(_)) => {
                Ok(ProjectionCandidateOutcome::Deferred)
            }
            Err(error) => Err(error.to_string()),
        },
        Ok(ProjectionApplicationOutcome::CompensatedToCanonicalHead { .. }) => {
            Err("non-retrieval projection attempted causal compensation".into())
        }
        Err(ProjectionReconciliationError::Deferred(_)) => Ok(ProjectionCandidateOutcome::Deferred),
        Err(error) => match mark_owned_projection_degraded(
            state,
            candidate.owner,
            &candidate.delivery,
            &error.to_string(),
            commit_lane,
        )
        .await
        {
            Ok(()) => Ok(ProjectionCandidateOutcome::Degraded),
            Err(ProjectionReconciliationError::Deferred(_)) => {
                Ok(ProjectionCandidateOutcome::Deferred)
            }
            Err(error) => Err(error.to_string()),
        },
    }
}

async fn reconcile_memory_retrieval_projection_candidate(
    state: &Arc<AppState>,
    candidate: &OwnedProjectionDelivery,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<ProjectionCandidateOutcome, String> {
    let _causal_guard = MEMORY_RETRIEVAL_PROJECTION_LOCK.lock().await;
    const MAX_HEAD_ADVANCE_RETRIES: usize = 4;
    for attempt in 0..MAX_HEAD_ADVANCE_RETRIES {
        let application = match apply_memory_retrieval_projection(
            state,
            candidate.owner,
            &candidate.delivery,
            commit_lane,
        )
        .await
        {
            Ok(application) => application,
            Err(ProjectionReconciliationError::Deferred(_)) => {
                return Ok(ProjectionCandidateOutcome::Deferred)
            }
            Err(error) => {
                return match mark_owned_projection_degraded(
                    state,
                    candidate.owner,
                    &candidate.delivery,
                    &error.to_string(),
                    commit_lane,
                )
                .await
                {
                    Ok(()) => Ok(ProjectionCandidateOutcome::Degraded),
                    Err(ProjectionReconciliationError::Deferred(_)) => {
                        Ok(ProjectionCandidateOutcome::Deferred)
                    }
                    Err(error) => Err(error.to_string()),
                };
            }
        };
        let finalized =
            finalize_memory_retrieval_projection(state, candidate, &application, commit_lane).await;
        match finalized {
            Ok(()) => return Ok(ProjectionCandidateOutcome::Applied),
            Err(ProjectionReconciliationError::HeadAdvanced(_))
                if attempt + 1 < MAX_HEAD_ADVANCE_RETRIES =>
            {
                continue;
            }
            Err(ProjectionReconciliationError::Deferred(_)) => {
                return Ok(ProjectionCandidateOutcome::Deferred)
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("canonical Memory retrieval projection head kept advancing".into())
}

async fn load_memory_retrieval_projection_head(
    state: &Arc<AppState>,
    owner: CanonicalOutboxOwner,
    delivery: &ProjectionDelivery,
) -> Result<CanonicalMemoryRetrievalState, String> {
    match owner {
        CanonicalOutboxOwner::MemoryStore => {
            let store = state.memory_store.lock().await;
            match store.load_memory_retrieval_state_for_projection(&delivery.event_id) {
                Ok(head) => Ok(head),
                Err(strict_error) => {
                    let head = store
                        .load_memory_retrieval_head_for_event(&delivery.event_id)
                        .map_err(|error| error.to_string())?;
                    if head.last_event_id == delivery.event_id {
                        Err(strict_error.to_string())
                    } else {
                        Ok(head)
                    }
                }
            }
        }
        CanonicalOutboxOwner::MemoryLifecycleStore => {
            let store = state
                .memory_lifecycle_store
                .as_ref()
                .ok_or_else(memory_lifecycle_store_missing)?
                .lock()
                .await;
            match store.load_memory_retrieval_state_for_projection(&delivery.event_id) {
                Ok(head) => Ok(head),
                Err(strict_error) => {
                    let head = store
                        .load_memory_retrieval_head_for_event(&delivery.event_id)
                        .map_err(|error| error.to_string())?;
                    if head.last_event_id == delivery.event_id {
                        Err(strict_error.to_string())
                    } else {
                        Ok(head)
                    }
                }
            }
        }
    }
}

async fn finalize_memory_retrieval_projection(
    state: &Arc<AppState>,
    candidate: &OwnedProjectionDelivery,
    application: &ProjectionApplicationOutcome,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<(), ProjectionReconciliationError> {
    let result = match candidate.owner {
        CanonicalOutboxOwner::MemoryStore => {
            let _commit_permit = acquire_canonical_projection_commit_permit(
                state,
                CanonicalWriteOwner::MemoryStore,
                commit_lane,
            )
            .await?;
            let store = state.memory_store.lock().await;
            match application {
                ProjectionApplicationOutcome::Applied => store
                    .mark_memory_retrieval_projection_applied_if_head(
                        &candidate.delivery.event_id,
                        candidate.delivery.aggregate_revision,
                        &candidate.delivery.projection_target,
                    ),
                ProjectionApplicationOutcome::CompensatedToCanonicalHead {
                    head_event_id,
                    head_revision,
                } => store.mark_memory_retrieval_projection_compensated_to_head(
                    &candidate.delivery.event_id,
                    head_event_id,
                    *head_revision,
                    &candidate.delivery.projection_target,
                ),
            }
        }
        CanonicalOutboxOwner::MemoryLifecycleStore => {
            let store = state
                .memory_lifecycle_store
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!(memory_lifecycle_store_missing()))?
                .lock()
                .await;
            match application {
                ProjectionApplicationOutcome::Applied => store
                    .mark_memory_retrieval_projection_applied_if_head(
                        &candidate.delivery.event_id,
                        candidate.delivery.aggregate_revision,
                        &candidate.delivery.projection_target,
                    ),
                ProjectionApplicationOutcome::CompensatedToCanonicalHead {
                    head_event_id,
                    head_revision,
                } => store.mark_memory_retrieval_projection_compensated_to_head(
                    &candidate.delivery.event_id,
                    head_event_id,
                    *head_revision,
                    &candidate.delivery.projection_target,
                ),
            }
        }
    };
    result.map_err(ProjectionReconciliationError::from_anyhow)
}

async fn select_projection_deliveries(
    state: &Arc<AppState>,
    selection: CanonicalOutboxSelection,
) -> Result<SelectedProjectionDeliveries, String> {
    let blocking_only = matches!(&selection, CanonicalOutboxSelection::BlockingOnly { .. });
    let foreground = matches!(&selection, CanonicalOutboxSelection::ForegroundExact { .. });
    match selection {
        CanonicalOutboxSelection::All { limit }
        | CanonicalOutboxSelection::BlockingOnly { limit } => {
            let bounded_limit = limit.clamp(1, 500);
            let mut candidates = Vec::new();
            let mut any_owner_saturated = false;
            let mut blocking_owner_saturated = false;
            for owner in [
                CanonicalOutboxOwner::MemoryStore,
                CanonicalOutboxOwner::MemoryLifecycleStore,
            ] {
                let mut deliveries =
                    load_owner_replayable_deliveries(state, owner, bounded_limit).await?;
                any_owner_saturated |= deliveries.len() >= bounded_limit;
                blocking_owner_saturated |= deliveries.len() >= bounded_limit
                    && deliveries
                        .last()
                        .is_some_and(projection_delivery_is_blocking);
                if blocking_only {
                    deliveries.retain(projection_delivery_is_blocking);
                }
                candidates.extend(
                    deliveries
                        .into_iter()
                        .map(|delivery| OwnedProjectionDelivery { owner, delivery }),
                );
            }
            sort_projection_deliveries(&mut candidates);
            let candidate_count = candidates.len();
            let blocking_candidate_count = candidates
                .iter()
                .filter(|candidate| projection_delivery_is_blocking(&candidate.delivery))
                .count();
            candidates.truncate(bounded_limit);
            let blocking_examined = candidates
                .iter()
                .filter(|candidate| projection_delivery_is_blocking(&candidate.delivery))
                .count();
            let blocking_backlog_may_remain = blocking_owner_saturated
                || blocking_candidate_count > blocking_examined
                || blocking_candidate_count >= bounded_limit;
            Ok(SelectedProjectionDeliveries {
                candidates,
                backlog_may_remain: if blocking_only {
                    blocking_backlog_may_remain
                } else {
                    any_owner_saturated || candidate_count >= bounded_limit
                },
                blocking_backlog_may_remain,
            })
        }
        CanonicalOutboxSelection::ForegroundExact { owner, event_id }
        | CanonicalOutboxSelection::BlockingExact { owner, event_id } => {
            let deliveries = load_owner_event_deliveries(state, owner, &event_id).await?;
            let mut candidates = deliveries
                .iter()
                .filter(|delivery| {
                    projection_delivery_is_blocking(delivery)
                        || (foreground && projection_delivery_is_foreground_local(delivery))
                })
                .cloned()
                .map(|delivery| OwnedProjectionDelivery { owner, delivery })
                .collect::<Vec<_>>();
            sort_projection_deliveries(&mut candidates);
            let skipped = deliveries.len().saturating_sub(candidates.len());
            let skipped_blocking = deliveries.iter().any(|delivery| {
                projection_delivery_is_blocking(delivery)
                    && !candidates.iter().any(|candidate| {
                        candidate.delivery.event_id == delivery.event_id
                            && candidate.delivery.projection_target == delivery.projection_target
                    })
            });
            Ok(SelectedProjectionDeliveries {
                candidates,
                backlog_may_remain: skipped > 0,
                blocking_backlog_may_remain: skipped_blocking,
            })
        }
    }
}

async fn load_owner_replayable_deliveries(
    state: &Arc<AppState>,
    owner: CanonicalOutboxOwner,
    limit: usize,
) -> Result<Vec<ProjectionDelivery>, String> {
    match owner {
        CanonicalOutboxOwner::MemoryStore => state
            .memory_store
            .lock()
            .await
            .list_replayable_projection_deliveries(limit)
            .map_err(|error| runtime_store_error(state, "MemoryStore", error).to_string()),
        CanonicalOutboxOwner::MemoryLifecycleStore => match state.memory_lifecycle_store.as_ref() {
            Some(store) => store
                .lock()
                .await
                .list_replayable_projection_deliveries(limit)
                .map_err(|error| error.to_string()),
            None => Ok(Vec::new()),
        },
    }
}

async fn load_owner_event_deliveries(
    state: &Arc<AppState>,
    owner: CanonicalOutboxOwner,
    event_id: &str,
) -> Result<Vec<ProjectionDelivery>, String> {
    match owner {
        CanonicalOutboxOwner::MemoryStore => state
            .memory_store
            .lock()
            .await
            .list_replayable_projection_deliveries_for_event(event_id)
            .map_err(|error| runtime_store_error(state, "MemoryStore", error).to_string()),
        CanonicalOutboxOwner::MemoryLifecycleStore => state
            .memory_lifecycle_store
            .as_ref()
            .ok_or_else(memory_lifecycle_store_missing)?
            .lock()
            .await
            .list_replayable_projection_deliveries_for_event(event_id)
            .map_err(|error| error.to_string()),
    }
}

fn sort_projection_deliveries(candidates: &mut [OwnedProjectionDelivery]) {
    candidates.sort_by(|left, right| {
        (!projection_delivery_is_blocking(&left.delivery))
            .cmp(&(!projection_delivery_is_blocking(&right.delivery)))
            .then_with(|| left.delivery.updated_at.cmp(&right.delivery.updated_at))
            .then_with(|| left.delivery.event_id.cmp(&right.delivery.event_id))
            .then_with(|| {
                left.delivery
                    .projection_target
                    .cmp(&right.delivery.projection_target)
            })
    });
}

async fn mark_owned_projection_applied(
    state: &Arc<AppState>,
    owner: CanonicalOutboxOwner,
    delivery: &ProjectionDelivery,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<(), ProjectionReconciliationError> {
    match owner {
        CanonicalOutboxOwner::MemoryStore => {
            let _commit_permit = acquire_canonical_projection_commit_permit(
                state,
                CanonicalWriteOwner::MemoryStore,
                commit_lane,
            )
            .await?;
            state
                .memory_store
                .lock()
                .await
                .mark_projection_applied(&delivery.event_id, &delivery.projection_target)
                .map_err(ProjectionReconciliationError::failed)
        }
        CanonicalOutboxOwner::MemoryLifecycleStore => state
            .memory_lifecycle_store
            .as_ref()
            .ok_or_else(memory_lifecycle_store_missing)?
            .lock()
            .await
            .mark_projection_applied(&delivery.event_id, &delivery.projection_target)
            .map_err(ProjectionReconciliationError::failed),
    }
}

async fn mark_owned_projection_degraded(
    state: &Arc<AppState>,
    owner: CanonicalOutboxOwner,
    delivery: &ProjectionDelivery,
    error: &str,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<(), ProjectionReconciliationError> {
    match owner {
        CanonicalOutboxOwner::MemoryStore => {
            let _commit_permit = acquire_canonical_projection_commit_permit(
                state,
                CanonicalWriteOwner::MemoryStore,
                commit_lane,
            )
            .await?;
            state
                .memory_store
                .lock()
                .await
                .mark_projection_degraded(&delivery.event_id, &delivery.projection_target, error)
                .map_err(ProjectionReconciliationError::failed)
        }
        CanonicalOutboxOwner::MemoryLifecycleStore => state
            .memory_lifecycle_store
            .as_ref()
            .ok_or_else(memory_lifecycle_store_missing)?
            .lock()
            .await
            .mark_projection_degraded(&delivery.event_id, &delivery.projection_target, error)
            .map_err(ProjectionReconciliationError::failed),
    }
}

async fn apply_projection_delivery(
    state: &Arc<AppState>,
    delivery: &ProjectionDelivery,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<ProjectionApplicationOutcome, ProjectionReconciliationError> {
    match delivery.aggregate_kind.as_str() {
        "knowledge_note" if delivery.mutation_kind == "created" => {
            apply_knowledge_note_projection(state, delivery, commit_lane)
                .await
                .map(|_| ProjectionApplicationOutcome::Applied)
        }
        "memory_lifecycle" => apply_memory_lifecycle_projection(state, delivery, commit_lane)
            .await
            .map(|_| ProjectionApplicationOutcome::Applied),
        "memory_retrieval" => Err(ProjectionReconciliationError::failed(
            "canonical Memory retrieval projection requires its exact outbox owner",
        )),
        other => Err(ProjectionReconciliationError::failed(format!(
            "unsupported canonical outbox aggregate: {other}:{}",
            delivery.mutation_kind
        ))),
    }
}

async fn apply_memory_retrieval_projection(
    state: &Arc<AppState>,
    owner: CanonicalOutboxOwner,
    delivery: &ProjectionDelivery,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<ProjectionApplicationOutcome, ProjectionReconciliationError> {
    if delivery.projection_target != "vector_store" {
        return Err(ProjectionReconciliationError::failed(format!(
            "unsupported canonical Memory retrieval projection target: {}",
            delivery.projection_target
        )));
    }
    let canonical_head = load_memory_retrieval_projection_head(state, owner, delivery)
        .await
        .map_err(ProjectionReconciliationError::failed)?;
    let owner = canonical_head
        .owner()
        .map_err(ProjectionReconciliationError::failed)?;
    let _commit_permit = acquire_canonical_projection_commit_permit(
        state,
        CanonicalWriteOwner::VectorStore,
        commit_lane,
    )
    .await?;
    let vector_store = state.vector_store.lock().await.clone();
    vector_store
        .project_memory_retrieval_state(
            &canonical_head.last_event_id,
            &owner,
            canonical_head.disposition != MemoryRetrievalDisposition::Active,
            canonical_head.revision,
        )
        .map_err(ProjectionReconciliationError::failed)?;
    if canonical_head.last_event_id == delivery.event_id {
        Ok(ProjectionApplicationOutcome::Applied)
    } else {
        Ok(ProjectionApplicationOutcome::CompensatedToCanonicalHead {
            head_event_id: canonical_head.last_event_id,
            head_revision: canonical_head.revision,
        })
    }
}

async fn apply_knowledge_note_projection(
    state: &Arc<AppState>,
    delivery: &ProjectionDelivery,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<(), ProjectionReconciliationError> {
    if delivery.projection_target != "vector_store" {
        return Err(ProjectionReconciliationError::failed(format!(
            "unsupported KnowledgeNote projection target: {}",
            delivery.projection_target
        )));
    }
    if delivery.aggregate_kind != "knowledge_note" || delivery.mutation_kind != "created" {
        return Err(ProjectionReconciliationError::failed(
            "unsupported canonical KnowledgeNote projection contract",
        ));
    }
    let owner = CanonicalVectorOwnerRef::new("knowledge_note", &delivery.aggregate_id)
        .map_err(ProjectionReconciliationError::failed)?;
    let record = state
        .memory_store
        .lock()
        .await
        .load_verified_knowledge_note_projection(&delivery.event_id, &owner)
        .map_err(ProjectionReconciliationError::failed)?;
    if state
        .vector_store
        .lock()
        .await
        .clone()
        .projected_materialization_vector_id(&delivery.event_id, &owner)
        .map_err(ProjectionReconciliationError::failed)?
        .is_some()
    {
        return Ok(());
    }
    let (embedding, profile, _) = require_embedding_success(
        "knowledge_note_outbox_projection",
        embed_memory_text_with_privacy(&record.content, state)
            .await
            .map_err(ProjectionReconciliationError::failed)?,
    )
    .map_err(ProjectionReconciliationError::failed)?;
    let _commit_permit = acquire_canonical_projection_commit_permit(
        state,
        CanonicalWriteOwner::VectorStore,
        commit_lane,
    )
    .await?;
    state
        .vector_store
        .lock()
        .await
        .clone()
        .project_memory_embedding(
            &delivery.event_id,
            &owner,
            &record.session_id,
            &record.content,
            &embedding,
            &profile,
        )
        .and_then(|vector_id| {
            vector_id
                .map(|_| ())
                .ok_or_else(|| anyhow::anyhow!("active KnowledgeNote projection was suppressed"))
        })
        .map_err(ProjectionReconciliationError::failed)
}

async fn apply_memory_lifecycle_projection(
    state: &Arc<AppState>,
    delivery: &ProjectionDelivery,
    commit_lane: CanonicalProjectionCommitLane,
) -> Result<(), ProjectionReconciliationError> {
    let record = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)
        .map_err(ProjectionReconciliationError::failed)?
        .lock()
        .await
        .get_record(&delivery.aggregate_id)
        .map_err(ProjectionReconciliationError::failed)?
        .ok_or_else(|| {
            ProjectionReconciliationError::failed("canonical MemoryLifecycle record missing")
        })?;
    if delivery.mutation_kind == "deleted" {
        return match delivery.projection_target.as_str() {
            "vector_store" => {
                let _commit_permit = acquire_canonical_projection_commit_permit(
                    state,
                    CanonicalWriteOwner::VectorStore,
                    commit_lane,
                )
                .await?;
                state
                    .vector_store
                    .lock()
                    .await
                    .clone()
                    .project_memory_lifecycle_tombstone(&delivery.event_id, &delivery.aggregate_id)
                    .map(|_| ())
                    .map_err(ProjectionReconciliationError::failed)
            }
            other => Err(ProjectionReconciliationError::failed(format!(
                "unsupported MemoryLifecycle tombstone projection target: {other}"
            ))),
        };
    }
    if delivery.mutation_kind != "materialized" {
        return Err(ProjectionReconciliationError::failed(format!(
            "unsupported MemoryLifecycle mutation: {}",
            delivery.mutation_kind
        )));
    }
    if !record.status.is_runtime_active() || record.runtime_context_excluded_at.is_some() {
        // A rollback may have committed while an older creation delivery was
        // degraded. Never let that late delivery resurrect stale content.
        return Ok(());
    }
    let vector_partition = record
        .scope_owner_ref
        .as_deref()
        .unwrap_or("agent-memory-global");
    match delivery.projection_target.as_str() {
        "vector_store" => {
            let vector_store = state.vector_store.lock().await.clone();
            let owner = CanonicalVectorOwnerRef::new("memory_lifecycle", &record.memory_id)
                .map_err(ProjectionReconciliationError::failed)?;
            if vector_store
                .projected_materialization_vector_id(&delivery.event_id, &owner)
                .map_err(ProjectionReconciliationError::failed)?
                .is_some()
            {
                return Ok(());
            }
            let (embedding, profile, _) = require_embedding_success(
                "memory_lifecycle_outbox_projection",
                embed_memory_text_with_privacy(&record.content, state)
                    .await
                    .map_err(ProjectionReconciliationError::failed)?,
            )
            .map_err(ProjectionReconciliationError::failed)?;
            let _commit_permit = acquire_canonical_projection_commit_permit(
                state,
                CanonicalWriteOwner::VectorStore,
                commit_lane,
            )
            .await?;
            vector_store
                .project_memory_embedding(
                    &delivery.event_id,
                    &owner,
                    vector_partition,
                    &record.content,
                    &embedding,
                    &profile,
                )
                .map(|_| ())
                .map_err(ProjectionReconciliationError::failed)
        }
        other => Err(ProjectionReconciliationError::failed(format!(
            "unsupported MemoryLifecycle projection target: {other}"
        ))),
    }
}

#[cfg(test)]
pub(crate) async fn create_knowledge_note_with_state(
    operation_id: String,
    session_id: String,
    content: String,
    source: String,
    state: &Arc<AppState>,
) -> Result<KnowledgeNoteWriteResult, AppError> {
    let canonical_write = {
        let _commit_permit =
            acquire_memory_vector_commit_permit(state, &[CanonicalWriteOwner::MemoryStore]).await?;
        let store = state.memory_store.lock().await;
        let tags = vec![
            "canonical_owner:knowledge_note".to_string(),
            format!("source:{}", source),
        ];
        store
            .save_knowledge_note_idempotent_with_outbox(
                &operation_id,
                &session_id,
                &content,
                "knowledge_note",
                &source,
                &tags,
                "private",
            )
            .map_err(|error| runtime_store_error(state, "MemoryStore", error))?
    };
    // Vector materialization is optional and may await a provider. Foreground
    // indexing commits only the canonical note plus outbox, then wakes the one
    // background consumer. Pending is a truthful product state, not failure.
    notify_canonical_outbox_background_worker(state);
    let reconciliation_error = reconcile_foreground_canonical_outbox_event_with_state(
        state,
        CanonicalOutboxOwner::MemoryStore,
        &canonical_write.canonical_mutation.event_id,
    )
    .await
    .err();
    let (projection_state, summary_error_digest) = match state
        .memory_store
        .lock()
        .await
        .projection_summary(&canonical_write.canonical_mutation.event_id)
    {
        Ok(summary) => (summary.state(), None),
        Err(error) => (
            ProjectionDeliveryState::Degraded,
            Some(openlife_core::persistence_outbox::metadata_digest(
                &error.to_string(),
            )),
        ),
    };
    let projection_error_digest = summary_error_digest.or_else(|| {
        reconciliation_error
            .as_deref()
            .map(openlife_core::persistence_outbox::metadata_digest)
    });
    let embedding_id = None;
    let embedding_profile = None;
    let embedding_receipt = None;
    Ok(KnowledgeNoteWriteResult {
        operation_id: canonical_write.operation_id,
        replayed: canonical_write.replayed,
        embedding_id,
        embedding_profile,
        embedding_receipt,
        knowledge_note_id: canonical_write.knowledge_note_id,
        outbox_event_id: canonical_write.canonical_mutation.event_id,
        canonical_committed: true,
        projection_state,
        projection_error_digest,
    })
}

#[cfg(test)]
pub(crate) async fn search_memory_with_state(
    query: String,
    top_k: usize,
    state: &Arc<AppState>,
) -> Result<MemorySearchResult, AppError> {
    search_memory_with_scope_filter(query, top_k, state, None).await
}

/// Runtime-context search restricted to canonical lifecycle owners that were
/// already admitted by the current conversation/workspace/project scope.
/// Filtering happens before access telemetry, so an out-of-scope candidate is
/// neither injected nor falsely recorded as used.
pub(crate) async fn search_lifecycle_memory_with_state(
    query: String,
    top_k: usize,
    allowed_memory_ids: &HashSet<String>,
    state: &Arc<AppState>,
) -> Result<MemorySearchResult, AppError> {
    search_memory_with_scope_filter(query, top_k, state, Some(allowed_memory_ids)).await
}

fn lifecycle_lexical_relevance(query: &str, content: &str) -> Option<f32> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return None;
    }
    let content = content.to_lowercase();
    if content.contains(&query) {
        return Some(0.95);
    }
    let mut terms = query
        .split(|ch: char| !ch.is_alphanumeric() && !(('\u{4e00}'..='\u{9fff}').contains(&ch)))
        .filter(|term| term.chars().count() >= 2)
        .map(str::to_string)
        .take(24)
        .collect::<Vec<_>>();
    let cjk = query
        .chars()
        .filter(|ch| matches!(ch, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'))
        .collect::<Vec<_>>();
    terms.extend(
        cjk.windows(2)
            .map(|window| window.iter().collect::<String>()),
    );
    terms.sort();
    terms.dedup();
    if terms.is_empty() {
        return None;
    }
    let matched = terms
        .iter()
        .filter(|term| content.contains(term.as_str()))
        .count();
    if matched == 0 {
        None
    } else {
        Some(0.35 + 0.5 * (matched as f32 / terms.len() as f32))
    }
}

async fn search_canonical_lifecycle_text(
    query: &str,
    top_k: usize,
    allowed_memory_ids: &HashSet<String>,
    state: &Arc<AppState>,
) -> Result<Vec<MemorySearchHit>, AppError> {
    let store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(|| {
            AppError::db_with_hint(
                "MemoryLifecycleStore unavailable",
                "memory_retrieval_degraded",
            )
        })?
        .lock()
        .await;
    let mut hits = allowed_memory_ids
        .iter()
        .filter_map(|memory_id| store.get_record(memory_id).ok().flatten())
        .filter_map(|record| {
            let relevance_score = lifecycle_lexical_relevance(query, &record.content)?;
            Some(MemorySearchHit {
                chunk: MemoryChunk {
                    id: 0,
                    session_id: record
                        .scope_owner_ref
                        .clone()
                        .unwrap_or_else(|| "global".into()),
                    content: record.content,
                    source: format!("memory_lifecycle:{}", record.memory_id),
                    created_at: record
                        .accepted_at
                        .map(|accepted_at| accepted_at.to_rfc3339())
                        .unwrap_or_default(),
                    tier: 0,
                    access_count: 0,
                    last_accessed_at: String::new(),
                    importance_score: record.confidence,
                    archived: false,
                    archived_at: None,
                    summary: None,
                },
                relevance_score,
                source_tier: "canonical_lifecycle_lexical".into(),
            })
        })
        .collect::<Vec<_>>();
    hits.sort_by(|left, right| {
        right
            .relevance_score
            .partial_cmp(&left.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.chunk.created_at.cmp(&left.chunk.created_at))
    });
    hits.truncate(top_k);
    Ok(hits)
}

async fn search_memory_with_scope_filter(
    query: String,
    top_k: usize,
    state: &Arc<AppState>,
    allowed_lifecycle_memory_ids: Option<&HashSet<String>>,
) -> Result<MemorySearchResult, AppError> {
    require_persistence_read(state, "VectorStore")?;
    require_persistence_read(state, "MemoryLifecycleStore")?;
    let desensitized_query = {
        let privacy_engine = state.privacy_engine.lock().await;
        privacy_engine.desensitize(&query).0
    };
    let (mut text_hits, text_telemetry_ticket) = if let Some(allowed) = allowed_lifecycle_memory_ids
    {
        (
            search_canonical_lifecycle_text(&desensitized_query, top_k, allowed, state).await?,
            None,
        )
    } else {
        require_persistence_read(state, "MemoryStore")?;
        let ticket = prepare_memory_search_access_telemetry(state);
        let store = state.memory_store.lock().await;
        (
            store
                .search_text_memories(None, &desensitized_query, top_k)
                .map_err(|error| {
                    AppError::db_with_hint(error.to_string(), "memory_retrieval_degraded")
                })?,
            Some(ticket),
        )
    };
    if let Some(allowed) = allowed_lifecycle_memory_ids {
        text_hits.retain(|hit| lifecycle_source_is_allowed(&hit.chunk.source, allowed));
    }
    let EmbeddingOutcome {
        profile,
        receipt,
        result: embedding_result,
    } = embed_memory_text_with_privacy(&query, state).await?;
    let route_quality = embedding_route_quality(&profile, embedding_result.is_ok());
    let (vector_hits, rebuild, vector_status, degraded_evidence) = match embedding_result {
        Err(_) => (
            Vec::new(),
            None,
            "embedding_failed",
            Some(MemoryVectorDegradedEvidence {
                reason_code: "embedding_invocation_failed".into(),
                error_digest: receipt.error_digest.clone(),
            }),
        ),
        Ok(_) if profile.id == UNKNOWN_EMBEDDING_PROFILE_ID => (
            Vec::new(),
            None,
            "rebuild_required",
            Some(MemoryVectorDegradedEvidence {
                reason_code: "embedding_profile_identity_unknown".into(),
                error_digest: None,
            }),
        ),
        Ok(embedding) => {
            let vector_telemetry_ticket = prepare_vector_search_access_telemetry(state);
            let store = state.vector_store.lock().await.clone();
            match store.search(&embedding, &profile, top_k) {
                Ok(VectorSearchOutcome::Matches {
                    mut matches,
                    rebuild,
                }) => {
                    if let Some(allowed) = allowed_lifecycle_memory_ids {
                        matches.retain(|(chunk, _)| {
                            lifecycle_source_is_allowed(&chunk.source, allowed)
                        });
                    }
                    let status = if rebuild.is_some() {
                        "rebuild_required"
                    } else {
                        "ready"
                    };
                    let telemetry_evidence = record_vector_search_access_telemetry_with_state(
                        &matches,
                        state,
                        vector_telemetry_ticket,
                    )
                    .await;
                    (matches, rebuild, status, telemetry_evidence)
                }
                Ok(VectorSearchOutcome::RebuildRequired(rebuild)) => {
                    (Vec::new(), Some(rebuild), "rebuild_required", None)
                }
                Err(_) => (
                    Vec::new(),
                    None,
                    "vector_search_failed",
                    Some(MemoryVectorDegradedEvidence {
                        reason_code: "vector_search_failed".into(),
                        error_digest: None,
                    }),
                ),
            }
        }
    };
    let text_telemetry_evidence = match text_telemetry_ticket {
        Some(ticket) => {
            record_text_search_access_telemetry_with_state(&text_hits, state, ticket).await
        }
        None => None,
    };
    if let Some(evidence) = text_telemetry_evidence.as_ref() {
        log::warn!(
            "Memory text search telemetry skipped after a successful read: reason={} error_digest={}",
            evidence.reason_code,
            evidence.error_digest.as_deref().unwrap_or("none")
        );
    }
    // Vector/embedding degradation is the primary search-quality signal. If
    // it is healthy, expose a skipped Memory telemetry commit without turning
    // the already-observed text result into a failed search.
    let degraded_evidence = degraded_evidence.or(text_telemetry_evidence);
    let hits = filter_canonical_retrievable_memory_results(
        merge_memory_hits(vector_hits, text_hits, top_k),
        state,
    )
    .await
    .map_err(|error| AppError::db_with_hint(error, "memory_retrieval_degraded"))?;
    Ok(MemorySearchResult {
        hits,
        embedding_profile: profile,
        embedding_receipt: receipt,
        vector_status: vector_status.into(),
        route_quality,
        rebuild,
        degraded_evidence,
    })
}

fn lifecycle_source_is_allowed(source: &str, allowed_memory_ids: &HashSet<String>) -> bool {
    source
        .strip_prefix("memory_lifecycle:")
        .is_some_and(|memory_id| allowed_memory_ids.contains(memory_id))
}

#[cfg(any(test, feature = "dev-extensions"))]
pub(crate) async fn run_memory_tier_maintenance_with_state(
    state: &Arc<AppState>,
) -> Result<(usize, usize), AppError> {
    let _commit_permit =
        acquire_memory_vector_commit_permit(state, &[CanonicalWriteOwner::VectorStore]).await?;
    let store = state.vector_store.lock().await;
    store.run_tier_maintenance().map_err(AppError::from)
}

pub(crate) async fn restore_archived_chunks_with_state(
    owner: &CanonicalMemoryOwnerInput,
    state: &Arc<AppState>,
) -> Result<MemoryRetrievalMutationResult, AppError> {
    set_memory_retrieval_disposition_with_state(
        owner,
        MemoryRetrievalDisposition::Active,
        "user_reviewed_restore",
        state,
    )
    .await
}

#[cfg(test)]
pub(crate) async fn list_archived_chunks_with_state(
    limit: usize,
    state: &Arc<AppState>,
) -> Result<Vec<ArchivedCanonicalMemoryView>, AppError> {
    list_archived_chunks_with_state_inner(limit, state, None).await
}

#[cfg(test)]
const ARCHIVED_MEMORY_PAGE_SIZE: usize = 64;

#[cfg(test)]
type ArchivedMemoryPageHook<'a> = dyn Fn(&openlife_core::memory::KnowledgeNoteProjectionStore) -> Result<(), AppError>
    + Sync
    + 'a;

#[cfg(test)]
async fn list_archived_chunks_with_state_inner(
    limit: usize,
    state: &Arc<AppState>,
    after_initial_pages: Option<&ArchivedMemoryPageHook<'_>>,
) -> Result<Vec<ArchivedCanonicalMemoryView>, AppError> {
    require_persistence_read(state, "MemoryStore")?;
    require_persistence_read(state, "MemoryLifecycleStore")?;
    let lifecycle_store = state.memory_lifecycle_store.as_ref().ok_or_else(|| {
        AppError::db_with_hint(
            "MemoryLifecycleStore unavailable while listing archived Memory",
            "memory_retrieval_degraded",
        )
    })?;
    let memory_store = state.memory_store.lock().await.clone();
    let limit = limit.clamp(1, 500);
    let mut memory_window = ARCHIVED_MEMORY_PAGE_SIZE;
    let mut lifecycle_window = ARCHIVED_MEMORY_PAGE_SIZE;
    let mut memory_exhausted = false;
    let mut lifecycle_exhausted = false;
    let mut memory_seen = HashSet::new();
    let mut lifecycle_seen = HashSet::new();
    let mut memory_page = VecDeque::new();
    let mut lifecycle_page = VecDeque::new();
    let mut after_initial_pages = after_initial_pages;
    let mut views = Vec::new();

    while views.len() < limit {
        while memory_page.is_empty() && !memory_exhausted {
            let page = memory_store
                .list_archived_memory_retrieval_states(memory_window)
                .map_err(|error| {
                    AppError::db_with_hint(error.to_string(), "memory_retrieval_degraded")
                })?;
            let page_len = page.len();
            memory_page.extend(
                page.into_iter()
                    .filter(|item| memory_seen.insert(item.last_event_id.clone())),
            );
            if page_len < memory_window {
                memory_exhausted = true;
            } else {
                let next_window = memory_window.saturating_mul(2);
                if next_window == memory_window {
                    memory_exhausted = true;
                } else {
                    memory_window = next_window;
                }
            }
        }
        while lifecycle_page.is_empty() && !lifecycle_exhausted {
            let page = lifecycle_store
                .lock()
                .await
                .list_archived_memory_retrieval_states(lifecycle_window)
                .map_err(|error| {
                    AppError::db_with_hint(error.to_string(), "memory_retrieval_degraded")
                })?;
            let page_len = page.len();
            lifecycle_page.extend(
                page.into_iter()
                    .filter(|item| lifecycle_seen.insert(item.last_event_id.clone())),
            );
            if page_len < lifecycle_window {
                lifecycle_exhausted = true;
            } else {
                let next_window = lifecycle_window.saturating_mul(2);
                if next_window == lifecycle_window {
                    lifecycle_exhausted = true;
                } else {
                    lifecycle_window = next_window;
                }
            }
        }
        if let Some(hook) = after_initial_pages.take() {
            hook(&memory_store)?;
        }

        let retrieval_state = match (memory_page.front(), lifecycle_page.front()) {
            (Some(memory), Some(lifecycle)) if memory.changed_at >= lifecycle.changed_at => {
                memory_page.pop_front()
            }
            (Some(_), Some(_)) => lifecycle_page.pop_front(),
            (Some(_), None) => memory_page.pop_front(),
            (None, Some(_)) => lifecycle_page.pop_front(),
            (None, None) => break,
        }
        .expect("archive merge selected a non-empty page");
        let owner = retrieval_state.owner().map_err(AppError::from)?;
        if canonical_memory_owner_is_current(state, &memory_store, &owner).await?
            && canonical_memory_retrieval_head_matches(
                state,
                &memory_store,
                &owner,
                &retrieval_state,
            )
            .await?
        {
            views.push(archived_memory_view(retrieval_state));
        }
    }
    Ok(views)
}

pub(crate) async fn get_memory_tier_stats_with_state(
    state: &Arc<AppState>,
) -> Result<TierStats, AppError> {
    require_persistence_read(state, "MemoryStore")?;
    require_persistence_read(state, "VectorStore")?;
    let mut stats = state
        .vector_store
        .lock()
        .await
        .tier_stats()
        .map_err(AppError::from)?;
    stats.archived = i64::try_from(current_archived_memory_count_with_state(state).await?)
        .map_err(|_| AppError::internal("canonical archived Memory count exceeds i64"))?;
    Ok(stats)
}

async fn current_archived_memory_count_with_state(
    state: &Arc<AppState>,
) -> Result<usize, AppError> {
    require_persistence_read(state, "MemoryStore")?;
    require_persistence_read(state, "MemoryLifecycleStore")?;
    let lifecycle_store = state.memory_lifecycle_store.as_ref().ok_or_else(|| {
        AppError::db_with_hint(
            "MemoryLifecycleStore unavailable while counting archived Memory",
            "memory_retrieval_degraded",
        )
    })?;
    let memory_count = state
        .memory_store
        .lock()
        .await
        .count_archived_memory_retrieval_states()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "memory_retrieval_degraded"))?;
    let lifecycle_count = lifecycle_store
        .lock()
        .await
        .count_archived_memory_retrieval_states()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "memory_retrieval_degraded"))?;
    memory_count
        .checked_add(lifecycle_count)
        .ok_or_else(|| AppError::internal("canonical archived Memory count exceeds usize"))
}

#[cfg(any(test, feature = "dev-extensions"))]
async fn canonical_memory_retrieval_head_matches(
    state: &Arc<AppState>,
    memory_store: &openlife_core::memory::KnowledgeNoteProjectionStore,
    owner: &CanonicalVectorOwnerRef,
    expected: &CanonicalMemoryRetrievalState,
) -> Result<bool, AppError> {
    let current = if owner.kind() == "memory_lifecycle" {
        state
            .memory_lifecycle_store
            .as_ref()
            .ok_or_else(|| {
                AppError::db_with_hint(
                    "MemoryLifecycleStore unavailable while validating archive head",
                    "memory_retrieval_degraded",
                )
            })?
            .lock()
            .await
            .memory_retrieval_state(owner.id())
            .map_err(|error| {
                AppError::db_with_hint(error.to_string(), "memory_retrieval_degraded")
            })?
    } else {
        memory_store
            .memory_retrieval_state(owner)
            .map_err(|error| {
                AppError::db_with_hint(error.to_string(), "memory_retrieval_degraded")
            })?
    };
    Ok(current.as_ref() == Some(expected))
}

#[cfg(any(test, feature = "dev-extensions"))]
async fn canonical_memory_owner_is_current(
    state: &Arc<AppState>,
    memory_store: &openlife_core::memory::KnowledgeNoteProjectionStore,
    owner: &CanonicalVectorOwnerRef,
) -> Result<bool, AppError> {
    if owner.kind() == "memory_lifecycle" {
        let lifecycle_store = state.memory_lifecycle_store.as_ref().ok_or_else(|| {
            AppError::db_with_hint(
                "MemoryLifecycleStore unavailable while validating canonical Memory owner",
                "canonical_state_unknown",
            )
        })?;
        return lifecycle_store
            .lock()
            .await
            .is_memory_active(owner.id())
            .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"));
    }
    memory_store
        .is_verified_canonical_memory_owner(owner)
        .map_err(AppError::from)
}

#[cfg(test)]
fn archived_memory_view(state: CanonicalMemoryRetrievalState) -> ArchivedCanonicalMemoryView {
    ArchivedCanonicalMemoryView {
        owner: CanonicalMemoryOwnerInput {
            owner_kind: state.owner_kind,
            owner_id: state.owner_id,
        },
        revision: state.revision,
        last_event_id: state.last_event_id,
        changed_at: state.changed_at,
        canonical_disposition: state.disposition.as_str().into(),
    }
}

async fn set_memory_retrieval_disposition_with_state(
    owner_input: &CanonicalMemoryOwnerInput,
    disposition: MemoryRetrievalDisposition,
    reason_code: &str,
    state: &Arc<AppState>,
) -> Result<MemoryRetrievalMutationResult, AppError> {
    set_memory_retrieval_dispositions_with_state(
        std::slice::from_ref(owner_input),
        disposition,
        reason_code,
        state,
    )
    .await?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::internal("canonical Memory retrieval result is missing"))
}

async fn set_memory_retrieval_dispositions_with_state(
    owner_inputs: &[CanonicalMemoryOwnerInput],
    disposition: MemoryRetrievalDisposition,
    reason_code: &str,
    state: &Arc<AppState>,
) -> Result<Vec<MemoryRetrievalMutationResult>, AppError> {
    // MemoryLifecycleStore is not one of the four import-observed owners, but
    // its branch still requires the ordinary global effect gate.
    require_persistence_write(state)?;
    if owner_inputs.is_empty() || owner_inputs.len() > 200 {
        return Err(AppError::permission(
            "canonical Memory retrieval batch must contain 1..=200 owners",
        ));
    }
    let owners = owner_inputs
        .iter()
        .map(CanonicalMemoryOwnerInput::owner)
        .collect::<Result<Vec<_>, _>>()?;
    let memory_store = state.memory_store.lock().await.clone();
    let lifecycle_owned = owners
        .iter()
        .filter(|owner| owner.kind() == "memory_lifecycle")
        .count();
    if lifecycle_owned != 0 && lifecycle_owned != owners.len() {
        return Err(AppError::permission(
            "one canonical Memory retrieval batch cannot span MemoryStore and MemoryLifecycleStore",
        ));
    }
    if lifecycle_owned == 0 && disposition == MemoryRetrievalDisposition::Paused {
        return Err(AppError::permission(
            "paused recall is supported only for MemoryLifecycleStore owners",
        ));
    }
    let (mutations, outbox_owner) = if lifecycle_owned == owners.len() {
        let memory_ids = owners
            .iter()
            .map(|owner| owner.id().to_string())
            .collect::<Vec<_>>();
        let lifecycle_store = state.memory_lifecycle_store.as_ref().ok_or_else(|| {
            AppError::db_with_hint(
                "MemoryLifecycleStore unavailable while mutating retrieval state",
                "canonical_state_unknown",
            )
        })?;
        let mutations = lifecycle_store
            .lock()
            .await
            .set_memory_retrieval_dispositions(&memory_ids, disposition, reason_code)
            .map_err(AppError::from)?;
        (mutations, CanonicalOutboxOwner::MemoryLifecycleStore)
    } else {
        let _commit_permit =
            acquire_memory_vector_commit_permit(state, &[CanonicalWriteOwner::MemoryStore]).await?;
        let mutations = memory_store
            .set_memory_retrieval_dispositions(&owners, disposition, reason_code)
            .map_err(AppError::from)?;
        (mutations, CanonicalOutboxOwner::MemoryStore)
    };
    notify_canonical_outbox_background_worker(state);
    let mut results = Vec::with_capacity(mutations.len());
    for (owner_input, mutation) in owner_inputs.iter().zip(mutations) {
        let mut projection_error_digest = None;
        if let Some(receipt) = mutation.canonical_mutation.as_ref() {
            if let Err(error) = reconcile_blocking_canonical_outbox_event_with_state(
                state,
                outbox_owner,
                &receipt.event_id,
            )
            .await
            {
                projection_error_digest =
                    Some(openlife_core::persistence_outbox::metadata_digest(&error));
            }
        }
        let projection_state = if let Some(receipt) = mutation.canonical_mutation.as_ref() {
            match outbox_owner {
                CanonicalOutboxOwner::MemoryStore => memory_store
                    .projection_summary(&receipt.event_id)
                    .map_err(AppError::from)?
                    .state(),
                CanonicalOutboxOwner::MemoryLifecycleStore => state
                    .memory_lifecycle_store
                    .as_ref()
                    .ok_or_else(|| {
                        AppError::db_with_hint(
                            "MemoryLifecycleStore unavailable while reading projection state",
                            "canonical_state_unknown",
                        )
                    })?
                    .lock()
                    .await
                    .projection_summary(&receipt.event_id)
                    .map_err(AppError::from)?
                    .state(),
            }
        } else {
            ProjectionDeliveryState::Applied
        };
        results.push(MemoryRetrievalMutationResult {
            owner: owner_input.clone(),
            disposition: disposition.as_str().into(),
            changed: mutation.changed,
            canonical_committed: mutation.state.is_some(),
            revision: mutation.state.as_ref().map(|state| state.revision),
            outbox_event_id: mutation
                .canonical_mutation
                .as_ref()
                .map(|receipt| receipt.event_id.clone()),
            projection_state,
            projection_error_digest,
        });
    }
    Ok(results)
}

pub(crate) async fn count_memory_chunks_with_state(state: &Arc<AppState>) -> Result<i64, AppError> {
    require_persistence_read(state, "VectorStore")?;
    let store = state.vector_store.lock().await;
    store.count_all_chunks().map_err(AppError::from)
}

#[cfg(test)]
pub(crate) async fn rebuild_memory_index_with_state(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    require_persistence_write(state)?;
    let vector_store = state.vector_store.lock().await.clone();
    let _execution_guard = vector_store.acquire_rebuild_execution().await;
    let memory_store = state.memory_store.lock().await.clone();
    let source_snapshot = memory_store
        .vector_rebuild_source_snapshot()
        .map_err(AppError::from)?;
    let mut job = commit_vector_store_mutation(state, || {
        vector_store.start_or_resume_rebuild(&source_snapshot)
    })
    .await?;
    if job.status == VectorRebuildJobStatus::CancelRequested {
        job =
            commit_vector_store_mutation(state, || vector_store.settle_rebuild_cancel(&job.job_id))
                .await?;
        return Ok(vector_rebuild_job_report(&job));
    }

    loop {
        if vector_store
            .rebuild_cancel_requested(&job.job_id)
            .map_err(AppError::from)?
        {
            job = commit_vector_store_mutation(state, || {
                vector_store.settle_rebuild_cancel(&job.job_id)
            })
            .await?;
            return Ok(vector_rebuild_job_report(&job));
        }
        let page = memory_store
            .load_vector_rebuild_source_page(
                job.last_processed_memory_id,
                job.source_snapshot.through_memory_id,
                VECTOR_REBUILD_BATCH_LIMIT,
            )
            .map_err(AppError::from)?;
        if page.is_empty() {
            let observed_source = memory_store
                .vector_rebuild_source_snapshot_through(job.source_snapshot.through_memory_id)
                .map_err(AppError::from)?;
            job = match commit_vector_store_mutation(state, || {
                vector_store.finalize_rebuild(&job.job_id, &observed_source)
            })
            .await
            {
                Ok(completed) => completed,
                Err(error) => {
                    let error_digest =
                        openlife_core::persistence_outbox::metadata_digest(&error.to_string());
                    let failed = commit_vector_store_mutation(state, || {
                        vector_store.fail_rebuild(&job.job_id, &error_digest)
                    })
                    .await?;
                    return Err(AppError::internal(
                        serde_json::json!({
                            "operation": "rebuild_memory_index",
                            "jobId": failed.job_id,
                            "status": failed.status,
                            "errorDigest": error_digest,
                        })
                        .to_string(),
                    ));
                }
            };
            return Ok(vector_rebuild_job_report(&job));
        }

        let mut batch = Vec::with_capacity(page.len());
        for source_record in page {
            let memory = source_record.memory;
            if vector_store
                .rebuild_cancel_requested(&job.job_id)
                .map_err(AppError::from)?
            {
                job = commit_vector_store_mutation(state, || {
                    vector_store.settle_rebuild_cancel(&job.job_id)
                })
                .await?;
                return Ok(vector_rebuild_job_report(&job));
            }
            let Some(canonical_owner) = source_record.canonical_owner else {
                batch.push(VectorRebuildBatchItem {
                    memory_id: memory.id,
                    chunk: None,
                    canonical_owner: None,
                    provider_dispatch_count: 0,
                    cache_hit: false,
                });
                continue;
            };
            let content = memory.content.trim().to_string();
            if content.is_empty() {
                batch.push(VectorRebuildBatchItem {
                    memory_id: memory.id,
                    chunk: None,
                    canonical_owner: None,
                    provider_dispatch_count: 0,
                    cache_hit: false,
                });
                continue;
            }
            let outcome =
                match cancellable_rebuild_embedding(&job.job_id, &content, state, &vector_store)
                    .await?
                {
                    Some(outcome) => outcome,
                    None => {
                        job = commit_vector_store_mutation(state, || {
                            vector_store
                                .settle_rebuild_cancel_with_remote_unknown(&job.job_id, true)
                        })
                        .await?;
                        return Ok(vector_rebuild_job_report(&job));
                    }
                };
            let (embedding, profile, receipt) =
                match require_embedding_success("rebuild_memory_index", outcome) {
                    Ok(result) => result,
                    Err(error) => {
                        let error_digest =
                            openlife_core::persistence_outbox::metadata_digest(&error.to_string());
                        let paused = commit_vector_store_mutation(state, || {
                            vector_store.pause_rebuild(&job.job_id, &error_digest)
                        })
                        .await?;
                        return Err(AppError::external(
                            serde_json::json!({
                                "operation": "rebuild_memory_index",
                                "jobId": paused.job_id,
                                "status": paused.status,
                                "checkpointMemoryId": paused.last_processed_memory_id,
                                "errorDigest": error_digest,
                            })
                            .to_string(),
                        ));
                    }
                };
            let vector_source = canonical_owner.source();
            batch.push(VectorRebuildBatchItem {
                memory_id: memory.id,
                chunk: Some(ExportedVectorChunk {
                    session_id: memory.session_id,
                    content,
                    embedding,
                    embedding_profile_id: profile.id,
                    embedding_dimension: profile.dimension,
                    source: vector_source,
                    created_at: memory.created_at,
                    tier: 2,
                    access_count: memory.access_count,
                    last_accessed_at: memory.last_accessed_at.unwrap_or_default(),
                    importance_score: memory.importance_score,
                    archived: false,
                    archived_at: None,
                    summary: None,
                }),
                canonical_owner: Some(canonical_owner),
                provider_dispatch_count: receipt.provider_dispatches.len(),
                cache_hit: receipt.cache_hit,
            });
        }
        job = match commit_vector_store_mutation(state, || {
            vector_store.stage_rebuild_batch(&job.job_id, &batch)
        })
        .await
        {
            Ok(progress) => progress,
            Err(error) => {
                let error_digest =
                    openlife_core::persistence_outbox::metadata_digest(&error.to_string());
                let failed = commit_vector_store_mutation(state, || {
                    vector_store.fail_rebuild(&job.job_id, &error_digest)
                })
                .await?;
                return Err(AppError::internal(
                    serde_json::json!({
                        "operation": "rebuild_memory_index",
                        "jobId": failed.job_id,
                        "status": failed.status,
                        "errorDigest": error_digest,
                    })
                    .to_string(),
                ));
            }
        };
    }
}

#[cfg(test)]
async fn cancellable_rebuild_embedding(
    job_id: &str,
    content: &str,
    state: &Arc<AppState>,
    vector_store: &openlife_core::vectors::VectorStore,
) -> Result<Option<EmbeddingOutcome>, AppError> {
    let mut embedding = Box::pin(embed_memory_text_with_privacy(content, state));
    loop {
        tokio::select! {
            result = &mut embedding => return result.map(Some),
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                if vector_store.rebuild_cancel_requested(job_id).map_err(AppError::from)? {
                    return Ok(None);
                }
            }
        }
    }
}

#[cfg(test)]
fn vector_rebuild_job_report(job: &VectorRebuildJob) -> serde_json::Value {
    serde_json::json!({
        "jobId": job.job_id,
        "status": job.status,
        "processed": job.processed,
        "total": job.source_snapshot.total_count,
        "indexed": job.indexed,
        "skipped": job.skipped,
        "checkpointMemoryId": job.last_processed_memory_id,
        "embeddingProfileId": job.embedding_profile_id,
        "embeddingProfileRoute": serde_json::Value::Null,
        "embeddingDimension": job.embedding_dimension,
        "providerInvocations": job.provider_invocations,
        "cacheHits": job.cache_hits,
        "remoteUnknownProviderAttempts": job.remote_unknown_provider_attempts,
        "resumable": matches!(job.status, VectorRebuildJobStatus::Running | VectorRebuildJobStatus::Paused),
        "cancellable": matches!(job.status, VectorRebuildJobStatus::Running | VectorRebuildJobStatus::Paused | VectorRebuildJobStatus::Prepared),
        "lastErrorDigest": job.last_error_digest,
    })
}

#[cfg(test)]
pub(crate) async fn get_memory_index_rebuild_progress_with_state(
    state: &Arc<AppState>,
) -> Result<Option<VectorRebuildJob>, AppError> {
    require_persistence_read(state, "VectorStore")?;
    let store = state.vector_store.lock().await.clone();
    store.latest_rebuild_job().map_err(AppError::from)
}

#[cfg(test)]
pub(crate) async fn cancel_memory_index_rebuild_with_state(
    job_id: Option<String>,
    state: &Arc<AppState>,
) -> Result<VectorRebuildJob, AppError> {
    require_persistence_write(state)?;
    let store = state.vector_store.lock().await.clone();
    let target = match job_id {
        Some(job_id) if !job_id.trim().is_empty() => job_id,
        _ => {
            store
                .latest_rebuild_job()
                .map_err(AppError::from)?
                .ok_or_else(|| AppError::internal("vector rebuild job not found"))?
                .job_id
        }
    };
    commit_vector_store_mutation(state, || store.request_rebuild_cancel(&target)).await
}

pub(crate) async fn materialize_memory_proposal_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    content: String,
    session_id: String,
    _original_source: String,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    require_persistence_write_string(state)?;
    let mut lifecycle_report = {
        let lifecycle_store = state
            .memory_lifecycle_store
            .as_ref()
            .ok_or_else(memory_lifecycle_store_missing)?;
        let store = lifecycle_store.lock().await;
        let mut lifecycle_input =
            match MemoryLifecycleAcceptanceInput::from_memory_proposal(proposal, content.clone()) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(patch_result(
                        proposal,
                        false,
                        "memory_write_not_committed",
                        Some(openlife_core::persistence_outbox::metadata_digest(
                            &error.to_string(),
                        )),
                    ));
                }
            };
        if let Err(error) = openlife_core::agent::bind_memory_fact_scope_owner(
            &mut lifecycle_input.fact,
            Some(&session_id),
            None,
            None,
        ) {
            return Ok(patch_result(
                proposal,
                false,
                "memory_write_scope_owner_unavailable",
                Some(openlife_core::persistence_outbox::metadata_digest(
                    &error.to_string(),
                )),
            ));
        }
        match store.accept_memory_proposal(lifecycle_input) {
            Ok(report) => report,
            Err(error) => {
                return Ok(patch_result(
                    proposal,
                    false,
                    "memory_write_not_committed",
                    Some(openlife_core::persistence_outbox::metadata_digest(
                        &error.to_string(),
                    )),
                ));
            }
        }
    };
    if lifecycle_report.admission_outcome
        == openlife_core::agent::MemoryAdmissionOutcome::TerminalHistorical
    {
        return Ok(patch_result(
            proposal,
            false,
            "memory_write_terminal_historical",
            Some(
                serde_json::json!({
                    "admissionOutcome": lifecycle_report.admission_outcome,
                    "admissionAt": lifecycle_report.admission_at,
                    "ownerAcceptedAt": lifecycle_report.owner_accepted_at,
                    "historicalMemoryId": lifecycle_report.record.memory_id,
                    "historicalStatus": lifecycle_report.record.status,
                    "canonicalCommitted": false,
                })
                .to_string(),
            ),
        ));
    }
    let canonical_mutation = lifecycle_report
        .canonical_mutation
        .as_ref()
        .ok_or_else(|| "active Memory admission is missing canonical mutation".to_string())?;
    notify_canonical_outbox_background_worker(state);
    let mut preceding_projection_states = Vec::new();
    let mut preceding_reconciliation_error_digests = Vec::new();
    for receipt in &lifecycle_report.preceding_canonical_mutations {
        if let Err(error) = reconcile_foreground_canonical_outbox_event_with_state(
            state,
            CanonicalOutboxOwner::MemoryLifecycleStore,
            &receipt.event_id,
        )
        .await
        {
            preceding_reconciliation_error_digests
                .push(openlife_core::persistence_outbox::metadata_digest(&error));
        }
        preceding_projection_states.push(
            state
                .memory_lifecycle_store
                .as_ref()
                .ok_or_else(memory_lifecycle_store_missing)?
                .lock()
                .await
                .projection_summary(&receipt.event_id)
                .map_err(|error| error.to_string())?
                .state(),
        );
    }
    let reconciliation_error = reconcile_foreground_canonical_outbox_event_with_state(
        state,
        CanonicalOutboxOwner::MemoryLifecycleStore,
        &canonical_mutation.event_id,
    )
    .await
    .err();
    let projection_summary = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?
        .lock()
        .await
        .projection_summary(&canonical_mutation.event_id)
        .map_err(|error| error.to_string())?;
    lifecycle_report.projection_state =
        if preceding_projection_states.contains(&ProjectionDeliveryState::Degraded) {
            ProjectionDeliveryState::Degraded
        } else if preceding_projection_states
            .iter()
            .any(|state| *state != ProjectionDeliveryState::Applied)
        {
            ProjectionDeliveryState::Pending
        } else {
            projection_summary.state()
        };
    let projection_issue = if lifecycle_report.projection_state == ProjectionDeliveryState::Applied
    {
        None
    } else {
        Some(
            serde_json::json!({
                "canonicalCommitted": true,
                "outboxEventId": canonical_mutation.event_id,
                "projectionState": lifecycle_report.projection_state,
                "pending": projection_summary.pending,
                "degraded": projection_summary.degraded,
                "precedingProjectionStates": preceding_projection_states,
                "precedingReconciliationErrorDigests": preceding_reconciliation_error_digests,
                "reconciliationErrorDigest": reconciliation_error
                    .as_deref()
                    .map(openlife_core::persistence_outbox::metadata_digest),
            })
            .to_string(),
        )
    };

    Ok(patch_result(
        proposal,
        true,
        match (
            lifecycle_report.admission_outcome,
            lifecycle_report.projection_state,
        ) {
            (
                openlife_core::agent::MemoryAdmissionOutcome::OwnerCreated,
                ProjectionDeliveryState::Applied,
            ) => "memory_write",
            (
                openlife_core::agent::MemoryAdmissionOutcome::GovernanceUpgraded,
                ProjectionDeliveryState::Applied,
            ) => "memory_write_governance_upgraded",
            (
                openlife_core::agent::MemoryAdmissionOutcome::AliasLinked,
                ProjectionDeliveryState::Applied,
            ) => "memory_write_alias_linked",
            (
                openlife_core::agent::MemoryAdmissionOutcome::ExactReplay,
                ProjectionDeliveryState::Applied,
            ) => "memory_write_exact_replay",
            (openlife_core::agent::MemoryAdmissionOutcome::TerminalHistorical, _) => {
                "memory_write_terminal_historical"
            }
            (_, ProjectionDeliveryState::Pending) => "memory_write_projection_pending",
            (_, ProjectionDeliveryState::Degraded) => "memory_write_projection_degraded",
            (_, ProjectionDeliveryState::Superseded) => "memory_write_projection_superseded",
            (_, ProjectionDeliveryState::Compensated) => "memory_write_projection_compensated",
        },
        projection_issue,
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
pub(crate) async fn commit_explicit_user_memory_for_turn_with_state(
    state: &Arc<AppState>,
    source_task_id: Option<String>,
    source_run_id: Option<String>,
    source_message_id: String,
    fact: CanonicalMemoryFactDescriptor,
    admission_proof: ExplicitMemoryAdmissionProof,
    source_user_message: &str,
    candidate: &MemoryCandidate,
    execution_epoch: &crate::main_chat_cancellation::MainChatExecutionEpoch,
) -> Result<ExplicitMemoryWriteReceipt, String> {
    require_persistence_write_string(state)?;
    commit_explicit_user_memory_inner(
        state,
        source_task_id,
        source_run_id,
        source_message_id,
        openlife_core::agent::metadata_safe::metadata_safe_text_digest(source_user_message).1,
        candidate.candidate_id.clone(),
        fact,
        admission_proof,
        execution_epoch,
    )
    .await
}

// Explicit Memory admission binds the current task/run/message, typed policy
// proof, rollback grant, and cancellation epoch as separate authority facts.
#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
async fn commit_explicit_user_memory_inner(
    state: &Arc<AppState>,
    source_task_id: Option<String>,
    source_run_id: Option<String>,
    source_message_id: String,
    source_message_digest: String,
    authorized_candidate_id: String,
    fact: CanonicalMemoryFactDescriptor,
    admission_proof: ExplicitMemoryAdmissionProof,
    execution_epoch: &crate::main_chat_cancellation::MainChatExecutionEpoch,
) -> Result<ExplicitMemoryWriteReceipt, String> {
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?;
    let store = lifecycle_store.lock().await;
    let result = {
        let source_owner = source_task_id
            .as_deref()
            .unwrap_or(source_message_id.as_str());
        let commit_permit = execution_epoch
            .begin_canonical_commit("memory", format!("explicit:{source_owner}"))
            .map_err(|rejection| format!("explicit memory commit rejected: {rejection:?}"))?;
        let result = store.commit_explicit_user_memory(
            ExplicitMemoryWriteInput {
                source_task_id,
                source_run_id,
                source_message_id,
                source_message_digest,
                authorized_candidate_id,
                fact,
            },
            admission_proof,
        );
        match &result {
            Ok(receipt) if receipt.newly_committed && receipt.canonical_committed => {
                commit_permit.finish_committed();
            }
            Ok(_) => {
                commit_permit.finish_not_modified();
            }
            Err(_) => {
                commit_permit.finish_failed();
            }
        }
        result
    };
    drop(store);
    match result {
        Ok(mut receipt) => {
            if receipt.admission_outcome
                == openlife_core::agent::MemoryAdmissionOutcome::TerminalHistorical
            {
                return Ok(receipt);
            }
            notify_canonical_outbox_background_worker(state);
            let reconciliation_error = match receipt.outbox_event_id.as_deref() {
                Some(event_id) => reconcile_foreground_canonical_outbox_event_with_state(
                    state,
                    CanonicalOutboxOwner::MemoryLifecycleStore,
                    event_id,
                )
                .await
                .err(),
                None => Some("explicit Memory outbox reference missing".to_string()),
            };
            if let Some(event_id) = receipt.outbox_event_id.as_deref() {
                match state
                    .memory_lifecycle_store
                    .as_ref()
                    .ok_or_else(memory_lifecycle_store_missing)?
                    .lock()
                    .await
                    .projection_summary(event_id)
                {
                    Ok(summary) => receipt.projection_state = summary.state(),
                    Err(error) => {
                        receipt.projection_state = ProjectionDeliveryState::Degraded;
                        receipt.projection_error_digest = Some(
                            openlife_core::persistence_outbox::metadata_digest(&error.to_string()),
                        );
                    }
                }
            } else {
                // New code always has an outbox reference. Preserve truthful
                // degradation if an unsupported legacy receipt reaches here.
                receipt.projection_state = ProjectionDeliveryState::Degraded;
                receipt.projection_error_digest =
                    Some(openlife_core::persistence_outbox::metadata_digest(
                        "explicit_memory_outbox_reference_missing",
                    ));
            }
            if let Some(error) = reconciliation_error {
                receipt.projection_error_digest =
                    Some(openlife_core::persistence_outbox::metadata_digest(&error));
                if receipt.projection_state == ProjectionDeliveryState::Applied {
                    // A different owner may have caused the coordinator error;
                    // the exact explicit-Memory delivery summary remains the
                    // higher-fidelity fact and must not be overwritten.
                    receipt.projection_error_digest = None;
                }
            }
            Ok(receipt)
        }
        Err(error) => Err(error.to_string()),
    }
}

async fn reconcile_memory_acceptance_report(
    state: &Arc<AppState>,
    report: &mut MemoryLifecycleAcceptanceReport,
) -> Result<Option<String>, String> {
    let canonical_mutation = report
        .canonical_mutation
        .as_ref()
        .ok_or_else(|| "active Memory admission is missing canonical mutation".to_string())?;
    notify_canonical_outbox_background_worker(state);
    let mut preceding_states = Vec::new();
    let mut error_digests = Vec::new();
    for receipt in &report.preceding_canonical_mutations {
        if let Err(error) = reconcile_foreground_canonical_outbox_event_with_state(
            state,
            CanonicalOutboxOwner::MemoryLifecycleStore,
            &receipt.event_id,
        )
        .await
        {
            error_digests.push(openlife_core::persistence_outbox::metadata_digest(&error));
        }
        preceding_states.push(
            state
                .memory_lifecycle_store
                .as_ref()
                .ok_or_else(memory_lifecycle_store_missing)?
                .lock()
                .await
                .projection_summary(&receipt.event_id)
                .map_err(|error| error.to_string())?
                .state(),
        );
    }
    if let Err(error) = reconcile_foreground_canonical_outbox_event_with_state(
        state,
        CanonicalOutboxOwner::MemoryLifecycleStore,
        &canonical_mutation.event_id,
    )
    .await
    {
        error_digests.push(openlife_core::persistence_outbox::metadata_digest(&error));
    }
    let projection_state = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?
        .lock()
        .await
        .projection_summary(&canonical_mutation.event_id)
        .map_err(|error| error.to_string())?
        .state();
    report.projection_state = if preceding_states.contains(&ProjectionDeliveryState::Degraded) {
        ProjectionDeliveryState::Degraded
    } else if preceding_states
        .iter()
        .any(|state| *state != ProjectionDeliveryState::Applied)
    {
        ProjectionDeliveryState::Pending
    } else {
        projection_state
    };
    Ok(
        (report.projection_state != ProjectionDeliveryState::Applied).then(|| {
            openlife_core::persistence_outbox::metadata_digest(&format!(
                "memory_projection:{}:{:?}:{:?}",
                canonical_mutation.event_id, report.projection_state, error_digests
            ))
        }),
    )
}

pub(crate) async fn correct_memory_asset_with_state(
    memory_id: String,
    content: String,
    state: &Arc<AppState>,
) -> Result<MemoryCorrectionResult, String> {
    require_persistence_write_string(state)?;
    ensure_exact_memory_id(&memory_id)?;
    let content = content.trim();
    if content.is_empty() || content.chars().count() > 32_768 {
        return Err("Memory correction must contain 1..=32,768 characters".into());
    }
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?;
    let mut report = {
        let store = lifecycle_store.lock().await;
        let record = store
            .get_record(&memory_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Memory correction target not found".to_string())?;
        if !store
            .is_memory_retrievable(&memory_id)
            .map_err(|error| error.to_string())?
            || record.content.is_empty()
        {
            return Err("Restore this Memory before correcting it".into());
        }
        if record.category == MemoryLifecycleCategory::Correction {
            return Err("A historical correction record cannot be corrected again".into());
        }
        if record.content == content {
            return Err("Memory correction must change the content".into());
        }
        let mut fact = CanonicalMemoryFactDescriptor::new(
            content.to_string(),
            record.scope,
            record.category,
            record.risk_level,
            record.sensitivity,
        )
        .map_err(|error| error.to_string())?;
        if let Some(owner) = record.scope_owner_ref.as_deref() {
            fact = fact
                .with_scope_owner_ref(owner)
                .map_err(|error| error.to_string())?;
        }
        store
            .accept_memory_proposal(MemoryLifecycleAcceptanceInput {
                proposal_id: format!("direct:memory-correction:{}", uuid::Uuid::new_v4()),
                source_task_id: record.source_task_id,
                source_run_id: record.source_run_id,
                fact,
                created_by: "user_direct_control".into(),
                accepted_by: "user".into(),
                evidence_ids: record.evidence_ids,
                confidence: "1.000".into(),
                conflict_ids: Vec::new(),
                supersedes_memory_id: Some(memory_id.clone()),
            })
            .map_err(|error| error.to_string())?
    };
    let projection_error_digest = reconcile_memory_acceptance_report(state, &mut report).await?;
    Ok(MemoryCorrectionResult {
        memory_id: report.record.memory_id,
        replaced_memory_id: memory_id,
        canonical_committed: report.canonical_committed,
        projection_state: report.projection_state,
        projection_error_digest,
        undo_available: true,
    })
}

pub(crate) async fn archive_memory_asset_with_state(
    memory_id: String,
    state: &Arc<AppState>,
) -> Result<MemoryRetrievalMutationResult, AppError> {
    ensure_exact_memory_id(&memory_id).map_err(AppError::permission)?;
    set_memory_retrieval_disposition_with_state(
        &CanonicalMemoryOwnerInput {
            owner_kind: "memory_lifecycle".into(),
            owner_id: memory_id,
        },
        MemoryRetrievalDisposition::Archived,
        "user_direct_archive",
        state,
    )
    .await
}

pub(crate) async fn restore_memory_asset_with_state(
    memory_id: String,
    state: &Arc<AppState>,
) -> Result<MemoryRetrievalMutationResult, AppError> {
    ensure_exact_memory_id(&memory_id).map_err(AppError::permission)?;
    restore_archived_chunks_with_state(
        &CanonicalMemoryOwnerInput {
            owner_kind: "memory_lifecycle".into(),
            owner_id: memory_id,
        },
        state,
    )
    .await
}

pub(crate) async fn archive_memory_for_proposal_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    owners: &[CanonicalMemoryOwnerInput],
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    let disposition = match proposal
        .after
        .get("recallDisposition")
        .or_else(|| proposal.after.get("recall_disposition"))
        .and_then(serde_json::Value::as_str)
    {
        Some("paused") => MemoryRetrievalDisposition::Paused,
        Some("archived") | None => MemoryRetrievalDisposition::Archived,
        Some(_) => return Err("MemoryArchive Proposal recall disposition is invalid".into()),
    };
    let reason_code = match disposition {
        MemoryRetrievalDisposition::Paused => "user_reviewed_stop_recall",
        MemoryRetrievalDisposition::Archived => "user_reviewed_archive",
        MemoryRetrievalDisposition::Active => unreachable!(),
    };
    let results =
        set_memory_retrieval_dispositions_with_state(owners, disposition, reason_code, state)
            .await
            .map_err(|error| error.to_string())?;
    let projection_issue = results
        .iter()
        .find(|result| result.projection_state != ProjectionDeliveryState::Applied)
        .map(|result| {
            format!(
                "memory_archive_projection_{}",
                result.projection_state.as_str()
            )
        });
    Ok(patch_result(
        proposal,
        !results.is_empty() && results.iter().all(|result| result.canonical_committed),
        "memory_archive",
        projection_issue,
    ))
}

pub(crate) async fn rollback_memory_asset_with_state(
    memory_id: String,
    reason: String,
    state: &Arc<AppState>,
) -> Result<MemoryRollbackReport, String> {
    require_persistence_write_string(state)?;
    ensure_exact_memory_id(&memory_id)?;
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("rollback_memory_asset requires a rollback reason.".into());
    }
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?;
    let store = lifecycle_store.lock().await;
    let mut report = store
        .rollback_memory_asset(&memory_id, "user", reason)
        .map_err(|e| e.to_string())?;
    drop(store);
    finish_memory_rollback_projection(state, &mut report).await?;
    Ok(report)
}

pub(crate) async fn privacy_erase_memory_asset_with_state(
    memory_id: String,
    state: &Arc<AppState>,
) -> Result<MemoryPrivacyEraseReport, String> {
    require_persistence_write_string(state)?;
    ensure_exact_memory_id(&memory_id)?;
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?;
    let proposal_id = lifecycle_store
        .lock()
        .await
        .get_record(&memory_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Memory privacy erase target not found".to_string())?
        .proposal_id;
    if proposal_id.starts_with("proposal:") {
        state
            .proposal_store
            .as_ref()
            .ok_or_else(|| "ProposalStore unavailable while privacy-erasing Memory".to_string())?
            .lock()
            .await
            .redact_accepted_memory_proposal_content(&proposal_id, &memory_id)
            .map_err(|error| error.to_string())?;
    }
    let mut report = lifecycle_store
        .lock()
        .await
        .privacy_erase_memory_asset(&memory_id)
        .map_err(|error| error.to_string())?;
    notify_canonical_outbox_background_worker(state);
    let reconciliation_error = reconcile_blocking_canonical_outbox_event_with_state(
        state,
        CanonicalOutboxOwner::MemoryLifecycleStore,
        &report.canonical_mutation.event_id,
    )
    .await
    .err();
    match lifecycle_store
        .lock()
        .await
        .projection_summary(&report.canonical_mutation.event_id)
    {
        Ok(summary) => report.projection_state = summary.state(),
        Err(error) => {
            report.projection_state = ProjectionDeliveryState::Degraded;
            report.projection_error_digest = Some(
                openlife_core::persistence_outbox::metadata_digest(&error.to_string()),
            );
        }
    }
    if report.projection_state != ProjectionDeliveryState::Applied {
        if let Some(error) = reconciliation_error {
            report.projection_error_digest =
                Some(openlife_core::persistence_outbox::metadata_digest(&error));
        }
    }
    Ok(report)
}

async fn finish_memory_rollback_projection(
    state: &Arc<AppState>,
    report: &mut MemoryRollbackReport,
) -> Result<(), String> {
    notify_canonical_outbox_background_worker(state);
    let reconciliation_error = reconcile_blocking_canonical_outbox_event_with_state(
        state,
        CanonicalOutboxOwner::MemoryLifecycleStore,
        &report.canonical_mutation.event_id,
    )
    .await
    .err();
    match state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?
        .lock()
        .await
        .projection_summary(&report.canonical_mutation.event_id)
    {
        Ok(summary) => report.projection_state = summary.state(),
        Err(error) => {
            report.projection_state = ProjectionDeliveryState::Degraded;
            report.projection_error_digest = Some(
                openlife_core::persistence_outbox::metadata_digest(&error.to_string()),
            );
        }
    }
    if report.projection_state != ProjectionDeliveryState::Applied {
        if let Some(error) = reconciliation_error {
            report.projection_error_digest =
                Some(openlife_core::persistence_outbox::metadata_digest(&error));
        }
    }
    Ok(())
}

fn memory_lifecycle_store_missing() -> String {
    "MemoryLifecycleStore unavailable; memory lifecycle governance is required.".into()
}

fn ensure_exact_memory_id(memory_id: &str) -> Result<(), String> {
    let trimmed = memory_id.trim();
    if trimmed != memory_id
        || trimmed.is_empty()
        || !trimmed.starts_with("memory:")
        || trimmed
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(
            "memory_id must be an exact metadata-safe memory lifecycle id without whitespace."
                .into(),
        );
    }
    Ok(())
}

fn patch_result(
    proposal: &AgentProposal,
    success: bool,
    operation: &str,
    error: Option<String>,
) -> openlife_core::life_model::patch::PatchApplyResult {
    openlife_core::life_model::patch::PatchApplyResult {
        patch_id: proposal.id.clone(),
        success,
        path: proposal.affected_path.clone(),
        operation: operation.to_string(),
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn reviewed_personal_memory_proposal(id: &str, content: &str) -> AgentProposal {
        let mut proposal = AgentProposal::new(
            openlife_core::agent::ProposalType::MemoryWrite,
            "memory.personal",
            serde_json::json!({
                "content": content,
                "scope": "global",
                "category": "preference",
                "candidateKind": "preference",
                "riskLevel": "low",
                "sensitivity": "internal"
            }),
            "Reviewed personal Memory",
            1.0,
            openlife_core::agent::RiskLevel::Low,
            openlife_core::agent::ProposalSource::Manual,
        );
        proposal.id = id.into();
        proposal
    }

    #[tokio::test]
    async fn reviewed_correction_waits_for_old_tombstone_and_new_projection() {
        let state = crate::test_utils::test_app_state();
        let original = reviewed_personal_memory_proposal(
            "proposal:correction-original",
            "Original communication preference.",
        );
        let original_result = materialize_memory_proposal_with_state(
            &state,
            &original,
            "Original communication preference.".into(),
            "session:test".into(),
            "manual".into(),
        )
        .await
        .unwrap();
        assert!(original_result.success);
        let original_record = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_retrievable_records(None, 10)
            .unwrap()
            .into_iter()
            .find(|record| record.content == "Original communication preference.")
            .unwrap();
        assert!(original_record.scope_owner_ref.is_none());
        let original_memory_id = original_record.memory_id;

        let mut correction = reviewed_personal_memory_proposal(
            "proposal:correction-replacement",
            "Corrected communication preference.",
        );
        correction.after["supersedesMemoryId"] = serde_json::json!(original_memory_id);
        let correction_result = materialize_memory_proposal_with_state(
            &state,
            &correction,
            "Corrected communication preference.".into(),
            "session:test".into(),
            "manual".into(),
        )
        .await
        .unwrap();
        assert!(correction_result.success, "{correction_result:?}");

        let projected = state
            .memory_store
            .lock()
            .await
            .export_active_memory_records()
            .unwrap();
        let projected_content = projected
            .into_iter()
            .map(|record| record.content)
            .collect::<Vec<_>>();
        assert!(projected_content.is_empty());
        let canonical = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_retrievable_records(None, 10)
            .unwrap();
        assert_eq!(canonical.len(), 1);
        assert_eq!(canonical[0].content, "Corrected communication preference.");
    }

    fn install_canonical_commit_admission_barrier(
        state: &Arc<AppState>,
    ) -> (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        install_canonical_commit_admission_barrier_after_skips(state, 0)
    }

    fn install_canonical_commit_admission_barrier_after_skips(
        state: &Arc<AppState>,
        remaining_skips: usize,
    ) -> (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (admitted_tx, admitted_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let key = Arc::as_ptr(&state.persistence_coordinator) as usize;
        let replaced = CANONICAL_COMMIT_ADMISSION_BARRIER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                key,
                CanonicalCommitAdmissionBarrier {
                    remaining_skips,
                    admitted: admitted_tx,
                    release: release_rx,
                },
            );
        assert!(replaced.is_none(), "test barrier must have one owner");
        (admitted_rx, release_tx)
    }

    #[tokio::test]
    async fn vector_gateway_rejects_admitted_writes_after_generation_invalidation() {
        let vector_state = crate::test_utils::test_app_state();
        let before_vectors = vector_state
            .vector_store
            .lock()
            .await
            .export_all_chunks()
            .unwrap();
        let (vector_admitted, release_vector) =
            install_canonical_commit_admission_barrier(&vector_state);
        let late_vector_state = Arc::clone(&vector_state);
        let vector_write = tokio::spawn(async move {
            run_memory_tier_maintenance_with_state(&late_vector_state).await
        });
        vector_admitted
            .await
            .expect("Vector write must pause after synchronous admission");
        vector_state
            .persistence_coordinator
            .degrade_globally("test_vector_admission_invalidated");
        release_vector.send(()).unwrap();
        let vector_error = vector_write
            .await
            .unwrap()
            .expect_err("stale Vector admission must not reach its owner lock");
        assert!(vector_error
            .message()
            .contains("persistence_admission_invalidated"));
        assert_eq!(
            vector_state
                .vector_store
                .lock()
                .await
                .export_all_chunks()
                .unwrap(),
            before_vectors
        );
    }

    #[tokio::test]
    async fn late_vector_access_telemetry_is_skipped_without_losing_search_results() {
        let state = crate::test_utils::test_app_state();
        let profile = EmbeddingProfile::new(
            EmbeddingRouteKind::DeterministicHash,
            "openlife-test",
            "telemetry-test-v1",
            "builtin:test",
            "telemetry-test-artifact-v1",
            4,
        )
        .unwrap();
        let embedding = [1.0, 0.0, 0.0, 0.0];
        let store = state.vector_store.lock().await.clone();
        store
            .insert(
                "telemetry-session",
                "RESULT_MUST_SURVIVE_SKIPPED_TELEMETRY",
                &embedding,
                &profile,
                "manual_note",
            )
            .unwrap();
        let telemetry_ticket = prepare_vector_search_access_telemetry(&state);
        let matches = match store.search(&embedding, &profile, 5).unwrap() {
            VectorSearchOutcome::Matches { matches, .. } => matches,
            VectorSearchOutcome::RebuildRequired(evidence) => {
                panic!("test vector profile unexpectedly requires rebuild: {evidence:?}")
            }
        };
        assert_eq!(matches.len(), 1);
        let before = store.export_all_chunks().unwrap();

        let (telemetry_admitted, release_telemetry) =
            install_canonical_commit_admission_barrier(&state);
        let late_state = Arc::clone(&state);
        let telemetry_matches = matches.clone();
        let telemetry = tokio::spawn(async move {
            record_vector_search_access_telemetry_with_state(
                &telemetry_matches,
                &late_state,
                telemetry_ticket,
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), telemetry_admitted)
            .await
            .expect("telemetry must enter canonical admission")
            .expect("telemetry admission signal");
        state
            .persistence_coordinator
            .degrade_globally("test_vector_telemetry_admission_invalidated");
        release_telemetry.send(()).unwrap();

        let evidence = telemetry
            .await
            .unwrap()
            .expect("optional telemetry rejection must remain observable");
        assert_eq!(evidence.reason_code, "vector_access_telemetry_skipped");
        assert!(evidence.error_digest.is_some());
        assert_eq!(
            matches[0].0.content,
            "RESULT_MUST_SURVIVE_SKIPPED_TELEMETRY"
        );
        assert_eq!(store.export_all_chunks().unwrap(), before);
    }

    #[tokio::test]
    async fn late_memory_access_telemetry_is_skipped_without_losing_search_results() {
        let state = crate::test_utils::test_app_state();
        create_knowledge_note_with_state(
            uuid::Uuid::new_v4().to_string(),
            "memory-telemetry-session".into(),
            "MEMORY_RESULT_MUST_SURVIVE_SKIPPED_TELEMETRY".into(),
            "test".into(),
            &state,
        )
        .await
        .unwrap();
        let store = state.memory_store.lock().await.clone();
        let telemetry_ticket = prepare_memory_search_access_telemetry(&state);
        let hits = store
            .search_text_memories(None, "MEMORY_RESULT_MUST_SURVIVE_SKIPPED_TELEMETRY", 5)
            .unwrap();
        assert_eq!(hits.len(), 1);
        let before_records = serde_json::to_value(store.export_active_memory_records().unwrap())
            .expect("serialize canonical Memory rows");
        let before_source = store.vector_rebuild_source_snapshot().unwrap();

        let (telemetry_admitted, release_telemetry) =
            install_canonical_commit_admission_barrier(&state);
        let late_state = Arc::clone(&state);
        let telemetry_hits = hits.clone();
        let telemetry = tokio::spawn(async move {
            record_text_search_access_telemetry_with_state(
                &telemetry_hits,
                &late_state,
                telemetry_ticket,
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), telemetry_admitted)
            .await
            .expect("telemetry must enter canonical admission")
            .expect("telemetry admission signal");
        state
            .persistence_coordinator
            .degrade_globally("test_memory_telemetry_admission_invalidated");
        release_telemetry.send(()).unwrap();

        let evidence = telemetry
            .await
            .unwrap()
            .expect("optional telemetry rejection must remain observable");
        assert_eq!(evidence.reason_code, "memory_access_telemetry_skipped");
        assert!(evidence.error_digest.is_some());
        assert_eq!(
            hits[0].chunk.content,
            "MEMORY_RESULT_MUST_SURVIVE_SKIPPED_TELEMETRY"
        );
        assert_eq!(
            serde_json::to_value(store.export_active_memory_records().unwrap())
                .expect("serialize canonical Memory rows"),
            before_records
        );
        assert_eq!(
            store.vector_rebuild_source_snapshot().unwrap(),
            before_source
        );
    }
    #[tokio::test]
    async fn late_memory_retrieval_materializer_rejects_invalidated_commit_admission() {
        let state = crate::test_utils::test_app_state();
        let (_input, owner) =
            seed_canonical_retrieval_owner(&state, "LATE_MATERIALIZER_MUST_NOT_COMMIT").await;
        let archived = state
            .memory_store
            .lock()
            .await
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "test_generation_fence",
            )
            .unwrap();
        let event_id = archived
            .canonical_mutation
            .expect("archive mutation")
            .event_id;
        let delivery = state
            .memory_store
            .lock()
            .await
            .list_replayable_projection_deliveries_for_event(&event_id)
            .unwrap()
            .into_iter()
            .find(|delivery| delivery.projection_target == "vector_store")
            .expect("VectorStore retrieval delivery");
        assert!(!state.vector_store.lock().await.export_all_chunks().unwrap()[0].archived);

        let (materializer_admitted, release_materializer) =
            install_canonical_commit_admission_barrier(&state);
        let late_state = Arc::clone(&state);
        let materializer = tokio::spawn(async move {
            apply_memory_retrieval_projection(
                &late_state,
                CanonicalOutboxOwner::MemoryStore,
                &delivery,
                CanonicalProjectionCommitLane::Normal,
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), materializer_admitted)
            .await
            .expect("materializer target commit must enter canonical admission")
            .expect("materializer admission signal");
        state
            .persistence_coordinator
            .degrade_globally("test_materializer_admission_invalidated");
        release_materializer.send(()).unwrap();

        let error = materializer
            .await
            .unwrap()
            .expect_err("stale materializer admission must not reach VectorStore");
        assert!(matches!(
            error,
            ProjectionReconciliationError::Deferred(
                PersistenceGateError::AdmissionInvalidated { .. }
            )
        ));
        assert!(!state.vector_store.lock().await.export_all_chunks().unwrap()[0].archived);
    }

    #[tokio::test]
    async fn late_memory_store_outbox_marker_rejects_invalidated_commit_admission() {
        let state = crate::test_utils::test_app_state();
        let canonical = state
            .memory_store
            .lock()
            .await
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "late-marker-session",
                "LATE_OUTBOX_MARKER_MUST_NOT_COMMIT",
                "knowledge_note",
                "test",
                &[],
                "private",
            )
            .unwrap();
        let event_id = canonical.canonical_mutation.event_id;
        let delivery = state
            .memory_store
            .lock()
            .await
            .list_replayable_projection_deliveries_for_event(&event_id)
            .unwrap()
            .into_iter()
            .next()
            .expect("MemoryStore outbox delivery");

        let (marker_admitted, release_marker) = install_canonical_commit_admission_barrier(&state);
        let late_state = Arc::clone(&state);
        let marker = tokio::spawn(async move {
            mark_owned_projection_applied(
                &late_state,
                CanonicalOutboxOwner::MemoryStore,
                &delivery,
                CanonicalProjectionCommitLane::Normal,
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), marker_admitted)
            .await
            .expect("MemoryStore marker must enter canonical admission")
            .expect("marker admission signal");
        state
            .persistence_coordinator
            .degrade_globally("test_memory_marker_admission_invalidated");
        release_marker.send(()).unwrap();

        let error = marker
            .await
            .unwrap()
            .expect_err("stale marker admission must not reach MemoryStore");
        assert!(matches!(
            error,
            ProjectionReconciliationError::Deferred(
                PersistenceGateError::AdmissionInvalidated { .. }
            )
        ));
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .projection_summary(&event_id)
                .unwrap()
                .state(),
            ProjectionDeliveryState::Pending
        );
    }
    async fn seed_canonical_retrieval_owner(
        state: &Arc<AppState>,
        body: &str,
    ) -> (CanonicalMemoryOwnerInput, CanonicalVectorOwnerRef) {
        let canonical = state
            .memory_store
            .lock()
            .await
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "retrieval-test-session",
                body,
                "knowledge_note",
                "manual",
                &[],
                "private",
            )
            .unwrap();
        let owner = CanonicalVectorOwnerRef::new(
            "knowledge_note",
            &canonical.knowledge_note_id.to_string(),
        )
        .unwrap();
        let profile = EmbeddingProfile::new(
            EmbeddingRouteKind::DeterministicHash,
            "openlife-test",
            "retrieval-test-v1",
            "builtin:test",
            "retrieval-test-artifact-v1",
            4,
        )
        .unwrap();
        state
            .vector_store
            .lock()
            .await
            .project_memory_embedding(
                &canonical.canonical_mutation.event_id,
                &owner,
                "retrieval-test-session",
                body,
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap();
        (
            CanonicalMemoryOwnerInput {
                owner_kind: owner.kind().into(),
                owner_id: owner.id().into(),
            },
            owner,
        )
    }

    #[tokio::test]
    async fn canonical_archive_filters_text_and_vector_hits_before_projection_catches_up() {
        let state = crate::test_utils::test_app_state();
        let (_input, owner) =
            seed_canonical_retrieval_owner(&state, "archive lag must not search sentinel").await;
        state
            .memory_store
            .lock()
            .await
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .unwrap();

        let raw_vector = state.vector_store.lock().await.export_all_chunks().unwrap();
        assert!(
            !raw_vector[0].archived,
            "projection is deliberately pending"
        );
        assert!(filter_canonical_retrievable_memory_results(
            vec![(
                MemoryChunk {
                    id: 1,
                    session_id: raw_vector[0].session_id.clone(),
                    content: raw_vector[0].content.clone(),
                    source: raw_vector[0].source.clone(),
                    created_at: raw_vector[0].created_at.clone(),
                    tier: raw_vector[0].tier,
                    access_count: raw_vector[0].access_count,
                    last_accessed_at: raw_vector[0].last_accessed_at.clone(),
                    importance_score: raw_vector[0].importance_score,
                    archived: raw_vector[0].archived,
                    archived_at: raw_vector[0].archived_at.clone(),
                    summary: raw_vector[0].summary.clone(),
                },
                1.0,
            )],
            &state,
        )
        .await
        .unwrap()
        .is_empty());
        assert!(state
            .memory_store
            .lock()
            .await
            .search_text_memories(None, "archive lag", 10)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn product_memory_search_reports_missing_lifecycle_authority() {
        let mut state = crate::test_utils::test_app_state();
        Arc::get_mut(&mut state)
            .expect("isolated state owner")
            .memory_lifecycle_store = None;

        let error = search_memory_with_state("healthy empty must be proven".into(), 5, &state)
            .await
            .expect_err("missing lifecycle authority must not return success with zero hits");
        assert!(
            error.to_string().contains("MemoryLifecycleStore")
                || error.to_string().contains("memory_retrieval_degraded"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn archived_list_and_count_require_lifecycle_authority() {
        let mut state = crate::test_utils::test_app_state();
        Arc::get_mut(&mut state)
            .expect("isolated state owner")
            .memory_lifecycle_store = None;

        let list_error = list_archived_chunks_with_state(20, &state)
            .await
            .expect_err("missing lifecycle authority must not become a healthy empty archive");
        let count_error = current_archived_memory_count_with_state(&state)
            .await
            .expect_err("missing lifecycle authority must not become an archived count of zero");

        for error in [list_error, count_error] {
            assert!(
                error.to_string().contains("MemoryLifecycleStore")
                    || error.to_string().contains("memory_retrieval_degraded"),
                "{error}"
            );
        }
    }

    #[tokio::test]
    async fn archived_product_read_future_remains_send() {
        let state = crate::test_utils::test_app_state();
        let task = tokio::spawn(async move { list_archived_chunks_with_state(20, &state).await });

        assert!(task.await.unwrap().unwrap().is_empty());
    }

    #[tokio::test]
    async fn archived_list_and_count_fail_closed_on_lifecycle_query_error() {
        let state = crate::test_utils::test_app_state();
        state
            .memory_lifecycle_store
            .as_ref()
            .expect("test lifecycle store")
            .lock()
            .await
            .retrieval_reader()
            .install_query_failure_for_test()
            .expect("install lifecycle read failure");

        let list_error = list_archived_chunks_with_state(20, &state)
            .await
            .expect_err("corrupt lifecycle archive truth must fail closed");
        let count_error = current_archived_memory_count_with_state(&state)
            .await
            .expect_err("corrupt lifecycle archive truth must fail closed");

        for error in [list_error, count_error] {
            match error {
                AppError::Database {
                    hint: Some(hint), ..
                } => assert_eq!(hint, "memory_retrieval_degraded"),
                other => panic!("expected structured retrieval degradation: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn pending_archive_is_superseded_before_restore_head_is_applied() {
        let state = crate::test_utils::test_app_state();
        let (_input, owner) = seed_canonical_retrieval_owner(&state, "STALE_ARCHIVE_HEAD").await;
        let archived = state
            .memory_store
            .lock()
            .await
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .unwrap();
        let restored = state
            .memory_store
            .lock()
            .await
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Active,
                "user_reviewed_restore",
            )
            .unwrap();
        let archived_event = archived.canonical_mutation.unwrap().event_id;
        let restored_event = restored.canonical_mutation.unwrap().event_id;

        reconcile_blocking_canonical_outbox_event_with_state(
            &state,
            CanonicalOutboxOwner::MemoryStore,
            &restored_event,
        )
        .await
        .unwrap();

        let store = state.memory_store.lock().await;
        assert_eq!(
            store.projection_summary(&archived_event).unwrap().state(),
            ProjectionDeliveryState::Superseded
        );
        assert_eq!(
            store.projection_summary(&restored_event).unwrap().state(),
            ProjectionDeliveryState::Applied
        );
        drop(store);
        assert!(!state.vector_store.lock().await.export_all_chunks().unwrap()[0].archived);
    }

    #[tokio::test]
    async fn archived_view_rejects_a_head_restored_after_the_list_snapshot() {
        let state = crate::test_utils::test_app_state();
        let (_input, owner) = seed_canonical_retrieval_owner(&state, "ARCHIVE_HEAD_RACE").await;
        let archived = state
            .memory_store
            .lock()
            .await
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .unwrap()
            .state
            .expect("archived head");
        state
            .memory_store
            .lock()
            .await
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Active,
                "user_reviewed_restore",
            )
            .unwrap();
        let memory_store = state.memory_store.lock().await.clone();

        assert!(
            !canonical_memory_retrieval_head_matches(&state, &memory_store, &owner, &archived)
                .await
                .unwrap(),
            "a stale archived snapshot must not survive a newer canonical restore head"
        );
    }

    #[tokio::test]
    async fn archived_list_refills_limit_after_newest_snapshot_is_restored() {
        let state = crate::test_utils::test_app_state();
        let (_input_b, owner_b) =
            seed_canonical_retrieval_owner(&state, "ARCHIVE_PAGE_VALID_B").await;
        let (_input_a, owner_a) =
            seed_canonical_retrieval_owner(&state, "ARCHIVE_PAGE_STALE_A").await;
        {
            let store = state.memory_store.lock().await;
            store
                .set_memory_retrieval_disposition(
                    &owner_b,
                    MemoryRetrievalDisposition::Archived,
                    "user_reviewed_archive",
                )
                .unwrap();
            store
                .set_memory_retrieval_disposition(
                    &owner_a,
                    MemoryRetrievalDisposition::Archived,
                    "user_reviewed_archive",
                )
                .unwrap();
        }
        let restore_newest = |store: &openlife_core::memory::KnowledgeNoteProjectionStore| {
            store
                .set_memory_retrieval_disposition(
                    &owner_a,
                    MemoryRetrievalDisposition::Active,
                    "test_restore_after_archive_page",
                )
                .map(|_| ())
                .map_err(AppError::from)
        };

        let listed = list_archived_chunks_with_state_inner(1, &state, Some(&restore_newest))
            .await
            .unwrap();

        assert_eq!(listed.len(), 1, "stale A must be replaced by valid B");
        assert_eq!(listed[0].owner.owner_kind, owner_b.kind());
        assert_eq!(listed[0].owner.owner_id, owner_b.id());
    }

    #[tokio::test]
    async fn archived_product_gateway_refills_five_hundred_after_head_changes_across_501_owners() {
        let state = crate::test_utils::test_app_state();
        let memory_store = state.memory_store.lock().await.clone();
        let mut owners = Vec::with_capacity(501);
        for index in 0..501 {
            let note = memory_store
                .save_knowledge_note_idempotent_with_outbox(
                    &uuid::Uuid::new_v4().to_string(),
                    "archive-product-page-session",
                    &format!("archive product page owner {index}"),
                    "knowledge_note",
                    "manual",
                    &[],
                    "private",
                )
                .unwrap();
            owners.push(
                CanonicalVectorOwnerRef::new("knowledge_note", &note.knowledge_note_id.to_string())
                    .unwrap(),
            );
        }
        for batch in owners.chunks(200) {
            memory_store
                .set_memory_retrieval_dispositions(
                    batch,
                    MemoryRetrievalDisposition::Archived,
                    "user_reviewed_archive",
                )
                .unwrap();
        }
        assert_eq!(
            current_archived_memory_count_with_state(&state)
                .await
                .unwrap(),
            501
        );

        let newest = memory_store
            .list_archived_memory_retrieval_states_page(1, 0)
            .unwrap()
            .into_iter()
            .next()
            .expect("501 archived owners must have a newest head")
            .owner()
            .unwrap();
        let newest_for_hook = newest.clone();
        let restore_after_first_page =
            move |store: &openlife_core::memory::KnowledgeNoteProjectionStore| {
                store
                    .set_memory_retrieval_disposition(
                        &newest_for_hook,
                        MemoryRetrievalDisposition::Active,
                        "test_restore_after_archive_page",
                    )
                    .map(|_| ())
                    .map_err(AppError::from)
            };

        let listed =
            list_archived_chunks_with_state_inner(500, &state, Some(&restore_after_first_page))
                .await
                .unwrap();

        assert_eq!(listed.len(), 500, "the restored head must be backfilled");
        assert!(listed.iter().all(|view| {
            view.owner.owner_kind != newest.kind() || view.owner.owner_id != newest.id()
        }));
        assert_eq!(
            listed
                .iter()
                .map(|view| (&view.owner.owner_kind, &view.owner.owner_id))
                .collect::<HashSet<_>>()
                .len(),
            500,
            "prefix refetch must not duplicate owners while filling the product limit"
        );
        assert_eq!(
            current_archived_memory_count_with_state(&state)
                .await
                .unwrap(),
            500,
            "the product count must resolve the post-race canonical head"
        );
    }

    #[tokio::test]
    async fn restore_projects_current_canonical_head_and_archived_count_ignores_vector_flags() {
        let state = crate::test_utils::test_app_state();
        let (input, owner) = seed_canonical_retrieval_owner(&state, "RESTORE_HEAD_OWNER").await;
        let archived = set_memory_retrieval_disposition_with_state(
            &input,
            MemoryRetrievalDisposition::Archived,
            "user_reviewed_archive",
            &state,
        )
        .await
        .unwrap();
        assert_eq!(archived.projection_state, ProjectionDeliveryState::Applied);
        assert!(state.vector_store.lock().await.export_all_chunks().unwrap()[0].archived);

        assert_eq!(
            get_memory_tier_stats_with_state(&state)
                .await
                .unwrap()
                .archived,
            1
        );

        let restored = restore_archived_chunks_with_state(&input, &state)
            .await
            .unwrap();
        assert_eq!(restored.projection_state, ProjectionDeliveryState::Applied);
        assert_eq!(restored.revision, Some(2));
        assert!(state
            .memory_store
            .lock()
            .await
            .is_memory_retrieval_active(&owner)
            .unwrap());
        let owner_chunk = state
            .vector_store
            .lock()
            .await
            .export_all_chunks()
            .unwrap()
            .into_iter()
            .find(|chunk| chunk.source == owner.source())
            .unwrap();
        assert!(!owner_chunk.archived);
    }

    #[tokio::test]
    async fn forged_canonical_owner_and_vector_row_id_cannot_archive_memory() {
        let state = crate::test_utils::test_app_state();
        let forged = CanonicalMemoryOwnerInput {
            owner_kind: "knowledge_note".into(),
            owner_id: "424242".into(),
        };
        assert!(set_memory_retrieval_disposition_with_state(
            &forged,
            MemoryRetrievalDisposition::Archived,
            "user_reviewed_archive",
            &state,
        )
        .await
        .is_err());
        assert!(CanonicalMemoryOwnerInput {
            owner_kind: "vector_row".into(),
            owner_id: "7".into(),
        }
        .owner()
        .is_err());
    }
    #[tokio::test]
    async fn existing_vector_marker_cannot_hide_lost_canonical_knowledge_note_proof() {
        let state = crate::test_utils::test_app_state();
        state.config.lock().await.llm.embedding_enabled = false;
        let write = create_knowledge_note_with_state(
            uuid::Uuid::new_v4().to_string(),
            "marker-canonical-proof-session".into(),
            "MARKER_CANNOT_REPLACE_CANONICAL_PROOF".into(),
            "manual".into(),
            &state,
        )
        .await
        .unwrap();
        let delivery = state
            .memory_store
            .lock()
            .await
            .list_replayable_projection_deliveries_for_event(&write.outbox_event_id)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let applied = reconcile_canonical_outboxes_with_state(&state, 20)
            .await
            .unwrap();
        assert_eq!(applied.applied, 1);
        assert_eq!(
            state.vector_store.lock().await.count_all_chunks().unwrap(),
            1
        );
        state
            .memory_store
            .lock()
            .await
            .remove_canonical_memory_row_for_corruption_test(write.knowledge_note_id)
            .unwrap();

        assert!(apply_knowledge_note_projection(
            &state,
            &delivery,
            CanonicalProjectionCommitLane::Normal,
        )
        .await
        .is_err());
        assert_eq!(
            state.vector_store.lock().await.count_all_chunks().unwrap(),
            1
        );
        let search =
            search_memory_with_state("MARKER_CANNOT_REPLACE_CANONICAL_PROOF".into(), 10, &state)
                .await
                .unwrap();
        assert!(search
            .hits
            .iter()
            .all(|(chunk, _)| chunk.content != "MARKER_CANNOT_REPLACE_CANONICAL_PROOF"));
    }

    fn test_explicit_fact(content: &str) -> CanonicalMemoryFactDescriptor {
        CanonicalMemoryFactDescriptor::new(
            content,
            openlife_core::agent::MemoryLifecycleScope::Global,
            openlife_core::agent::MemoryLifecycleCategory::Fact,
            openlife_core::agent::MemoryLifecycleRiskLevel::Low,
            openlife_core::agent::MemoryLifecycleSensitivity::Internal,
        )
        .unwrap()
    }

    fn test_explicit_candidate_and_fact(
        source_message_id: &str,
        source_user_message: &str,
    ) -> (MemoryCandidate, CanonicalMemoryFactDescriptor) {
        let claim = source_user_message.trim().trim_end_matches('.').to_string();
        let candidate = MemoryCandidate {
            candidate_id: format!(
                "test-memory-candidate:{}",
                openlife_core::agent::metadata_safe_text_digest(&format!(
                    "{source_message_id}\0{claim}"
                ))
                .1
            ),
            source_span_id: source_message_id.to_string(),
            kind: openlife_core::agent::MemoryCandidateKind::SemanticUserFact,
            destination: openlife_core::agent::MemoryDestination::MemoryProposal,
            evidence_text: claim.clone(),
            source_preview: claim.clone(),
            normalized_claim: claim.clone(),
            sensitivity: "internal".into(),
            stability: "user_directed".into(),
            explicitness: "explicit".into(),
            future_actionability: "user_directed".into(),
            confidence: 1.0,
            reason_codes: vec!["typed_test_fixture".into()],
        };
        let fact = test_explicit_fact(&claim);
        (candidate, fact)
    }

    fn test_explicit_admission_proof(
        source_message_id: &str,
        source_user_message: &str,
        candidate: &MemoryCandidate,
        fact: &CanonicalMemoryFactDescriptor,
    ) -> ExplicitMemoryAdmissionProof {
        ExplicitMemoryAdmissionProof::test_fixture_for_explicit_input(
            &openlife_core::agent::ExplicitMemoryWriteInput {
                source_task_id: None,
                source_run_id: None,
                source_message_id: source_message_id.to_string(),
                source_message_digest:
                    openlife_core::agent::metadata_safe::metadata_safe_text_digest(
                        source_user_message,
                    )
                    .1,
                authorized_candidate_id: candidate.candidate_id.clone(),
                fact: fact.clone(),
            },
        )
    }

    async fn commit_test_explicit_memory(
        state: &Arc<AppState>,
        task_id: &str,
        source_run_id: &str,
        source_message_id: &str,
        fact: CanonicalMemoryFactDescriptor,
    ) -> Result<ExplicitMemoryWriteReceipt, String> {
        let source_user_message = format!("Please remember: {}", fact.canonical_body);
        let (candidate, fact) =
            test_explicit_candidate_and_fact(source_message_id, &source_user_message);
        let proof = test_explicit_admission_proof(
            source_message_id,
            &source_user_message,
            &candidate,
            &fact,
        );
        let registry = {
            state
                .main_chat_runtime_state
                .lock()
                .await
                .cancellation_registry
                .clone()
        };
        let registration = registry.register(task_id);
        commit_explicit_user_memory_for_turn_with_state(
            state,
            Some(task_id.to_string()),
            Some(source_run_id.to_string()),
            source_message_id.to_string(),
            fact,
            proof,
            &source_user_message,
            &candidate,
            &registration.execution_epoch(),
        )
        .await
    }

    async fn hanging_embedding_endpoint() -> (String, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let accepted_for_server = Arc::clone(&accepted);
        tokio::spawn(async move {
            if let Ok(Ok((_socket, _))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), listener.accept()).await
            {
                accepted_for_server.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        });
        (format!("http://{address}"), accepted)
    }

    async fn configure_hanging_cloud_embeddings(state: &Arc<AppState>, endpoint: String) {
        let mut config = state.config.lock().await;
        config.llm.provider = "openai".into();
        config.llm.openai_base = endpoint;
        config.llm.openai_key = "sk-test".into();
        config.llm.embedding_model = "text-embedding-3-small".into();
        config.llm.embedding_enabled = true;
    }

    #[test]
    fn route_quality_does_not_call_hash_or_unknown_profiles_semantic_ready() {
        let hash = EmbeddingProfile::new(
            EmbeddingRouteKind::DeterministicHash,
            "openlife",
            openlife_core::embedding::DETERMINISTIC_HASH_MODEL_V1,
            "builtin:openlife",
            openlife_core::embedding::DETERMINISTIC_HASH_ARTIFACT_V1,
            openlife_core::embedding::DETERMINISTIC_HASH_DIMENSION_V1,
        )
        .unwrap();

        assert_eq!(
            embedding_route_quality(&hash, true),
            EmbeddingRouteQuality::DeterministicHashApproximation
        );
        assert_eq!(
            embedding_route_quality(&EmbeddingProfile::unknown(), true),
            EmbeddingRouteQuality::IdentityUnknown
        );
        assert_eq!(
            embedding_route_quality(&hash, false),
            EmbeddingRouteQuality::Unavailable
        );
    }

    #[tokio::test]
    async fn explicit_memory_gateway_rejects_proof_bound_to_a_different_message_id() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let task_id = "memory-proof-message-mismatch";
        let registry = {
            state
                .main_chat_runtime_state
                .lock()
                .await
                .cancellation_registry
                .clone()
        };
        let registration = registry.register(task_id);
        let source_user_message = "Remember this proof-bound fact.".to_string();
        let (candidate, fact) =
            test_explicit_candidate_and_fact("message-policy-authorized", &source_user_message);
        let proof = test_explicit_admission_proof(
            "message-policy-authorized",
            &source_user_message,
            &candidate,
            &fact,
        );

        let error = commit_explicit_user_memory_for_turn_with_state(
            &state,
            Some(task_id.into()),
            Some("run-proof-message-mismatch".into()),
            "message-spoofed".into(),
            fact,
            proof,
            &source_user_message,
            &candidate,
            &registration.execution_epoch(),
        )
        .await
        .unwrap_err();

        assert!(error.contains("proof does not match canonical write input"));
        assert!(state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_active_records(None, 10)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn cancel_winning_epoch_rejects_explicit_memory_before_canonical_commit() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let task_id = "memory-cancel-wins";
        let registry = {
            state
                .main_chat_runtime_state
                .lock()
                .await
                .cancellation_registry
                .clone()
        };
        let registration = registry.register(task_id);
        registry.request_cancel(task_id);
        let source_user_message = "Remember this only if the canonical commit wins.".to_string();
        let (candidate, fact) =
            test_explicit_candidate_and_fact("message-memory-cancel", &source_user_message);
        let proof = test_explicit_admission_proof(
            "message-memory-cancel",
            &source_user_message,
            &candidate,
            &fact,
        );

        let error = commit_explicit_user_memory_for_turn_with_state(
            &state,
            Some(task_id.into()),
            Some("run-memory-cancel".into()),
            "message-memory-cancel".into(),
            fact,
            proof,
            &source_user_message,
            &candidate,
            &registration.execution_epoch(),
        )
        .await
        .expect_err("cancel-winning epoch must reject explicit Memory commit");
        assert!(error.contains("explicit memory commit rejected"));

        let records = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_active_records(None, 10)
            .unwrap();
        assert!(records.is_empty());
        let snapshot = registration.execution_epoch().snapshot();
        assert!(snapshot.cancel_requested);
        assert!(snapshot.commit_facts.iter().any(|fact| {
            fact.domain == "memory"
                && fact.outcome
                    == crate::main_chat_cancellation::MainChatCanonicalCommitOutcome::RejectedAfterCancel
        }));
    }

    #[tokio::test]
    async fn explicit_memory_commit_winning_epoch_remains_committed_after_cancel() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let task_id = "memory-commit-wins";
        let registry = {
            state
                .main_chat_runtime_state
                .lock()
                .await
                .cancellation_registry
                .clone()
        };
        let registration = registry.register(task_id);
        let source_user_message =
            "Remember that canonical commit won before cancellation.".to_string();
        let (candidate, fact) =
            test_explicit_candidate_and_fact("message-memory-commit", &source_user_message);
        let proof = test_explicit_admission_proof(
            "message-memory-commit",
            &source_user_message,
            &candidate,
            &fact,
        );
        let receipt = commit_explicit_user_memory_for_turn_with_state(
            &state,
            Some(task_id.into()),
            Some("run-memory-commit".into()),
            "message-memory-commit".into(),
            fact,
            proof,
            &source_user_message,
            &candidate,
            &registration.execution_epoch(),
        )
        .await
        .expect("explicit Memory commit wins");
        registry.request_cancel(task_id);

        let persisted = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_record(&receipt.memory_id)
            .unwrap()
            .expect("committed Memory remains canonical");
        assert_eq!(persisted.memory_id, receipt.memory_id);
        assert!(registration
            .execution_epoch()
            .snapshot()
            .commit_facts
            .iter()
            .any(|fact| {
                fact.domain == "memory"
                    && fact.outcome
                        == crate::main_chat_cancellation::MainChatCanonicalCommitOutcome::Committed
            }));
    }

    #[tokio::test]
    async fn foreground_and_startup_blocking_reconciliation_skip_hanging_optional_backlog() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let (endpoint, accepted) = hanging_embedding_endpoint().await;
        configure_hanging_cloud_embeddings(&state, endpoint).await;

        let unrelated = state
            .memory_store
            .lock()
            .await
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "unrelated-backlog-session",
                "UNRELATED_OPTIONAL_EMBEDDING_BACKLOG",
                "knowledge_note",
                "manual",
                &[],
                "private",
            )
            .unwrap();

        let receipt = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            commit_test_explicit_memory(
                &state,
                "foreground-session",
                "foreground-run",
                "foreground-message",
                test_explicit_fact("FOREGROUND_CANONICAL_MEMORY"),
            ),
        )
        .await
        .expect("foreground exact reconciliation must not await embedding")
        .unwrap();

        assert!(receipt.canonical_committed);
        assert_eq!(receipt.projection_state, ProjectionDeliveryState::Pending);
        let own_event_id = receipt.outbox_event_id.as_deref().unwrap();
        let own = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .projection_summary(own_event_id)
            .unwrap();
        assert_eq!(own.applied, 0);
        assert_eq!(own.pending, 1);
        let unrelated_summary = state
            .memory_store
            .lock()
            .await
            .projection_summary(&unrelated.canonical_mutation.event_id)
            .unwrap();
        assert_eq!(unrelated_summary.pending, 1);

        let startup_delete = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .rollback_memory_asset(&receipt.memory_id, "test", "startup_reconciliation_test")
            .unwrap()
            .canonical_mutation;
        assert_eq!(
            state
                .memory_lifecycle_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .projection_summary(&startup_delete.event_id)
                .unwrap()
                .pending,
            1
        );

        // Exercise the real pre-seal startup contract, not only the isolated
        // evaluation admission path: ordinary effects are still forbidden,
        // while the bounded BlockingOnly recovery lane is allowed after every
        // canonical store has reported healthy.
        let startup_coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            startup_coordinator.register_read_write(*store);
        }
        assert!(startup_coordinator.startup_reconciliation_mutations_safe());
        assert!(startup_coordinator.require_effects_allowed().is_err());
        Arc::get_mut(&mut state)
            .expect("test state must have one outer owner")
            .persistence_coordinator = Arc::clone(&startup_coordinator);
        let stale_startup_admission = startup_coordinator
            .admit_startup_reconciliation_writes(&[CanonicalWriteOwner::MemoryStore])
            .expect("healthy pre-seal startup admission");

        let startup = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            reconcile_blocking_canonical_outboxes_with_state(&state, 200),
        )
        .await
        .expect("startup BlockingOnly must not await optional embedding")
        .unwrap();
        assert_eq!(startup.examined, 1);
        assert_eq!(startup.applied, 1);
        assert_eq!(startup.blocking_degraded, 0);
        assert!(!startup.blocking_backlog_may_remain);
        assert_eq!(
            state
                .memory_lifecycle_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .projection_summary(&startup_delete.event_id)
                .unwrap()
                .state(),
            ProjectionDeliveryState::Applied
        );
        assert_eq!(accepted.load(Ordering::SeqCst), 0);
        assert_eq!(
            state
                .memory_store
                .lock()
                .await
                .projection_summary(&unrelated.canonical_mutation.event_id)
                .unwrap()
                .pending,
            1
        );
        startup_coordinator.seal();
        assert_eq!(
            startup_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite
        );
        assert!(startup_coordinator
            .acquire_canonical_commit_permit(&stale_startup_admission)
            .await
            .is_err());
        assert!(startup_coordinator
            .admit_startup_reconciliation_writes(&[CanonicalWriteOwner::MemoryStore,])
            .is_err());
    }
    #[tokio::test]
    async fn explicit_memory_foreground_stays_pending_and_background_recovers_same_outbox() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        state
            .vector_store
            .lock()
            .await
            .install_memory_projection_failure_for_test()
            .unwrap();

        let receipt = commit_test_explicit_memory(
            &state,
            "memory-projection-session",
            "memory-projection-run",
            "memory-projection-message",
            test_explicit_fact("explicit projection recovery sentinel"),
        )
        .await
        .unwrap();

        assert!(receipt.canonical_committed);
        assert_eq!(receipt.projection_state, ProjectionDeliveryState::Pending);
        let event_id = receipt.outbox_event_id.as_deref().unwrap();
        let pending = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .projection_summary(event_id)
            .unwrap();
        assert_eq!(pending.applied, 0);
        assert_eq!(pending.pending, 1);
        assert_eq!(pending.degraded, 0);
        assert_eq!(
            state.vector_store.lock().await.count_all_chunks().unwrap(),
            0
        );

        state
            .vector_store
            .lock()
            .await
            .remove_memory_projection_failure_for_test()
            .unwrap();
        let replay = reconcile_canonical_outboxes_with_state(&state, 20)
            .await
            .unwrap();
        assert_eq!(replay.blocking_degraded, 0);
        assert_eq!(
            state
                .memory_lifecycle_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .projection_summary(event_id)
                .unwrap()
                .state(),
            ProjectionDeliveryState::Applied
        );
        assert_eq!(
            state.vector_store.lock().await.count_all_chunks().unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn rollback_tombstone_wins_over_late_degraded_creation_replay() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        state
            .vector_store
            .lock()
            .await
            .install_memory_projection_failure_for_test()
            .unwrap();
        let created = commit_test_explicit_memory(
            &state,
            "memory-late-create-session",
            "memory-late-create-run",
            "memory-late-create-message",
            test_explicit_fact("late creation must not resurrect"),
        )
        .await
        .unwrap();
        assert_eq!(created.projection_state, ProjectionDeliveryState::Pending);

        let rolled_back = rollback_memory_asset_with_state(
            created.memory_id.clone(),
            "user requested undo".into(),
            &state,
        )
        .await
        .unwrap();
        assert!(rolled_back.canonical_committed);
        assert_eq!(
            rolled_back.projection_state,
            ProjectionDeliveryState::Degraded
        );

        state
            .vector_store
            .lock()
            .await
            .remove_memory_projection_failure_for_test()
            .unwrap();
        reconcile_canonical_outboxes_with_state(&state, 20)
            .await
            .unwrap();

        assert!(!state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .is_memory_active(&created.memory_id)
            .unwrap());
        assert!(state
            .memory_store
            .lock()
            .await
            .search_text_memories(None, "resurrect", 10)
            .unwrap()
            .is_empty());
        assert_eq!(
            state.vector_store.lock().await.count_all_chunks().unwrap(),
            0
        );
        assert_eq!(
            state
                .memory_lifecycle_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .projection_summary(&rolled_back.canonical_mutation.event_id)
                .unwrap()
                .state(),
            ProjectionDeliveryState::Applied
        );
    }
}
