use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::tool_manifest::ToolIdempotencyContract;

/// Typed effect semantics come from a manifest or an internal gateway contract.
/// `Unknown` is deliberately fail-closed and never grants automatic retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolActionEffect {
    ReadOnly,
    LocalMutation,
    ExternalMutation,
    ProposalOnly,
    Unknown,
}

impl ToolActionEffect {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::LocalMutation => "local_mutation",
            Self::ExternalMutation => "external_mutation",
            Self::ProposalOnly => "proposal_only",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_contract(action_type: &str, capabilities: &[String]) -> Self {
        let action_type = action_type.trim().to_ascii_lowercase();
        let has_capability = |candidate: &str| {
            capabilities
                .iter()
                .any(|capability| capability.trim().eq_ignore_ascii_case(candidate))
        };

        if action_type == "proposal_only_write" {
            return Self::ProposalOnly;
        }
        if action_type == "external_side_effect" || has_capability("external_side_effect") {
            return Self::ExternalMutation;
        }
        if action_type == "write" || has_capability("write") {
            return Self::LocalMutation;
        }
        if matches!(action_type.as_str(), "read" | "network") {
            return Self::ReadOnly;
        }
        Self::Unknown
    }

    fn may_create_effect(self) -> bool {
        matches!(
            self,
            Self::LocalMutation | Self::ExternalMutation | Self::ProposalOnly | Self::Unknown
        )
    }

    fn can_confirm_effect(self) -> bool {
        matches!(
            self,
            Self::LocalMutation | Self::ExternalMutation | Self::ProposalOnly
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolTransportStatus {
    NotAttempted,
    Dispatched,
    ResponseObserved,
    LocalAborted,
    RemoteUnknown,
}

/// The concrete execution boundary crossed by a tool invocation.
///
/// Transport status alone cannot distinguish a cancelled local file read from
/// a network or MCP request that may continue after the local future is
/// dropped. This category is therefore durable receipt truth, not UI metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolDispatchKind {
    #[default]
    NotAttempted,
    Local,
    Network,
    McpStdio,
    A2a,
    Simulated,
    Unknown,
}

impl ToolDispatchKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::Local => "local",
            Self::Network => "network",
            Self::McpStdio => "mcp_stdio",
            Self::A2a => "a2a",
            Self::Simulated => "simulated",
            Self::Unknown => "unknown",
        }
    }

    fn may_outlive_local_wait(self) -> bool {
        matches!(
            self,
            Self::Network | Self::McpStdio | Self::A2a | Self::Unknown
        )
    }
}

impl ToolTransportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::Dispatched => "dispatched",
            Self::ResponseObserved => "response_observed",
            Self::LocalAborted => "local_aborted",
            Self::RemoteUnknown => "remote_unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffectStatus {
    NotAttempted,
    Confirmed,
    Unknown,
}

/// Adapter-observed execution outcome. Transport completion and effect
/// certainty are intentionally separate: a remote endpoint can return a
/// definite failure while a mutation's effect remains unknown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionOutcome {
    #[default]
    NotObserved,
    Succeeded,
    Failed,
    Unknown,
}

/// Durable audit persistence is a separate fact from the tool's transport,
/// effect, and execution outcome. A failed audit commit must never erase an
/// already-observed tool effect, while a pending commit must never be exposed
/// as a mechanically terminal receipt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAuditPersistenceStatus {
    /// This execution path does not use the MCP audit store. Pre-dispatch
    /// blockers and gateway-owned internal reads remain in this state.
    NotRequired,
    /// A manifest-backed adapter is running or has returned, but the one audit
    /// insert has not yet reached a definite result.
    Pending,
    /// The minimized audit receipt was durably inserted exactly once.
    Committed,
    /// The insert returned a definite failure.
    Failed,
    /// The runtime cannot prove whether the insert committed. Historical
    /// receipts also deserialize to this fail-closed state.
    #[default]
    Unknown,
}

impl ToolAuditPersistenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Pending => "pending",
            Self::Committed => "committed",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

/// Non-serializable provenance proving that this receipt came from the live
/// ToolGateway tracker in the current process. Structural receipt fields are
/// intentionally serializable for events and diagnostics, but deserializing
/// those fields must never recreate replay or success authorization.
struct ToolReceiptRuntimeSeal {
    _nonce: uuid::Uuid,
    structural_digest: String,
    action_binding_digest: Mutex<Option<String>>,
}

#[derive(Clone, Default)]
struct ToolReceiptRuntimeAuthenticity(Option<Arc<ToolReceiptRuntimeSeal>>);

impl ToolReceiptRuntimeAuthenticity {
    fn issued(structural_digest: String, action_binding_digest: Option<String>) -> Self {
        Self(Some(Arc::new(ToolReceiptRuntimeSeal {
            _nonce: uuid::Uuid::new_v4(),
            structural_digest,
            action_binding_digest: Mutex::new(action_binding_digest),
        })))
    }

    fn matches(&self, structural_digest: &str) -> bool {
        self.0
            .as_ref()
            .is_some_and(|seal| seal.structural_digest == structural_digest)
    }

    #[cfg(any(test, feature = "test-utils"))]
    fn bind_action(&self, action_binding_digest: String) -> bool {
        let Some(seal) = self.0.as_ref() else {
            return false;
        };
        let Ok(mut current) = seal.action_binding_digest.lock() else {
            return false;
        };
        if current
            .as_ref()
            .is_some_and(|existing| existing != &action_binding_digest)
        {
            return false;
        }
        *current = Some(action_binding_digest);
        true
    }

    fn action_binding_matches(&self, action_binding_digest: &str) -> bool {
        self.0
            .as_ref()
            .and_then(|seal| seal.action_binding_digest.lock().ok())
            .is_some_and(|binding| binding.as_deref() == Some(action_binding_digest))
    }
}

impl PartialEq for ToolReceiptRuntimeAuthenticity {
    fn eq(&self, other: &Self) -> bool {
        self.0.is_some() == other.0.is_some()
    }
}

impl Eq for ToolReceiptRuntimeAuthenticity {}

impl fmt::Debug for ToolReceiptRuntimeAuthenticity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolReceiptRuntimeAuthenticity")
            .field("issued", &self.0.is_some())
            .finish()
    }
}

impl ToolExecutionOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotObserved => "not_observed",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

impl ToolEffectStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::Confirmed => "confirmed",
            Self::Unknown => "unknown",
        }
    }
}

