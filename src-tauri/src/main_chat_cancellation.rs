use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use openlife_core::llm::ProviderPolicyReceiptEvidence;
use openlife_core::tool_execution_receipt::{
    ToolEffectStatus, ToolExecutionReceipt, ToolExecutionReceiptRegistration, ToolTransportStatus,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

pub(crate) const MAX_PROVIDER_ATTEMPTS_PER_TURN: usize = 32;
const MAX_PROVIDER_REQUEST_ID_BYTES: usize = 192;
const MAX_PROVIDER_LABEL_BYTES: usize = 128;
const MAX_PROVIDER_ERROR_DIGEST_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
enum MainChatProviderAttemptStateStatus {
    Started,
    Completed {
        finished_at: DateTime<Utc>,
    },
    Failed {
        finished_at: DateTime<Utc>,
        error_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MainChatProviderAttemptState {
    request_id: String,
    provider: String,
    model: String,
    started_at: DateTime<Utc>,
    policy_evidence: ProviderPolicyReceiptEvidence,
    status: MainChatProviderAttemptStateStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatProviderAttemptStatus {
    Completed,
    Failed,
    RemoteUnknown,
}

impl MainChatProviderAttemptStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::RemoteUnknown => "remote_unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatProviderAttemptSnapshot {
    pub(crate) request_id: String,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) policy_evidence: ProviderPolicyReceiptEvidence,
    pub(crate) status: MainChatProviderAttemptStatus,
    pub(crate) observed_at: DateTime<Utc>,
    pub(crate) finished_at: Option<DateTime<Utc>>,
    pub(crate) error_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatProviderCancellationSnapshot {
    pub(crate) observed_at: DateTime<Utc>,
    pub(crate) attempts: Vec<MainChatProviderAttemptSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatProviderAttemptRecordDisposition {
    Recorded,
    Duplicate,
    IgnoredAfterCancel,
}

impl MainChatProviderAttemptRecordDisposition {
    pub(crate) fn should_emit(self) -> bool {
        self == Self::Recorded
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatProviderAttemptError {
    NoActiveTurn,
    CancelRequested,
    DuplicateStart,
    InvalidMetadata,
    CapacityExceeded,
    MissingStart,
    MetadataConflict,
    TerminalConflict,
}

impl std::fmt::Display for MainChatProviderAttemptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NoActiveTurn => "provider_attempt_no_active_turn",
            Self::CancelRequested => "provider_start_admission_rejected:cancel_requested",
            Self::DuplicateStart => "provider_start_admission_rejected:duplicate_start",
            Self::InvalidMetadata => "provider_attempt_invalid_metadata",
            Self::CapacityExceeded => "provider_attempt_capacity_exceeded",
            Self::MissingStart => "provider_attempt_terminal_without_start",
            Self::MetadataConflict => "provider_attempt_metadata_conflict",
            Self::TerminalConflict => "provider_attempt_terminal_conflict",
        })
    }
}

#[derive(Debug, Clone)]
struct ActiveTurnCancellation {
    token: CancellationToken,
    provider_attempts: Vec<MainChatProviderAttemptState>,
    provider_attempt_error: Option<MainChatProviderAttemptError>,
    cancel_observed_at: Option<DateTime<Utc>>,
    registration_id: u64,
    execution_epoch: MainChatExecutionEpoch,
}

#[derive(Debug, Clone)]
enum TurnCancellationEntry {
    CancelledBeforeRegistration { observed_at: DateTime<Utc> },
    Active(ActiveTurnCancellation),
}

#[derive(Debug, Default)]
struct MainChatCancellationRegistryState {
    entries: HashMap<String, TurnCancellationEntry>,
    next_registration_id: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MainChatCancellationRegistry {
    state: Arc<Mutex<MainChatCancellationRegistryState>>,
}

#[derive(Debug)]
pub(crate) struct RegisteredMainChatCancellation {
    pub(crate) token: CancellationToken,
    execution_epoch: MainChatExecutionEpoch,
    registry: MainChatCancellationRegistry,
    task_session_id: String,
    registration_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MainChatCancelOutcome {
    pub(crate) active_turn_found: bool,
    pub(crate) provider_attempt_count: usize,
    pub(crate) provider_terminal_count: usize,
    pub(crate) provider_inflight_unknown_count: usize,
    pub(crate) provider_attempt_state_valid: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct MainChatCancellationRequest {
    pub(crate) outcome: MainChatCancelOutcome,
    pub(crate) execution_epoch: Option<MainChatExecutionEpoch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatCancellationRegistrationError {
    ActiveOwner,
}

impl std::fmt::Display for MainChatCancellationRegistrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ActiveOwner => "main_chat_cancellation_active_owner_conflict",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatCanonicalCommitOutcome {
    RejectedAfterCancel,
    RejectedAfterTerminalizationDegraded,
    Committed,
    Failed,
    NotModified,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatCanonicalCommitFact {
    pub(crate) domain: String,
    pub(crate) object_ref: String,
    pub(crate) outcome: MainChatCanonicalCommitOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatExecutionEpochSnapshot {
    pub(crate) execution_id: String,
    pub(crate) cancel_requested: bool,
    pub(crate) inflight_commit_count: usize,
    pub(crate) commit_facts: Vec<MainChatCanonicalCommitFact>,
    pub(crate) tool_receipts: Vec<ToolExecutionReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatCancellationTerminalDisposition {
    Cancelled,
    InterruptedAfterCommittedEffect,
    InterruptedWithUnknownEffect,
}

impl MainChatCancellationTerminalDisposition {
    pub(crate) fn status(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::InterruptedAfterCommittedEffect | Self::InterruptedWithUnknownEffect => {
                "interrupted"
            }
        }
    }

    pub(crate) fn reason_code(self) -> &'static str {
        match self {
            Self::Cancelled => "cancel_without_canonical_effect",
            Self::InterruptedAfterCommittedEffect => "cancel_after_canonical_commit",
            Self::InterruptedWithUnknownEffect => "cancel_with_canonical_commit_unknown",
        }
    }

    pub(crate) fn canonical_commit_state(self) -> &'static str {
        match self {
            Self::Cancelled => "none",
            Self::InterruptedAfterCommittedEffect => "committed",
            Self::InterruptedWithUnknownEffect => "unknown",
        }
    }
}

impl MainChatExecutionEpochSnapshot {
    pub(crate) fn cancellation_terminal_disposition(
        &self,
    ) -> MainChatCancellationTerminalDisposition {
        if self.tool_receipts.iter().any(|receipt| {
            receipt.effect_status == ToolEffectStatus::Unknown
                && receipt.transport_status != ToolTransportStatus::NotAttempted
        }) || self
            .commit_facts
            .iter()
            .any(|fact| fact.outcome == MainChatCanonicalCommitOutcome::Unknown)
        {
            MainChatCancellationTerminalDisposition::InterruptedWithUnknownEffect
        } else if self
            .tool_receipts
            .iter()
            .any(|receipt| receipt.effect_status == ToolEffectStatus::Confirmed)
            || self
                .commit_facts
                .iter()
                .any(|fact| fact.outcome == MainChatCanonicalCommitOutcome::Committed)
        {
            MainChatCancellationTerminalDisposition::InterruptedAfterCommittedEffect
        } else {
            MainChatCancellationTerminalDisposition::Cancelled
        }
    }

    pub(crate) fn committed_fact_count(&self) -> usize {
        self.commit_facts
            .iter()
            .filter(|fact| fact.outcome == MainChatCanonicalCommitOutcome::Committed)
            .count()
    }

    pub(crate) fn unknown_fact_count(&self) -> usize {
        self.commit_facts
            .iter()
            .filter(|fact| fact.outcome == MainChatCanonicalCommitOutcome::Unknown)
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatCanonicalCommitRejection {
    CancelRequested,
    TerminalizationDegraded,
    InvalidDomain,
    InvalidObjectReference,
}

const MAX_CANONICAL_COMMIT_DOMAIN_BYTES: usize = 64;
const MAX_CANONICAL_COMMIT_OBJECT_REF_BYTES: usize = 192;

#[derive(Debug)]
struct InflightCanonicalCommit {
    domain: String,
    object_ref: String,
}

#[derive(Debug, Default)]
struct MainChatExecutionEpochState {
    cancel_requested: bool,
    terminalization_degraded: bool,
    terminal_owner_generation: Option<u64>,
    next_commit_id: u64,
    inflight_commits: HashMap<u64, InflightCanonicalCommit>,
    commit_facts: Vec<MainChatCanonicalCommitFact>,
    tool_receipt_registrations: Vec<ToolExecutionReceiptRegistration>,
}

#[derive(Debug)]
struct MainChatExecutionEpochInner {
    execution_id: String,
    state: Mutex<MainChatExecutionEpochState>,
    inflight_commits_finished: Notify,
}

#[derive(Debug, Clone)]
pub(crate) struct MainChatExecutionEpoch {
    inner: Arc<MainChatExecutionEpochInner>,
}

impl MainChatExecutionEpoch {
    fn new(execution_id: String) -> Self {
        Self {
            inner: Arc::new(MainChatExecutionEpochInner {
                execution_id,
                state: Mutex::new(MainChatExecutionEpochState::default()),
                inflight_commits_finished: Notify::new(),
            }),
        }
    }

    pub(crate) fn execution_id(&self) -> &str {
        &self.inner.execution_id
    }

    pub(crate) fn bind_terminal_owner_generation(&self, generation: u64) -> Result<(), String> {
        if generation == 0 {
            return Err("main_chat_terminal_owner_generation_invalid".into());
        }
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| "main_chat execution epoch mutex poisoned".to_string())?;
        match state.terminal_owner_generation {
            None => {
                state.terminal_owner_generation = Some(generation);
                Ok(())
            }
            Some(existing) if existing == generation => Ok(()),
            Some(_) => Err("main_chat_terminal_owner_generation_rebind_forbidden".into()),
        }
    }

    pub(crate) fn terminal_owner_generation(&self) -> Result<u64, String> {
        self.inner
            .state
            .lock()
            .map_err(|_| "main_chat execution epoch mutex poisoned".to_string())?
            .terminal_owner_generation
            .ok_or_else(|| "main_chat_terminal_owner_generation_unbound".to_string())
    }

    fn request_cancel(&self) {
        self.inner
            .state
            .lock()
            .expect("main chat execution epoch mutex poisoned")
            .cancel_requested = true;
    }

    fn fence_terminalization_degraded(&self) {
        self.inner
            .state
            .lock()
            .expect("main chat execution epoch mutex poisoned")
            .terminalization_degraded = true;
    }

    pub(crate) fn begin_canonical_commit(
        &self,
        domain: impl Into<String>,
        object_ref: impl Into<String>,
    ) -> Result<MainChatCanonicalCommitPermit, MainChatCanonicalCommitRejection> {
        let domain = validate_canonical_commit_domain(domain.into())?;
        let object_ref = validate_canonical_commit_object_ref(object_ref.into())?;
        let mut state = self
            .inner
            .state
            .lock()
            .expect("main chat execution epoch mutex poisoned");

        if state.cancel_requested {
            state.commit_facts.push(MainChatCanonicalCommitFact {
                domain,
                object_ref,
                outcome: MainChatCanonicalCommitOutcome::RejectedAfterCancel,
            });
            return Err(MainChatCanonicalCommitRejection::CancelRequested);
        }
        if state.terminalization_degraded {
            state.commit_facts.push(MainChatCanonicalCommitFact {
                domain,
                object_ref,
                outcome: MainChatCanonicalCommitOutcome::RejectedAfterTerminalizationDegraded,
            });
            return Err(MainChatCanonicalCommitRejection::TerminalizationDegraded);
        }

        state.next_commit_id = state
            .next_commit_id
            .checked_add(1)
            .expect("main chat canonical commit id exhausted");
        let commit_id = state.next_commit_id;
        state
            .inflight_commits
            .insert(commit_id, InflightCanonicalCommit { domain, object_ref });
        drop(state);

        Ok(MainChatCanonicalCommitPermit {
            epoch: self.clone(),
            commit_id,
            finished: false,
            _not_send: PhantomData,
        })
    }

    pub(crate) async fn wait_for_inflight_commits(&self) -> MainChatExecutionEpochSnapshot {
        loop {
            let notified = self.inner.inflight_commits_finished.notified();
            tokio::pin!(notified);
            let _ = notified.as_mut().enable();

            let snapshot = self.snapshot();
            if snapshot.inflight_commit_count == 0 {
                return snapshot;
            }

            notified.await;
        }
    }

    pub(crate) fn snapshot(&self) -> MainChatExecutionEpochSnapshot {
        let state = self
            .inner
            .state
            .lock()
            .expect("main chat execution epoch mutex poisoned");
        MainChatExecutionEpochSnapshot {
            execution_id: self.inner.execution_id.clone(),
            cancel_requested: state.cancel_requested,
            inflight_commit_count: state.inflight_commits.len(),
            commit_facts: state.commit_facts.clone(),
            tool_receipts: state
                .tool_receipt_registrations
                .iter()
                .map(ToolExecutionReceiptRegistration::snapshot)
                .collect(),
        }
    }

    /// Retains the exact ToolGateway-owned receipt tracker outside the
    /// execution future. If the owning kernel future is later dropped by local
    /// cancellation, the runtime can still settle and persist the observed
    /// transport boundary instead of guessing from a select branch.
    pub(crate) fn observe_tool_execution(&self, registration: ToolExecutionReceiptRegistration) {
        let receipt_id = registration.snapshot().receipt_id;
        let mut state = self
            .inner
            .state
            .lock()
            .expect("main chat execution epoch mutex poisoned");
        if state
            .tool_receipt_registrations
            .iter()
            .any(|existing| existing.snapshot().receipt_id == receipt_id)
        {
            return;
        }
        state.tool_receipt_registrations.push(registration);
    }

    /// Called only after the kernel/tool future has been dropped. Completed
    /// receipts are preserved exactly; unfinished dispatched work becomes a
    /// local abort with an unknown effect, while never-dispatched work remains
    /// effect-not-attempted and cannot be mistaken for a remote call.
    pub(crate) fn settle_tool_receipts_after_local_abort(&self) -> MainChatExecutionEpochSnapshot {
        let registrations = {
            self.inner
                .state
                .lock()
                .expect("main chat execution epoch mutex poisoned")
                .tool_receipt_registrations
                .clone()
        };
        for registration in registrations {
            registration.settle_after_local_abort();
        }
        self.snapshot()
    }

    /// Called only after the kernel/tool future has ended with a runtime
    /// failure. Preserve completed adapter facts; close never-dispatched work as
    /// failed/not-attempted, dispatched work without a response as remote
    /// unknown, and response-observed work as failed.
    pub(crate) fn settle_tool_receipts_after_runtime_failure(
        &self,
    ) -> MainChatExecutionEpochSnapshot {
        let registrations = {
            self.inner
                .state
                .lock()
                .expect("main chat execution epoch mutex poisoned")
                .tool_receipt_registrations
                .clone()
        };
        for registration in registrations {
            registration.settle_after_runtime_failure();
        }
        self.snapshot()
    }

    fn finish_canonical_commit(
        &self,
        commit_id: u64,
        outcome: MainChatCanonicalCommitOutcome,
    ) -> bool {
        let (recorded, all_finished) = {
            let mut state = self
                .inner
                .state
                .lock()
                .expect("main chat execution epoch mutex poisoned");
            let Some(inflight) = state.inflight_commits.remove(&commit_id) else {
                return false;
            };
            state.commit_facts.push(MainChatCanonicalCommitFact {
                domain: inflight.domain,
                object_ref: inflight.object_ref,
                outcome,
            });
            (true, state.inflight_commits.is_empty())
        };
        if all_finished {
            self.inner.inflight_commits_finished.notify_waiters();
        }
        recorded
    }
}

fn validate_canonical_commit_domain(
    domain: String,
) -> Result<String, MainChatCanonicalCommitRejection> {
    if domain.is_empty()
        || domain.len() > MAX_CANONICAL_COMMIT_DOMAIN_BYTES
        || !domain.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
    {
        return Err(MainChatCanonicalCommitRejection::InvalidDomain);
    }
    Ok(domain)
}

fn validate_canonical_commit_object_ref(
    object_ref: String,
) -> Result<String, MainChatCanonicalCommitRejection> {
    if object_ref.is_empty()
        || object_ref.len() > MAX_CANONICAL_COMMIT_OBJECT_REF_BYTES
        || !object_ref
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/#@%+-".contains(&byte))
    {
        return Err(MainChatCanonicalCommitRejection::InvalidObjectReference);
    }
    Ok(object_ref)
}

#[must_use = "canonical commit permits must be finished with an observed outcome"]
#[derive(Debug)]
pub(crate) struct MainChatCanonicalCommitPermit {
    epoch: MainChatExecutionEpoch,
    commit_id: u64,
    finished: bool,
    _not_send: PhantomData<Rc<()>>,
}

impl MainChatCanonicalCommitPermit {
    pub(crate) fn finish_committed(mut self) {
        let recorded = self
            .epoch
            .finish_canonical_commit(self.commit_id, MainChatCanonicalCommitOutcome::Committed);
        self.finished = true;
        debug_assert!(recorded, "canonical commit permit was already finished");
    }

    pub(crate) fn finish_failed(mut self) {
        let recorded = self
            .epoch
            .finish_canonical_commit(self.commit_id, MainChatCanonicalCommitOutcome::Failed);
        self.finished = true;
        debug_assert!(recorded, "canonical commit permit was already finished");
    }

    pub(crate) fn finish_not_modified(mut self) {
        let recorded = self
            .epoch
            .finish_canonical_commit(self.commit_id, MainChatCanonicalCommitOutcome::NotModified);
        self.finished = true;
        debug_assert!(recorded, "canonical commit permit was already finished");
    }
}

impl openlife_core::agent::CanonicalWritePermit for MainChatCanonicalCommitPermit {
    fn finish_committed(self: Box<Self>) {
        MainChatCanonicalCommitPermit::finish_committed(*self);
    }

    fn finish_failed(self: Box<Self>) {
        MainChatCanonicalCommitPermit::finish_failed(*self);
    }

    fn finish_noop(self: Box<Self>) {
        MainChatCanonicalCommitPermit::finish_not_modified(*self);
    }
}

impl openlife_core::agent::CanonicalWriteAdmission for MainChatExecutionEpoch {
    fn acquire(
        &self,
        request: openlife_core::agent::CanonicalWriteAdmissionRequest,
    ) -> Result<
        Box<dyn openlife_core::agent::CanonicalWritePermit>,
        openlife_core::agent::CanonicalWriteAdmissionRejection,
    > {
        self.begin_canonical_commit(request.domain, request.object_ref)
            .map(|permit| Box::new(permit) as Box<dyn openlife_core::agent::CanonicalWritePermit>)
            .map_err(|rejection| {
                let reason_code = match rejection {
                    MainChatCanonicalCommitRejection::CancelRequested => "cancel_requested",
                    MainChatCanonicalCommitRejection::TerminalizationDegraded => {
                        "terminalization_degraded"
                    }
                    MainChatCanonicalCommitRejection::InvalidDomain => "invalid_domain",
                    MainChatCanonicalCommitRejection::InvalidObjectReference => {
                        "invalid_object_reference"
                    }
                };
                openlife_core::agent::CanonicalWriteAdmissionRejection::new(reason_code)
            })
    }
}

impl Drop for MainChatCanonicalCommitPermit {
    fn drop(&mut self) {
        if !self.finished {
            self.finished = true;
            let recorded = self
                .epoch
                .finish_canonical_commit(self.commit_id, MainChatCanonicalCommitOutcome::Unknown);
            debug_assert!(recorded, "canonical commit permit was already finished");
        }
    }
}

fn validate_provider_attempt_value(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/#@%+-".contains(&byte))
}

fn validate_provider_attempt_metadata(
    request_id: &str,
    provider: &str,
    model: &str,
) -> Result<(), MainChatProviderAttemptError> {
    if !validate_provider_attempt_value(request_id, MAX_PROVIDER_REQUEST_ID_BYTES)
        || !validate_provider_attempt_value(provider, MAX_PROVIDER_LABEL_BYTES)
        || !validate_provider_attempt_value(model, MAX_PROVIDER_LABEL_BYTES)
    {
        return Err(MainChatProviderAttemptError::InvalidMetadata);
    }
    Ok(())
}

fn poison_provider_attempt_state(
    active: &mut ActiveTurnCancellation,
    error: MainChatProviderAttemptError,
) -> MainChatProviderAttemptError {
    let stored = *active.provider_attempt_error.get_or_insert(error);
    active.cancel_observed_at.get_or_insert_with(Utc::now);
    active.execution_epoch.request_cancel();
    active.token.cancel();
    stored
}

fn provider_attempt_summary(active: &ActiveTurnCancellation) -> MainChatCancelOutcome {
    let provider_terminal_count = active
        .provider_attempts
        .iter()
        .filter(|attempt| {
            matches!(
                attempt.status,
                MainChatProviderAttemptStateStatus::Completed { .. }
                    | MainChatProviderAttemptStateStatus::Failed { .. }
            )
        })
        .count();
    let provider_inflight_unknown_count = active
        .provider_attempts
        .len()
        .saturating_sub(provider_terminal_count);
    MainChatCancelOutcome {
        active_turn_found: true,
        provider_attempt_count: active.provider_attempts.len(),
        provider_terminal_count,
        provider_inflight_unknown_count,
        provider_attempt_state_valid: active.provider_attempt_error.is_none(),
    }
}

fn no_active_turn_cancel_outcome() -> MainChatCancelOutcome {
    MainChatCancelOutcome {
        active_turn_found: false,
        provider_attempt_count: 0,
        provider_terminal_count: 0,
        provider_inflight_unknown_count: 0,
        provider_attempt_state_valid: true,
    }
}

impl MainChatCancellationRegistry {
    /// Registers the one execution owner for a task without disturbing an
    /// existing owner. A duplicate registration is a product invariant breach:
    /// it must fail closed, and it must not replace or cancel the token, epoch,
    /// or provider-attempt facts owned by the already-running execution.
    pub(crate) fn try_register(
        &self,
        task_session_id: &str,
    ) -> Result<RegisteredMainChatCancellation, MainChatCancellationRegistrationError> {
        let token = CancellationToken::new();
        let execution_id = uuid::Uuid::new_v4().to_string();
        let execution_epoch = MainChatExecutionEpoch::new(execution_id);
        let mut state = self
            .state
            .lock()
            .expect("main chat cancellation registry mutex poisoned");
        let cancel_observed_at = match state.entries.get(task_session_id) {
            Some(TurnCancellationEntry::CancelledBeforeRegistration { observed_at }) => {
                Some(*observed_at)
            }
            Some(TurnCancellationEntry::Active(_)) => {
                return Err(MainChatCancellationRegistrationError::ActiveOwner);
            }
            None => None,
        };
        state.next_registration_id = state
            .next_registration_id
            .checked_add(1)
            .expect("main chat cancellation registration id exhausted");
        let registration_id = state.next_registration_id;

        // A pre-registration cancellation tombstone is consumed only by the
        // execution it was created to stop. The newly registered execution
        // remains present until its guard drops, so terminalization can still
        // observe the same epoch and provider-attempt facts.
        state.entries.remove(task_session_id);
        if cancel_observed_at.is_some() {
            execution_epoch.request_cancel();
            token.cancel();
        }
        state.entries.insert(
            task_session_id.to_string(),
            TurnCancellationEntry::Active(ActiveTurnCancellation {
                token: token.clone(),
                provider_attempts: Vec::new(),
                provider_attempt_error: None,
                cancel_observed_at,
                registration_id,
                execution_epoch: execution_epoch.clone(),
            }),
        );
        drop(state);

        Ok(RegisteredMainChatCancellation {
            token,
            execution_epoch,
            registry: self.clone(),
            task_session_id: task_session_id.to_string(),
            registration_id,
        })
    }

    #[cfg(test)]
    pub(crate) fn register(&self, task_session_id: &str) -> RegisteredMainChatCancellation {
        self.try_register(task_session_id)
            .expect("test registration must acquire the single execution owner")
    }

    pub(crate) fn record_provider_started(
        &self,
        task_session_id: &str,
        request_id: &str,
        provider: &str,
        model: &str,
        started_at: DateTime<Utc>,
        policy_evidence: &ProviderPolicyReceiptEvidence,
    ) -> Result<MainChatProviderAttemptRecordDisposition, MainChatProviderAttemptError> {
        let metadata_result = validate_provider_attempt_metadata(request_id, provider, model)
            .and_then(|_| {
                policy_evidence
                    .validate_minimal_truth()
                    .map_err(|_| MainChatProviderAttemptError::InvalidMetadata)
            });
        let mut state = self
            .state
            .lock()
            .expect("main chat cancellation registry mutex poisoned");
        let Some(TurnCancellationEntry::Active(active)) = state.entries.get_mut(task_session_id)
        else {
            return Err(MainChatProviderAttemptError::NoActiveTurn);
        };
        if let Some(error) = active.provider_attempt_error {
            return Err(error);
        }
        if active.cancel_observed_at.is_some() {
            return Ok(MainChatProviderAttemptRecordDisposition::IgnoredAfterCancel);
        }
        if let Err(error) = metadata_result {
            return Err(poison_provider_attempt_state(active, error));
        }
        if let Some(existing) = active
            .provider_attempts
            .iter()
            .find(|attempt| attempt.request_id == request_id)
        {
            if existing.provider == provider
                && existing.model == model
                && existing.started_at == started_at
                && &existing.policy_evidence == policy_evidence
            {
                return Ok(MainChatProviderAttemptRecordDisposition::Duplicate);
            }
            return Err(poison_provider_attempt_state(
                active,
                MainChatProviderAttemptError::MetadataConflict,
            ));
        }
        if active.provider_attempts.len() >= MAX_PROVIDER_ATTEMPTS_PER_TURN {
            return Err(poison_provider_attempt_state(
                active,
                MainChatProviderAttemptError::CapacityExceeded,
            ));
        }
        active.provider_attempts.push(MainChatProviderAttemptState {
            request_id: request_id.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            started_at,
            policy_evidence: policy_evidence.clone(),
            status: MainChatProviderAttemptStateStatus::Started,
        });
        Ok(MainChatProviderAttemptRecordDisposition::Recorded)
    }

    /// Linearize the real provider adapter-start edge against cancellation.
    /// `request_cancel` and `record_provider_started` both acquire the same
    /// registry mutex, so exactly one side wins: a recorded start may proceed
    /// to HTTP and later becomes `remote_unknown` on cancellation, while a
    /// cancel-first observation rejects the adapter before `.send()`.
    pub(crate) fn admit_provider_start(
        &self,
        task_session_id: &str,
        request_id: &str,
        provider: &str,
        model: &str,
        started_at: DateTime<Utc>,
        policy_evidence: &ProviderPolicyReceiptEvidence,
    ) -> Result<MainChatProviderAttemptRecordDisposition, MainChatProviderAttemptError> {
        match self.record_provider_started(
            task_session_id,
            request_id,
            provider,
            model,
            started_at,
            policy_evidence,
        )? {
            MainChatProviderAttemptRecordDisposition::IgnoredAfterCancel => {
                Err(MainChatProviderAttemptError::CancelRequested)
            }
            MainChatProviderAttemptRecordDisposition::Duplicate => {
                let mut state = self
                    .state
                    .lock()
                    .expect("main chat cancellation registry mutex poisoned");
                let Some(TurnCancellationEntry::Active(active)) =
                    state.entries.get_mut(task_session_id)
                else {
                    return Err(MainChatProviderAttemptError::NoActiveTurn);
                };
                if active.cancel_observed_at.is_some() {
                    return Err(MainChatProviderAttemptError::CancelRequested);
                }
                // Reject and stop this execution, but keep the first recorded
                // attempt readable. Terminalization must still project that
                // real adapter edge as attempted/remote_unknown even though
                // the duplicate-start invariant makes the overall turn fail.
                active.cancel_observed_at.get_or_insert_with(Utc::now);
                active.execution_epoch.request_cancel();
                active.token.cancel();
                Err(MainChatProviderAttemptError::DuplicateStart)
            }
            disposition => Ok(disposition),
        }
    }

    pub(crate) fn record_provider_completed(
        &self,
        task_session_id: &str,
        request_id: &str,
        provider: &str,
        model: &str,
        finished_at: DateTime<Utc>,
    ) -> Result<MainChatProviderAttemptRecordDisposition, MainChatProviderAttemptError> {
        self.record_provider_terminal(
            task_session_id,
            request_id,
            provider,
            model,
            MainChatProviderAttemptStateStatus::Completed { finished_at },
        )
    }

    pub(crate) fn record_provider_failed(
        &self,
        task_session_id: &str,
        request_id: &str,
        provider: &str,
        model: &str,
        finished_at: DateTime<Utc>,
        error_digest: &str,
    ) -> Result<MainChatProviderAttemptRecordDisposition, MainChatProviderAttemptError> {
        self.record_provider_terminal(
            task_session_id,
            request_id,
            provider,
            model,
            MainChatProviderAttemptStateStatus::Failed {
                finished_at,
                error_digest: error_digest.to_string(),
            },
        )
    }

    fn record_provider_terminal(
        &self,
        task_session_id: &str,
        request_id: &str,
        provider: &str,
        model: &str,
        terminal: MainChatProviderAttemptStateStatus,
    ) -> Result<MainChatProviderAttemptRecordDisposition, MainChatProviderAttemptError> {
        let metadata_result = validate_provider_attempt_metadata(request_id, provider, model);
        let mut state = self
            .state
            .lock()
            .expect("main chat cancellation registry mutex poisoned");
        let Some(TurnCancellationEntry::Active(active)) = state.entries.get_mut(task_session_id)
        else {
            return Err(MainChatProviderAttemptError::NoActiveTurn);
        };
        if let Some(error) = active.provider_attempt_error {
            return Err(error);
        }
        if active.cancel_observed_at.is_some() {
            return Ok(MainChatProviderAttemptRecordDisposition::IgnoredAfterCancel);
        }
        if let Err(error) = metadata_result {
            return Err(poison_provider_attempt_state(active, error));
        }
        if matches!(
            &terminal,
            MainChatProviderAttemptStateStatus::Failed { error_digest, .. }
                if !validate_provider_attempt_value(
                    error_digest,
                    MAX_PROVIDER_ERROR_DIGEST_BYTES,
                )
        ) {
            return Err(poison_provider_attempt_state(
                active,
                MainChatProviderAttemptError::InvalidMetadata,
            ));
        }
        let Some(index) = active
            .provider_attempts
            .iter()
            .position(|attempt| attempt.request_id == request_id)
        else {
            return Err(poison_provider_attempt_state(
                active,
                MainChatProviderAttemptError::MissingStart,
            ));
        };
        let attempt = &active.provider_attempts[index];
        if attempt.provider != provider || attempt.model != model {
            return Err(poison_provider_attempt_state(
                active,
                MainChatProviderAttemptError::MetadataConflict,
            ));
        }
        match &attempt.status {
            MainChatProviderAttemptStateStatus::Started => {
                active.provider_attempts[index].status = terminal;
                Ok(MainChatProviderAttemptRecordDisposition::Recorded)
            }
            existing if existing == &terminal => {
                Ok(MainChatProviderAttemptRecordDisposition::Duplicate)
            }
            _ => Err(poison_provider_attempt_state(
                active,
                MainChatProviderAttemptError::TerminalConflict,
            )),
        }
    }

    pub(crate) fn snapshot_provider_attempts_for_cancel(
        &self,
        task_session_id: &str,
        observed_at: DateTime<Utc>,
    ) -> Result<MainChatProviderCancellationSnapshot, MainChatProviderAttemptError> {
        self.snapshot_provider_attempts_for_terminalization(task_session_id, observed_at)
    }

    pub(crate) fn snapshot_provider_attempts_for_terminalization(
        &self,
        task_session_id: &str,
        observed_at: DateTime<Utc>,
    ) -> Result<MainChatProviderCancellationSnapshot, MainChatProviderAttemptError> {
        let state = self
            .state
            .lock()
            .expect("main chat cancellation registry mutex poisoned");
        let Some(TurnCancellationEntry::Active(active)) = state.entries.get(task_session_id) else {
            return Err(MainChatProviderAttemptError::NoActiveTurn);
        };
        if let Some(error) = active.provider_attempt_error {
            return Err(error);
        }
        let cancel_observed_at = active.cancel_observed_at.unwrap_or(observed_at);
        let attempts = active
            .provider_attempts
            .iter()
            .map(|attempt| {
                let (status, terminal_observed_at, finished_at, error_digest) =
                    match &attempt.status {
                        MainChatProviderAttemptStateStatus::Started => (
                            MainChatProviderAttemptStatus::RemoteUnknown,
                            cancel_observed_at,
                            None,
                            None,
                        ),
                        MainChatProviderAttemptStateStatus::Completed { finished_at } => (
                            MainChatProviderAttemptStatus::Completed,
                            *finished_at,
                            Some(*finished_at),
                            None,
                        ),
                        MainChatProviderAttemptStateStatus::Failed {
                            finished_at,
                            error_digest,
                        } => (
                            MainChatProviderAttemptStatus::Failed,
                            *finished_at,
                            Some(*finished_at),
                            Some(error_digest.clone()),
                        ),
                    };
                MainChatProviderAttemptSnapshot {
                    request_id: attempt.request_id.clone(),
                    provider: attempt.provider.clone(),
                    model: attempt.model.clone(),
                    started_at: attempt.started_at,
                    policy_evidence: attempt.policy_evidence.clone(),
                    status,
                    observed_at: terminal_observed_at,
                    finished_at,
                    error_digest,
                }
            })
            .collect();
        Ok(MainChatProviderCancellationSnapshot {
            observed_at: cancel_observed_at,
            attempts,
        })
    }

    pub(crate) fn request_cancel(&self, task_session_id: &str) -> MainChatCancellationRequest {
        let mut state = self
            .state
            .lock()
            .expect("main chat cancellation registry mutex poisoned");
        match state.entries.get_mut(task_session_id) {
            Some(TurnCancellationEntry::Active(active)) => {
                // The epoch must observe cancellation before the token wakes runtime
                // terminalization; otherwise a late canonical commit could slip in.
                active.cancel_observed_at.get_or_insert_with(Utc::now);
                active.execution_epoch.request_cancel();
                active.token.cancel();
                MainChatCancellationRequest {
                    outcome: provider_attempt_summary(active),
                    execution_epoch: Some(active.execution_epoch.clone()),
                }
            }
            Some(TurnCancellationEntry::CancelledBeforeRegistration { .. }) => {
                MainChatCancellationRequest {
                    outcome: no_active_turn_cancel_outcome(),
                    execution_epoch: None,
                }
            }
            None => {
                state.entries.insert(
                    task_session_id.to_string(),
                    TurnCancellationEntry::CancelledBeforeRegistration {
                        observed_at: Utc::now(),
                    },
                );
                MainChatCancellationRequest {
                    outcome: no_active_turn_cancel_outcome(),
                    execution_epoch: None,
                }
            }
        }
    }

    pub(crate) fn is_cancellation_requested(&self, task_session_id: &str) -> bool {
        let state = self
            .state
            .lock()
            .expect("main chat cancellation registry mutex poisoned");
        match state.entries.get(task_session_id) {
            Some(TurnCancellationEntry::CancelledBeforeRegistration { .. }) => true,
            Some(TurnCancellationEntry::Active(active)) => active.cancel_observed_at.is_some(),
            None => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn cancel(&self, task_session_id: &str) -> MainChatCancelOutcome {
        self.request_cancel(task_session_id).outcome
    }

    fn remove_registration(&self, task_session_id: &str, registration_id: u64) {
        let mut state = self
            .state
            .lock()
            .expect("main chat cancellation registry mutex poisoned");
        let owns_current_registration = matches!(
            state.entries.get(task_session_id),
            Some(TurnCancellationEntry::Active(ActiveTurnCancellation {
                registration_id: current_registration_id,
                ..
            })) if *current_registration_id == registration_id
        );
        if owns_current_registration {
            state.entries.remove(task_session_id);
        }
    }

    #[cfg(test)]
    pub(crate) fn has_active_registration(&self, task_session_id: &str) -> bool {
        matches!(
            self.state
                .lock()
                .expect("inspect main chat cancellation registry")
                .entries
                .get(task_session_id),
            Some(TurnCancellationEntry::Active(_))
        )
    }

    #[cfg(test)]
    pub(crate) fn active_registration_count(&self) -> usize {
        self.state
            .lock()
            .expect("inspect main chat cancellation registry")
            .entries
            .values()
            .filter(|entry| matches!(entry, TurnCancellationEntry::Active(_)))
            .count()
    }
}

impl Drop for RegisteredMainChatCancellation {
    fn drop(&mut self) {
        self.registry
            .remove_registration(&self.task_session_id, self.registration_id);
    }
}

impl RegisteredMainChatCancellation {
    pub(crate) fn execution_id(&self) -> &str {
        self.execution_epoch.execution_id()
    }

    pub(crate) fn execution_epoch(&self) -> MainChatExecutionEpoch {
        self.execution_epoch.clone()
    }

    pub(crate) fn fence_terminalization_degraded(&self) {
        self.execution_epoch.fence_terminalization_degraded();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MainChatCancellationRegistry, MainChatCancellationTerminalDisposition,
        MainChatCanonicalCommitOutcome, MainChatCanonicalCommitRejection, MainChatExecutionEpoch,
    };
    use openlife_core::tool_execution_receipt::{
        ToolEffectStatus, ToolExecutionReceiptRegistration, ToolTransportStatus,
    };
    use std::sync::{mpsc, Arc, Barrier};
    use std::time::Duration;

    fn test_provider_policy_evidence(
        request_id: &str,
    ) -> openlife_core::llm::ProviderPolicyReceiptEvidence {
        openlife_core::llm::ProviderPolicyReceiptEvidence {
            decision_id: format!("policy-{request_id}"),
            policy_version: "main_chat_policy_v2".into(),
            issuing_authority: openlife_core::llm::ProviderPolicyAuthority::MainChatPolicyRouter,
            effective_data_route: openlife_core::llm::ProviderDataRoute::PolicyAllowed,
            effective_local_restriction: None,
            subject_scope_digest: format!("sha256:{}", "b".repeat(64)),
            payload_purpose: Some(openlife_core::llm::ProviderPayloadPurpose::MainChatDirectAnswer),
            unfiltered_payload_digest: Some(format!("sha256:{}", "c".repeat(64))),
            context_manifest_digest: format!("sha256:{}", "a".repeat(64)),
            prepared_envelope_digest: Some(format!("sha256:{}", "d".repeat(64))),
            provider_config_generation: "test-provider-generation".into(),
            network_policy_decision_digest: format!("sha256:{}", "e".repeat(64)),
            selected_context_refs: Vec::new(),
            included_context_categories: Vec::new(),
            declared_payload_categories: vec![
                openlife_core::llm::ProviderPayloadCategory::CurrentUserConversation,
            ],
            policy_provenance_refs: Vec::new(),
            raw_life_model_included: false,
            raw_unbounded_memory_included: false,
        }
    }

    struct PauseAfterCanonicalAdmission {
        epoch: MainChatExecutionEpoch,
        acquired: Arc<Barrier>,
        resume: Arc<Barrier>,
    }

    impl openlife_core::agent::CanonicalWriteAdmission for PauseAfterCanonicalAdmission {
        fn acquire(
            &self,
            request: openlife_core::agent::CanonicalWriteAdmissionRequest,
        ) -> Result<
            Box<dyn openlife_core::agent::CanonicalWritePermit>,
            openlife_core::agent::CanonicalWriteAdmissionRejection,
        > {
            let permit =
                openlife_core::agent::CanonicalWriteAdmission::acquire(&self.epoch, request)?;
            self.acquired.wait();
            self.resume.wait();
            Ok(permit)
        }
    }

    #[test]
    fn cancellation_settles_dispatched_tool_as_unknown_without_erasing_completed_receipts() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-tool-receipts");
        let epoch = registration.execution_epoch();

        let completed = ToolExecutionReceiptRegistration::test_observed_mcp_read(
            Some("run-tool-receipts".into()),
            Some("mcp:read-only".into()),
            "sha256:completed-read".into(),
        );
        epoch.observe_tool_execution(completed.clone());

        let inflight = ToolExecutionReceiptRegistration::test_inflight_network_mutation(
            Some("run-tool-receipts".into()),
            Some("mcp:external-mutation".into()),
            "sha256:inflight-write".into(),
        );
        epoch.observe_tool_execution(inflight.clone());

        registry.request_cancel("task-tool-receipts");
        let snapshot = epoch.settle_tool_receipts_after_local_abort();

        assert_eq!(snapshot.tool_receipts.len(), 2);
        let completed_receipt = snapshot
            .tool_receipts
            .iter()
            .find(|receipt| receipt.manifest_id.as_deref() == Some("mcp:read-only"))
            .expect("completed receipt remains observable");
        assert_eq!(
            completed_receipt.transport_status,
            ToolTransportStatus::ResponseObserved
        );
        assert_eq!(
            completed_receipt.effect_status,
            ToolEffectStatus::NotAttempted
        );

        let inflight_receipt = snapshot
            .tool_receipts
            .iter()
            .find(|receipt| receipt.manifest_id.as_deref() == Some("mcp:external-mutation"))
            .expect("inflight receipt remains observable");
        assert_eq!(
            inflight_receipt.transport_status,
            ToolTransportStatus::RemoteUnknown
        );
        assert_eq!(inflight_receipt.effect_status, ToolEffectStatus::Unknown);
        assert!(!inflight_receipt.automatic_retry_safe());
        assert_eq!(
            snapshot.cancellation_terminal_disposition(),
            super::MainChatCancellationTerminalDisposition::InterruptedWithUnknownEffect
        );
    }

    #[test]
    fn cancellation_keeps_never_dispatched_tool_definitely_not_attempted() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-tool-never-dispatched");
        let epoch = registration.execution_epoch();
        let never_dispatched = ToolExecutionReceiptRegistration::test_never_dispatched_read(
            Some("run-tool-never-dispatched".into()),
            Some("builtin:read-only".into()),
            "sha256:never-dispatched".into(),
        );
        epoch.observe_tool_execution(never_dispatched);

        registry.request_cancel("task-tool-never-dispatched");
        let snapshot = epoch.settle_tool_receipts_after_local_abort();
        let receipt = &snapshot.tool_receipts[0];

        assert_eq!(receipt.transport_status, ToolTransportStatus::NotAttempted);
        assert_eq!(receipt.effect_status, ToolEffectStatus::NotAttempted);
        assert!(receipt.dispatched_at.is_none());
        assert!(receipt.finished_at.is_some());
        assert!(receipt.automatic_retry_safe());
        assert_eq!(
            snapshot.cancellation_terminal_disposition(),
            MainChatCancellationTerminalDisposition::Cancelled
        );
    }

    #[test]
    fn runtime_failure_settles_each_tool_at_its_observed_boundary() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-tool-runtime-failure");
        let epoch = registration.execution_epoch();

        epoch.observe_tool_execution(
            ToolExecutionReceiptRegistration::test_never_dispatched_read(
                Some("run-tool-runtime-failure".into()),
                Some("builtin:pre-dispatch".into()),
                "pre-dispatch".into(),
            ),
        );
        epoch.observe_tool_execution(
            ToolExecutionReceiptRegistration::test_inflight_network_mutation(
                Some("run-tool-runtime-failure".into()),
                Some("remote:no-response".into()),
                "no-response".into(),
            ),
        );
        epoch.observe_tool_execution(
            ToolExecutionReceiptRegistration::test_response_observed_read_without_terminal(
                Some("run-tool-runtime-failure".into()),
                Some("remote:response-observed".into()),
                "response-observed".into(),
            ),
        );

        let snapshot = epoch.settle_tool_receipts_after_runtime_failure();
        let receipt = |manifest_id: &str| {
            snapshot
                .tool_receipts
                .iter()
                .find(|receipt| receipt.manifest_id.as_deref() == Some(manifest_id))
                .expect("registered receipt")
        };

        let pre_dispatch = receipt("builtin:pre-dispatch");
        assert_eq!(
            pre_dispatch.transport_status,
            ToolTransportStatus::NotAttempted
        );
        assert_eq!(pre_dispatch.effect_status, ToolEffectStatus::NotAttempted);
        assert_eq!(
            pre_dispatch.execution_outcome,
            openlife_core::tool_execution_receipt::ToolExecutionOutcome::Failed
        );
        assert!(pre_dispatch.finished_at.is_some());

        let no_response = receipt("remote:no-response");
        assert_eq!(
            no_response.transport_status,
            ToolTransportStatus::RemoteUnknown
        );
        assert_eq!(no_response.effect_status, ToolEffectStatus::Unknown);
        assert!(no_response.finished_at.is_some());

        let response_observed = receipt("remote:response-observed");
        assert_eq!(
            response_observed.transport_status,
            ToolTransportStatus::ResponseObserved
        );
        assert_eq!(
            response_observed.execution_outcome,
            openlife_core::tool_execution_receipt::ToolExecutionOutcome::Failed
        );
        assert!(response_observed.finished_at.is_some());
    }

    #[test]
    fn cancel_that_linearizes_before_register_is_observed_by_the_registration() {
        let registry = MainChatCancellationRegistry::default();

        let cancel = registry.cancel("task-cancel-before-register");
        assert!(!cancel.active_turn_found);

        let registration = registry.register("task-cancel-before-register");
        assert!(
            registration.token.is_cancelled(),
            "a cancellation that wins before registration must not be lost"
        );
    }

    #[test]
    fn registration_is_removed_when_the_kernel_path_returns_an_error() {
        fn simulate_kernel_error(
            registry: &MainChatCancellationRegistry,
        ) -> Result<(), &'static str> {
            let _registration = registry.register("task-kernel-error");
            Err("kernel failed")
        }

        let registry = registry_for_error_test();
        assert_eq!(simulate_kernel_error(&registry), Err("kernel failed"));
        assert!(
            !registry.has_active_registration("task-kernel-error"),
            "an error return must not leave a ghost active registration"
        );
    }

    fn registry_for_error_test() -> MainChatCancellationRegistry {
        MainChatCancellationRegistry::default()
    }

    #[test]
    fn concurrent_cancel_and_register_have_no_lost_cancellation_schedule() {
        const ITERATIONS: usize = 10_000;
        let registry = MainChatCancellationRegistry::default();
        let barrier = Arc::new(Barrier::new(3));
        let (registration_tx, registration_rx) = mpsc::sync_channel(1);
        let (cancel_tx, cancel_rx) = mpsc::sync_channel(1);
        let (register_done_tx, register_done_rx) = mpsc::sync_channel(1);
        let (cancel_done_tx, cancel_done_rx) = mpsc::sync_channel(1);

        let register_registry = registry.clone();
        let register_barrier = Arc::clone(&barrier);
        let register_thread = std::thread::spawn(move || {
            for index in 0..ITERATIONS {
                register_barrier.wait();
                let registration = register_registry.register(&format!("task-concurrent-{index}"));
                registration_tx
                    .send(registration)
                    .expect("send registration to the test thread");
                register_barrier.wait();
            }
            register_done_tx
                .send(())
                .expect("signal register worker done");
        });

        let cancel_registry = registry.clone();
        let cancel_barrier = Arc::clone(&barrier);
        let cancel_thread = std::thread::spawn(move || {
            for index in 0..ITERATIONS {
                cancel_barrier.wait();
                let outcome = cancel_registry.cancel(&format!("task-concurrent-{index}"));
                cancel_tx
                    .send(outcome)
                    .expect("send cancellation outcome to the test thread");
                cancel_barrier.wait();
            }
            cancel_done_tx.send(()).expect("signal cancel worker done");
        });

        for index in 0..ITERATIONS {
            barrier.wait();
            let registration = registration_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("receive concurrent registration without deadlock");
            let _cancel_outcome = cancel_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("receive concurrent cancellation without deadlock");
            assert!(
                registration.token.is_cancelled(),
                "iteration {index}: whichever operation linearizes first, cancellation must be observed"
            );
            drop(registration);
            barrier.wait();
        }

        register_done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("register worker must terminate");
        cancel_done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("cancel worker must terminate");
        register_thread.join().expect("join register worker");
        cancel_thread.join().expect("join cancel worker");
        assert_eq!(registry.active_registration_count(), 0);
    }

    #[test]
    fn duplicate_registration_preserves_the_existing_owner_unchanged() {
        use chrono::Utc;

        let registry = MainChatCancellationRegistry::default();
        let older = registry.register("task-registration-generation");
        let provider_started_at = Utc::now();
        registry
            .record_provider_started(
                "task-registration-generation",
                "request-existing-owner",
                "openai",
                "gpt-test",
                provider_started_at,
                &test_provider_policy_evidence("request-existing-owner"),
            )
            .expect("existing owner records provider start");
        let older_epoch = older.execution_epoch().snapshot();
        let duplicate = registry
            .try_register("task-registration-generation")
            .expect_err("a second execution owner must fail closed");

        assert_eq!(
            duplicate,
            super::MainChatCancellationRegistrationError::ActiveOwner
        );
        assert!(
            !older.token.is_cancelled(),
            "a duplicate registration must not cancel the existing owner"
        );
        assert_eq!(
            older.execution_epoch().snapshot(),
            older_epoch,
            "a duplicate registration must not replace or mutate the existing epoch"
        );
        assert!(
            registry.has_active_registration("task-registration-generation"),
            "the existing owner must remain registered"
        );
        registry
            .record_provider_completed(
                "task-registration-generation",
                "request-existing-owner",
                "openai",
                "gpt-test",
                Utc::now(),
            )
            .expect("duplicate registration must not poison existing provider attempts");
        let provider_snapshot = registry
            .snapshot_provider_attempts_for_cancel("task-registration-generation", Utc::now())
            .expect("existing provider facts remain readable");
        assert_eq!(provider_snapshot.attempts.len(), 1);
        assert_eq!(
            provider_snapshot.attempts[0].status,
            super::MainChatProviderAttemptStatus::Completed
        );

        drop(older);
        assert!(!registry.has_active_registration("task-registration-generation"));
    }

    #[test]
    fn cancel_winning_epoch_rejects_canonical_commit_permit() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-cancel-wins-commit");
        let epoch = registration.execution_epoch();

        registry.cancel("task-cancel-wins-commit");
        let rejection = epoch
            .begin_canonical_commit("memory", "memory:item-123")
            .expect_err("cancel must reject a later canonical commit permit");

        assert_eq!(rejection, MainChatCanonicalCommitRejection::CancelRequested);
        let snapshot = epoch.snapshot();
        assert!(snapshot.cancel_requested);
        assert_eq!(snapshot.inflight_commit_count, 0);
        assert_eq!(snapshot.commit_facts.len(), 1);
        assert_eq!(
            snapshot.commit_facts[0].outcome,
            MainChatCanonicalCommitOutcome::RejectedAfterCancel
        );
    }

    #[test]
    fn core_canonical_write_admission_adapter_rejects_after_cancel() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-core-admission-cancel-wins");
        let epoch = registration.execution_epoch();
        registry.cancel("task-core-admission-cancel-wins");

        let rejection = match openlife_core::agent::CanonicalWriteAdmission::acquire(
            &epoch,
            openlife_core::agent::CanonicalWriteAdmissionRequest::new(
                "proposal",
                "proposal:00000000-0000-4000-8000-000000000001",
            ),
        ) {
            Ok(_) => panic!("the Tauri execution owner must reject a core write after cancel"),
            Err(rejection) => rejection,
        };

        assert_eq!(rejection.reason_code(), "cancel_requested");
        let snapshot = epoch.snapshot();
        assert_eq!(snapshot.commit_facts.len(), 1);
        assert_eq!(
            snapshot.commit_facts[0].outcome,
            MainChatCanonicalCommitOutcome::RejectedAfterCancel
        );
    }

    #[test]
    fn core_canonical_write_admission_adapter_preserves_commit_winner_truth() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-core-admission-commit-wins");
        let epoch = registration.execution_epoch();
        let permit = openlife_core::agent::CanonicalWriteAdmission::acquire(
            &epoch,
            openlife_core::agent::CanonicalWriteAdmissionRequest::new(
                "proposal",
                "proposal:00000000-0000-4000-8000-000000000002",
            ),
        )
        .expect("the write linearizes before cancel");

        registry.cancel("task-core-admission-commit-wins");
        permit.finish_committed();

        let snapshot = epoch.snapshot();
        assert_eq!(
            snapshot.cancellation_terminal_disposition(),
            MainChatCancellationTerminalDisposition::InterruptedAfterCommittedEffect
        );
        assert_eq!(
            snapshot.commit_facts[0].outcome,
            MainChatCanonicalCommitOutcome::Committed
        );
    }

    #[test]
    fn core_canonical_write_admission_adapter_distinguishes_idempotent_noop() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-core-admission-noop");
        let epoch = registration.execution_epoch();
        let permit = openlife_core::agent::CanonicalWriteAdmission::acquire(
            &epoch,
            openlife_core::agent::CanonicalWriteAdmissionRequest::new(
                "proposal",
                "proposal:00000000-0000-4000-8000-000000000003",
            ),
        )
        .expect("the idempotent lookup may enter the write boundary");

        registry.cancel("task-core-admission-noop");
        permit.finish_noop();

        let snapshot = epoch.snapshot();
        assert_eq!(
            snapshot.cancellation_terminal_disposition(),
            MainChatCancellationTerminalDisposition::Cancelled
        );
        assert_eq!(
            snapshot.commit_facts[0].outcome,
            MainChatCanonicalCommitOutcome::NotModified
        );
    }

    #[test]
    fn cancel_winning_real_proposal_barrier_persists_nothing() {
        for iteration in 0..64 {
            let registry = MainChatCancellationRegistry::default();
            let task_id = format!("task-real-proposal-cancel-wins-{iteration}");
            let registration = registry.register(&task_id);
            let epoch = registration.execution_epoch();
            let proposal_store = openlife_core::agent::ProposalStore::new_in_memory()
                .expect("create isolated proposal store");
            let proposal = openlife_core::agent::AgentProposal::new(
                openlife_core::agent::ProposalType::ToolPermission,
                "tool_permission.builtin.notes.write",
                serde_json::json!({"permission": "allow_once"}),
                "Test a cancel-winning canonical proposal schedule.",
                1.0,
                openlife_core::agent::RiskLevel::Medium,
                openlife_core::agent::ProposalSource::Manual,
            );
            let proposal_ref = format!("proposal:{}", proposal.id);
            let barrier = Arc::new(Barrier::new(2));
            let cancel_barrier = Arc::clone(&barrier);
            let cancel_registry = registry.clone();
            let cancel_task_id = task_id.clone();
            let canceller = std::thread::spawn(move || {
                cancel_barrier.wait();
                cancel_registry.request_cancel(&cancel_task_id)
            });

            barrier.wait();
            assert!(
                canceller
                    .join()
                    .expect("canceller joins")
                    .outcome
                    .active_turn_found,
                "iteration {iteration}: cancellation must linearize against the active turn"
            );
            let admission = openlife_core::agent::CanonicalWriteAdmission::acquire(
                &epoch,
                openlife_core::agent::CanonicalWriteAdmissionRequest::new("proposal", proposal_ref),
            );
            assert!(
                admission.is_err(),
                "iteration {iteration}: cancel winner must reject proposal admission"
            );
            assert_eq!(
                proposal_store.pending_count().expect("count proposals"),
                0,
                "iteration {iteration}: rejected admission must persist no Proposal"
            );
            assert_eq!(
                epoch.snapshot().cancellation_terminal_disposition(),
                MainChatCancellationTerminalDisposition::Cancelled,
                "iteration {iteration}: cancel winner with no write remains a pure cancellation"
            );
        }
    }

    #[test]
    fn permit_winning_real_proposal_barrier_is_not_relabelled_pure_cancel() {
        for iteration in 0..64 {
            let registry = MainChatCancellationRegistry::default();
            let task_id = format!("task-real-proposal-commit-wins-{iteration}");
            let registration = registry.register(&task_id);
            let epoch = registration.execution_epoch();
            let proposal_store = openlife_core::agent::ProposalStore::new_in_memory()
                .expect("create isolated proposal store");
            let mut proposal = openlife_core::agent::AgentProposal::new(
                openlife_core::agent::ProposalType::ToolPermission,
                "tool_permission.builtin.notes.write",
                serde_json::json!({"permission": "allow_once"}),
                "Test a commit-winning canonical proposal schedule.",
                1.0,
                openlife_core::agent::RiskLevel::Medium,
                openlife_core::agent::ProposalSource::Manual,
            );
            proposal.run_id = Some(format!("run-real-proposal-{iteration}"));
            let permit = openlife_core::agent::CanonicalWriteAdmission::acquire(
                &epoch,
                openlife_core::agent::CanonicalWriteAdmissionRequest::new(
                    "proposal",
                    format!("proposal:{}", proposal.id),
                ),
            )
            .expect("proposal permit must linearize before cancellation");
            let barrier = Arc::new(Barrier::new(2));
            let cancel_barrier = Arc::clone(&barrier);
            let cancel_registry = registry.clone();
            let cancel_task_id = task_id.clone();
            let canceller = std::thread::spawn(move || {
                cancel_barrier.wait();
                cancel_registry.request_cancel(&cancel_task_id)
            });

            barrier.wait();
            let outcome = openlife_core::agent::ReviewWorkflow::new(&proposal_store)
                .submit(
                    openlife_core::agent::DurableWriteRequest::from_agent_proposal(
                        openlife_core::agent::DurableWriteSource::ToolPermission,
                        openlife_core::agent::DurableWriteSubject::ToolPermission,
                        proposal,
                        "Tool permission proposal is pending Review Center approval.",
                    ),
                )
                .expect("the admitted Proposal transaction commits");
            permit.finish_committed();
            assert!(
                canceller
                    .join()
                    .expect("canceller joins")
                    .outcome
                    .active_turn_found,
                "iteration {iteration}: cancellation must observe the active turn"
            );

            assert_eq!(
                proposal_store.pending_count().expect("count proposals"),
                1,
                "iteration {iteration}: commit winner persists exactly one Proposal"
            );
            assert!(
                proposal_store
                    .get_proposal(outcome.proposal_id())
                    .expect("read committed proposal")
                    .is_some(),
                "iteration {iteration}: the committed proposal identity remains observable"
            );
            assert_eq!(
                epoch.snapshot().cancellation_terminal_disposition(),
                MainChatCancellationTerminalDisposition::InterruptedAfterCommittedEffect,
                "iteration {iteration}: commit winner cannot be relabelled pure cancelled"
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancel_winner_blocks_real_tool_gateway_network_consent_proposal() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-tool-gateway-proposal-cancel-wins");
        let epoch = registration.execution_epoch();
        registry.request_cancel("task-tool-gateway-proposal-cancel-wins");

        let tool_registry = openlife_core::mcp::McpRegistry::new();
        let permission_store =
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let proposal_store = openlife_core::agent::ProposalStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = openlife_core::mcp_audit::McpAuditStore::new(audit_file.path());
        let privacy_engine = openlife_core::privacy::PrivacyEngine::new();
        let network_policy = openlife_core::config::NetworkPolicy::default();
        let context = openlife_core::agent::ActionExecutionContext::new(
            &tool_registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("must remain behind admission")
        .with_proposal_store(&proposal_store)
        .with_canonical_write_admission(&epoch);

        let result = openlife_core::agent::ToolGateway::from_executor_config(
            openlife_core::agent::ActionExecutorConfig::default(),
        )
        .execute(
            openlife_core::agent::AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "web.search".into(),
                input: serde_json::json!({"arguments": {"query": "OpenLife"}}),
                source_run_id: Some("run-tool-gateway-proposal-cancel-wins".into()),
                step_index: 0,
            },
            &context,
        )
        .await
        .expect("cancel rejection becomes a typed ToolGateway result");

        assert_eq!(
            result.status,
            openlife_core::agent::ActionExecutionStatus::Failed
        );
        assert_eq!(
            result.execution_receipt.transport_status,
            ToolTransportStatus::NotAttempted
        );
        assert_eq!(proposal_store.pending_count().expect("count proposals"), 0);
        let snapshot = epoch.snapshot();
        assert_eq!(
            snapshot.cancellation_terminal_disposition(),
            MainChatCancellationTerminalDisposition::Cancelled
        );
        assert_eq!(
            snapshot.commit_facts[0].outcome,
            MainChatCanonicalCommitOutcome::RejectedAfterCancel
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permit_winner_real_tool_gateway_proposal_commits_then_interrupts() {
        let registry = MainChatCancellationRegistry::default();
        let task_id = "task-tool-gateway-proposal-commit-wins";
        let registration = registry.register(task_id);
        let epoch = registration.execution_epoch();
        let acquired = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let admission = PauseAfterCanonicalAdmission {
            epoch: epoch.clone(),
            acquired: Arc::clone(&acquired),
            resume: Arc::clone(&resume),
        };
        let cancel_registry = registry.clone();
        let canceller = std::thread::spawn(move || {
            acquired.wait();
            let outcome = cancel_registry.request_cancel(task_id);
            resume.wait();
            outcome
        });

        let tool_registry = openlife_core::mcp::McpRegistry::new();
        let permission_store =
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let proposal_store = openlife_core::agent::ProposalStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = openlife_core::mcp_audit::McpAuditStore::new(audit_file.path());
        let privacy_engine = openlife_core::privacy::PrivacyEngine::new();
        let network_policy = openlife_core::config::NetworkPolicy::default();
        let context = openlife_core::agent::ActionExecutionContext::new(
            &tool_registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("must remain behind network consent")
        .with_proposal_store(&proposal_store)
        .with_canonical_write_admission(&admission);

        let result = openlife_core::agent::ToolGateway::from_executor_config(
            openlife_core::agent::ActionExecutorConfig::default(),
        )
        .execute(
            openlife_core::agent::AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "web.search".into(),
                input: serde_json::json!({"arguments": {"query": "OpenLife"}}),
                source_run_id: Some("run-tool-gateway-proposal-commit-wins".into()),
                step_index: 0,
            },
            &context,
        )
        .await
        .expect("admitted network consent proposal returns a typed result");
        assert!(
            canceller
                .join()
                .expect("canceller joins")
                .outcome
                .active_turn_found,
            "the cancellation must observe the same active execution owner"
        );

        assert_eq!(
            result.status,
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation
        );
        let proposals = proposal_store
            .list_pending_proposals(10)
            .expect("read committed Proposal");
        assert_eq!(proposals.len(), 1);
        assert_eq!(
            proposals[0].run_id.as_deref(),
            Some("run-tool-gateway-proposal-commit-wins"),
            "source run identity must be attached inside the admitted Proposal mutation"
        );
        let snapshot = epoch.snapshot();
        assert_eq!(snapshot.committed_fact_count(), 1);
        assert_eq!(
            snapshot.cancellation_terminal_disposition(),
            MainChatCancellationTerminalDisposition::InterruptedAfterCommittedEffect,
            "a committed Proposal must never be relabelled as pure cancelled"
        );
    }

    #[test]
    fn commit_winning_epoch_remains_committed_after_cancel_request() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-commit-wins-cancel");
        let epoch = registration.execution_epoch();
        let permit = epoch
            .begin_canonical_commit("life_model", "life-model:identity")
            .expect("commit permit begins before cancellation");

        registry.cancel("task-commit-wins-cancel");
        permit.finish_committed();

        let snapshot = epoch.snapshot();
        assert!(snapshot.cancel_requested);
        assert_eq!(snapshot.inflight_commit_count, 0);
        assert_eq!(snapshot.commit_facts.len(), 1);
        assert_eq!(snapshot.commit_facts[0].domain, "life_model");
        assert_eq!(snapshot.commit_facts[0].object_ref, "life-model:identity");
        assert_eq!(
            snapshot.commit_facts[0].outcome,
            MainChatCanonicalCommitOutcome::Committed
        );
    }

    #[test]
    fn unfinished_commit_permit_becomes_unknown_not_not_attempted() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-unfinished-commit");
        let epoch = registration.execution_epoch();
        let permit = epoch
            .begin_canonical_commit("memory", "memory:item-unknown")
            .expect("commit permit begins");

        drop(permit);

        let snapshot = epoch.snapshot();
        assert_eq!(snapshot.inflight_commit_count, 0);
        assert_eq!(snapshot.commit_facts.len(), 1);
        assert_eq!(
            snapshot.commit_facts[0].outcome,
            MainChatCanonicalCommitOutcome::Unknown
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn terminalization_waits_for_pre_cancel_commit_permits() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-terminal-waits");
        let epoch = registration.execution_epoch();
        let permit = epoch
            .begin_canonical_commit("memory", "memory:item-inflight")
            .expect("commit permit begins before cancellation");
        registry.cancel("task-terminal-waits");

        let waiter_epoch = epoch.clone();
        let waiter = tokio::spawn(async move { waiter_epoch.wait_for_inflight_commits().await });
        tokio::task::yield_now().await;
        assert!(
            !waiter.is_finished(),
            "terminalization cannot pass an in-flight pre-cancel commit permit"
        );

        permit.finish_committed();
        let snapshot = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("terminalization wakes after the permit is finished")
            .expect("waiter task joins");
        assert_eq!(snapshot.inflight_commit_count, 0);
        assert_eq!(
            snapshot.commit_facts[0].outcome,
            MainChatCanonicalCommitOutcome::Committed
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancel_commit_barrier_cannot_publish_cancelled_after_a_commit_wins() {
        use super::MainChatCancellationTerminalDisposition;

        for iteration in 0..64 {
            let registry = MainChatCancellationRegistry::default();
            let task_id = format!("task-cancel-commit-barrier-{iteration}");
            let registration = registry.register(&task_id);
            let epoch = registration.execution_epoch();
            let permit = epoch
                .begin_canonical_commit("memory", format!("memory:item-{iteration}"))
                .expect("commit begins before barrier");
            let barrier = Arc::new(tokio::sync::Barrier::new(2));
            let waiter_barrier = Arc::clone(&barrier);
            let waiter_epoch = epoch.clone();
            let waiter_registry = registry.clone();
            let waiter_task_id = task_id.clone();
            let terminalizer = tokio::spawn(async move {
                waiter_barrier.wait().await;
                waiter_registry.request_cancel(&waiter_task_id);
                waiter_epoch.wait_for_inflight_commits().await
            });

            barrier.wait().await;
            permit.finish_committed();

            let snapshot = tokio::time::timeout(Duration::from_secs(1), terminalizer)
                .await
                .expect("terminalizer must wake")
                .expect("terminalizer joins");
            assert_eq!(
                snapshot.cancellation_terminal_disposition(),
                MainChatCancellationTerminalDisposition::InterruptedAfterCommittedEffect,
                "iteration {iteration}: a committed effect must forbid a pure cancelled terminal"
            );
        }
    }

    #[test]
    fn cancellation_terminal_disposition_is_derived_from_settled_commit_facts() {
        use super::MainChatCancellationTerminalDisposition;

        let registry = MainChatCancellationRegistry::default();

        let pure = registry.register("task-pure-cancel-disposition");
        registry.cancel("task-pure-cancel-disposition");
        assert_eq!(
            pure.execution_epoch()
                .snapshot()
                .cancellation_terminal_disposition(),
            MainChatCancellationTerminalDisposition::Cancelled
        );

        let committed = registry.register("task-committed-cancel-disposition");
        let committed_epoch = committed.execution_epoch();
        let committed_permit = committed_epoch
            .begin_canonical_commit("memory", "memory:committed")
            .expect("begin committed effect");
        registry.cancel("task-committed-cancel-disposition");
        committed_permit.finish_committed();
        assert_eq!(
            committed_epoch
                .snapshot()
                .cancellation_terminal_disposition(),
            MainChatCancellationTerminalDisposition::InterruptedAfterCommittedEffect
        );

        let unknown = registry.register("task-unknown-cancel-disposition");
        let unknown_epoch = unknown.execution_epoch();
        let unknown_permit = unknown_epoch
            .begin_canonical_commit("proposal", "proposal:unknown")
            .expect("begin unknown effect");
        registry.cancel("task-unknown-cancel-disposition");
        drop(unknown_permit);
        assert_eq!(
            unknown_epoch.snapshot().cancellation_terminal_disposition(),
            MainChatCancellationTerminalDisposition::InterruptedWithUnknownEffect
        );
    }

    #[test]
    fn canonical_commit_fact_contract_rejects_prose_instead_of_storing_a_body() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-minimal-commit-facts");
        let epoch = registration.execution_epoch();

        let rejection = epoch
            .begin_canonical_commit(
                "memory",
                "this is user-authored prose and must not become a commit reference",
            )
            .expect_err("commit facts may contain an object reference, never a body");

        assert_eq!(
            rejection,
            MainChatCanonicalCommitRejection::InvalidObjectReference
        );
        assert!(epoch.snapshot().commit_facts.is_empty());
    }

    #[test]
    fn every_registration_gets_a_distinct_uuid_execution_id() {
        let registry = MainChatCancellationRegistry::default();
        let first = registry.register("task-execution-id-first");
        let second = registry.register("task-execution-id-second");

        assert_ne!(first.execution_id(), second.execution_id());
        assert!(uuid::Uuid::parse_str(first.execution_id()).is_ok());
        assert!(uuid::Uuid::parse_str(second.execution_id()).is_ok());
        assert_eq!(
            first.execution_id(),
            first.execution_epoch().snapshot().execution_id
        );
    }

    #[test]
    fn explicitly_failed_commit_records_failed_instead_of_unknown() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-failed-commit");
        let epoch = registration.execution_epoch();
        let permit = epoch
            .begin_canonical_commit("memory", "memory:item-failed")
            .expect("commit permit begins");

        permit.finish_failed();

        let snapshot = epoch.snapshot();
        assert_eq!(snapshot.commit_facts.len(), 1);
        assert_eq!(
            snapshot.commit_facts[0].outcome,
            MainChatCanonicalCommitOutcome::Failed
        );
    }

    #[test]
    fn cancellation_snapshot_preserves_terminal_attempts_and_only_marks_inflight_unknown() {
        use chrono::{Duration as ChronoDuration, TimeZone, Utc};

        let registry = MainChatCancellationRegistry::default();
        let _registration = registry.register("task-request-attempts");
        let ranking_started = Utc.timestamp_opt(1_720_000_000, 0).unwrap();
        let ranking_finished = ranking_started + ChronoDuration::milliseconds(40);
        let generation_started = ranking_finished + ChronoDuration::milliseconds(10);
        let failed_started = generation_started + ChronoDuration::milliseconds(1);
        let failed_finished = failed_started + ChronoDuration::milliseconds(5);

        registry
            .record_provider_started(
                "task-request-attempts",
                "request-ranking",
                "openai",
                "gpt-test",
                ranking_started,
                &test_provider_policy_evidence("request-ranking"),
            )
            .unwrap();
        registry
            .record_provider_completed(
                "task-request-attempts",
                "request-ranking",
                "openai",
                "gpt-test",
                ranking_finished,
            )
            .unwrap();
        registry
            .record_provider_started(
                "task-request-attempts",
                "request-generation",
                "openai",
                "gpt-test",
                generation_started,
                &test_provider_policy_evidence("request-generation"),
            )
            .unwrap();
        registry
            .record_provider_started(
                "task-request-attempts",
                "request-failed",
                "openai",
                "gpt-test",
                failed_started,
                &test_provider_policy_evidence("request-failed"),
            )
            .unwrap();
        registry
            .record_provider_failed(
                "task-request-attempts",
                "request-failed",
                "openai",
                "gpt-test",
                failed_finished,
                "hash:sha256:provider-failure",
            )
            .unwrap();

        let cancel_outcome = registry.cancel("task-request-attempts");
        let snapshot_fallback = Utc::now();
        let cancellation_snapshot = registry
            .snapshot_provider_attempts_for_cancel("task-request-attempts", snapshot_fallback)
            .unwrap();
        assert!(cancellation_snapshot.observed_at <= snapshot_fallback);
        let cancellation_observed_at = cancellation_snapshot.observed_at;
        let attempts = cancellation_snapshot.attempts;
        assert_eq!(attempts.len(), 3);
        assert_eq!(
            attempts[0].status,
            super::MainChatProviderAttemptStatus::Completed
        );
        assert_eq!(attempts[0].finished_at, Some(ranking_finished));
        assert_eq!(
            attempts[1].status,
            super::MainChatProviderAttemptStatus::RemoteUnknown
        );
        assert_eq!(attempts[1].finished_at, None);
        assert_eq!(attempts[1].observed_at, cancellation_observed_at);
        assert_eq!(
            attempts[1].policy_evidence,
            test_provider_policy_evidence("request-generation")
        );
        assert_eq!(
            attempts[2].status,
            super::MainChatProviderAttemptStatus::Failed
        );
        assert_eq!(attempts[2].finished_at, Some(failed_finished));
        assert_eq!(
            attempts[2].error_digest.as_deref(),
            Some("hash:sha256:provider-failure")
        );
        assert_eq!(cancel_outcome.provider_attempt_count, 3);
        assert_eq!(cancel_outcome.provider_terminal_count, 2);
        assert_eq!(cancel_outcome.provider_inflight_unknown_count, 1);
        assert!(cancel_outcome.provider_attempt_state_valid);
    }

    #[test]
    fn provider_terminal_observation_survives_wall_clock_rollback() {
        use chrono::{Duration as ChronoDuration, TimeZone, Utc};

        let registry = MainChatCancellationRegistry::default();
        let _registration = registry.register("task-provider-clock-rollback");
        let started_at = Utc.timestamp_opt(1_720_000_000, 0).unwrap();
        let finished_at = started_at - ChronoDuration::milliseconds(1);
        registry
            .record_provider_started(
                "task-provider-clock-rollback",
                "request-provider-clock-rollback",
                "openai",
                "gpt-test",
                started_at,
                &test_provider_policy_evidence("request-provider-clock-rollback"),
            )
            .expect("record typed provider start");

        let disposition = registry
            .record_provider_completed(
                "task-provider-clock-rollback",
                "request-provider-clock-rollback",
                "openai",
                "gpt-test",
                finished_at,
            )
            .expect("typed terminal observation is authoritative over wall-clock ordering");
        assert_eq!(
            disposition,
            super::MainChatProviderAttemptRecordDisposition::Recorded
        );

        let snapshot = registry
            .snapshot_provider_attempts_for_cancel(
                "task-provider-clock-rollback",
                started_at + ChronoDuration::milliseconds(1),
            )
            .expect("clock rollback must not poison provider attempt state");
        assert_eq!(snapshot.attempts.len(), 1);
        assert_eq!(
            snapshot.attempts[0].status,
            super::MainChatProviderAttemptStatus::Completed
        );
        assert_eq!(snapshot.attempts[0].finished_at, Some(finished_at));
    }

    #[test]
    fn provider_start_admission_linearizes_cancel_first_and_start_first() {
        use chrono::{TimeZone, Utc};

        let started_at = Utc.timestamp_opt(1_720_000_025, 0).unwrap();

        let cancel_first = MainChatCancellationRegistry::default();
        let _cancel_first_registration = cancel_first.register("task-provider-cancel-first");
        cancel_first.request_cancel("task-provider-cancel-first");
        assert_eq!(
            cancel_first
                .admit_provider_start(
                    "task-provider-cancel-first",
                    "request-provider-cancel-first",
                    "openai",
                    "gpt-test",
                    started_at,
                    &test_provider_policy_evidence("request-provider-cancel-first"),
                )
                .expect_err("cancel must reject the adapter-start edge before HTTP send"),
            super::MainChatProviderAttemptError::CancelRequested
        );
        assert!(cancel_first
            .snapshot_provider_attempts_for_cancel("task-provider-cancel-first", started_at,)
            .expect("cancel-first snapshot remains valid")
            .attempts
            .is_empty());

        let start_first = MainChatCancellationRegistry::default();
        let _start_first_registration = start_first.register("task-provider-start-first");
        assert_eq!(
            start_first
                .admit_provider_start(
                    "task-provider-start-first",
                    "request-provider-start-first",
                    "openai",
                    "gpt-test",
                    started_at,
                    &test_provider_policy_evidence("request-provider-start-first"),
                )
                .expect("start must win atomically before cancellation"),
            super::MainChatProviderAttemptRecordDisposition::Recorded
        );
        start_first.request_cancel("task-provider-start-first");
        let snapshot = start_first
            .snapshot_provider_attempts_for_cancel("task-provider-start-first", started_at)
            .expect("start-first snapshot remains valid");
        assert_eq!(snapshot.attempts.len(), 1);
        assert_eq!(
            snapshot.attempts[0].status,
            super::MainChatProviderAttemptStatus::RemoteUnknown
        );

        let duplicate = MainChatCancellationRegistry::default();
        let duplicate_registration = duplicate.register("task-provider-duplicate-start");
        duplicate
            .admit_provider_start(
                "task-provider-duplicate-start",
                "request-provider-duplicate-start",
                "openai",
                "gpt-test",
                started_at,
                &test_provider_policy_evidence("request-provider-duplicate-start"),
            )
            .expect("first physical dispatch admission succeeds");
        assert_eq!(
            duplicate
                .admit_provider_start(
                    "task-provider-duplicate-start",
                    "request-provider-duplicate-start",
                    "openai",
                    "gpt-test",
                    started_at,
                    &test_provider_policy_evidence("request-provider-duplicate-start"),
                )
                .expect_err("one request id cannot authorize a second physical dispatch"),
            super::MainChatProviderAttemptError::DuplicateStart
        );
        assert!(duplicate_registration.token.is_cancelled());
        let duplicate_snapshot = duplicate
            .snapshot_provider_attempts_for_cancel("task-provider-duplicate-start", started_at)
            .expect("duplicate rejection must preserve the first real adapter attempt");
        assert_eq!(duplicate_snapshot.attempts.len(), 1);
        assert_eq!(
            duplicate_snapshot.attempts[0].request_id,
            "request-provider-duplicate-start"
        );
        assert_eq!(
            duplicate_snapshot.attempts[0].status,
            super::MainChatProviderAttemptStatus::RemoteUnknown
        );
    }

    #[test]
    fn provider_start_with_unsafe_policy_evidence_poison_attempt_state() {
        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-invalid-provider-policy-evidence");
        let mut evidence = test_provider_policy_evidence("request-invalid-policy");
        evidence.raw_life_model_included = true;

        let error = registry
            .record_provider_started(
                "task-invalid-provider-policy-evidence",
                "request-invalid-policy",
                "openai",
                "gpt-test",
                chrono::Utc::now(),
                &evidence,
            )
            .expect_err("unsafe provider evidence must fail closed before attempt truth is stored");

        assert_eq!(error, super::MainChatProviderAttemptError::InvalidMetadata);
        assert!(registration.token.is_cancelled());
    }

    #[test]
    fn cancel_winning_provider_attempt_linearization_ignores_a_late_terminal() {
        use chrono::{Duration as ChronoDuration, TimeZone, Utc};

        let registry = MainChatCancellationRegistry::default();
        let _registration = registry.register("task-cancel-wins-provider-terminal");
        let started_at = Utc.timestamp_opt(1_720_000_050, 0).unwrap();
        registry
            .record_provider_started(
                "task-cancel-wins-provider-terminal",
                "request-race",
                "openai",
                "gpt-test",
                started_at,
                &test_provider_policy_evidence("request-race"),
            )
            .unwrap();

        registry.cancel("task-cancel-wins-provider-terminal");
        let disposition = registry
            .record_provider_completed(
                "task-cancel-wins-provider-terminal",
                "request-race",
                "openai",
                "gpt-test",
                started_at + ChronoDuration::milliseconds(10),
            )
            .expect("a late terminal is ignored without poisoning receipt state");
        assert_eq!(
            disposition,
            super::MainChatProviderAttemptRecordDisposition::IgnoredAfterCancel
        );

        let snapshot = registry
            .snapshot_provider_attempts_for_cancel("task-cancel-wins-provider-terminal", Utc::now())
            .unwrap();
        assert_eq!(snapshot.attempts.len(), 1);
        assert_eq!(
            snapshot.attempts[0].status,
            super::MainChatProviderAttemptStatus::RemoteUnknown
        );
        assert_eq!(snapshot.attempts[0].observed_at, snapshot.observed_at);
    }

    #[test]
    fn conflicting_metadata_for_one_request_id_poison_attempt_state_fail_closed() {
        use chrono::{Duration as ChronoDuration, TimeZone, Utc};

        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-request-conflict");
        let started_at = Utc.timestamp_opt(1_720_000_100, 0).unwrap();
        registry
            .record_provider_started(
                "task-request-conflict",
                "request-conflict",
                "openai",
                "gpt-truth",
                started_at,
                &test_provider_policy_evidence("request-conflict"),
            )
            .unwrap();

        let error = registry
            .record_provider_completed(
                "task-request-conflict",
                "request-conflict",
                "openai",
                "gpt-conflicting-model",
                started_at + ChronoDuration::milliseconds(10),
            )
            .expect_err("conflicting provider metadata must fail closed");
        assert_eq!(error, super::MainChatProviderAttemptError::MetadataConflict);
        assert!(registration.token.is_cancelled());
        assert_eq!(
            registry
                .snapshot_provider_attempts_for_cancel(
                    "task-request-conflict",
                    started_at + ChronoDuration::milliseconds(20),
                )
                .expect_err("a poisoned request state cannot be projected as completed"),
            super::MainChatProviderAttemptError::MetadataConflict
        );
    }

    #[test]
    fn provider_attempt_cap_fails_closed_instead_of_dropping_the_extra_request() {
        use chrono::{TimeZone, Utc};

        let registry = MainChatCancellationRegistry::default();
        let registration = registry.register("task-request-cap");
        let started_at = Utc.timestamp_opt(1_720_000_200, 0).unwrap();
        for index in 0..super::MAX_PROVIDER_ATTEMPTS_PER_TURN {
            let request_id = format!("request-{index}");
            registry
                .record_provider_started(
                    "task-request-cap",
                    &request_id,
                    "openai",
                    "gpt-test",
                    started_at,
                    &test_provider_policy_evidence(&request_id),
                )
                .expect("attempt within cap is recorded");
        }

        assert_eq!(
            registry
                .record_provider_started(
                    "task-request-cap",
                    "request-over-cap",
                    "openai",
                    "gpt-test",
                    started_at,
                    &test_provider_policy_evidence("request-over-cap"),
                )
                .expect_err("attempt over cap must fail closed"),
            super::MainChatProviderAttemptError::CapacityExceeded
        );
        assert!(
            registration.token.is_cancelled(),
            "capacity overflow must abort the turn instead of dropping an attempt"
        );
        assert!(registry
            .snapshot_provider_attempts_for_cancel("task-request-cap", started_at)
            .is_err());
    }
}