/// Minimal execution fact. It intentionally contains no arguments, output,
/// prompt, error body, or tool payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ToolExecutionReceipt {
    pub receipt_id: String,
    pub source_run_id: Option<String>,
    pub manifest_id: Option<String>,
    pub request_digest: String,
    pub action_effect: ToolActionEffect,
    pub idempotency_contract: ToolIdempotencyContract,
    #[serde(default)]
    pub dispatch_kind: ToolDispatchKind,
    #[serde(default)]
    pub dispatch_attempt_count: u32,
    /// True only after the adapter itself exposed a concrete dispatch edge:
    /// response headers for HTTP, the accepted MCP frame delimiter, or return
    /// from a local callback. Merely entering `.send()` is an ambiguous attempt.
    #[serde(default)]
    pub dispatch_observed: bool,
    pub transport_status: ToolTransportStatus,
    pub effect_status: ToolEffectStatus,
    #[serde(default)]
    pub execution_outcome: ToolExecutionOutcome,
    #[serde(default)]
    pub audit_persistence_status: ToolAuditPersistenceStatus,
    pub started_at: DateTime<Utc>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub response_observed_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    /// Opaque binding to the exact AgentAction produced by ToolGateway. The
    /// digest includes receipt-scoped randomness plus immutable action
    /// identity, so two receipts from the same run/manifest are not
    /// interchangeable. Older/serde-shaped receipts deliberately have no
    /// binding and cannot authorize replay.
    #[serde(skip)]
    action_binding_digest: Option<String>,
    #[serde(skip)]
    runtime_authenticity: ToolReceiptRuntimeAuthenticity,
}

impl ToolExecutionReceipt {
    /// A caller-visible factory for a definite failure that occurred before
    /// any adapter dispatch boundary. It cannot manufacture success or an
    /// observed remote transition.
    pub fn failed_before_dispatch(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
        action_effect: ToolActionEffect,
        idempotency_contract: ToolIdempotencyContract,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            action_effect,
            idempotency_contract,
        );
        tracker.mark_execution_failed();
        tracker.finish();
        let mut receipt = tracker.snapshot();
        // Orchestration layers may use this factory to render a structurally
        // honest pre-dispatch blocker. They are not the ToolGateway execution
        // boundary, so caller-supplied fields must not mint replay authority.
        receipt.runtime_authenticity = ToolReceiptRuntimeAuthenticity::default();
        receipt
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_gateway_failed_before_dispatch(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
        action_effect: ToolActionEffect,
        idempotency_contract: ToolIdempotencyContract,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            action_effect,
            idempotency_contract,
        );
        tracker.mark_execution_failed();
        tracker.finish();
        tracker.snapshot()
    }

    /// Test-only proof fixture for a fully observed, idempotent MCP read. The
    /// production API intentionally has no equivalent arbitrary-state builder.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_observed_mcp_read(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_mcp_dispatched();
        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.finish();
        tracker.snapshot()
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_observed_local_read(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
        succeeded: bool,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_local_dispatched();
        tracker.mark_response_observed();
        if succeeded {
            tracker.mark_execution_succeeded();
        } else {
            tracker.mark_execution_failed();
        }
        tracker.finish();
        tracker.snapshot()
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_remote_unknown(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
        action_effect: ToolActionEffect,
        idempotency_contract: ToolIdempotencyContract,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            action_effect,
            idempotency_contract,
        );
        tracker.mark_network_dispatched();
        tracker.mark_remote_unknown();
        tracker.finish();
        tracker.snapshot()
    }

    /// Test-only terminal for an HTTP attempt whose response-header edge was
    /// never observed. This must project as dispatch-ambiguous, never as a
    /// concrete `tool.started` fact.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_ambiguous_network_attempt(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
        action_effect: ToolActionEffect,
        idempotency_contract: ToolIdempotencyContract,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            action_effect,
            idempotency_contract,
        );
        tracker.mark_network_dispatch_attempted();
        tracker.mark_local_aborted();
        tracker.finish();
        tracker.snapshot()
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_observed_local_mutation_failure(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::LocalMutation,
            ToolIdempotencyContract::NonIdempotent,
        );
        tracker.mark_local_dispatched();
        tracker.mark_response_observed();
        tracker.mark_execution_failed();
        tracker.mark_effect_unknown_if_dispatched();
        tracker.finish();
        tracker.snapshot()
    }

    /// Test-only counterfactual for a successful adapter response whose
    /// declared action effect is itself unknown. A successful response cannot
    /// be promoted to a confirmed side effect in that state.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_observed_unknown_effect_succeeded(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::Unknown,
            ToolIdempotencyContract::Unspecified,
        );
        tracker.mark_local_dispatched();
        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.mark_effect_confirmed();
        tracker.finish();
        tracker.snapshot()
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_bound_to_action(
        self,
        run_id: &str,
        action_id: &str,
        action_type: &str,
        target: Option<&str>,
        input: &serde_json::Value,
    ) -> Self {
        assert!(self.test_bind_to_action(run_id, action_id, action_type, target, input,));
        self
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_bind_to_action(
        &self,
        run_id: &str,
        action_id: &str,
        action_type: &str,
        target: Option<&str>,
        input: &serde_json::Value,
    ) -> bool {
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(input);
        self.test_bind_to_action_metadata(
            run_id,
            action_id,
            action_type,
            target,
            &input_hash,
            input_length_bytes as u64,
        )
    }

    /// Test-only equivalent of the runtime action-binding boundary for
    /// fixtures that intentionally persist only an input digest and length.
    /// Production dependency graphs must not enable `test-utils`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_bind_to_action_metadata(
        &self,
        run_id: &str,
        action_id: &str,
        action_type: &str,
        target: Option<&str>,
        input_hash: &str,
        input_length_bytes: u64,
    ) -> bool {
        assert_eq!(self.source_run_id.as_deref(), Some(run_id));
        let binding = self.action_binding_digest_for(
            action_id,
            action_type,
            target,
            input_hash,
            input_length_bytes,
        );
        self.runtime_authenticity.bind_action(binding)
    }

    pub fn automatic_retry_safe(&self) -> bool {
        self.is_runtime_issued()
            && self.mechanically_valid_terminal().is_ok()
            && self.idempotency_contract == ToolIdempotencyContract::Idempotent
            && self.action_effect != ToolActionEffect::Unknown
            && self.dispatch_kind == ToolDispatchKind::NotAttempted
            && self.dispatch_attempt_count == 0
            && !self.dispatch_observed
            && self.transport_status == ToolTransportStatus::NotAttempted
            && self.effect_status == ToolEffectStatus::NotAttempted
            && self.dispatched_at.is_none()
            && self.response_observed_at.is_none()
    }

    /// Proves that the live ToolGateway receipt reached a terminal state
    /// without crossing any adapter attempt boundary. Unlike
    /// `automatic_retry_safe`, this fact is independent of retry policy: it is
    /// the execution truth used to close a durable `tool.dispatch_prepared`
    /// fence. Serde-shaped or caller-constructed receipts intentionally lose
    /// the process-local seal and cannot satisfy this proof.
    pub fn proves_not_dispatched(&self) -> bool {
        self.is_runtime_issued()
            && self.mechanically_valid_terminal().is_ok()
            && self.dispatch_kind == ToolDispatchKind::NotAttempted
            && self.dispatch_attempt_count == 0
            && !self.dispatch_observed
            && self.transport_status == ToolTransportStatus::NotAttempted
            && self.effect_status == ToolEffectStatus::NotAttempted
            && matches!(
                self.execution_outcome,
                ToolExecutionOutcome::NotObserved | ToolExecutionOutcome::Failed
            )
            && self.dispatched_at.is_none()
            && self.response_observed_at.is_none()
    }

    pub fn proves_success(&self) -> bool {
        self.is_runtime_issued()
            && self.mechanically_valid_terminal().is_ok()
            && self.transport_status == ToolTransportStatus::ResponseObserved
            && self.execution_outcome == ToolExecutionOutcome::Succeeded
            && match self.action_effect {
                ToolActionEffect::ReadOnly => self.effect_status == ToolEffectStatus::NotAttempted,
                ToolActionEffect::LocalMutation
                | ToolActionEffect::ExternalMutation
                | ToolActionEffect::ProposalOnly => {
                    self.effect_status == ToolEffectStatus::Confirmed
                }
                ToolActionEffect::Unknown => false,
            }
    }

    /// Validate the minimal state-machine facts required of a returned or
    /// durably projected terminal receipt. This deliberately rejects a
    /// finished `Dispatched` state: a caller must settle it as local-aborted or
    /// remote-unknown instead of hiding uncertainty behind a generic failure.
    pub fn mechanically_valid_terminal(&self) -> Result<(), &'static str> {
        let Some(finished_at) = self.finished_at else {
            return Err("tool_receipt_terminal_finished_at_missing");
        };
        if self.audit_persistence_status == ToolAuditPersistenceStatus::Pending {
            return Err("tool_receipt_terminal_audit_persistence_pending");
        }
        if uuid::Uuid::parse_str(&self.receipt_id).is_err() {
            return Err("tool_receipt_id_invalid");
        }
        if self.request_digest.trim().is_empty() {
            return Err("tool_receipt_request_digest_missing");
        }
        if self.started_at > finished_at
            || self.dispatched_at.is_some_and(|dispatched_at| {
                dispatched_at < self.started_at || dispatched_at > finished_at
            })
            || self.response_observed_at.is_some_and(|response_at| {
                response_at < self.started_at
                    || response_at > finished_at
                    || self
                        .dispatched_at
                        .is_none_or(|dispatched_at| response_at < dispatched_at)
            })
        {
            return Err("tool_receipt_timestamp_order_invalid");
        }

        match self.dispatch_kind {
            ToolDispatchKind::NotAttempted => {
                if self.dispatch_attempt_count != 0
                    || self.dispatch_observed
                    || self.dispatched_at.is_some()
                {
                    return Err("tool_receipt_not_attempted_has_dispatch_fact");
                }
            }
            ToolDispatchKind::Unknown => {
                return Err("tool_receipt_dispatch_kind_unknown");
            }
            _ => {
                if self.dispatch_attempt_count == 0
                    || self.dispatch_observed != self.dispatched_at.is_some()
                {
                    return Err("tool_receipt_dispatch_kind_missing_attempt_fact");
                }
            }
        }

        match self.transport_status {
            ToolTransportStatus::NotAttempted => {
                if self.dispatch_kind != ToolDispatchKind::NotAttempted
                    || self.response_observed_at.is_some()
                {
                    return Err("tool_receipt_not_attempted_transport_conflict");
                }
                if !matches!(
                    self.execution_outcome,
                    ToolExecutionOutcome::NotObserved | ToolExecutionOutcome::Failed
                ) {
                    return Err("tool_receipt_not_attempted_outcome_conflict");
                }
            }
            ToolTransportStatus::Dispatched => {
                return Err("tool_receipt_dispatch_has_no_terminal_certainty");
            }
            ToolTransportStatus::ResponseObserved => {
                if self.dispatch_kind == ToolDispatchKind::NotAttempted
                    || !self.dispatch_observed
                    || self.response_observed_at.is_none()
                {
                    return Err("tool_receipt_response_without_dispatch");
                }
                if !matches!(
                    self.execution_outcome,
                    ToolExecutionOutcome::Succeeded | ToolExecutionOutcome::Failed
                ) {
                    return Err("tool_receipt_response_outcome_missing");
                }
            }
            ToolTransportStatus::LocalAborted => {
                if self.dispatch_kind.may_outlive_local_wait()
                    && self.response_observed_at.is_none()
                    && self.dispatched_at.is_some()
                {
                    return Err("tool_receipt_remote_dispatch_mislabeled_local_abort");
                }
                if !matches!(
                    self.execution_outcome,
                    ToolExecutionOutcome::NotObserved | ToolExecutionOutcome::Unknown
                ) {
                    return Err("tool_receipt_local_abort_outcome_conflict");
                }
            }
            ToolTransportStatus::RemoteUnknown => {
                if !self.dispatch_kind.may_outlive_local_wait()
                    || self.dispatch_attempt_count == 0
                    || self.response_observed_at.is_some()
                {
                    return Err("tool_receipt_remote_unknown_boundary_invalid");
                }
                if self.execution_outcome != ToolExecutionOutcome::Unknown {
                    return Err("tool_receipt_remote_unknown_outcome_conflict");
                }
            }
        }

        match self.effect_status {
            ToolEffectStatus::NotAttempted => {
                if self.action_effect != ToolActionEffect::ReadOnly
                    && self.transport_status != ToolTransportStatus::NotAttempted
                    && self.transport_status != ToolTransportStatus::LocalAborted
                {
                    return Err("tool_receipt_mutation_dispatch_has_no_effect_certainty");
                }
            }
            ToolEffectStatus::Confirmed => {
                if !matches!(
                    self.action_effect,
                    ToolActionEffect::LocalMutation
                        | ToolActionEffect::ExternalMutation
                        | ToolActionEffect::ProposalOnly
                ) || self.transport_status != ToolTransportStatus::ResponseObserved
                {
                    return Err("tool_receipt_confirmed_effect_without_response");
                }
            }
            ToolEffectStatus::Unknown => {
                if !self.action_effect.may_create_effect() || self.dispatch_attempt_count == 0 {
                    return Err("tool_receipt_unknown_effect_without_mutation_dispatch");
                }
            }
        }
        Ok(())
    }

    /// Replay authority may only be projected from the exact live receipt
    /// returned by ToolGateway. Durable/display deserialization intentionally
    /// loses this marker and therefore cannot mint a new authorization.
    pub(crate) fn is_runtime_issued(&self) -> bool {
        let structural_digest = self.runtime_structural_digest();
        self.runtime_authenticity.matches(&structural_digest)
    }

    /// Proves that this live receipt belongs to the exact in-process action,
    /// not merely to another action in the same run or manifest.
    pub fn is_runtime_bound_to_action(
        &self,
        run_id: &str,
        action_id: &str,
        action_type: &str,
        target: Option<&str>,
        input: &serde_json::Value,
    ) -> bool {
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(input);
        self.is_runtime_bound_to_action_metadata(
            run_id,
            action_id,
            action_type,
            target,
            &input_hash,
            input_length_bytes as u64,
        )
    }

    pub(crate) fn is_runtime_bound_to_action_metadata(
        &self,
        run_id: &str,
        action_id: &str,
        action_type: &str,
        target: Option<&str>,
        input_hash: &str,
        input_length_bytes: u64,
    ) -> bool {
        if !self.is_runtime_issued() || self.source_run_id.as_deref() != Some(run_id) {
            return false;
        }
        self.runtime_authenticity
            .action_binding_matches(&self.action_binding_digest_for(
                action_id,
                action_type,
                target,
                input_hash,
                input_length_bytes,
            ))
    }

    fn action_binding_digest_for(
        &self,
        action_id: &str,
        action_type: &str,
        target: Option<&str>,
        input_hash: &str,
        input_length_bytes: u64,
    ) -> String {
        crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "domain": "openlife.tool_execution_receipt.action_binding.v1",
            "receiptId": &self.receipt_id,
            "requestDigest": &self.request_digest,
            "sourceRunId": &self.source_run_id,
            "manifestId": &self.manifest_id,
            "actionId": action_id,
            "actionType": action_type,
            "target": target,
            "inputHash": input_hash,
            "inputLengthBytes": input_length_bytes,
        }))
        .1
    }

    fn runtime_structural_digest(&self) -> String {
        crate::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::to_value(self).unwrap_or(serde_json::Value::Null),
        )
        .1
    }
}

/// Read authority plus the terminal settlement operations required by a
/// runtime owner after its adapter future has ended. Callers cannot construct
/// this registration or move a receipt into dispatched, successful,
/// response-observed, or confirmed-effect states; those transitions stay
/// inside concrete adapters.
#[derive(Debug, Clone)]
pub struct ToolExecutionReceiptRegistration {
    tracker: ToolExecutionReceiptTracker,
}

impl ToolExecutionReceiptRegistration {
    pub(crate) fn new(tracker: ToolExecutionReceiptTracker) -> Self {
        Self { tracker }
    }

    pub fn snapshot(&self) -> ToolExecutionReceipt {
        self.tracker.snapshot()
    }

    pub fn settle_after_local_abort(&self) -> ToolExecutionReceipt {
        self.tracker.settle_after_local_abort();
        self.tracker.mark_audit_persistence_unknown_if_pending();
        self.tracker.snapshot()
    }

    /// Settle an execution whose owning runtime failed after the registration
    /// was published. This differs from local cancellation: an adapter response
    /// already observed at the boundary is a definite failed execution, while a
    /// remote dispatch without a response remains unknown.
    pub fn settle_after_runtime_failure(&self) -> ToolExecutionReceipt {
        self.tracker.settle_failed_terminal();
        self.tracker.mark_audit_persistence_unknown_if_pending();
        self.tracker.snapshot()
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_observed_mcp_read(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_mcp_dispatched();
        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.finish();
        Self::new(tracker)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_inflight_network_mutation(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::ExternalMutation,
            ToolIdempotencyContract::NonIdempotent,
        );
        tracker.mark_network_dispatched();
        Self::new(tracker)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_response_observed_read_without_terminal(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_network_dispatched();
        tracker.mark_response_observed();
        Self::new(tracker)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_observed_external_mutation(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
    ) -> Self {
        let tracker = ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::ExternalMutation,
            ToolIdempotencyContract::NonIdempotent,
        );
        tracker.mark_network_dispatched();
        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.mark_effect_confirmed();
        tracker.finish();
        Self::new(tracker)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_never_dispatched_read(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
    ) -> Self {
        Self::new(ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        ))
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_never_dispatched_external_mutation(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
    ) -> Self {
        Self::new(ToolExecutionReceiptTracker::new(
            source_run_id,
            manifest_id,
            request_digest_material,
            ToolActionEffect::ExternalMutation,
            ToolIdempotencyContract::NonIdempotent,
        ))
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_mark_a2a_dispatch_attempted(&self) {
        self.tracker.mark_a2a_dispatch_attempted();
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_mark_a2a_response_observed(&self) {
        self.tracker.mark_a2a_dispatch_observed();
        self.tracker.mark_response_observed();
    }
}

/// Shared state survives a timed-out or dropped execution future, allowing the
/// gateway to project what was actually observed at the transport boundary.
#[derive(Debug, Clone)]
pub(crate) struct ToolExecutionReceiptTracker {
    inner: Arc<Mutex<ToolExecutionReceipt>>,
    started_transition_claimed: Arc<AtomicBool>,
}

impl ToolExecutionReceiptTracker {
    pub(crate) fn new(
        source_run_id: Option<String>,
        manifest_id: Option<String>,
        request_digest_material: String,
        action_effect: ToolActionEffect,
        idempotency_contract: ToolIdempotencyContract,
    ) -> Self {
        let receipt_id = uuid::Uuid::new_v4().to_string();
        // This nonce is deliberately never persisted. The resulting digest is
        // an opaque correlation token; a database reader who knows receipt_id
        // cannot enumerate low-entropy tool arguments by recomputing hashes.
        let digest_nonce = uuid::Uuid::new_v4().to_string();
        let request_digest =
            receipt_scoped_request_digest(&receipt_id, &digest_nonce, &request_digest_material);
        Self {
            inner: Arc::new(Mutex::new(ToolExecutionReceipt {
                receipt_id,
                source_run_id,
                manifest_id,
                request_digest,
                action_effect,
                idempotency_contract,
                dispatch_kind: ToolDispatchKind::NotAttempted,
                dispatch_attempt_count: 0,
                dispatch_observed: false,
                transport_status: ToolTransportStatus::NotAttempted,
                effect_status: ToolEffectStatus::NotAttempted,
                execution_outcome: ToolExecutionOutcome::NotObserved,
                audit_persistence_status: ToolAuditPersistenceStatus::NotRequired,
                started_at: Utc::now(),
                dispatched_at: None,
                response_observed_at: None,
                finished_at: None,
                action_binding_digest: None,
                runtime_authenticity: ToolReceiptRuntimeAuthenticity::default(),
            })),
            started_transition_claimed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn claim_first_concrete_dispatch_observation(&self) -> bool {
        self.snapshot().dispatch_observed
            && self
                .started_transition_claimed
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }

    pub(crate) fn bind_action_identity(
        &self,
        action_id: &str,
        action_type: &str,
        target: Option<&str>,
        input: &serde_json::Value,
    ) -> Result<(), &'static str> {
        if action_id.trim().is_empty() || action_type.trim().is_empty() {
            return Err("tool_receipt_action_binding_identity_missing");
        }
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(input);
        let mut receipt = self.lock();
        let binding = receipt.action_binding_digest_for(
            action_id,
            action_type,
            target,
            &input_hash,
            input_length_bytes as u64,
        );
        if receipt
            .action_binding_digest
            .as_ref()
            .is_some_and(|existing| existing != &binding)
        {
            return Err("tool_receipt_action_binding_conflict");
        }
        receipt.action_binding_digest = Some(binding);
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> ToolExecutionReceipt {
        let mut receipt = self.lock().clone();
        let structural_digest = receipt.runtime_structural_digest();
        receipt.runtime_authenticity = ToolReceiptRuntimeAuthenticity::issued(
            structural_digest,
            receipt.action_binding_digest.clone(),
        );
        receipt
    }

    pub(crate) fn mark_local_dispatched(&self) {
        self.mark_typed_dispatched(ToolDispatchKind::Local);
    }

    pub(crate) fn mark_local_dispatch_attempted(&self) {
        self.mark_typed_dispatch_attempt(ToolDispatchKind::Local);
    }

    pub(crate) fn mark_local_dispatch_observed(&self) {
        self.mark_typed_dispatch_observed(ToolDispatchKind::Local);
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub(crate) fn mark_network_dispatched(&self) {
        self.mark_typed_dispatched(ToolDispatchKind::Network);
    }

    pub(crate) fn mark_network_dispatch_attempted(&self) {
        self.mark_typed_dispatch_attempt(ToolDispatchKind::Network);
    }

    pub(crate) fn mark_network_dispatch_observed(&self) {
        self.mark_typed_dispatch_observed(ToolDispatchKind::Network);
    }

    pub(crate) fn mark_mcp_dispatched(&self) {
        self.mark_typed_dispatched(ToolDispatchKind::McpStdio);
    }

    #[cfg(test)]
    pub(crate) fn mark_a2a_dispatched(&self) {
        self.mark_typed_dispatched(ToolDispatchKind::A2a);
    }

    pub(crate) fn mark_a2a_dispatch_attempted(&self) {
        self.mark_typed_dispatch_attempt(ToolDispatchKind::A2a);
    }

    pub(crate) fn mark_a2a_dispatch_observed(&self) {
        self.mark_typed_dispatch_observed(ToolDispatchKind::A2a);
    }

    pub(crate) fn mark_simulated_dispatched(&self) {
        self.mark_typed_dispatched(ToolDispatchKind::Simulated);
    }

    fn mark_typed_dispatched(&self, dispatch_kind: ToolDispatchKind) {
        self.mark_typed_dispatch_attempt(dispatch_kind);
        self.mark_typed_dispatch_observed(dispatch_kind);
    }

    fn mark_typed_dispatch_attempt(&self, dispatch_kind: ToolDispatchKind) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_some() {
            return;
        }
        receipt.dispatch_kind = match receipt.dispatch_kind {
            ToolDispatchKind::NotAttempted => dispatch_kind,
            existing if existing == dispatch_kind => existing,
            _ => ToolDispatchKind::Unknown,
        };
        receipt.dispatch_attempt_count = receipt.dispatch_attempt_count.saturating_add(1);
        receipt.response_observed_at = None;
        receipt.transport_status = ToolTransportStatus::Dispatched;
    }

    fn mark_typed_dispatch_observed(&self, dispatch_kind: ToolDispatchKind) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_some() || receipt.dispatch_attempt_count == 0 {
            return;
        }
        receipt.dispatch_kind = match receipt.dispatch_kind {
            existing if existing == dispatch_kind => existing,
            _ => ToolDispatchKind::Unknown,
        };
        receipt.dispatch_observed = true;
        if receipt.dispatched_at.is_none() {
            receipt.dispatched_at = Some(Utc::now());
        }
        receipt.transport_status = ToolTransportStatus::Dispatched;
    }

    pub(crate) fn mark_response_observed(&self) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_some() {
            return;
        }
        if !receipt.dispatch_observed {
            return;
        }
        receipt.response_observed_at = Some(Utc::now());
        receipt.transport_status = ToolTransportStatus::ResponseObserved;
    }

    pub(crate) fn mark_remote_unknown(&self) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_some() {
            return;
        }
        if receipt.dispatch_attempt_count > 0 && receipt.dispatch_kind.may_outlive_local_wait() {
            receipt.transport_status = ToolTransportStatus::RemoteUnknown;
            receipt.execution_outcome = ToolExecutionOutcome::Unknown;
            if receipt.action_effect.may_create_effect()
                && receipt.effect_status != ToolEffectStatus::Confirmed
            {
                receipt.effect_status = ToolEffectStatus::Unknown;
            }
        } else {
            receipt.transport_status = ToolTransportStatus::LocalAborted;
            receipt.execution_outcome = ToolExecutionOutcome::Unknown;
        }
    }

    pub(crate) fn mark_effect_confirmed(&self) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_some() {
            return;
        }
        if receipt.action_effect.can_confirm_effect() {
            receipt.effect_status = ToolEffectStatus::Confirmed;
        } else if receipt.action_effect == ToolActionEffect::Unknown
            && receipt.dispatch_attempt_count > 0
        {
            receipt.effect_status = ToolEffectStatus::Unknown;
        }
    }

    pub(crate) fn mark_effect_unknown_if_dispatched(&self) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_some() {
            return;
        }
        if receipt.dispatch_attempt_count > 0
            && receipt.action_effect.may_create_effect()
            && receipt.effect_status != ToolEffectStatus::Confirmed
        {
            receipt.effect_status = ToolEffectStatus::Unknown;
        }
    }

    pub(crate) fn mark_execution_succeeded(&self) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_none() {
            receipt.execution_outcome = ToolExecutionOutcome::Succeeded;
        }
    }

    pub(crate) fn mark_execution_failed(&self) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_none() {
            receipt.execution_outcome = ToolExecutionOutcome::Failed;
        }
    }

    pub(crate) fn mark_audit_persistence_pending(&self) {
        let mut receipt = self.lock();
        if receipt.audit_persistence_status == ToolAuditPersistenceStatus::NotRequired {
            receipt.audit_persistence_status = ToolAuditPersistenceStatus::Pending;
        }
    }

    pub(crate) fn mark_audit_persistence_committed(&self) {
        let mut receipt = self.lock();
        if receipt.audit_persistence_status == ToolAuditPersistenceStatus::Pending {
            receipt.audit_persistence_status = ToolAuditPersistenceStatus::Committed;
        }
    }

    pub(crate) fn mark_audit_persistence_failed(&self) {
        let mut receipt = self.lock();
        if receipt.audit_persistence_status == ToolAuditPersistenceStatus::Pending {
            receipt.audit_persistence_status = ToolAuditPersistenceStatus::Failed;
        }
    }

    pub(crate) fn mark_audit_persistence_unknown_if_pending(&self) {
        let mut receipt = self.lock();
        if receipt.audit_persistence_status == ToolAuditPersistenceStatus::Pending {
            receipt.audit_persistence_status = ToolAuditPersistenceStatus::Unknown;
        }
    }

    pub(crate) fn mark_local_aborted(&self) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_some() {
            return;
        }
        receipt.transport_status = if receipt.dispatch_attempt_count > 0
            && receipt.response_observed_at.is_none()
            && receipt.dispatch_kind.may_outlive_local_wait()
        {
            // Dropping the local wait/request handle does not prove that a
            // dispatched remote operation stopped. Preserve that uncertainty
            // separately from a definitely pre-dispatch local abort.
            ToolTransportStatus::RemoteUnknown
        } else {
            ToolTransportStatus::LocalAborted
        };
        receipt.execution_outcome = ToolExecutionOutcome::Unknown;
        if receipt.dispatch_attempt_count > 0
            && receipt.action_effect.may_create_effect()
            && receipt.effect_status != ToolEffectStatus::Confirmed
        {
            receipt.effect_status = ToolEffectStatus::Unknown;
        }
    }

    pub(crate) fn finish(&self) {
        let mut receipt = self.lock();
        if receipt.finished_at.is_none() {
            receipt.finished_at = Some(Utc::now());
        }
    }

    pub(crate) fn settle_failed_terminal(&self) {
        let snapshot = self.snapshot();
        match snapshot.transport_status {
            ToolTransportStatus::Dispatched => {
                if snapshot.dispatch_kind.may_outlive_local_wait() {
                    self.mark_remote_unknown();
                } else {
                    self.mark_local_aborted();
                }
            }
            ToolTransportStatus::ResponseObserved => {
                self.mark_execution_failed();
                self.mark_effect_unknown_if_dispatched();
            }
            ToolTransportStatus::NotAttempted => self.mark_execution_failed(),
            ToolTransportStatus::LocalAborted | ToolTransportStatus::RemoteUnknown => {}
        }
        self.finish();
    }

    fn settle_after_local_abort(&self) {
        let snapshot = self.snapshot();
        if snapshot.finished_at.is_some() {
            return;
        }
        match snapshot.transport_status {
            ToolTransportStatus::NotAttempted => {}
            ToolTransportStatus::Dispatched | ToolTransportStatus::ResponseObserved => {
                self.mark_local_aborted();
            }
            ToolTransportStatus::LocalAborted | ToolTransportStatus::RemoteUnknown => {}
        }
        self.finish();
    }

    fn lock(&self) -> MutexGuard<'_, ToolExecutionReceipt> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn receipt_scoped_request_digest(
    receipt_id: &str,
    digest_nonce: &str,
    request_digest_material: &str,
) -> String {
    let material = format!(
        "openlife-tool-execution-receipt-v1\0{receipt_id}\0private-nonce\0{digest_nonce}\0canonical-request-digest\0{request_digest_material}"
    );
    crate::agent::metadata_safe::metadata_safe_text_digest(&material).1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatched_mutation_abort_is_unknown_and_not_retry_safe() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-1".into()),
            Some("manifest-1".into()),
            "digest-1".into(),
            ToolActionEffect::ExternalMutation,
            ToolIdempotencyContract::NonIdempotent,
        );
        tracker.mark_network_dispatched();
        tracker.mark_local_aborted();
        tracker.finish();

        let receipt = tracker.snapshot();
        assert_eq!(receipt.transport_status, ToolTransportStatus::RemoteUnknown);
        assert_eq!(receipt.effect_status, ToolEffectStatus::Unknown);
        assert!(!receipt.automatic_retry_safe());
    }

    #[test]
    fn dispatched_read_abort_is_not_automatically_retryable() {
        let tracker = ToolExecutionReceiptTracker::new(
            None,
            Some("manifest-with-write-looking-name".into()),
            "digest-2".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_network_dispatched();
        tracker.mark_local_aborted();

        assert_eq!(
            tracker.snapshot().transport_status,
            ToolTransportStatus::RemoteUnknown
        );
        assert!(
            !tracker.snapshot().automatic_retry_safe(),
            "crossing dispatch or observing local abort must fail closed even for a read-only tool"
        );
    }

    #[test]
    fn pre_dispatch_local_abort_does_not_claim_remote_unknown() {
        let tracker = ToolExecutionReceiptTracker::new(
            None,
            Some("manifest-pre-dispatch".into()),
            "digest-pre-dispatch".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_local_aborted();
        tracker.finish();

        let receipt = tracker.snapshot();
        assert_eq!(receipt.transport_status, ToolTransportStatus::LocalAborted);
        assert_eq!(receipt.effect_status, ToolEffectStatus::NotAttempted);
        assert!(receipt.dispatched_at.is_none());
        assert!(receipt.response_observed_at.is_none());
        assert!(!receipt.automatic_retry_safe());
    }

    #[test]
    fn finished_receipt_is_a_frozen_terminal_snapshot() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-terminal-freeze".into()),
            Some("manifest-terminal-freeze".into()),
            "digest-terminal-freeze".into(),
            ToolActionEffect::ExternalMutation,
            ToolIdempotencyContract::NonIdempotent,
        );
        tracker.mark_network_dispatched();
        tracker.mark_remote_unknown();
        tracker.finish();
        let terminal = tracker.snapshot();

        tracker.mark_local_dispatched();
        tracker.mark_mcp_dispatched();
        tracker.mark_a2a_dispatched();
        tracker.mark_simulated_dispatched();
        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.mark_execution_failed();
        tracker.mark_effect_confirmed();
        tracker.mark_effect_unknown_if_dispatched();
        tracker.mark_local_aborted();
        tracker.mark_remote_unknown();
        tracker.finish();

        assert_eq!(
            tracker.snapshot(),
            terminal,
            "late adapter callbacks must not rewrite a finished receipt"
        );
    }

    #[test]
    fn serialized_receipt_contains_only_refs_digest_category_status_and_time() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-3".into()),
            Some("manifest-3".into()),
            "sha256:request".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.finish();
        let value = serde_json::to_value(tracker.snapshot()).unwrap();
        let mut keys = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        keys.sort_unstable();

        assert_eq!(
            keys,
            vec![
                "actionEffect",
                "auditPersistenceStatus",
                "dispatchAttemptCount",
                "dispatchKind",
                "dispatchObserved",
                "dispatchedAt",
                "effectStatus",
                "executionOutcome",
                "finishedAt",
                "idempotencyContract",
                "manifestId",
                "receiptId",
                "requestDigest",
                "responseObservedAt",
                "sourceRunId",
                "startedAt",
                "transportStatus",
            ]
        );
    }

    #[test]
    fn audit_persistence_settles_after_adapter_finish_without_rewriting_tool_facts() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-audit-settlement".into()),
            Some("manifest-audit-settlement".into()),
            "digest-audit-settlement".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_audit_persistence_pending();
        tracker.mark_local_dispatched();
        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.finish();
        let adapter_finished = tracker.snapshot();
        assert_eq!(
            adapter_finished.audit_persistence_status,
            ToolAuditPersistenceStatus::Pending
        );
        assert_eq!(
            adapter_finished.mechanically_valid_terminal(),
            Err("tool_receipt_terminal_audit_persistence_pending")
        );

        tracker.mark_audit_persistence_committed();
        let terminal = tracker.snapshot();
        assert_eq!(
            terminal.audit_persistence_status,
            ToolAuditPersistenceStatus::Committed
        );
        assert_eq!(terminal.transport_status, adapter_finished.transport_status);
        assert_eq!(terminal.effect_status, adapter_finished.effect_status);
        assert_eq!(
            terminal.execution_outcome,
            adapter_finished.execution_outcome
        );
        assert_eq!(terminal.finished_at, adapter_finished.finished_at);
        assert!(terminal.mechanically_valid_terminal().is_ok());
        assert!(terminal.proves_success());
    }

    #[test]
    fn runtime_finalizers_settle_pending_audit_as_unknown_not_invalid_terminal() {
        for settle_after_runtime_failure in [false, true] {
            let tracker = ToolExecutionReceiptTracker::new(
                Some("run-audit-runtime-finalizer".into()),
                Some("manifest-audit-runtime-finalizer".into()),
                "digest-audit-runtime-finalizer".into(),
                ToolActionEffect::ReadOnly,
                ToolIdempotencyContract::Idempotent,
            );
            tracker.mark_local_dispatched();
            tracker.mark_audit_persistence_pending();
            tracker.mark_local_aborted();
            tracker.finish();
            let registration = ToolExecutionReceiptRegistration::new(tracker);

            let receipt = if settle_after_runtime_failure {
                registration.settle_after_runtime_failure()
            } else {
                registration.settle_after_local_abort()
            };

            assert_eq!(
                receipt.audit_persistence_status,
                ToolAuditPersistenceStatus::Unknown
            );
            assert!(receipt.mechanically_valid_terminal().is_ok());
        }
    }

    #[test]
    fn request_digest_is_opaque_even_for_identical_low_entropy_material() {
        let first = ToolExecutionReceiptTracker::new(
            Some("run-low-entropy".into()),
            Some("calendar.exists".into()),
            "sha256:public-candidate-yes".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        )
        .snapshot();
        let second = ToolExecutionReceiptTracker::new(
            Some("run-low-entropy".into()),
            Some("calendar.exists".into()),
            "sha256:public-candidate-yes".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        )
        .snapshot();

        assert_ne!(first.receipt_id, second.receipt_id);
        assert_ne!(first.request_digest, second.request_digest);
        assert_ne!(first.request_digest, "sha256:public-candidate-yes");
        assert_ne!(second.request_digest, "sha256:public-candidate-yes");
        assert_eq!(first.request_digest.len(), "sha256:".len() + 64);
        assert_eq!(second.request_digest.len(), "sha256:".len() + 64);
        let serialized = serde_json::to_string(&[first, second]).unwrap();
        assert!(!serialized.contains("private-nonce"));
        assert!(!serialized.contains("public-candidate-yes"));
    }

    #[test]
    fn unknown_effect_contract_never_becomes_confirmed() {
        let tracker = ToolExecutionReceiptTracker::new(
            None,
            None,
            "sha256:unknown".into(),
            ToolActionEffect::Unknown,
            ToolIdempotencyContract::Unspecified,
        );
        tracker.mark_network_dispatched();
        tracker.mark_response_observed();
        tracker.mark_effect_confirmed();

        assert_eq!(tracker.snapshot().effect_status, ToolEffectStatus::Unknown);
    }

    #[test]
    fn automatic_retry_requires_explicit_idempotency_and_definite_pre_dispatch_receipt() {
        let idempotent = ToolExecutionReceiptTracker::new(
            Some("run-idempotent".into()),
            Some("manifest-idempotent".into()),
            "request-idempotent".into(),
            ToolActionEffect::LocalMutation,
            ToolIdempotencyContract::Idempotent,
        );
        idempotent.finish();
        assert!(idempotent.snapshot().automatic_retry_safe());

        let non_idempotent = ToolExecutionReceiptTracker::new(
            Some("run-non-idempotent".into()),
            Some("manifest-non-idempotent".into()),
            "request-non-idempotent".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::NonIdempotent,
        );
        non_idempotent.finish();
        assert!(!non_idempotent.snapshot().automatic_retry_safe());

        let undeclared = ToolExecutionReceiptTracker::new(
            Some("run-undeclared".into()),
            Some("manifest-undeclared".into()),
            "request-undeclared".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Unspecified,
        );
        undeclared.finish();
        assert!(!undeclared.snapshot().automatic_retry_safe());
    }

    #[test]
    fn not_dispatched_proof_requires_the_live_seal_and_zero_attempts() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-live-not-dispatched".into()),
            Some("mcp:live-not-dispatched".into()),
            "request-live-not-dispatched".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_execution_failed();
        tracker.finish();
        let live = tracker.snapshot();
        assert!(live.proves_not_dispatched());

        let serialized = serde_json::to_vec(&live).unwrap();
        let restored: ToolExecutionReceipt = serde_json::from_slice(&serialized).unwrap();
        assert!(
            !restored.proves_not_dispatched(),
            "serialized structure must not recreate the process-local proof"
        );

        let attempted = ToolExecutionReceipt::test_ambiguous_network_attempt(
            Some("run-live-not-dispatched".into()),
            Some("mcp:live-not-dispatched".into()),
            "request-live-not-dispatched".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        assert!(!attempted.proves_not_dispatched());
    }

    #[test]
    fn local_dispatch_cancel_is_local_aborted_never_remote_unknown() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-local-cancel".into()),
            Some("file.read".into()),
            "local-read".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_local_dispatched();
        tracker.mark_local_aborted();
        tracker.finish();

        let receipt = tracker.snapshot();
        assert_eq!(receipt.dispatch_kind, ToolDispatchKind::Local);
        assert_eq!(receipt.dispatch_attempt_count, 1);
        assert_eq!(receipt.transport_status, ToolTransportStatus::LocalAborted);
        assert_eq!(receipt.effect_status, ToolEffectStatus::NotAttempted);
        assert!(receipt.mechanically_valid_terminal().is_ok());
        assert!(!receipt.automatic_retry_safe());
    }

    #[test]
    fn remote_dispatch_cancel_is_remote_unknown() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-network-cancel".into()),
            Some("web.fetch".into()),
            "network-read".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_network_dispatched();
        tracker.mark_local_aborted();
        tracker.finish();

        let receipt = tracker.snapshot();
        assert_eq!(receipt.dispatch_kind, ToolDispatchKind::Network);
        assert_eq!(receipt.transport_status, ToolTransportStatus::RemoteUnknown);
        assert!(receipt.mechanically_valid_terminal().is_ok());
    }

    #[test]
    fn simulated_fixture_is_typed_local_and_cannot_claim_network_dispatch() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-fixture".into()),
            Some("web.search".into()),
            "fixture-read".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_simulated_dispatched();
        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.finish();

        let receipt = tracker.snapshot();
        assert_eq!(receipt.dispatch_kind, ToolDispatchKind::Simulated);
        assert_eq!(receipt.dispatch_attempt_count, 1);
        assert_eq!(
            receipt.transport_status,
            ToolTransportStatus::ResponseObserved
        );
        assert!(receipt.mechanically_valid_terminal().is_ok());
        assert!(receipt.proves_success());
    }

    #[test]
    fn bounded_network_retries_are_counted_in_the_receipt() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-network-retry".into()),
            Some("web.search".into()),
            "network-retry".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_network_dispatched();
        tracker.mark_network_dispatched();
        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.finish();

        let receipt = tracker.snapshot();
        assert_eq!(receipt.dispatch_kind, ToolDispatchKind::Network);
        assert_eq!(receipt.dispatch_attempt_count, 2);
        assert!(receipt.mechanically_valid_terminal().is_ok());
    }

    #[test]
    fn finished_dispatched_without_terminal_certainty_is_rejected() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-invalid-terminal".into()),
            Some("web.fetch".into()),
            "invalid-terminal".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_network_dispatched();
        tracker.finish();

        let receipt = tracker.snapshot();
        assert_eq!(
            receipt.mechanically_valid_terminal(),
            Err("tool_receipt_dispatch_has_no_terminal_certainty")
        );
        assert!(!receipt.proves_success());
    }

    #[test]
    fn response_observed_without_typed_outcome_cannot_be_success() {
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-response-no-outcome".into()),
            Some("file.read".into()),
            "response-no-outcome".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        tracker.mark_local_dispatched();
        tracker.mark_response_observed();
        tracker.finish();
        let receipt = tracker.snapshot();
        assert_eq!(
            receipt.mechanically_valid_terminal(),
            Err("tool_receipt_response_outcome_missing")
        );
        assert!(!receipt.proves_success());
    }

    #[test]
    fn public_registration_surface_has_no_success_transition_methods() {
        let source = include_str!("tool_execution_receipt.rs");
        let registration = source
            .split("impl ToolExecutionReceiptRegistration")
            .nth(1)
            .and_then(|tail| tail.split("#[cfg(any(test").next())
            .expect("registration implementation source");
        assert!(!registration.contains("mark_execution_succeeded"));
        assert!(!registration.contains("mark_response_observed"));
        assert!(!registration.contains("mark_effect_confirmed"));
        assert!(source.contains("pub(crate) struct ToolExecutionReceiptTracker"));
    }

    #[test]
    fn serialized_receipt_cannot_recreate_runtime_success_or_retry_authority() {
        let pre_dispatch = ToolExecutionReceiptTracker::new(
            Some("run-pre-dispatch".into()),
            Some("manifest-pre-dispatch".into()),
            "pre-dispatch".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        pre_dispatch.finish();
        assert!(pre_dispatch.snapshot().automatic_retry_safe());

        let restored_pre_dispatch: ToolExecutionReceipt =
            serde_json::from_value(serde_json::to_value(pre_dispatch.snapshot()).unwrap()).unwrap();
        assert!(restored_pre_dispatch.mechanically_valid_terminal().is_ok());
        assert!(!restored_pre_dispatch.is_runtime_issued());
        assert!(!restored_pre_dispatch.automatic_retry_safe());

        let succeeded = ToolExecutionReceiptTracker::new(
            Some("run-success".into()),
            Some("manifest-success".into()),
            "success".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        succeeded.mark_local_dispatched();
        succeeded.mark_response_observed();
        succeeded.mark_execution_succeeded();
        succeeded.finish();
        assert!(succeeded.snapshot().proves_success());

        let restored_success: ToolExecutionReceipt =
            serde_json::from_value(serde_json::to_value(succeeded.snapshot()).unwrap()).unwrap();
        assert!(restored_success.mechanically_valid_terminal().is_ok());
        assert!(!restored_success.is_runtime_issued());
        assert!(!restored_success.proves_success());

        let mut field_mutated = succeeded.snapshot();
        field_mutated.execution_outcome = ToolExecutionOutcome::Failed;
        assert!(field_mutated.mechanically_valid_terminal().is_ok());
        assert!(!field_mutated.is_runtime_issued());
        assert!(!field_mutated.proves_success());

        let caller_declared = ToolExecutionReceipt::failed_before_dispatch(
            Some("run-caller-declared".into()),
            Some("manifest-caller-declared".into()),
            "caller-declared".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        assert!(caller_declared.mechanically_valid_terminal().is_ok());
        assert!(!caller_declared.is_runtime_issued());
        assert!(!caller_declared.automatic_retry_safe());
    }
}
