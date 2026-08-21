use crate::agent::ModelRouteTrace;
use crate::config::NetworkPolicy;
use crate::llm::{
    BoundedContextBlock, ChatMessage, ContextManifest, PreparedProviderOutcome,
    PreparedProviderRequest, ProviderDataRoute, ProviderInvocationReceipt,
    ProviderInvocationStatus, ProviderPayloadCategory, ProviderPayloadPurpose,
    ProviderPolicyAuthority, ProviderPolicyAuthorization, ProviderPolicyProvenanceKind,
    ProviderPolicyProvenanceRef, ProviderPolicyReceiptEvidence, StreamResult,
};
use crate::network_client::NetworkPolicyDecision;
use crate::ollama::{prepare_ollama_chat_target, resolve_ollama_model};
use crate::tasks::{ScheduledTaskClaim, TaskStore};
use anyhow::Result;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::agent::model_router::ModelRouter;

/// Adapter-edge lifecycle facts emitted synchronously with the underlying
/// provider operation. A start fact means the local client began a dispatch
/// attempt; it does not claim that the remote provider accepted or observed it.
/// A terminal fact retains the exact receipt for that request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderInvocationProgress {
    Started {
        request_id: String,
        provider: String,
        model: String,
        started_at: chrono::DateTime<chrono::Utc>,
        policy_evidence: ProviderPolicyReceiptEvidence,
    },
    Completed(ProviderInvocationReceipt),
    Failed(ProviderInvocationReceipt),
    RemoteUnknown(ProviderInvocationReceipt),
}

/// Non-serializable runtime authority for one exact prepared-provider
/// terminal. Public receipts remain portable metadata; only this proof can
/// authorize a provider-validation write.
#[derive(Clone)]
pub struct ProviderInvocationTerminalProof {
    receipt: ProviderInvocationReceipt,
    provider_endpoint: String,
    provider_config_generation: String,
    credential_identity: String,
    credential_version: u64,
    network_policy: NetworkPolicy,
    network_policy_decision: NetworkPolicyDecision,
    runtime_seal: ProviderInvocationTerminalSeal,
}

#[derive(Clone)]
struct ProviderInvocationTerminalSeal {
    issuance_id: uuid::Uuid,
    origin: ProviderInvocationTerminalOrigin,
}

#[derive(Clone, Copy)]
enum ProviderInvocationTerminalOrigin {
    RuntimeAdapter,
    #[cfg(feature = "test-utils")]
    SyntheticTestFixture,
}

impl std::fmt::Debug for ProviderInvocationTerminalProof {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderInvocationTerminalProof")
            .field("request_id", &self.receipt.request_id)
            .field("provider", &self.receipt.provider)
            .field("model", &self.receipt.model)
            .field("status", &self.receipt.status)
            .field(
                "provider_config_generation",
                &self.provider_config_generation,
            )
            .field("credential_identity", &"[REDACTED]")
            .field("credential_version", &self.credential_version)
            .field(
                "network_policy_decision_id",
                &self.network_policy_decision.decision_id,
            )
            .finish()
    }
}

impl ProviderInvocationTerminalProof {
    pub fn receipt(&self) -> &ProviderInvocationReceipt {
        &self.receipt
    }

    pub fn is_runtime_adapter_terminal(&self) -> bool {
        matches!(
            self.runtime_seal.origin,
            ProviderInvocationTerminalOrigin::RuntimeAdapter
        ) && !self.runtime_seal.issuance_id.is_nil()
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub(crate) fn reconciliation_source_id(&self) -> Result<String> {
        if !self.is_runtime_adapter_terminal() {
            anyhow::bail!("provider reconciliation source was not issued by a runtime adapter");
        }
        Ok(
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "schema": "provider_runtime_terminal_reconciliation_source_v1",
                "issuanceId": self.runtime_seal.issuance_id,
                "requestId": self.receipt.request_id,
                "provider": self.receipt.provider,
                "model": self.receipt.model,
                "status": self.receipt.status,
                "startedAt": self.receipt.started_at.to_rfc3339(),
                "finishedAt": self.receipt.finished_at.to_rfc3339(),
                "errorDigest": self.receipt.error_digest,
            }))
            .1,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub fn validate_runtime_binding(
        &self,
        provider_target: &str,
        model_target: &str,
        provider_endpoint: &str,
        provider_config_generation: &str,
        credential_identity: &str,
        credential_version: u64,
        network_policy: &NetworkPolicy,
        network_policy_decision: &NetworkPolicyDecision,
    ) -> Result<()> {
        if !self.is_runtime_adapter_terminal() {
            anyhow::bail!("provider terminal proof was not issued by the runtime adapter");
        }
        self.validate_binding(
            provider_target,
            model_target,
            provider_endpoint,
            provider_config_generation,
            credential_identity,
            credential_version,
            network_policy,
            network_policy_decision,
        )
    }

    #[cfg(feature = "test-utils")]
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub fn validate_synthetic_test_binding(
        &self,
        provider_target: &str,
        model_target: &str,
        provider_endpoint: &str,
        provider_config_generation: &str,
        credential_identity: &str,
        credential_version: u64,
        network_policy: &NetworkPolicy,
        network_policy_decision: &NetworkPolicyDecision,
    ) -> Result<()> {
        if !matches!(
            self.runtime_seal.origin,
            ProviderInvocationTerminalOrigin::SyntheticTestFixture
        ) || self.runtime_seal.issuance_id.is_nil()
        {
            anyhow::bail!("provider terminal proof is not a synthetic test fixture");
        }
        self.validate_binding(
            provider_target,
            model_target,
            provider_endpoint,
            provider_config_generation,
            credential_identity,
            credential_version,
            network_policy,
            network_policy_decision,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    fn validate_binding(
        &self,
        provider_target: &str,
        model_target: &str,
        provider_endpoint: &str,
        provider_config_generation: &str,
        credential_identity: &str,
        credential_version: u64,
        network_policy: &NetworkPolicy,
        network_policy_decision: &NetworkPolicyDecision,
    ) -> Result<()> {
        let evidence = self
            .receipt
            .policy_evidence
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("provider terminal proof lost policy evidence"))?;
        if self.receipt.provider != provider_target.trim().to_ascii_lowercase()
            || self.receipt.model != model_target.trim()
            || self.provider_endpoint != provider_endpoint
            || self.provider_config_generation != provider_config_generation
            || self.credential_identity != credential_identity
            || self.credential_version != credential_version
            || &self.network_policy != network_policy
            || &self.network_policy_decision != network_policy_decision
            || evidence.provider_config_generation != self.provider_config_generation
            || evidence.network_policy_decision_digest
                != crate::llm::provider_network_policy_decision_digest(
                    &self.network_policy_decision,
                )
        {
            anyhow::bail!("provider terminal proof runtime binding mismatch");
        }
        Ok(())
    }
}

struct ProviderInvocationTerminalBinding {
    request_id: String,
    provider: String,
    model: String,
    provider_endpoint: String,
    provider_config_generation: String,
    credential_identity: String,
    credential_version: u64,
    network_policy: NetworkPolicy,
    network_policy_decision: NetworkPolicyDecision,
    policy_evidence_digest: String,
}

impl ProviderInvocationTerminalBinding {
    fn capture(
        request: &PreparedProviderRequest,
        execution_binding: &crate::llm::ProviderExecutionBinding,
    ) -> Result<Self> {
        let policy_evidence = request.policy_receipt_evidence();
        Ok(Self {
            request_id: request.context_manifest.request_id.clone(),
            provider: request.provider_target.clone(),
            model: request.model_target.clone(),
            provider_endpoint: request.provider_endpoint.clone(),
            provider_config_generation: request.provider_config_generation.clone(),
            credential_identity: crate::llm::provider_credential_identity(
                execution_binding.api_key(),
            ),
            credential_version: request.provider_credential_version,
            network_policy: request.network_policy.clone(),
            network_policy_decision: request.network_policy_decision.clone(),
            policy_evidence_digest: policy_evidence.evidence_digest()?,
        })
    }

    fn issue(
        self,
        receipt: ProviderInvocationReceipt,
        origin: ProviderInvocationTerminalOrigin,
    ) -> Result<ProviderInvocationTerminalProof> {
        let evidence = receipt.policy_evidence.as_ref().ok_or_else(|| {
            anyhow::anyhow!("provider terminal receipt is missing policy evidence")
        })?;
        let terminal_shape_valid = match receipt.status {
            ProviderInvocationStatus::Completed => receipt.error_digest.is_none(),
            ProviderInvocationStatus::Failed | ProviderInvocationStatus::RemoteUnknown => {
                receipt.error_digest.is_some()
            }
        };
        if receipt.request_id != self.request_id
            || receipt.provider != self.provider
            || receipt.model != self.model
            || receipt.simulated
            || !terminal_shape_valid
            || evidence.evidence_digest()? != self.policy_evidence_digest
            || evidence.provider_config_generation != self.provider_config_generation
            || evidence.network_policy_decision_digest
                != crate::llm::provider_network_policy_decision_digest(
                    &self.network_policy_decision,
                )
        {
            anyhow::bail!("provider terminal receipt does not match its prepared adapter binding");
        }
        Ok(ProviderInvocationTerminalProof {
            receipt,
            provider_endpoint: self.provider_endpoint,
            provider_config_generation: self.provider_config_generation,
            credential_identity: self.credential_identity,
            credential_version: self.credential_version,
            network_policy: self.network_policy,
            network_policy_decision: self.network_policy_decision,
            runtime_seal: ProviderInvocationTerminalSeal {
                issuance_id: uuid::Uuid::new_v4(),
                origin,
            },
        })
    }
}

const MAX_RETAINED_PROVIDER_RECEIPTS: usize = 16;
const MAX_IN_FLIGHT_PROVIDER_ATTEMPTS: usize = 256;

/// Metadata-only adapter start fact. It is retained until the exact attempt is
/// terminal, so an orchestration timeout can bind `remote_unknown` to the
/// dispatch that was actually in flight rather than inventing a generic state.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderStartedAttempt {
    request_id: String,
    provider: String,
    model: String,
    started_at: chrono::DateTime<chrono::Utc>,
    policy_evidence: ProviderPolicyReceiptEvidence,
}

/// Runtime-only capability proving that one exact provider request crossed the
/// adapter start edge. A matching terminal proof is attached only when the
/// adapter itself observed and sealed the terminal. Runtime cancellation and
/// kernel-failure finalizers may use the start proof to persist
/// `remote_unknown`, but cannot mint an adapter completion or failure.
#[derive(Clone)]
pub struct ProviderInvocationDurabilityProof {
    start: ProviderStartedAttempt,
    lifecycle_evidence_digest: String,
    terminal: Option<ProviderInvocationTerminalProof>,
    synthetic_terminal: Option<ProviderInvocationReceipt>,
    runtime_seal: ProviderInvocationTerminalSeal,
}

impl std::fmt::Debug for ProviderInvocationDurabilityProof {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderInvocationDurabilityProof")
            .field("request_id", &self.start.request_id)
            .field("provider", &self.start.provider)
            .field("model", &self.start.model)
            .field(
                "provider_config_generation",
                &self.start.policy_evidence.provider_config_generation,
            )
            .field("terminal_attached", &self.terminal.is_some())
            .finish()
    }
}

impl ProviderInvocationDurabilityProof {
    fn issue_runtime_start(start: ProviderStartedAttempt) -> Result<Self> {
        let lifecycle_evidence_digest = crate::llm::provider_lifecycle_evidence_digest(
            &start.request_id,
            &start.provider,
            &start.model,
            &start.policy_evidence,
        )?;
        Ok(Self {
            start,
            lifecycle_evidence_digest,
            terminal: None,
            synthetic_terminal: None,
            runtime_seal: ProviderInvocationTerminalSeal {
                issuance_id: uuid::Uuid::new_v4(),
                origin: ProviderInvocationTerminalOrigin::RuntimeAdapter,
            },
        })
    }

    #[cfg(feature = "test-utils")]
    pub fn synthetic_for_test(receipt: ProviderInvocationReceipt) -> Result<Self> {
        let policy_evidence = receipt
            .policy_evidence
            .clone()
            .ok_or_else(|| anyhow::anyhow!("synthetic lifecycle proof needs policy evidence"))?;
        let start = ProviderStartedAttempt {
            request_id: receipt.request_id.clone(),
            provider: receipt.provider.clone(),
            model: receipt.model.clone(),
            started_at: receipt.started_at,
            policy_evidence,
        };
        let lifecycle_evidence_digest = crate::llm::provider_lifecycle_evidence_digest(
            &start.request_id,
            &start.provider,
            &start.model,
            &start.policy_evidence,
        )?;
        Ok(Self {
            start,
            lifecycle_evidence_digest,
            terminal: None,
            synthetic_terminal: Some(receipt),
            runtime_seal: ProviderInvocationTerminalSeal {
                issuance_id: uuid::Uuid::new_v4(),
                origin: ProviderInvocationTerminalOrigin::SyntheticTestFixture,
            },
        })
    }

    /// Test-only start-edge proof with no adapter terminal attached. Runtime
    /// cancellation and kernel-failure fixtures use this to exercise the same
    /// authority boundary as production without falsely minting a completed,
    /// failed, or remote-unknown adapter receipt.
    #[cfg(feature = "test-utils")]
    pub fn synthetic_start_for_test(
        request_id: String,
        provider: String,
        model: String,
        started_at: chrono::DateTime<chrono::Utc>,
        policy_evidence: ProviderPolicyReceiptEvidence,
    ) -> Result<Self> {
        let start = ProviderStartedAttempt {
            request_id,
            provider,
            model,
            started_at,
            policy_evidence,
        };
        let lifecycle_evidence_digest = crate::llm::provider_lifecycle_evidence_digest(
            &start.request_id,
            &start.provider,
            &start.model,
            &start.policy_evidence,
        )?;
        Ok(Self {
            start,
            lifecycle_evidence_digest,
            terminal: None,
            synthetic_terminal: None,
            runtime_seal: ProviderInvocationTerminalSeal {
                issuance_id: uuid::Uuid::new_v4(),
                origin: ProviderInvocationTerminalOrigin::SyntheticTestFixture,
            },
        })
    }

    pub fn request_id(&self) -> &str {
        &self.start.request_id
    }

    pub fn provider(&self) -> &str {
        &self.start.provider
    }

    pub fn model(&self) -> &str {
        &self.start.model
    }

    pub fn started_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.start.started_at
    }

    pub fn policy_evidence(&self) -> &ProviderPolicyReceiptEvidence {
        &self.start.policy_evidence
    }

    pub fn lifecycle_evidence_digest(&self) -> &str {
        &self.lifecycle_evidence_digest
    }

    pub fn terminal_receipt(&self) -> Option<&ProviderInvocationReceipt> {
        self.terminal
            .as_ref()
            .map(ProviderInvocationTerminalProof::receipt)
            .or(self.synthetic_terminal.as_ref())
    }

    pub fn is_runtime_adapter_start(&self) -> bool {
        matches!(
            self.runtime_seal.origin,
            ProviderInvocationTerminalOrigin::RuntimeAdapter
        ) && !self.runtime_seal.issuance_id.is_nil()
    }

    #[cfg(feature = "test-utils")]
    pub fn is_synthetic_test_fixture(&self) -> bool {
        matches!(
            self.runtime_seal.origin,
            ProviderInvocationTerminalOrigin::SyntheticTestFixture
        ) && !self.runtime_seal.issuance_id.is_nil()
    }

    pub fn validate_runtime_start(
        &self,
        request_id: &str,
        provider: &str,
        model: &str,
        started_at: chrono::DateTime<chrono::Utc>,
        policy_evidence: &ProviderPolicyReceiptEvidence,
        lifecycle_evidence_digest: &str,
    ) -> Result<()> {
        if !self.is_runtime_adapter_start()
            || self.start.request_id != request_id
            || self.start.provider != provider
            || self.start.model != model
            || self.start.started_at != started_at
            || &self.start.policy_evidence != policy_evidence
            || self.lifecycle_evidence_digest != lifecycle_evidence_digest
            || crate::llm::provider_lifecycle_evidence_digest(
                request_id,
                provider,
                model,
                policy_evidence,
            )? != lifecycle_evidence_digest
        {
            anyhow::bail!("provider durability start proof mismatch");
        }
        Ok(())
    }

    pub fn validate_runtime_adapter_terminal(
        &self,
        receipt: &ProviderInvocationReceipt,
    ) -> Result<()> {
        let terminal = self
            .terminal
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("provider durability terminal proof missing"))?;
        if !terminal.is_runtime_adapter_terminal() || terminal.receipt() != receipt {
            anyhow::bail!("provider durability terminal proof mismatch");
        }
        let evidence = receipt
            .policy_evidence
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("provider terminal receipt policy evidence missing"))?;
        self.validate_runtime_start(
            &receipt.request_id,
            &receipt.provider,
            &receipt.model,
            receipt.started_at,
            evidence,
            &self.lifecycle_evidence_digest,
        )
    }

    #[cfg(feature = "test-utils")]
    pub fn validate_synthetic_test_receipt(
        &self,
        receipt: &ProviderInvocationReceipt,
    ) -> Result<()> {
        if !self.is_synthetic_test_fixture() {
            anyhow::bail!("provider durability proof is not a synthetic fixture");
        }
        let evidence = receipt
            .policy_evidence
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("synthetic receipt policy evidence missing"))?;
        if self.start.request_id != receipt.request_id
            || self.start.provider != receipt.provider
            || self.start.model != receipt.model
            || self.start.started_at != receipt.started_at
            || &self.start.policy_evidence != evidence
        {
            anyhow::bail!("synthetic provider durability receipt mismatch");
        }
        Ok(())
    }
}

impl ProviderStartedAttempt {
    fn terminal_receipt(
        &self,
        status: ProviderInvocationStatus,
        error: Option<&str>,
    ) -> ProviderInvocationReceipt {
        ProviderInvocationReceipt {
            request_id: self.request_id.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
            status,
            started_at: self.started_at,
            finished_at: chrono::Utc::now(),
            error_digest: error.map(provider_stream_error_digest),
            simulated: false,
            policy_evidence: Some(self.policy_evidence.clone()),
        }
    }

    fn identity_digest(&self) -> String {
        provider_attempt_identity_digest(
            &self.request_id,
            &self.provider,
            &self.model,
            self.started_at,
            &self.policy_evidence.provider_config_generation,
        )
    }
}

const MAX_SCHEDULED_PROVIDER_TRUTH_ATTEMPTS: usize = 256;
const MAX_SCHEDULED_PROVIDER_TRUTH_ADMISSIONS: usize = MAX_SCHEDULED_PROVIDER_TRUTH_ATTEMPTS * 2;

/// Closed local reasons that may conservatively tighten a real adapter start
/// to `remote_unknown`. The caller cannot supply prose or a receipt, and this
/// enum cannot create a terminal unless the exact scheduled request already
/// crossed the adapter start hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledProviderLocalAbortCause {
    ExecutionTimeout,
    CancellationRequested,
    RuntimeFutureAborted,
}

impl ScheduledProviderLocalAbortCause {
    fn reason_code(self) -> &'static str {
        match self {
            Self::ExecutionTimeout => "scheduled_provider_execution_timeout",
            Self::CancellationRequested => "scheduled_provider_cancellation_requested",
            Self::RuntimeFutureAborted => "scheduled_provider_runtime_future_aborted",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ScheduledProviderTruthClaimBinding {
    canonical_store_identity: String,
    database_slot_verifier: String,
    runtime_store_instance_id: uuid::Uuid,
    task_id: String,
    task_revision_digest: String,
    attempt_id: String,
    attempt_number: u32,
    claim_token_digest: String,
    grant_id: String,
    grant_binding_digest: String,
    policy_decision_digest: String,
    policy_version: String,
    data_route: ProviderDataRoute,
    payload_purpose: ProviderPayloadPurpose,
    subject_scope_digest: String,
    grant_expires_at: Option<String>,
}

impl std::fmt::Debug for ScheduledProviderTruthClaimBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScheduledProviderTruthClaimBinding")
            .field("canonical_store_identity", &self.canonical_store_identity)
            .field("database_slot_verifier", &"[HMAC-ONLY]")
            .field("runtime_store_instance_id", &self.runtime_store_instance_id)
            .field("task_id", &self.task_id)
            .field("attempt_id", &self.attempt_id)
            .field("attempt_number", &self.attempt_number)
            .field("claim_token", &"[DIGEST-ONLY]")
            .field("grant_id", &self.grant_id)
            .field("policy_decision_digest", &self.policy_decision_digest)
            .finish()
    }
}

impl ScheduledProviderTruthClaimBinding {
    fn capture(claim: &ScheduledTaskClaim) -> Result<Self> {
        claim.validate_policy_authority()?;
        let authorization = ProviderPolicyAuthorization::from_scheduled_claim(claim)?;
        let grant_value = serde_json::to_value(claim.provider_grant())
            .map_err(|error| anyhow::anyhow!("scheduled provider grant digest failed: {error}"))?;
        let task_revision_digest =
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "schema": "scheduled_task_claim_revision_v1",
                "id": claim.task().id,
                "title": claim.task().title,
                "description": claim.task().description,
                "dueDate": claim.task().due_date,
                "priority": claim.task().priority,
                "status": claim.task().status,
                "createdAt": claim.task().created_at,
                "completedAt": claim.task().completed_at,
                "sourceRunId": claim.task().source_run_id,
                "sourceProposalId": claim.task().source_proposal_id,
                "actionType": claim.task().action_type,
                "attemptCount": claim.task().attempt_count,
                "claimTokenDigest": crate::agent::metadata_safe::metadata_safe_text_digest(
                    claim.task().claim_token.as_deref().unwrap_or("none"),
                ).1,
                "leaseExpiresAt": claim.task().lease_expires_at,
                "lastError": claim.task().last_error,
                "resultDigest": claim.task().result_digest,
                "resultRef": claim.task().result_ref,
                "providerGrant": grant_value,
            }))
            .1;
        Ok(Self {
            canonical_store_identity: claim.canonical_store_identity().to_string(),
            database_slot_verifier: claim.database_slot_verifier().to_string(),
            runtime_store_instance_id: claim.runtime_store_instance_id(),
            task_id: claim.task().id.clone(),
            task_revision_digest,
            attempt_id: claim.attempt_id().to_string(),
            attempt_number: claim.attempt_number(),
            claim_token_digest: crate::agent::metadata_safe::metadata_safe_text_digest(
                claim.claim_token(),
            )
            .1,
            grant_id: claim.provider_grant().grant_id.clone(),
            grant_binding_digest: crate::agent::metadata_safe::metadata_safe_value_digest(
                &grant_value,
            )
            .1,
            policy_decision_digest: claim.provider_grant().policy_decision_digest.clone(),
            policy_version: claim.provider_grant().policy_version.clone(),
            data_route: claim.provider_grant().data_route,
            payload_purpose: claim.provider_grant().payload_purpose,
            subject_scope_digest: authorization.subject_scope_digest(),
            grant_expires_at: claim.provider_grant().grant_expires_at.clone(),
        })
    }

    fn validate_claim(&self, claim: &ScheduledTaskClaim) -> Result<()> {
        let observed = Self::capture(claim)?;
        if &observed != self {
            anyhow::bail!(
                "scheduled provider truth admission does not match task/attempt/claim/grant"
            );
        }
        Ok(())
    }

    fn validate_dispatch_time(&self, observed_at: chrono::DateTime<chrono::Utc>) -> Result<()> {
        if let Some(expires_at) = self.grant_expires_at.as_deref() {
            let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at)
                .map_err(|error| {
                    anyhow::anyhow!("scheduled provider grant expiry invalid: {error}")
                })?
                .with_timezone(&chrono::Utc);
            if expires_at <= observed_at {
                anyhow::bail!("scheduled provider grant expired before adapter dispatch");
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct ScheduledPreparedProviderTruthBinding {
    claim: ScheduledProviderTruthClaimBinding,
    request_id: String,
    provider: String,
    model: String,
    prepared_request_digest: String,
    policy_evidence_digest: String,
    policy_evidence: ProviderPolicyReceiptEvidence,
}

impl ScheduledPreparedProviderTruthBinding {
    fn capture(
        claim: &ScheduledProviderTruthClaimBinding,
        request: &PreparedProviderRequest,
    ) -> Result<Self> {
        request.validate()?;
        let authorization = request.policy_authorization();
        let policy_evidence = request.policy_receipt_evidence();
        policy_evidence.validate_minimal_truth()?;
        if authorization.authority() != ProviderPolicyAuthority::ScheduledPolicy
            || authorization.decision_id() != claim.policy_decision_digest
            || authorization.policy_version() != claim.policy_version
            || authorization.data_route() != claim.data_route
            || policy_evidence.issuing_authority != ProviderPolicyAuthority::ScheduledPolicy
            || policy_evidence.decision_id != claim.policy_decision_digest
            || policy_evidence.policy_version != claim.policy_version
            || policy_evidence.effective_data_route != claim.data_route
            || policy_evidence.payload_purpose != Some(claim.payload_purpose)
            || policy_evidence.subject_scope_digest != claim.subject_scope_digest
        {
            anyhow::bail!(
                "prepared provider request does not match scheduled claim policy authority"
            );
        }
        let prepared_envelope_digest = policy_evidence
            .prepared_envelope_digest
            .as_deref()
            .ok_or_else(|| {
                anyhow::anyhow!("scheduled provider request has no prepared-envelope digest")
            })?;
        let policy_evidence_digest = policy_evidence.evidence_digest()?;
        let endpoint_digest =
            crate::agent::metadata_safe::metadata_safe_text_digest(&request.provider_endpoint).1;
        let prepared_request_digest =
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "schema": "scheduled_prepared_provider_request_v1",
                "requestId": request.context_manifest.request_id,
                "provider": request.provider_target,
                "model": request.model_target,
                "endpointDigest": endpoint_digest,
                "providerConfigGeneration": request.provider_config_generation,
                "providerCredentialVersion": request.provider_credential_version,
                "dataRoute": request.data_route,
                "preparedEnvelopeDigest": prepared_envelope_digest,
                "contextManifestDigest": policy_evidence.context_manifest_digest,
                "networkPolicyDecisionDigest": policy_evidence.network_policy_decision_digest,
                "policyEvidenceDigest": policy_evidence_digest,
            }))
            .1;
        Ok(Self {
            claim: claim.clone(),
            request_id: request.context_manifest.request_id.clone(),
            provider: request.provider_target.clone(),
            model: request.model_target.clone(),
            prepared_request_digest,
            policy_evidence_digest,
            policy_evidence,
        })
    }

    fn validate_start(&self, attempt: &ProviderStartedAttempt) -> Result<()> {
        self.claim.validate_dispatch_time(attempt.started_at)?;
        if attempt.request_id != self.request_id
            || attempt.provider != self.provider
            || attempt.model != self.model
            || attempt.policy_evidence.evidence_digest()? != self.policy_evidence_digest
            || attempt.policy_evidence != self.policy_evidence
        {
            anyhow::bail!("scheduled provider start differs from its prepared request binding");
        }
        Ok(())
    }
}

/// The only durable transitions a scheduled provider admission may carry.
/// This enum is metadata, not authority; only `ScheduledProviderTruthAdmission`
/// authorizes a canonical write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledProviderTruthTransition {
    Started,
    Completed,
    Failed,
    RemoteUnknown,
}

impl ScheduledProviderTruthTransition {
    fn from_status(status: ProviderInvocationStatus) -> Self {
        match status {
            ProviderInvocationStatus::Completed => Self::Completed,
            ProviderInvocationStatus::Failed => Self::Failed,
            ProviderInvocationStatus::RemoteUnknown => Self::RemoteUnknown,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::RemoteUnknown => "remote_unknown",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ScheduledProviderTruthProgressKey {
    transition: ScheduledProviderTruthTransition,
    request_id: String,
    provider: String,
    model: String,
    started_at: chrono::DateTime<chrono::Utc>,
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    error_digest: Option<String>,
    policy_evidence_digest: String,
}

impl ScheduledProviderTruthProgressKey {
    fn from_progress(progress: &ProviderInvocationProgress) -> Result<Self> {
        match progress {
            ProviderInvocationProgress::Started {
                request_id,
                provider,
                model,
                started_at,
                policy_evidence,
            } => {
                policy_evidence.validate_minimal_truth()?;
                Ok(Self {
                    transition: ScheduledProviderTruthTransition::Started,
                    request_id: request_id.clone(),
                    provider: provider.clone(),
                    model: model.clone(),
                    started_at: *started_at,
                    finished_at: None,
                    error_digest: None,
                    policy_evidence_digest: policy_evidence.evidence_digest()?,
                })
            }
            ProviderInvocationProgress::Completed(receipt) => {
                if receipt.status != ProviderInvocationStatus::Completed {
                    anyhow::bail!("scheduled provider completed progress has another status");
                }
                Self::from_receipt(receipt)
            }
            ProviderInvocationProgress::Failed(receipt) => {
                if receipt.status != ProviderInvocationStatus::Failed {
                    anyhow::bail!("scheduled provider failed progress has another status");
                }
                Self::from_receipt(receipt)
            }
            ProviderInvocationProgress::RemoteUnknown(receipt) => {
                if receipt.status != ProviderInvocationStatus::RemoteUnknown {
                    anyhow::bail!("scheduled provider unknown progress has another status");
                }
                Self::from_receipt(receipt)
            }
        }
    }

    fn from_receipt(receipt: &ProviderInvocationReceipt) -> Result<Self> {
        validate_scheduled_provider_terminal_shape(receipt)?;
        let policy_evidence = receipt.policy_evidence.as_ref().ok_or_else(|| {
            anyhow::anyhow!("scheduled provider terminal has no exact policy evidence")
        })?;
        Ok(Self {
            transition: ScheduledProviderTruthTransition::from_status(receipt.status),
            request_id: receipt.request_id.clone(),
            provider: receipt.provider.clone(),
            model: receipt.model.clone(),
            started_at: receipt.started_at,
            finished_at: Some(receipt.finished_at),
            error_digest: receipt.error_digest.clone(),
            policy_evidence_digest: policy_evidence.evidence_digest()?,
        })
    }
}

fn validate_scheduled_provider_terminal_shape(receipt: &ProviderInvocationReceipt) -> Result<()> {
    if receipt.simulated {
        anyhow::bail!("simulated provider receipt cannot become scheduled provider truth");
    }
    let valid = match receipt.status {
        ProviderInvocationStatus::Completed => receipt.error_digest.is_none(),
        ProviderInvocationStatus::Failed | ProviderInvocationStatus::RemoteUnknown => {
            receipt.error_digest.is_some()
        }
    };
    // Observation order is established by the typed lifecycle, not by wall
    // clock monotonicity; the system clock may move backwards between hooks.
    if !valid {
        anyhow::bail!("scheduled provider terminal shape is invalid");
    }
    receipt
        .policy_evidence
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("scheduled provider terminal lost policy evidence"))?
        .validate_minimal_truth()?;
    Ok(())
}

/// Metadata-only record revealed after a one-shot admission is consumed. It is
/// intentionally crate-private: future TaskStore wiring may read it, but the
/// record itself is not accepted as write authority.
pub(crate) struct ScheduledProviderTruthRecord {
    claim: ScheduledProviderTruthClaimBinding,
    transition: ScheduledProviderTruthTransition,
    request_id: String,
    provider: String,
    model: String,
    started_at: chrono::DateTime<chrono::Utc>,
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    error_digest: Option<String>,
    policy_evidence: ProviderPolicyReceiptEvidence,
    policy_evidence_digest: String,
    prepared_request_digest: String,
}

impl ScheduledProviderTruthRecord {
    fn started(
        prepared: &ScheduledPreparedProviderTruthBinding,
        attempt: &ProviderStartedAttempt,
    ) -> Self {
        Self {
            claim: prepared.claim.clone(),
            transition: ScheduledProviderTruthTransition::Started,
            request_id: attempt.request_id.clone(),
            provider: attempt.provider.clone(),
            model: attempt.model.clone(),
            started_at: attempt.started_at,
            finished_at: None,
            error_digest: None,
            policy_evidence: attempt.policy_evidence.clone(),
            policy_evidence_digest: prepared.policy_evidence_digest.clone(),
            prepared_request_digest: prepared.prepared_request_digest.clone(),
        }
    }

    fn terminal(
        prepared: &ScheduledPreparedProviderTruthBinding,
        receipt: &ProviderInvocationReceipt,
    ) -> Result<Self> {
        validate_scheduled_provider_terminal_shape(receipt)?;
        let policy_evidence = receipt
            .policy_evidence
            .clone()
            .ok_or_else(|| anyhow::anyhow!("scheduled provider terminal lost policy evidence"))?;
        Ok(Self {
            claim: prepared.claim.clone(),
            transition: ScheduledProviderTruthTransition::from_status(receipt.status),
            request_id: receipt.request_id.clone(),
            provider: receipt.provider.clone(),
            model: receipt.model.clone(),
            started_at: receipt.started_at,
            finished_at: Some(receipt.finished_at),
            error_digest: receipt.error_digest.clone(),
            policy_evidence,
            policy_evidence_digest: prepared.policy_evidence_digest.clone(),
            prepared_request_digest: prepared.prepared_request_digest.clone(),
        })
    }

    fn progress_key(&self) -> ScheduledProviderTruthProgressKey {
        ScheduledProviderTruthProgressKey {
            transition: self.transition,
            request_id: self.request_id.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
            started_at: self.started_at,
            finished_at: self.finished_at,
            error_digest: self.error_digest.clone(),
            policy_evidence_digest: self.policy_evidence_digest.clone(),
        }
    }

    fn authority_digest(&self) -> String {
        crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "schema": "scheduled_provider_truth_admission_v2",
            "canonicalStoreIdentity": self.claim.canonical_store_identity,
            "databaseSlotVerifier": self.claim.database_slot_verifier,
            "runtimeStoreInstanceId": self.claim.runtime_store_instance_id,
            "taskId": self.claim.task_id,
            "taskRevisionDigest": self.claim.task_revision_digest,
            "attemptId": self.claim.attempt_id,
            "attemptNumber": self.claim.attempt_number,
            "claimTokenDigest": self.claim.claim_token_digest,
            "grantId": self.claim.grant_id,
            "grantBindingDigest": self.claim.grant_binding_digest,
            "policyDecisionDigest": self.claim.policy_decision_digest,
            "preparedRequestDigest": self.prepared_request_digest,
            "policyEvidenceDigest": self.policy_evidence_digest,
            "transition": self.transition.as_str(),
            "requestId": self.request_id,
            "provider": self.provider,
            "model": self.model,
            "startedAt": self.started_at.to_rfc3339(),
            "finishedAt": self.finished_at.map(|value| value.to_rfc3339()),
            "errorDigest": self.error_digest,
        }))
        .1
    }

    pub(crate) fn transition(&self) -> ScheduledProviderTruthTransition {
        self.transition
    }

    pub(crate) fn request_id(&self) -> &str {
        &self.request_id
    }

    pub(crate) fn provider(&self) -> &str {
        &self.provider
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn started_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.started_at
    }

    pub(crate) fn finished_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.finished_at
    }

    pub(crate) fn error_digest(&self) -> Option<&str> {
        self.error_digest.as_deref()
    }

    pub(crate) fn policy_evidence(&self) -> &ProviderPolicyReceiptEvidence {
        &self.policy_evidence
    }

    pub(crate) fn prepared_request_digest(&self) -> &str {
        &self.prepared_request_digest
    }
}

struct ScheduledProviderTruthLifecycle {
    prepared: ScheduledPreparedProviderTruthBinding,
    attempt: ProviderStartedAttempt,
    terminal: Option<ScheduledProviderTruthProgressKey>,
}

struct ScheduledProviderTruthPendingAdmission {
    issuance_id: uuid::Uuid,
    record_digest: String,
    record: ScheduledProviderTruthRecord,
}

struct ScheduledProviderTruthOutstandingAdmission {
    request_id: String,
    record_digest: String,
}

struct ScheduledProviderTruthAdmissionState {
    claim: ScheduledProviderTruthClaimBinding,
    lifecycles: HashMap<String, ScheduledProviderTruthLifecycle>,
    pending: Vec<ScheduledProviderTruthPendingAdmission>,
    outstanding: HashMap<uuid::Uuid, ScheduledProviderTruthOutstandingAdmission>,
}

impl ScheduledProviderTruthAdmissionState {
    fn new(claim: ScheduledProviderTruthClaimBinding) -> Self {
        Self {
            claim,
            lifecycles: HashMap::new(),
            pending: Vec::new(),
            outstanding: HashMap::new(),
        }
    }

    fn queue(&mut self, record: ScheduledProviderTruthRecord) -> Result<()> {
        if self.pending.len().saturating_add(self.outstanding.len())
            >= MAX_SCHEDULED_PROVIDER_TRUTH_ADMISSIONS
        {
            anyhow::bail!("scheduled provider truth admission limit reached");
        }
        let key = record.progress_key();
        if self
            .pending
            .iter()
            .any(|pending| pending.record.progress_key() == key)
        {
            anyhow::bail!("scheduled provider truth admission already queued");
        }
        self.pending.push(ScheduledProviderTruthPendingAdmission {
            issuance_id: uuid::Uuid::new_v4(),
            record_digest: record.authority_digest(),
            record,
        });
        Ok(())
    }

    fn register_started(
        &mut self,
        prepared: ScheduledPreparedProviderTruthBinding,
        attempt: ProviderStartedAttempt,
    ) -> Result<()> {
        if prepared.claim != self.claim {
            anyhow::bail!("scheduled provider start belongs to another claim scope");
        }
        prepared.validate_start(&attempt)?;
        if self.lifecycles.contains_key(&attempt.request_id) {
            anyhow::bail!("scheduled provider request id already crossed the start edge");
        }
        if self.lifecycles.len() >= MAX_SCHEDULED_PROVIDER_TRUTH_ATTEMPTS {
            anyhow::bail!("scheduled provider truth attempt limit reached");
        }
        self.queue(ScheduledProviderTruthRecord::started(&prepared, &attempt))?;
        self.lifecycles.insert(
            attempt.request_id.clone(),
            ScheduledProviderTruthLifecycle {
                prepared,
                attempt,
                terminal: None,
            },
        );
        Ok(())
    }

    fn discard_started(&mut self, attempt: &ProviderStartedAttempt) {
        let exact = self
            .lifecycles
            .get(&attempt.request_id)
            .is_some_and(|lifecycle| lifecycle.attempt == *attempt && lifecycle.terminal.is_none());
        if !exact {
            return;
        }
        self.lifecycles.remove(&attempt.request_id);
        self.pending
            .retain(|pending| pending.record.request_id != attempt.request_id);
        self.outstanding
            .retain(|_, outstanding| outstanding.request_id != attempt.request_id);
    }

    fn register_terminal(
        &mut self,
        prepared: &ScheduledPreparedProviderTruthBinding,
        receipt: &ProviderInvocationReceipt,
    ) -> Result<()> {
        if prepared.claim != self.claim {
            anyhow::bail!("scheduled provider terminal belongs to another claim scope");
        }
        let lifecycle = self
            .lifecycles
            .get(&receipt.request_id)
            .ok_or_else(|| anyhow::anyhow!("scheduled provider terminal arrived before start"))?;
        if lifecycle.terminal.is_some() {
            anyhow::bail!("scheduled provider first terminal already won");
        }
        if lifecycle.prepared.prepared_request_digest != prepared.prepared_request_digest
            || lifecycle.attempt.request_id != receipt.request_id
            || lifecycle.attempt.provider != receipt.provider
            || lifecycle.attempt.model != receipt.model
            || lifecycle.attempt.started_at != receipt.started_at
            || receipt
                .policy_evidence
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("scheduled provider terminal lost policy evidence"))?
                .evidence_digest()?
                != prepared.policy_evidence_digest
        {
            anyhow::bail!("scheduled provider terminal differs from its exact start binding");
        }
        let record = ScheduledProviderTruthRecord::terminal(prepared, receipt)?;
        let key = record.progress_key();
        self.queue(record)?;
        let lifecycle = self
            .lifecycles
            .get_mut(&receipt.request_id)
            .ok_or_else(|| anyhow::anyhow!("scheduled provider lifecycle disappeared"))?;
        lifecycle.terminal = Some(key);
        Ok(())
    }

    fn take_pending(
        &mut self,
        key: &ScheduledProviderTruthProgressKey,
    ) -> Result<ScheduledProviderTruthPendingAdmission> {
        let index = self
            .pending
            .iter()
            .position(|pending| pending.record.progress_key() == *key)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "scheduled provider truth admission missing for caller-shaped progress"
                )
            })?;
        let pending = self.pending.remove(index);
        self.outstanding.insert(
            pending.issuance_id,
            ScheduledProviderTruthOutstandingAdmission {
                request_id: pending.record.request_id.clone(),
                record_digest: pending.record_digest.clone(),
            },
        );
        Ok(pending)
    }

    fn consume(&mut self, issuance_id: uuid::Uuid, record_digest: &str) -> Result<()> {
        let outstanding = self.outstanding.remove(&issuance_id).ok_or_else(|| {
            anyhow::anyhow!("scheduled provider truth admission was already consumed or revoked")
        })?;
        if outstanding.record_digest != record_digest {
            anyhow::bail!("scheduled provider truth admission digest mismatch");
        }
        Ok(())
    }

    fn revoke(&mut self, issuance_id: uuid::Uuid) {
        self.outstanding.remove(&issuance_id);
    }

    fn active_attempts(
        &self,
    ) -> Vec<(
        ScheduledPreparedProviderTruthBinding,
        ProviderStartedAttempt,
    )> {
        self.lifecycles
            .values()
            .filter(|lifecycle| lifecycle.terminal.is_none())
            .map(|lifecycle| (lifecycle.prepared.clone(), lifecycle.attempt.clone()))
            .collect()
    }
}

#[derive(Clone)]
struct ScheduledProviderTruthScope {
    state: Arc<Mutex<ScheduledProviderTruthAdmissionState>>,
    store: Arc<TaskStore>,
    claim: Arc<ScheduledTaskClaim>,
}

impl ScheduledProviderTruthScope {
    fn capture_prepared(
        &self,
        request: &PreparedProviderRequest,
    ) -> Result<ScheduledPreparedProviderTruthBinding> {
        let claim = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled provider truth authority is poisoned"))?
            .claim
            .clone();
        ScheduledPreparedProviderTruthBinding::capture(&claim, request)
    }

    fn record_started(
        &self,
        prepared: ScheduledPreparedProviderTruthBinding,
        attempt: ProviderStartedAttempt,
    ) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled provider truth authority is poisoned"))?
            .register_started(prepared, attempt)
    }

    fn discard_started(&self, attempt: &ProviderStartedAttempt) {
        if let Ok(mut state) = self.state.lock() {
            state.discard_started(attempt);
        }
    }

    fn record_adapter_terminal(
        &self,
        prepared: &ScheduledPreparedProviderTruthBinding,
        proof: &ProviderInvocationTerminalProof,
    ) -> Result<()> {
        if !proof.is_runtime_adapter_terminal() {
            anyhow::bail!("scheduled provider terminal lacks runtime adapter proof");
        }
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled provider truth authority is poisoned"))?
            .register_terminal(prepared, proof.receipt())
    }

    fn persist_registered_progress(&self, progress: &ProviderInvocationProgress) -> Result<()> {
        let key = ScheduledProviderTruthProgressKey::from_progress(progress)?;
        let pending = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled provider truth authority is poisoned"))?
            .take_pending(&key)?;
        let admission = ScheduledProviderTruthAdmission {
            issuance_id: pending.issuance_id,
            record_digest: pending.record_digest,
            record: Some(pending.record),
            authority_state: Arc::clone(&self.state),
        };
        if !self.store.record_provider_truth(&self.claim, admission)? {
            anyhow::bail!("scheduled provider truth durable CAS rejected the exact transition");
        }
        Ok(())
    }
}

/// A runtime-only, single-use capability for one exact scheduled provider
/// truth transition.
///
/// It deliberately implements neither `Clone` nor serde. Moving it twice,
/// cloning it, or serializing it must remain a compile error:
///
/// ```compile_fail
/// use openlife_core::scheduler::ScheduledProviderTruthAdmission;
/// fn clone_is_not_authority(proof: ScheduledProviderTruthAdmission) {
///     let copied = proof.clone();
///     drop((proof, copied));
/// }
/// ```
///
/// ```compile_fail
/// use openlife_core::scheduler::ScheduledProviderTruthAdmission;
/// fn serde_is_not_authority(proof: ScheduledProviderTruthAdmission) {
///     let _ = serde_json::to_string(&proof).unwrap();
/// }
/// ```
///
/// ```compile_fail
/// use openlife_core::scheduler::ScheduledProviderTruthAdmission;
/// fn consume_once(_: ScheduledProviderTruthAdmission) {}
/// fn cannot_reuse(proof: ScheduledProviderTruthAdmission) {
///     consume_once(proof);
///     consume_once(proof);
/// }
/// ```
pub struct ScheduledProviderTruthAdmission {
    issuance_id: uuid::Uuid,
    record_digest: String,
    record: Option<ScheduledProviderTruthRecord>,
    authority_state: Arc<Mutex<ScheduledProviderTruthAdmissionState>>,
}

impl std::fmt::Debug for ScheduledProviderTruthAdmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let record = self.record.as_ref();
        formatter
            .debug_struct("ScheduledProviderTruthAdmission")
            .field("issuance_id", &self.issuance_id)
            .field("transition", &record.map(|record| record.transition))
            .field(
                "request_id",
                &record.map(|record| record.request_id.as_str()),
            )
            .field("claim_token", &"[REDACTED]")
            .finish()
    }
}

impl ScheduledProviderTruthAdmission {
    pub(crate) fn consume_for_claim(
        mut self,
        claim: &ScheduledTaskClaim,
    ) -> Result<ScheduledProviderTruthRecord> {
        let record = self.record.as_ref().ok_or_else(|| {
            anyhow::anyhow!("scheduled provider truth admission has no unconsumed record")
        })?;
        record.claim.validate_claim(claim)?;
        self.authority_state
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled provider truth authority is poisoned"))?
            .consume(self.issuance_id, &self.record_digest)?;
        self.record.take().ok_or_else(|| {
            anyhow::anyhow!("scheduled provider truth admission was already consumed")
        })
    }
}

impl Drop for ScheduledProviderTruthAdmission {
    fn drop(&mut self) {
        if self.record.is_some() {
            if let Ok(mut state) = self.authority_state.lock() {
                state.revoke(self.issuance_id);
            }
        }
    }
}

/// Cloneable access to a scoped queue is safe because it can only take a
/// non-cloneable admission that was already issued inside the exact adapter
/// hook. Caller-shaped progress can select an existing fact, never mint one.
#[derive(Clone)]
pub struct ScheduledProviderTruthAdmissionHandle {
    state: Arc<Mutex<ScheduledProviderTruthAdmissionState>>,
    provider_receipt_collector: ProviderReceiptCollector,
}

impl ScheduledProviderTruthAdmissionHandle {
    pub fn take_for_progress(
        &self,
        progress: &ProviderInvocationProgress,
    ) -> Result<ScheduledProviderTruthAdmission> {
        let key = ScheduledProviderTruthProgressKey::from_progress(progress)?;
        let pending = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled provider truth authority is poisoned"))?
            .take_pending(&key)?;
        Ok(ScheduledProviderTruthAdmission {
            issuance_id: pending.issuance_id,
            record_digest: pending.record_digest,
            record: Some(pending.record),
            authority_state: Arc::clone(&self.state),
        })
    }

    /// Conservatively terminalize only exact attempts that already crossed
    /// the adapter start hook. If an adapter terminal won the race, this method
    /// does not replace it or invent another terminal.
    pub fn take_remote_unknown_after_local_abort(
        &self,
        cause: ScheduledProviderLocalAbortCause,
    ) -> Result<Vec<ScheduledProviderTruthAdmission>> {
        let active = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("scheduled provider truth authority is poisoned"))?
            .active_attempts();
        if active.is_empty() {
            return Ok(Vec::new());
        }
        self.provider_receipt_collector
            .mark_in_flight_remote_unknown(cause.reason_code());
        let mut admissions = Vec::new();
        for (prepared, attempt) in active {
            let Some(receipt) = self
                .provider_receipt_collector
                .terminal_for_attempt(&attempt)
            else {
                continue;
            };
            if receipt.status != ProviderInvocationStatus::RemoteUnknown {
                continue;
            }
            let key = ScheduledProviderTruthProgressKey::from_receipt(&receipt)?;
            let pending = {
                let mut state = self.state.lock().map_err(|_| {
                    anyhow::anyhow!("scheduled provider truth authority is poisoned")
                })?;
                if state
                    .lifecycles
                    .get(&receipt.request_id)
                    .is_some_and(|lifecycle| lifecycle.terminal.is_some())
                {
                    continue;
                }
                state.register_terminal(&prepared, &receipt)?;
                state.take_pending(&key)?
            };
            admissions.push(ScheduledProviderTruthAdmission {
                issuance_id: pending.issuance_id,
                record_digest: pending.record_digest,
                record: Some(pending.record),
                authority_state: Arc::clone(&self.state),
            });
        }
        Ok(admissions)
    }
}

/// Test-only bridge for TaskStore persistence tests. It exercises the same
/// admission state machine while remaining absent from production builds; real
/// product code can receive admissions only from bound adapter hooks.
#[cfg(any(test, feature = "test-utils"))]
#[doc(hidden)]
pub fn issue_scheduled_provider_truth_test_admission(
    claim: &ScheduledTaskClaim,
    progress: &ProviderInvocationProgress,
) -> Result<ScheduledProviderTruthAdmission> {
    let (request_id, provider, model, started_at, policy_evidence) = match progress {
        ProviderInvocationProgress::Started {
            request_id,
            provider,
            model,
            started_at,
            policy_evidence,
        } => (
            request_id.clone(),
            provider.clone(),
            model.clone(),
            *started_at,
            policy_evidence.clone(),
        ),
        ProviderInvocationProgress::Completed(receipt)
        | ProviderInvocationProgress::Failed(receipt)
        | ProviderInvocationProgress::RemoteUnknown(receipt) => (
            receipt.request_id.clone(),
            receipt.provider.clone(),
            receipt.model.clone(),
            receipt.started_at,
            receipt
                .policy_evidence
                .clone()
                .ok_or_else(|| anyhow::anyhow!("test provider terminal has no policy evidence"))?,
        ),
    };
    let claim_binding = ScheduledProviderTruthClaimBinding::capture(claim)?;
    let policy_evidence_digest = policy_evidence.evidence_digest()?;
    let prepared = ScheduledPreparedProviderTruthBinding {
        claim: claim_binding.clone(),
        request_id,
        provider,
        model,
        prepared_request_digest: crate::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::json!({
                "schema": "scheduled_provider_truth_test_prepared_v1",
                "requestId": progress_request_id(progress),
                "provider": progress_provider(progress),
                "model": progress_model(progress),
                "policyEvidenceDigest": policy_evidence_digest,
            }),
        )
        .1,
        policy_evidence_digest,
        policy_evidence: policy_evidence.clone(),
    };
    let attempt = ProviderStartedAttempt {
        request_id: prepared.request_id.clone(),
        provider: prepared.provider.clone(),
        model: prepared.model.clone(),
        started_at,
        policy_evidence,
    };
    let state = Arc::new(Mutex::new(ScheduledProviderTruthAdmissionState::new(
        claim_binding,
    )));
    let pending = {
        let mut state_guard = state
            .lock()
            .map_err(|_| anyhow::anyhow!("test provider truth authority is poisoned"))?;
        state_guard.register_started(prepared.clone(), attempt)?;
        if let ProviderInvocationProgress::Completed(receipt)
        | ProviderInvocationProgress::Failed(receipt)
        | ProviderInvocationProgress::RemoteUnknown(receipt) = progress
        {
            state_guard.register_terminal(&prepared, receipt)?;
        }
        let key = ScheduledProviderTruthProgressKey::from_progress(progress)?;
        state_guard.take_pending(&key)?
    };
    Ok(ScheduledProviderTruthAdmission {
        issuance_id: pending.issuance_id,
        record_digest: pending.record_digest,
        record: Some(pending.record),
        authority_state: state,
    })
}

#[cfg(any(test, feature = "test-utils"))]
fn progress_request_id(progress: &ProviderInvocationProgress) -> &str {
    match progress {
        ProviderInvocationProgress::Started { request_id, .. } => request_id,
        ProviderInvocationProgress::Completed(receipt)
        | ProviderInvocationProgress::Failed(receipt)
        | ProviderInvocationProgress::RemoteUnknown(receipt) => &receipt.request_id,
    }
}

#[cfg(any(test, feature = "test-utils"))]
fn progress_provider(progress: &ProviderInvocationProgress) -> &str {
    match progress {
        ProviderInvocationProgress::Started { provider, .. } => provider,
        ProviderInvocationProgress::Completed(receipt)
        | ProviderInvocationProgress::Failed(receipt)
        | ProviderInvocationProgress::RemoteUnknown(receipt) => &receipt.provider,
    }
}

#[cfg(any(test, feature = "test-utils"))]
fn progress_model(progress: &ProviderInvocationProgress) -> &str {
    match progress {
        ProviderInvocationProgress::Started { model, .. } => model,
        ProviderInvocationProgress::Completed(receipt)
        | ProviderInvocationProgress::Failed(receipt)
        | ProviderInvocationProgress::RemoteUnknown(receipt) => &receipt.model,
    }
}

/// Sticky aggregate plus a fixed-size detail window. Counts are never derived
/// from the retained detail window, so an early failure cannot disappear when
/// later attempts overflow the response budget.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderReceiptSummary {
    pub started_attempt_count: u64,
    pub completed_count: u64,
    pub confirmed_failed_count: u64,
    pub remote_unknown_count: u64,
    pub in_flight_count: usize,
    pub retained_receipts: Vec<ProviderInvocationReceipt>,
    pub overflow_count: u64,
    pub overflow_digest: Option<String>,
}

#[derive(Default)]
struct ProviderReceiptCollectorState {
    in_flight: Vec<ProviderStartedAttempt>,
    retained_receipts: Vec<ProviderInvocationReceipt>,
    terminal_receipts_by_identity: HashMap<String, ProviderInvocationReceipt>,
    durability_proofs_by_identity: HashMap<String, ProviderInvocationDurabilityProof>,
    started_attempt_count: u64,
    completed_count: u64,
    confirmed_failed_count: u64,
    remote_unknown_count: u64,
    overflow_count: u64,
    overflow_digest: Option<String>,
}

/// Per-execution receipt collector used to carry adapter-edge facts across
/// orchestration layers without copying request or response bodies.
#[derive(Clone, Default)]
pub struct ProviderReceiptCollector {
    state: Arc<Mutex<ProviderReceiptCollectorState>>,
}

impl ProviderReceiptCollector {
    fn record_started(&self, attempt: ProviderStartedAttempt) -> Result<()> {
        let durability_proof =
            ProviderInvocationDurabilityProof::issue_runtime_start(attempt.clone())?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let identity_digest = attempt.identity_digest();
        if state
            .in_flight
            .iter()
            .any(|existing| existing.identity_digest() == identity_digest)
            || state
                .terminal_receipts_by_identity
                .contains_key(&identity_digest)
        {
            return Ok(());
        }
        if state.in_flight.len() >= MAX_IN_FLIGHT_PROVIDER_ATTEMPTS {
            anyhow::bail!("provider_receipt_collector_in_flight_limit");
        }
        state.started_attempt_count = state.started_attempt_count.saturating_add(1);
        state.in_flight.push(attempt);
        state
            .durability_proofs_by_identity
            .insert(identity_digest, durability_proof);
        Ok(())
    }

    fn discard_started(&self, attempt: &ProviderStartedAttempt) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let identity_digest = attempt.identity_digest();
        if let Some(index) = state
            .in_flight
            .iter()
            .position(|existing| existing.identity_digest() == identity_digest)
        {
            state.in_flight.remove(index);
            state.started_attempt_count = state.started_attempt_count.saturating_sub(1);
            state.durability_proofs_by_identity.remove(&identity_digest);
        }
    }

    /// First terminal wins for an exact adapter attempt. The returned receipt
    /// is therefore the canonical fact that every caller must forward, even
    /// when a late EOF races with a cancellation-owned `remote_unknown`.
    fn record_terminal(&self, receipt: ProviderInvocationReceipt) -> ProviderInvocationReceipt {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let identity_digest = provider_receipt_identity_digest(&receipt);
        if let Some(existing) = state.terminal_receipts_by_identity.get(&identity_digest) {
            return existing.clone();
        }
        if let Some(index) = state
            .in_flight
            .iter()
            .position(|attempt| attempt.identity_digest() == identity_digest)
        {
            state.in_flight.remove(index);
        } else {
            // Some unit-level and compatibility callers retain an already
            // terminal receipt without routing the Started callback through
            // this collector. Count that terminal as one observed attempt.
            state.started_attempt_count = state.started_attempt_count.saturating_add(1);
        }
        match receipt.status {
            ProviderInvocationStatus::Completed => {
                state.completed_count = state.completed_count.saturating_add(1)
            }
            ProviderInvocationStatus::Failed => {
                state.confirmed_failed_count = state.confirmed_failed_count.saturating_add(1)
            }
            ProviderInvocationStatus::RemoteUnknown => {
                state.remote_unknown_count = state.remote_unknown_count.saturating_add(1)
            }
        }
        if state.retained_receipts.len() == MAX_RETAINED_PROVIDER_RECEIPTS {
            let overflowed = state.retained_receipts.remove(0);
            fold_receipt_overflow(&mut state, &overflowed);
        }
        state.retained_receipts.push(receipt.clone());
        state
            .terminal_receipts_by_identity
            .insert(identity_digest, receipt.clone());
        receipt
    }

    fn record_terminal_proof(&self, proof: ProviderInvocationTerminalProof) -> Result<()> {
        let receipt = proof.receipt();
        if !proof.is_runtime_adapter_terminal() {
            anyhow::bail!("provider terminal proof was not issued by runtime adapter");
        }
        let identity_digest = provider_receipt_identity_digest(receipt);
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.terminal_receipts_by_identity.get(&identity_digest) != Some(receipt) {
            anyhow::bail!("provider terminal proof receipt was not retained");
        }
        let durability_proof = state
            .durability_proofs_by_identity
            .get_mut(&identity_digest)
            .ok_or_else(|| anyhow::anyhow!("provider terminal proof lost its start proof"))?;
        durability_proof.terminal = Some(proof);
        Ok(())
    }

    fn durability_proof_for_receipt(
        &self,
        receipt: &ProviderInvocationReceipt,
    ) -> Option<ProviderInvocationDurabilityProof> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .durability_proofs_by_identity
            .get(&provider_receipt_identity_digest(receipt))
            .cloned()
    }

    fn durability_proof_for_start(
        &self,
        request_id: &str,
        provider: &str,
        model: &str,
        started_at: chrono::DateTime<chrono::Utc>,
        policy_evidence: &ProviderPolicyReceiptEvidence,
    ) -> Option<ProviderInvocationDurabilityProof> {
        let identity_digest = provider_attempt_identity_digest(
            request_id,
            provider,
            model,
            started_at,
            &policy_evidence.provider_config_generation,
        );
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .durability_proofs_by_identity
            .get(&identity_digest)
            .cloned()
    }

    pub fn snapshot(&self) -> Vec<ProviderInvocationReceipt> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retained_receipts
            .clone()
    }

    pub fn summary(&self) -> ProviderReceiptSummary {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        ProviderReceiptSummary {
            started_attempt_count: state.started_attempt_count,
            completed_count: state.completed_count,
            confirmed_failed_count: state.confirmed_failed_count,
            remote_unknown_count: state.remote_unknown_count,
            in_flight_count: state.in_flight.len(),
            retained_receipts: state.retained_receipts.clone(),
            overflow_count: state.overflow_count,
            overflow_digest: state.overflow_digest.clone(),
        }
    }

    /// Convert only attempts that actually crossed the adapter start boundary
    /// into remote-unknown terminals. A local timeout before provider start
    /// therefore remains `not_attempted`.
    pub fn mark_in_flight_remote_unknown(&self, reason_code: &'static str) {
        let attempts = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.in_flight.clone()
        };
        for attempt in attempts {
            let _ = self.record_terminal(
                attempt
                    .terminal_receipt(ProviderInvocationStatus::RemoteUnknown, Some(reason_code)),
            );
        }
    }

    fn terminal_for_attempt(
        &self,
        attempt: &ProviderStartedAttempt,
    ) -> Option<ProviderInvocationReceipt> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .terminal_receipts_by_identity
            .get(&attempt.identity_digest())
            .cloned()
    }
}

fn provider_attempt_identity_digest(
    request_id: &str,
    provider: &str,
    model: &str,
    started_at: chrono::DateTime<chrono::Utc>,
    provider_config_generation: &str,
) -> String {
    crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
        "requestId": request_id,
        "provider": provider,
        "model": model,
        "startedAt": started_at,
        "providerConfigGeneration": provider_config_generation,
    }))
    .1
}

fn provider_receipt_identity_digest(receipt: &ProviderInvocationReceipt) -> String {
    provider_attempt_identity_digest(
        &receipt.request_id,
        &receipt.provider,
        &receipt.model,
        receipt.started_at,
        receipt
            .policy_evidence
            .as_ref()
            .map(|evidence| evidence.provider_config_generation.as_str())
            .unwrap_or("unverified_generation"),
    )
}

fn fold_receipt_overflow(
    state: &mut ProviderReceiptCollectorState,
    receipt: &ProviderInvocationReceipt,
) {
    state.overflow_count = state.overflow_count.saturating_add(1);
    state.overflow_digest = Some(
        crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "previousOverflowDigest": state.overflow_digest.clone(),
            "receiptIdentityDigest": provider_receipt_identity_digest(receipt),
            "status": receipt.status,
            "errorDigest": receipt.error_digest.clone(),
        }))
        .1,
    );
}

/// The typed event stream returned by the prepared-provider seam. Tokens stay
/// transient; the only terminal fact is the receipt created at the adapter
/// boundary below.
#[derive(Debug)]
pub enum PreparedProviderStreamEvent {
    Token(String),
    Terminal(PreparedProviderStreamTerminal),
}

/// Exhaustive terminal truth for a prepared provider stream. `NotAttempted`
/// is reserved for deterministic scripted fixtures that never cross an
/// adapter edge. Every real invocation carries the exact retained receipt;
/// consumers forward it and never reconstruct provider status or timestamps.
#[derive(Debug)]
pub enum PreparedProviderStreamTerminal {
    NotAttempted,
    Completed(Box<ProviderInvocationReceipt>),
    Failed {
        receipt: Box<ProviderInvocationReceipt>,
        error: String,
    },
    RemoteUnknown {
        receipt: Box<ProviderInvocationReceipt>,
        error: String,
    },
}

impl PreparedProviderStreamTerminal {
    fn from_receipt(receipt: ProviderInvocationReceipt, error: Option<String>) -> Self {
        match receipt.status {
            ProviderInvocationStatus::Completed => Self::Completed(Box::new(receipt)),
            ProviderInvocationStatus::Failed => Self::Failed {
                receipt: Box::new(receipt),
                error: error.unwrap_or_else(|| "provider_confirmed_failure".into()),
            },
            ProviderInvocationStatus::RemoteUnknown => Self::RemoteUnknown {
                receipt: Box::new(receipt),
                error: error.unwrap_or_else(|| "provider_remote_state_unknown".into()),
            },
        }
    }
}

pub type PreparedProviderStream = Pin<Box<dyn Stream<Item = PreparedProviderStreamEvent> + Send>>;

/// Makes terminal receipt retention a property of the provider stream itself.
/// Callers receive typed token/terminal events; completion, late failure, and
/// local drop can no longer silently discard or reclassify the adapter attempt.
struct ReceiptRetainingProviderStream {
    inner: StreamResult,
    seed: ProviderStartedAttempt,
    collector: ProviderReceiptCollector,
    terminal_binding: Option<ProviderInvocationTerminalBinding>,
    terminal_recorded: bool,
}

impl ReceiptRetainingProviderStream {
    fn terminal(
        &mut self,
        status: ProviderInvocationStatus,
        error: Option<String>,
    ) -> PreparedProviderStreamTerminal {
        let receipt = self.seed.terminal_receipt(status, error.as_deref());
        let canonical_receipt = self.collector.record_terminal(receipt);
        let error = (canonical_receipt.status == status)
            .then_some(error)
            .flatten();
        if let Some(binding) = self.terminal_binding.take() {
            if let Ok(proof) = binding.issue(
                canonical_receipt.clone(),
                ProviderInvocationTerminalOrigin::RuntimeAdapter,
            ) {
                let _ = self.collector.record_terminal_proof(proof);
            }
        }
        self.terminal_recorded = true;
        PreparedProviderStreamTerminal::from_receipt(canonical_receipt, error)
    }
}

impl Stream for ReceiptRetainingProviderStream {
    type Item = PreparedProviderStreamEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.terminal_recorded {
            return Poll::Ready(None);
        }
        if let Some(canonical_receipt) = this.collector.terminal_for_attempt(&this.seed) {
            this.terminal_recorded = true;
            return Poll::Ready(Some(PreparedProviderStreamEvent::Terminal(
                PreparedProviderStreamTerminal::from_receipt(canonical_receipt, None),
            )));
        }
        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Err(error))) => {
                let status = crate::llm::provider_error_terminal_status(&error);
                let terminal = this.terminal(status, Some(error.to_string()));
                Poll::Ready(Some(PreparedProviderStreamEvent::Terminal(terminal)))
            }
            Poll::Ready(Some(Ok(token))) => {
                Poll::Ready(Some(PreparedProviderStreamEvent::Token(token)))
            }
            Poll::Ready(None) => {
                let terminal = this.terminal(ProviderInvocationStatus::Completed, None);
                Poll::Ready(Some(PreparedProviderStreamEvent::Terminal(terminal)))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for ReceiptRetainingProviderStream {
    fn drop(&mut self) {
        if self.terminal_recorded {
            return;
        }
        let receipt = self.seed.terminal_receipt(
            ProviderInvocationStatus::RemoteUnknown,
            Some("provider_stream_dropped_local_aborted_remote_state_unknown"),
        );
        let _ = self.collector.record_terminal(receipt);
        self.terminal_recorded = true;
    }
}

fn provider_stream_error_digest(error: &str) -> String {
    crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({ "error": error }))
        .1
}

/// Inference scheduler bound to the user-selected local or cloud route.
///
/// A selected local route fails closed when its exact Ollama model is unavailable;
/// it never widens the transmission boundary by silently switching to cloud.
#[derive(Clone)]
pub struct InferenceScheduler {
    pub local_model: String,
    pub prefer_local: bool,
    pub provider: String,
    pub openai_base: String,
    pub openai_key: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub embedding_enabled: bool,
    /// Unique in-process generation for one coherent provider configuration.
    /// Clones retain it; constructing/replacing a scheduler creates a new one.
    provider_config_generation: String,
    /// Non-secret credential identity version owned by canonical AppConfig.
    provider_credential_version: u64,
    /// Immutable identity captured when this executable provider generation is
    /// constructed. Public compatibility fields may still be read by existing
    /// projections, but mutating them can no longer silently alter the adapter
    /// target or credential used by a prepared request.
    provider_runtime_identity: ProviderRuntimeIdentity,
    /// Private verifier bound by the canonical ToolPermissionStore. The
    /// scheduler never owns or exposes the paired issuer.
    explicit_provider_probe_verifier: Option<crate::network_client::ExplicitProviderProbeVerifier>,
    /// Canonical provider/model routing authority.
    pub model_router: ModelRouter,
    /// Optional deterministic generation response for runtime harnesses.
    /// Production constructors leave this unset, so normal provider behavior is unchanged.
    pub scripted_generation_response: Option<String>,
    /// Bounded metadata-only adapter receipts. Product owners can replace this
    /// with a per-execution collector, but an unowned scheduler still cannot
    /// silently discard a real adapter invocation.
    pub(crate) provider_receipt_collector: ProviderReceiptCollector,
}

/// Scheduled-only provider executor. It owns an exact TaskStore/claim scope and
/// intentionally exposes no generic `execute_prepared*` API. Its public
/// scheduled-specific execution seam internally persists the exact
/// provider-start CAS before the HTTP edge can be entered.
#[derive(Clone)]
pub struct ScheduledInferenceScheduler {
    inner: InferenceScheduler,
    truth_scope: ScheduledProviderTruthScope,
}

fn configured_model_router(provider: &str, model: &str, has_configured_key: bool) -> ModelRouter {
    let mut router = ModelRouter::new();
    router.seed_configured_cloud_provider(provider, model, has_configured_key);
    router
}

#[derive(Clone, PartialEq, Eq)]
struct ProviderRuntimeIdentity {
    provider: String,
    model: String,
    endpoint: String,
    credential_version: u64,
    credential_identity: String,
}

#[derive(Debug)]
struct PreparedProviderGenerationMismatch {
    reason_code: &'static str,
}

impl std::fmt::Display for PreparedProviderGenerationMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "prepared provider request belongs to another config generation: {}",
            self.reason_code
        )
    }
}

impl std::error::Error for PreparedProviderGenerationMismatch {}

fn prepared_provider_generation_mismatch(reason_code: &'static str) -> anyhow::Error {
    anyhow::Error::new(PreparedProviderGenerationMismatch { reason_code })
}

impl ProviderRuntimeIdentity {
    fn capture(
        provider: &str,
        openai_base: &str,
        model: &str,
        configured_key: &str,
        credential_version: u64,
    ) -> Self {
        let provider = provider.trim().to_ascii_lowercase();
        let model = crate::llm::resolve_provider_chat_model(&provider, model);
        let endpoint = crate::llm::chat_completions_url(&provider, openai_base);
        let effective_key =
            crate::llm::effective_api_key_for_endpoint(&provider, openai_base, configured_key);
        Self {
            provider,
            model,
            endpoint,
            credential_version,
            credential_identity: crate::llm::provider_credential_identity(&effective_key),
        }
    }
}

impl Default for InferenceScheduler {
    fn default() -> Self {
        let provider = "openai".to_string();
        let openai_base = "https://api.openai.com/v1".to_string();
        let chat_model = crate::llm::resolve_provider_chat_model("openai", "gpt-4o-mini");
        let provider_config_generation = uuid::Uuid::new_v4().to_string();
        let provider_runtime_identity =
            ProviderRuntimeIdentity::capture(&provider, &openai_base, &chat_model, "", 0);
        Self {
            local_model: "qwen2.5:7b".into(),
            prefer_local: true,
            provider,
            openai_base,
            openai_key: "".into(),
            chat_model,
            embedding_model: "text-embedding-3-small".into(),
            embedding_enabled: true,
            provider_config_generation,
            provider_credential_version: 0,
            provider_runtime_identity,
            explicit_provider_probe_verifier: None,
            model_router: configured_model_router("openai", "gpt-4o-mini", false),
            scripted_generation_response: None,
            provider_receipt_collector: ProviderReceiptCollector::default(),
        }
    }
}

impl InferenceScheduler {
    pub fn provider_label(&self) -> String {
        crate::llm::provider_label(&self.provider)
    }

    pub fn effective_api_key(&self) -> String {
        let key = crate::llm::effective_api_key_for_endpoint(
            &self.provider,
            &self.openai_base,
            &self.openai_key,
        );
        if self.validate_provider_runtime_identity().is_ok() {
            key
        } else {
            String::new()
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub fn new(
        local_model: String,
        prefer_local: bool,
        provider: String,
        openai_base: String,
        openai_key: String,
        chat_model: String,
        embedding_model: String,
        embedding_enabled: bool,
    ) -> Self {
        let provider = provider.trim().to_ascii_lowercase();
        let chat_model = crate::llm::resolve_provider_chat_model(&provider, &chat_model);
        let model_router = configured_model_router(
            &provider,
            &chat_model,
            !crate::llm::effective_api_key_for_endpoint(&provider, &openai_base, &openai_key)
                .trim()
                .is_empty(),
        );
        let provider_config_generation = uuid::Uuid::new_v4().to_string();
        let provider_runtime_identity =
            ProviderRuntimeIdentity::capture(&provider, &openai_base, &chat_model, &openai_key, 0);
        Self {
            local_model,
            prefer_local,
            provider,
            openai_base,
            openai_key,
            chat_model,
            embedding_model,
            embedding_enabled,
            provider_config_generation,
            provider_credential_version: 0,
            provider_runtime_identity,
            explicit_provider_probe_verifier: None,
            model_router,
            scripted_generation_response: None,
            provider_receipt_collector: ProviderReceiptCollector::default(),
        }
    }

    pub fn with_model_router(mut self, mut router: ModelRouter) -> Self {
        router.seed_configured_cloud_provider(
            &self.provider,
            &self.chat_model,
            !self.effective_api_key().trim().is_empty(),
        );
        self.model_router = router;
        self
    }

    pub fn with_provider_credential_version(mut self, credential_version: u64) -> Self {
        self.provider_credential_version = credential_version;
        // Credential identity is part of the executable generation. Rotating
        // it must also fence clones that retained the earlier scheduler.
        self.provider_config_generation = uuid::Uuid::new_v4().to_string();
        self.provider_runtime_identity = ProviderRuntimeIdentity::capture(
            &self.provider,
            &self.openai_base,
            &self.chat_model,
            &self.openai_key,
            credential_version,
        );
        self
    }

    pub fn provider_config_generation(&self) -> &str {
        &self.provider_config_generation
    }

    pub fn provider_credential_version(&self) -> u64 {
        self.provider_credential_version
    }

    fn validate_provider_runtime_identity(&self) -> Result<()> {
        let observed = ProviderRuntimeIdentity::capture(
            &self.provider,
            &self.openai_base,
            &self.chat_model,
            &self.openai_key,
            self.provider_credential_version,
        );
        if observed != self.provider_runtime_identity {
            anyhow::bail!("provider runtime identity changed after generation construction");
        }
        Ok(())
    }

    pub fn provider_runtime_identity_is_valid(&self) -> bool {
        self.validate_provider_runtime_identity().is_ok()
    }

    pub(crate) fn with_explicit_provider_probe_verifier(
        mut self,
        verifier: crate::network_client::ExplicitProviderProbeVerifier,
    ) -> Self {
        self.explicit_provider_probe_verifier = Some(verifier);
        self
    }

    /// Return a non-authorizing description of this immutable generation. The
    /// canonical ToolPermissionStore may bind it to reviewed network consent;
    /// possessing the challenge alone cannot prepare or execute a request.
    pub fn explicit_provider_probe_challenge(
        &self,
    ) -> Result<crate::network_client::ExplicitProviderProbeChallenge> {
        self.validate_provider_runtime_identity()?;
        Ok(crate::network_client::ExplicitProviderProbeChallenge::new(
            self.provider_runtime_identity.provider.clone(),
            self.provider_runtime_identity.model.clone(),
            self.provider_runtime_identity.endpoint.clone(),
            self.provider_config_generation.clone(),
            self.provider_runtime_identity.credential_version,
            self.provider_runtime_identity.credential_identity.clone(),
        ))
    }

    pub fn with_scripted_generation_response(mut self, response: impl Into<String>) -> Self {
        self.scripted_generation_response = Some(response.into());
        self
    }

    pub fn with_provider_receipt_collector(mut self, collector: ProviderReceiptCollector) -> Self {
        self.provider_receipt_collector = collector;
        self
    }

    /// Create one execution-local scheduled provider truth authority. The
    /// returned handle can only take admissions issued by this scoped
    /// scheduler's real adapter hooks. Rebinding is rejected, and the isolated
    /// collector prevents timeout/cancel finalization from touching unrelated
    /// provider attempts.
    pub fn bind_scheduled_provider_truth_scope(
        mut self,
        store: Arc<TaskStore>,
        claim: Arc<ScheduledTaskClaim>,
    ) -> Result<(
        ScheduledInferenceScheduler,
        ScheduledProviderTruthAdmissionHandle,
    )> {
        if !store.owns_executing_claim(&claim)? {
            anyhow::bail!("scheduled provider scope requires the exact durable executing claim");
        }
        let claim_binding = ScheduledProviderTruthClaimBinding::capture(&claim)?;
        let state = Arc::new(Mutex::new(ScheduledProviderTruthAdmissionState::new(
            claim_binding,
        )));
        self.provider_receipt_collector = ProviderReceiptCollector::default();
        let handle = ScheduledProviderTruthAdmissionHandle {
            state: Arc::clone(&state),
            provider_receipt_collector: self.provider_receipt_collector.clone(),
        };
        Ok((
            ScheduledInferenceScheduler {
                inner: self,
                truth_scope: ScheduledProviderTruthScope {
                    state,
                    store,
                    claim,
                },
            },
            handle,
        ))
    }

    pub fn provider_receipts_snapshot(&self) -> Vec<ProviderInvocationReceipt> {
        self.provider_receipt_collector.snapshot()
    }

    /// Return runtime-only durability authority for exact adapter terminals.
    /// Public receipts alone are intentionally insufficient to write provider
    /// lifecycle facts.
    pub fn provider_durability_proofs_for_receipts(
        &self,
        receipts: &[ProviderInvocationReceipt],
    ) -> Result<Vec<ProviderInvocationDurabilityProof>> {
        receipts
            .iter()
            .map(|receipt| self.provider_durability_proof_for_receipt(receipt))
            .collect()
    }

    pub fn provider_durability_proof_for_receipt(
        &self,
        receipt: &ProviderInvocationReceipt,
    ) -> Result<ProviderInvocationDurabilityProof> {
        let proof = self
            .provider_receipt_collector
            .durability_proof_for_receipt(receipt)
            .ok_or_else(|| {
                anyhow::anyhow!("provider durability proof missing:{}", receipt.request_id)
            })?;
        proof.validate_runtime_adapter_terminal(receipt)?;
        Ok(proof)
    }

    /// Return start-only durability authority for cancellation or kernel
    /// failure terminalization. The caller may persist only the exact start and
    /// a runtime-owned `remote_unknown`; it cannot claim adapter completion.
    pub fn provider_durability_proof_for_start(
        &self,
        request_id: &str,
        provider: &str,
        model: &str,
        started_at: chrono::DateTime<chrono::Utc>,
        policy_evidence: &ProviderPolicyReceiptEvidence,
    ) -> Result<ProviderInvocationDurabilityProof> {
        let proof = self
            .provider_receipt_collector
            .durability_proof_for_start(request_id, provider, model, started_at, policy_evidence)
            .ok_or_else(|| {
                anyhow::anyhow!("provider durability start proof missing:{request_id}")
            })?;
        let lifecycle_evidence_digest = crate::llm::provider_lifecycle_evidence_digest(
            request_id,
            provider,
            model,
            policy_evidence,
        )?;
        proof.validate_runtime_start(
            request_id,
            provider,
            model,
            started_at,
            policy_evidence,
            &lifecycle_evidence_digest,
        )?;
        Ok(proof)
    }

    /// Synthetic terminal proof fixture for dependent-crate tests only.
    /// Normal/release builds do not contain this API, and production provider
    /// validation explicitly rejects its non-runtime origin.
    #[cfg(feature = "test-utils")]
    pub fn synthetic_explicit_probe_terminal_proof_for_test(
        &self,
        request: PreparedProviderRequest,
        status: ProviderInvocationStatus,
        finished_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<ProviderInvocationTerminalProof> {
        self.synthetic_explicit_probe_terminal_proof_with_started_at_for_test(
            request,
            status,
            finished_at - chrono::Duration::milliseconds(1),
            finished_at,
        )
    }

    /// Variant used to prove that typed adapter observation order remains
    /// authoritative when the system wall clock moves backwards between the
    /// start and terminal observations.
    #[cfg(feature = "test-utils")]
    pub fn synthetic_explicit_probe_terminal_proof_with_started_at_for_test(
        &self,
        request: PreparedProviderRequest,
        status: ProviderInvocationStatus,
        started_at: chrono::DateTime<chrono::Utc>,
        finished_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<ProviderInvocationTerminalProof> {
        let execution_binding = self.validate_prepared_execution_owner(&request)?;
        let terminal_binding =
            ProviderInvocationTerminalBinding::capture(&request, &execution_binding)?;
        let receipt = ProviderInvocationReceipt {
            request_id: request.context_manifest.request_id.clone(),
            provider: request.provider_target.clone(),
            model: request.model_target.clone(),
            status,
            started_at,
            finished_at,
            error_digest: (status != ProviderInvocationStatus::Completed)
                .then(|| provider_stream_error_digest("synthetic_test_provider_terminal")),
            simulated: false,
            policy_evidence: Some(request.policy_receipt_evidence()),
        };
        terminal_binding.issue(
            receipt,
            ProviderInvocationTerminalOrigin::SyntheticTestFixture,
        )
    }

    /// Prepare the one fixed payload used by the explicit Settings connection
    /// test. This is a policy seam, not another provider adapter: execution
    /// still goes through `execute_prepared`, which is the only owner of the
    /// HTTP-edge receipt.
    ///
    /// Unlike conversation generation, this command has no current-user
    /// message subject. Its private capability binds the exact configured
    /// provider/model/endpoint, the already-authorized network decision, the
    /// consent reference, and the literal `ping` payload.
    pub fn prepare_explicit_provider_probe(
        &self,
        grant: crate::network_client::ExplicitProviderProbeGrant,
    ) -> Result<PreparedProviderRequest> {
        self.validate_provider_runtime_identity()?;
        self.explicit_provider_probe_verifier
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "explicit provider probe verifier is not bound by ToolPermissionStore"
                )
            })?
            .verify(&grant)?;
        let provider_target = self.provider.trim().to_ascii_lowercase();
        let model_target = self.chat_model.trim().to_string();
        if provider_target.is_empty()
            || provider_target == "ollama"
            || model_target.is_empty()
            || self.effective_api_key().trim().is_empty()
        {
            anyhow::bail!("explicit provider probe cloud target is not configured");
        }
        let endpoint = crate::llm::chat_completions_url(&provider_target, &self.openai_base);
        if grant.provider_target() != provider_target
            || grant.model_target() != model_target
            || grant.endpoint() != endpoint
            || grant.provider_config_generation() != self.provider_config_generation
            || grant.credential_version() != self.provider_credential_version
            || grant.credential_identity() != self.provider_runtime_identity.credential_identity
        {
            anyhow::bail!("explicit provider probe grant differs from scheduler generation");
        }

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "ping".into(),
        }];
        let context_blocks = Vec::new();
        let policy_authorization = ProviderPolicyAuthorization::from_explicit_provider_probe(
            &grant,
            &messages,
            &context_blocks,
        )?;
        let network_policy_decision = grant.network_policy_decision();
        let decision_id = policy_authorization.decision_id().to_string();
        let provenance_digest = crate::agent::metadata_safe::metadata_safe_text_digest(&format!(
            "{}:{}:{:?}",
            policy_authorization.decision_id(),
            policy_authorization.policy_version(),
            policy_authorization.data_route(),
        ))
        .1;
        let context_manifest = ContextManifest {
            request_id: uuid::Uuid::new_v4().to_string(),
            privacy_decision_id: decision_id.clone(),
            selected_context_refs: Vec::new(),
            included_context_categories: Vec::new(),
            declared_payload_categories: vec![ProviderPayloadCategory::ExplicitProviderProbe],
            policy_provenance_refs: vec![ProviderPolicyProvenanceRef::new(
                ProviderPolicyProvenanceKind::ExplicitProviderProbeDecision,
                decision_id,
                provenance_digest,
            )],
            raw_life_model_included: false,
            raw_unbounded_memory_included: false,
        };
        context_manifest.validate_context_truth(&context_blocks)?;
        policy_authorization.validate_unfiltered_payload(&messages, &context_blocks)?;
        policy_authorization.validate_explicit_provider_probe_target(
            &provider_target,
            &model_target,
            &endpoint,
            network_policy_decision,
        )?;
        let policy_authorization = policy_authorization.bind_prepared_envelope(
            &messages,
            &context_blocks,
            &context_manifest,
            &provider_target,
            &model_target,
            &endpoint,
            &self.provider_config_generation,
            self.provider_credential_version,
            false,
        )?;
        let (network_policy, network_policy_decision) = grant.into_network_authority();
        let request = PreparedProviderRequest {
            messages,
            context_blocks,
            context_manifest,
            provider_target,
            model_target,
            provider_endpoint: endpoint.clone(),
            provider_config_generation: self.provider_config_generation.clone(),
            provider_credential_version: self.provider_credential_version,
            data_route: ProviderDataRoute::PolicyAllowed,
            policy_authorization,
            network_policy,
            network_policy_decision,
            tools_required: false,
            execution_binding: Some(crate::llm::ProviderExecutionBinding::new(
                endpoint,
                self.effective_api_key(),
                self.provider_config_generation.clone(),
                self.provider_credential_version,
            )),
        };
        request.validate()?;
        Ok(request)
    }

    /// Resolve one concrete provider/model target and bind it to already
    /// selected context plus a mechanically validated policy capability. This
    /// method never accepts a naked route enum as authorization.
    pub async fn prepare_chat_request_with_authorization(
        &self,
        messages: Vec<ChatMessage>,
        context_blocks: Vec<BoundedContextBlock>,
        context_manifest: ContextManifest,
        policy_authorization: ProviderPolicyAuthorization,
        network_policy: NetworkPolicy,
        tools_required: bool,
    ) -> Result<PreparedProviderRequest> {
        self.prepare_chat_request_with_authorized_filter(
            messages,
            context_blocks,
            context_manifest,
            policy_authorization,
            network_policy,
            tools_required,
            |_, _, _, _| Ok(()),
        )
        .await
        .map(|(request, ())| request)
    }

    /// Resolve the provider first, apply one bounded synchronous privacy/context
    /// filter, then seal the exact resulting payload into the authorization
    /// envelope. Callers cannot mutate the prepared payload afterwards without
    /// invalidating adapter-edge validation.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub async fn prepare_chat_request_with_authorized_filter<T, F>(
        &self,
        mut messages: Vec<ChatMessage>,
        mut context_blocks: Vec<BoundedContextBlock>,
        mut context_manifest: ContextManifest,
        policy_authorization: ProviderPolicyAuthorization,
        network_policy: NetworkPolicy,
        tools_required: bool,
        payload_filter: F,
    ) -> Result<(PreparedProviderRequest, T)>
    where
        F: FnOnce(
            &str,
            &mut Vec<ChatMessage>,
            &mut Vec<BoundedContextBlock>,
            &mut ContextManifest,
        ) -> Result<T>,
    {
        self.validate_provider_runtime_identity()?;
        // Validate the canonical subject or an explicitly scoped derived
        // payload before provider selection, privacy filtering, credential
        // lookup, or any adapter edge can be reached.
        context_manifest.validate_context_truth(&context_blocks)?;
        policy_authorization.validate_unfiltered_payload(&messages, &context_blocks)?;
        let data_route = policy_authorization.data_route();
        let tools_route_marker = tools_required.then_some("typed_tool_contract");
        let mut prepared_ollama_target = None;
        let (provider_target, model_target) = if self.scripted_generation_response.is_some() {
            if data_route == ProviderDataRoute::LocalOnly {
                if self.local_model.trim().is_empty() {
                    anyhow::bail!("scripted local-only provider route has no configured model");
                }
                ("ollama".to_string(), self.local_model.clone())
            } else {
                // A scripted response is an eval fixture, not an adapter call.
                // Preserve an available router target when one exists, but do
                // not require live credentials merely to run the fixture.
                self.model_router
                    .route_chat(tools_route_marker, self.prefer_local)
                    .map(|decision| (decision.provider, decision.model))
                    .unwrap_or_else(|_| {
                        if self.prefer_local && !self.local_model.trim().is_empty() {
                            ("ollama".to_string(), self.local_model.clone())
                        } else {
                            (self.provider.clone(), self.chat_model.clone())
                        }
                    })
            }
        } else if data_route == ProviderDataRoute::LocalOnly {
            let target = prepare_ollama_chat_target(&self.local_model)
                .await
                .ok_or_else(|| anyhow::anyhow!("local-only provider route is unavailable"))?;
            let model = target.model.clone();
            prepared_ollama_target = Some(target);
            ("ollama".to_string(), model)
        } else if self.prefer_local {
            let target = prepare_ollama_chat_target(&self.local_model)
                .await
                .ok_or_else(|| anyhow::anyhow!("selected local provider is unavailable"))?;
            let model = target.model.clone();
            prepared_ollama_target = Some(target);
            ("ollama".to_string(), model)
        } else {
            let decision = self
                .model_router
                .route_chat(tools_route_marker, self.prefer_local)?;
            if decision.provider == "ollama" {
                let target = prepare_ollama_chat_target(&self.local_model)
                    .await
                    .ok_or_else(|| anyhow::anyhow!("selected local provider is unavailable"))?;
                let model = target.model.clone();
                prepared_ollama_target = Some(target);
                ("ollama".to_string(), model)
            } else {
                if decision.provider != self.provider || decision.model != self.chat_model {
                    anyhow::bail!(
                        "provider route target does not match the configured cloud adapter"
                    );
                }
                (decision.provider, decision.model)
            }
        };

        let provider_endpoint = if provider_target == "ollama" {
            prepared_ollama_target
                .map(|target| target.endpoint)
                // Scripted fixtures never cross an adapter edge and therefore
                // retain a non-routable local marker rather than discovering a
                // deployment they will not use.
                .unwrap_or_else(|| "local://ollama".to_string())
        } else {
            crate::llm::chat_completions_url(&provider_target, &self.openai_base)
        };
        let network_capability = format!("provider.{provider_target}");
        let network_policy_decision = if provider_target == "ollama" {
            crate::network_client::NetworkPolicyDecision::local_only(&network_capability)
        } else {
            crate::network_client::resolve_network_policy_decision(
                &network_policy,
                &provider_endpoint,
                &network_capability,
            )?
        };
        policy_authorization.validate_explicit_provider_probe_target(
            &provider_target,
            &model_target,
            &provider_endpoint,
            &network_policy_decision,
        )?;
        let filter_output = payload_filter(
            &provider_target,
            &mut messages,
            &mut context_blocks,
            &mut context_manifest,
        )?;
        context_manifest.validate_context_truth(&context_blocks)?;
        let policy_authorization = policy_authorization.bind_prepared_envelope(
            &messages,
            &context_blocks,
            &context_manifest,
            &provider_target,
            &model_target,
            &provider_endpoint,
            &self.provider_config_generation,
            self.provider_credential_version,
            tools_required,
        )?;
        let execution_api_key = if provider_target == "ollama" {
            String::new()
        } else {
            self.effective_api_key()
        };
        let request = PreparedProviderRequest {
            messages,
            context_blocks,
            context_manifest,
            provider_target,
            model_target,
            provider_endpoint: provider_endpoint.clone(),
            provider_config_generation: self.provider_config_generation.clone(),
            provider_credential_version: self.provider_credential_version,
            data_route,
            policy_authorization,
            network_policy,
            network_policy_decision,
            tools_required,
            execution_binding: Some(crate::llm::ProviderExecutionBinding::new(
                provider_endpoint,
                execution_api_key,
                self.provider_config_generation.clone(),
                self.provider_credential_version,
            )),
        };
        request.validate()?;
        Ok((request, filter_output))
    }

    fn validate_prepared_execution_owner(
        &self,
        request: &PreparedProviderRequest,
    ) -> Result<crate::llm::ProviderExecutionBinding> {
        if self.validate_provider_runtime_identity().is_err() {
            return Err(prepared_provider_generation_mismatch(
                "scheduler_runtime_identity_changed",
            ));
        }
        request.validate()?;
        if request.provider_config_generation != self.provider_config_generation
            || request.provider_credential_version != self.provider_credential_version
        {
            return Err(prepared_provider_generation_mismatch(
                "generation_or_credential_version_changed",
            ));
        }
        let binding = request
            .execution_binding
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("prepared provider execution binding is absent"))?;
        if request.provider_target == "ollama" {
            let scripted_local_fixture = self.scripted_generation_response.is_some()
                && request.provider_endpoint == "local://ollama";
            if !scripted_local_fixture {
                crate::ollama::validate_prepared_ollama_chat_endpoint(&request.provider_endpoint)?;
            }
            if binding.endpoint() != request.provider_endpoint || !binding.api_key().is_empty() {
                return Err(prepared_provider_generation_mismatch(
                    "local_endpoint_binding_changed",
                ));
            }
        } else {
            let expected_endpoint = &self.provider_runtime_identity.endpoint;
            if request.provider_endpoint.as_str() != expected_endpoint.as_str()
                || binding.api_key() != self.effective_api_key()
                || binding.endpoint() != expected_endpoint
            {
                return Err(prepared_provider_generation_mismatch(
                    "cloud_endpoint_or_credential_changed",
                ));
            }
        }
        Ok(binding.clone())
    }

    async fn generate_prepared_inner<F>(
        &self,
        request: PreparedProviderRequest,
        execution_binding: crate::llm::ProviderExecutionBinding,
        on_started: F,
    ) -> Result<String>
    where
        F: FnOnce() -> Result<()>,
    {
        let system_prompt = request.system_prompt();
        let request_id = request.context_manifest.request_id.clone();
        let payload_purpose = request.policy_receipt_evidence().payload_purpose;
        // Artifact drafts use the ordinary model response transport. OpenLife
        // still requires the exact artifact envelope in the trusted prompt and
        // rejects the response locally unless the file-specific parser and
        // schema checks pass. This avoids coupling successful artifact delivery
        // to a provider-native JSON/thinking mode; those smaller native modes
        // remain useful for work-plan and evidence-check envelopes.
        let structured_json_output = cloud_provider_uses_native_structured_output(payload_purpose);
        let provider_native_json_mode = structured_json_output;

        if self.scripted_generation_response.is_some()
            && (payload_purpose == Some(ProviderPayloadPurpose::MainChatWorkPlan)
                || system_prompt
                    .as_deref()
                    .is_some_and(|prompt| prompt.contains("openlife.work-plan.v2")))
        {
            // The scripted provider is a test fixture for the execution
            // response, not a second queue of planner responses. Keep its
            // planner surface typed and deterministic so product-path tests
            // still exercise plan admission without teaching fixtures to
            // masquerade as a real adaptive model.
            return Ok(
                r#"{"schemaVersion":"openlife.work-plan.v2","steps":[{"id":"analyze","kind":"analyze","required":true,"dependsOn":[]},{"id":"deliver","kind":"deliver_result","required":true,"dependsOn":["analyze"]}],"completion":{"resultKind":"answer","requiresVerification":false}}"#
                    .to_string(),
            );
        }

        if let Some(ref response) = self.scripted_generation_response {
            return Ok(response.clone());
        }

        if request.provider_target == "ollama" {
            let deterministic_output = non_streaming_ollama_requires_deterministic_output(
                payload_purpose,
                &request.context_manifest.included_context_categories,
            );
            return crate::ollama::chat_with_ollama_raw_at_endpoint_with_start_observer(
                execution_binding.endpoint(),
                &request.model_target,
                request.messages,
                system_prompt.as_deref(),
                crate::ollama::OllamaOutputContract {
                    structured_format: match payload_purpose {
                        Some(ProviderPayloadPurpose::MainChatEvidenceCheck) => {
                            Some(crate::ollama::main_chat_evidence_check_json_schema())
                        }
                        Some(ProviderPayloadPurpose::MainChatArtifactDraft) => {
                            Some(serde_json::Value::String("json".into()))
                        }
                        Some(ProviderPayloadPurpose::MainChatWorkPlan) => {
                            Some(crate::ollama::main_chat_work_plan_json_schema())
                        }
                        _ => None,
                    },
                    deterministic: deterministic_output,
                },
                Some(&request_id),
                on_started,
            )
            .await;
        }

        if request.provider_target != self.provider || request.model_target != self.chat_model {
            anyhow::bail!("prepared provider target does not match the configured cloud adapter");
        }

        crate::llm::chat_with_openrouter_raw_with_start_observer(
            crate::llm::OpenAiCompatibleAdapterRequest {
                messages: request.messages,
                system_prompt: system_prompt.as_deref(),
                provider: &request.provider_target,
                endpoint: execution_binding.endpoint(),
                api_key: execution_binding.api_key(),
                model: &request.model_target,
                structured_json_output,
                provider_native_json_mode,
                network_policy: &request.network_policy,
                network_policy_decision: &request.network_policy_decision,
                request_id: Some(&request_id),
            },
            on_started,
        )
        .await
    }

    /// Execute a prepared request and derive provider truth at the adapter boundary.
    pub async fn execute_prepared(
        &self,
        request: PreparedProviderRequest,
    ) -> PreparedProviderOutcome {
        self.execute_prepared_with_observer(request, |_| Ok(()))
            .await
    }

    /// Execute a prepared request and synchronously expose the exact adapter-edge
    /// start fact before the external future is awaited.
    pub async fn execute_prepared_with_start_observer<F>(
        &self,
        request: PreparedProviderRequest,
        on_started: F,
    ) -> PreparedProviderOutcome
    where
        F: FnOnce(
            &str,
            &str,
            &str,
            chrono::DateTime<chrono::Utc>,
            &ProviderPolicyReceiptEvidence,
        ) -> Result<()>,
    {
        let mut on_started = Some(on_started);
        self.execute_prepared_with_observer(request, move |progress| {
            if let ProviderInvocationProgress::Started {
                request_id,
                provider,
                model,
                started_at,
                policy_evidence,
            } = progress
            {
                if let Some(on_started) = on_started.take() {
                    return on_started(
                        &request_id,
                        &provider,
                        &model,
                        started_at,
                        &policy_evidence,
                    );
                }
            }
            Ok(())
        })
        .await
    }

    /// Execute a prepared request while exposing start and terminal facts at
    /// their actual adapter boundaries. Observers must remain metadata-only and
    /// must not block the provider future.
    pub async fn execute_prepared_with_observer<F>(
        &self,
        request: PreparedProviderRequest,
        on_progress: F,
    ) -> PreparedProviderOutcome
    where
        F: FnMut(ProviderInvocationProgress) -> Result<()>,
    {
        self.execute_prepared_with_observer_and_scope(request, on_progress, None)
            .await
    }

    async fn execute_prepared_with_observer_and_scope<F>(
        &self,
        request: PreparedProviderRequest,
        mut on_progress: F,
        scheduled_scope: Option<&ScheduledProviderTruthScope>,
    ) -> PreparedProviderOutcome
    where
        F: FnMut(ProviderInvocationProgress) -> Result<()>,
    {
        let request_authority = request.policy_authorization().authority();
        match (scheduled_scope.is_some(), request_authority) {
            (false, ProviderPolicyAuthority::ScheduledPolicy) => {
                return PreparedProviderOutcome {
                    receipt: None,
                    terminal_proof: None,
                    result: Err(
                        "provider_pre_dispatch_rejected:scheduled_policy_requires_scheduled_executor"
                            .into(),
                    ),
                };
            }
            (true, ProviderPolicyAuthority::ScheduledPolicy) => {}
            (true, _) => {
                return PreparedProviderOutcome {
                    receipt: None,
                    terminal_proof: None,
                    result: Err(
                        "provider_pre_dispatch_rejected:scheduled_executor_requires_scheduled_policy"
                            .into(),
                    ),
                };
            }
            (false, _) => {}
        }
        let execution_binding = match self.validate_prepared_execution_owner(&request) {
            Ok(binding) => binding,
            Err(error) => {
                return PreparedProviderOutcome {
                    receipt: None,
                    terminal_proof: None,
                    result: Err(format!("provider_pre_dispatch_rejected:{error}")),
                };
            }
        };
        let request_id = request.context_manifest.request_id.clone();
        let provider = request.provider_target.clone();
        let model = request.model_target.clone();
        let simulated = self.scripted_generation_response.is_some();
        let policy_evidence = request.policy_receipt_evidence();
        let terminal_binding =
            match ProviderInvocationTerminalBinding::capture(&request, &execution_binding) {
                Ok(binding) => binding,
                Err(error) => {
                    return PreparedProviderOutcome {
                        receipt: None,
                        terminal_proof: None,
                        result: Err(format!(
                            "provider_terminal_binding_pre_dispatch_rejected:{error}"
                        )),
                    };
                }
            };
        let scheduled_truth_binding = match scheduled_scope {
            Some(scope) => match scope.capture_prepared(&request) {
                Ok(binding) => Some(binding),
                Err(error) => {
                    return PreparedProviderOutcome {
                        receipt: None,
                        terminal_proof: None,
                        result: Err(format!(
                            "scheduled_provider_truth_pre_dispatch_rejected:{error}"
                        )),
                    };
                }
            },
            None => None,
        };
        let mut started_at = None;
        let execution_result = self
            .generate_prepared_inner(request, execution_binding, || {
                let observed_at = chrono::Utc::now();
                let attempt = ProviderStartedAttempt {
                    request_id: request_id.clone(),
                    provider: provider.clone(),
                    model: model.clone(),
                    started_at: observed_at,
                    policy_evidence: policy_evidence.clone(),
                };
                self.provider_receipt_collector
                    .record_started(attempt.clone())?;
                if let (Some(scope), Some(binding)) =
                    (scheduled_scope, scheduled_truth_binding.as_ref())
                {
                    if let Err(error) = scope.record_started(binding.clone(), attempt.clone()) {
                        self.provider_receipt_collector.discard_started(&attempt);
                        return Err(error);
                    }
                }
                let progress = ProviderInvocationProgress::Started {
                    request_id: attempt.request_id.clone(),
                    provider: attempt.provider.clone(),
                    model: attempt.model.clone(),
                    started_at: attempt.started_at,
                    policy_evidence: attempt.policy_evidence.clone(),
                };
                if let Some(scope) = scheduled_scope {
                    if let Err(error) = scope.persist_registered_progress(&progress) {
                        self.provider_receipt_collector.discard_started(&attempt);
                        scope.discard_started(&attempt);
                        return Err(error);
                    }
                }
                if let Err(error) = on_progress(progress) {
                    self.provider_receipt_collector.discard_started(&attempt);
                    if let Some(scope) = scheduled_scope {
                        scope.discard_started(&attempt);
                    }
                    return Err(error);
                }
                started_at = Some(observed_at);
                Ok(())
            })
            .await;
        let finished_at = chrono::Utc::now();
        let (status, error_digest) = match execution_result.as_ref() {
            Ok(_) => (ProviderInvocationStatus::Completed, None),
            Err(error) => (
                crate::llm::provider_error_terminal_status(error),
                Some(provider_stream_error_digest(&error.to_string())),
            ),
        };
        let mut result = execution_result.map_err(|error| error.to_string());
        let receipt = started_at.map(|started_at| ProviderInvocationReceipt {
            request_id,
            provider,
            model,
            status,
            started_at,
            finished_at,
            error_digest,
            simulated,
            policy_evidence: Some(policy_evidence),
        });
        let receipt =
            receipt.map(|receipt| self.provider_receipt_collector.record_terminal(receipt));
        let mut terminal_proof = None;
        if let Some(receipt) = receipt.as_ref() {
            if !receipt.simulated {
                match terminal_binding.issue(
                    receipt.clone(),
                    ProviderInvocationTerminalOrigin::RuntimeAdapter,
                ) {
                    Ok(proof) => {
                        if let Err(error) = self
                            .provider_receipt_collector
                            .record_terminal_proof(proof.clone())
                        {
                            result =
                                Err(format!("provider_terminal_proof_retention_failed:{error}"));
                        } else {
                            terminal_proof = Some(proof);
                        }
                    }
                    Err(error) => {
                        result = Err(format!("provider_terminal_proof_issuance_failed:{error}"));
                    }
                }
            }
            if let (Some(scope), Some(binding), Some(proof)) = (
                scheduled_scope,
                scheduled_truth_binding.as_ref(),
                terminal_proof.as_ref(),
            ) {
                if let Err(error) = scope.record_adapter_terminal(binding, proof) {
                    result = Err(format!(
                        "scheduled_provider_truth_terminal_admission_failed:{error}"
                    ));
                } else {
                    let progress = match receipt.status {
                        ProviderInvocationStatus::Completed => {
                            ProviderInvocationProgress::Completed(receipt.clone())
                        }
                        ProviderInvocationStatus::Failed => {
                            ProviderInvocationProgress::Failed(receipt.clone())
                        }
                        ProviderInvocationStatus::RemoteUnknown => {
                            ProviderInvocationProgress::RemoteUnknown(receipt.clone())
                        }
                    };
                    if let Err(error) = scope.persist_registered_progress(&progress) {
                        result = Err(format!(
                            "scheduled_provider_truth_terminal_persistence_failed:{error}"
                        ));
                    }
                }
            }
            if receipt.status != ProviderInvocationStatus::Completed && result.is_ok() {
                result = Err("provider_terminal_race_remote_state_unknown".into());
            }
            if let Err(observer_error) = on_progress(match receipt.status {
                ProviderInvocationStatus::Completed => {
                    ProviderInvocationProgress::Completed(receipt.clone())
                }
                ProviderInvocationStatus::Failed => {
                    ProviderInvocationProgress::Failed(receipt.clone())
                }
                ProviderInvocationStatus::RemoteUnknown => {
                    ProviderInvocationProgress::RemoteUnknown(receipt.clone())
                }
            }) {
                result = Err(format!(
                    "provider_progress_observer_failed: {observer_error}"
                ));
            }
        }
        PreparedProviderOutcome {
            receipt,
            terminal_proof,
            result,
        }
    }

    fn bind_prepared_provider_stream(
        &self,
        result: Result<StreamResult>,
        seed: Option<ProviderStartedAttempt>,
        terminal_binding: ProviderInvocationTerminalBinding,
    ) -> Result<PreparedProviderStream> {
        match (result, seed) {
            (Ok(stream), Some(seed)) => Ok(Box::pin(ReceiptRetainingProviderStream {
                inner: stream,
                seed,
                collector: self.provider_receipt_collector.clone(),
                terminal_binding: Some(terminal_binding),
                terminal_recorded: false,
            })),
            (Ok(_), None) => {
                anyhow::bail!("provider stream opened without an adapter-start receipt")
            }
            (Err(error), Some(seed)) => {
                let error_message = error.to_string();
                let receipt = seed.terminal_receipt(
                    crate::llm::provider_error_terminal_status(&error),
                    Some(&error_message),
                );
                let proposed_status = receipt.status;
                let canonical_receipt = self.provider_receipt_collector.record_terminal(receipt);
                let proof = terminal_binding.issue(
                    canonical_receipt.clone(),
                    ProviderInvocationTerminalOrigin::RuntimeAdapter,
                )?;
                self.provider_receipt_collector
                    .record_terminal_proof(proof)?;
                let proposed_terminal_won = canonical_receipt.status == proposed_status;
                let terminal = PreparedProviderStreamTerminal::from_receipt(
                    canonical_receipt,
                    proposed_terminal_won.then_some(error_message),
                );
                Ok(Box::pin(futures::stream::once(async move {
                    PreparedProviderStreamEvent::Terminal(terminal)
                })))
            }
            (Err(error), None) => Err(error),
        }
    }

    pub async fn generate_prepared_stream_with_start_observer<F>(
        &self,
        request: PreparedProviderRequest,
        on_started: F,
    ) -> Result<PreparedProviderStream>
    where
        F: FnOnce(
            &str,
            &str,
            &str,
            chrono::DateTime<chrono::Utc>,
            &ProviderPolicyReceiptEvidence,
        ) -> Result<()>,
    {
        let execution_binding = self.validate_prepared_execution_owner(&request)?;
        let terminal_binding =
            ProviderInvocationTerminalBinding::capture(&request, &execution_binding)?;
        let system_prompt = request.system_prompt();
        let request_id = request.context_manifest.request_id.clone();
        let provider = request.provider_target.clone();
        let model = request.model_target.clone();
        let policy_evidence = request.policy_receipt_evidence();
        let mut on_started = Some(on_started);

        if let Some(response) = self.scripted_generation_response.clone() {
            return Ok(Box::pin(futures::stream::iter([
                PreparedProviderStreamEvent::Token(response),
                PreparedProviderStreamEvent::Terminal(PreparedProviderStreamTerminal::NotAttempted),
            ])));
        }

        if request.provider_target == "ollama" {
            let mut started_at = None;
            let result =
                crate::ollama::chat_with_ollama_raw_stream_at_endpoint_with_start_observer(
                    execution_binding.endpoint(),
                    &request.model_target,
                    request.messages,
                    system_prompt.as_deref(),
                    Some(&request_id),
                    || {
                        let observed_at = chrono::Utc::now();
                        let attempt = ProviderStartedAttempt {
                            request_id: request_id.clone(),
                            provider: provider.clone(),
                            model: model.clone(),
                            started_at: observed_at,
                            policy_evidence: policy_evidence.clone(),
                        };
                        self.provider_receipt_collector
                            .record_started(attempt.clone())?;
                        if let Some(on_started) = on_started.take() {
                            if let Err(error) = on_started(
                                &request_id,
                                &provider,
                                &model,
                                observed_at,
                                &policy_evidence,
                            ) {
                                self.provider_receipt_collector.discard_started(&attempt);
                                return Err(error);
                            }
                        }
                        started_at = Some(observed_at);
                        Ok(())
                    },
                )
                .await;
            let seed = started_at.map(|started_at| ProviderStartedAttempt {
                request_id,
                provider,
                model,
                started_at,
                policy_evidence,
            });
            return self.bind_prepared_provider_stream(result, seed, terminal_binding);
        }

        if request.provider_target != self.provider || request.model_target != self.chat_model {
            anyhow::bail!("prepared provider target does not match the configured cloud adapter");
        }

        let mut started_at = None;
        let result = crate::llm::chat_with_openrouter_raw_stream_with_start_observer(
            crate::llm::OpenAiCompatibleAdapterRequest {
                messages: request.messages,
                system_prompt: system_prompt.as_deref(),
                provider: &request.provider_target,
                endpoint: execution_binding.endpoint(),
                api_key: execution_binding.api_key(),
                model: &request.model_target,
                structured_json_output: matches!(
                    policy_evidence.payload_purpose,
                    Some(
                        ProviderPayloadPurpose::MainChatArtifactDraft
                            | ProviderPayloadPurpose::MainChatEvidenceCheck
                    )
                ),
                provider_native_json_mode: matches!(
                    policy_evidence.payload_purpose,
                    Some(ProviderPayloadPurpose::MainChatEvidenceCheck)
                ),
                network_policy: &request.network_policy,
                network_policy_decision: &request.network_policy_decision,
                request_id: Some(&request_id),
            },
            || {
                let observed_at = chrono::Utc::now();
                let attempt = ProviderStartedAttempt {
                    request_id: request_id.clone(),
                    provider: provider.clone(),
                    model: model.clone(),
                    started_at: observed_at,
                    policy_evidence: policy_evidence.clone(),
                };
                self.provider_receipt_collector
                    .record_started(attempt.clone())?;
                if let Some(on_started) = on_started.take() {
                    if let Err(error) = on_started(
                        &request_id,
                        &provider,
                        &model,
                        observed_at,
                        &policy_evidence,
                    ) {
                        self.provider_receipt_collector.discard_started(&attempt);
                        return Err(error);
                    }
                }
                started_at = Some(observed_at);
                Ok(())
            },
        )
        .await;
        let seed = started_at.map(|started_at| ProviderStartedAttempt {
            request_id,
            provider,
            model,
            started_at,
            policy_evidence,
        });
        self.bind_prepared_provider_stream(result, seed, terminal_binding)
    }

    /// Preview the routing decision for a chat request without actually calling the LLM.
    /// Returns a ModelRouteTrace describing which backend would be chosen and why.
    pub async fn preview_chat_route(&self, tools_prompt: Option<&str>) -> ModelRouteTrace {
        if self.prefer_local {
            return match resolve_ollama_model(&self.local_model).await {
                Some(model) => ModelRouteTrace {
                    provider: "ollama".into(),
                    model,
                    route_type: "local".into(),
                    prefer_local: true,
                    local_model: self.local_model.clone(),
                    reason: "user_selected_local_model_available".into(),
                    privacy_level: crate::agent::types::RedactionLevel::None,
                    latency_ms: None,
                    retry_count: 0,
                    fallback_reason: None,
                    provider_health_is_estimated: Some(false),
                },
                None => ModelRouteTrace {
                    provider: "none".into(),
                    model: String::new(),
                    route_type: "blocked".into(),
                    prefer_local: true,
                    local_model: self.local_model.clone(),
                    reason: "selected_local_provider_unavailable".into(),
                    privacy_level: crate::agent::types::RedactionLevel::Strict,
                    latency_ms: None,
                    retry_count: 0,
                    fallback_reason: None,
                    provider_health_is_estimated: Some(false),
                },
            };
        }
        match self.model_router.route_chat(tools_prompt, false) {
            Ok(decision) => decision.to_trace(),
            Err(error) => ModelRouteTrace {
                provider: "none".into(),
                model: String::new(),
                route_type: "blocked".into(),
                prefer_local: self.prefer_local,
                local_model: self.local_model.clone(),
                reason: format!("model_router_blocked:{error}"),
                privacy_level: crate::agent::types::RedactionLevel::Strict,
                latency_ms: None,
                retry_count: 0,
                fallback_reason: None,
                provider_health_is_estimated: Some(false),
            },
        }
    }
}

impl ScheduledInferenceScheduler {
    /// Prepare a request under the exact scheduled policy capability. This is
    /// side-effect free; execution remains owned by the crate-private durable
    /// scheduled seam below.
    pub async fn prepare_scheduled_chat_request(
        &self,
        messages: Vec<ChatMessage>,
        context_blocks: Vec<BoundedContextBlock>,
        context_manifest: ContextManifest,
        policy_authorization: ProviderPolicyAuthorization,
        network_policy: NetworkPolicy,
        tools_required: bool,
    ) -> Result<PreparedProviderRequest> {
        self.inner
            .prepare_chat_request_with_authorization(
                messages,
                context_blocks,
                context_manifest,
                policy_authorization,
                network_policy,
                tools_required,
            )
            .await
    }

    pub async fn execute_scheduled_provider_request(
        &self,
        request: PreparedProviderRequest,
    ) -> PreparedProviderOutcome {
        self.inner
            .execute_prepared_with_observer_and_scope(request, |_| Ok(()), Some(&self.truth_scope))
            .await
    }

    #[cfg(test)]
    fn truth_scope(&self) -> &ScheduledProviderTruthScope {
        &self.truth_scope
    }
}

fn non_streaming_ollama_requires_deterministic_output(
    payload_purpose: Option<ProviderPayloadPurpose>,
    included_context_categories: &[String],
) -> bool {
    matches!(
        payload_purpose,
        Some(
            ProviderPayloadPurpose::MainChatDirectAnswer
                | ProviderPayloadPurpose::AgentMemoryExtraction
                | ProviderPayloadPurpose::MainChatEvidenceCheck
                | ProviderPayloadPurpose::MainChatArtifactDraft
        )
    ) || included_context_categories
        .iter()
        .any(|category| category == crate::web_search::WEB_SEARCH_CONTEXT_CATEGORY)
}

fn cloud_provider_uses_native_structured_output(
    payload_purpose: Option<ProviderPayloadPurpose>,
) -> bool {
    matches!(
        payload_purpose,
        Some(
            ProviderPayloadPurpose::MainChatEvidenceCheck
                | ProviderPayloadPurpose::MainChatWorkPlan
                | ProviderPayloadPurpose::AgentMemoryExtraction
        )
    )
}

#[cfg(test)]
mod tests {
    use super::{
        cloud_provider_uses_native_structured_output,
        non_streaming_ollama_requires_deterministic_output, InferenceScheduler,
        PreparedProviderGenerationMismatch, PreparedProviderStreamEvent,
        PreparedProviderStreamTerminal, ProviderInvocationProgress,
        ProviderInvocationTerminalBinding, ProviderStartedAttempt, ScheduledInferenceScheduler,
        ScheduledPreparedProviderTruthBinding, ScheduledProviderLocalAbortCause,
        ScheduledProviderTruthAdmissionState, ScheduledProviderTruthClaimBinding,
        MAX_SCHEDULED_PROVIDER_TRUTH_ATTEMPTS,
    };
    use crate::agent::{ModelRouter, ProviderAvailability};
    use crate::llm::{
        BoundedContextBlock, ChatMessage, ContextManifest, ProviderDataRoute,
        ProviderInvocationStatus, ProviderLocalOnlyReason, ProviderPayloadPurpose,
        ProviderPolicyAuthorization, ProviderPolicyReceiptEvidence,
    };
    use crate::tasks::{ScheduledTask, ScheduledTaskClaim, TaskStore};
    use futures::StreamExt;
    use std::sync::Arc;

    #[test]
    fn non_streaming_direct_answer_uses_deterministic_local_generation() {
        assert!(non_streaming_ollama_requires_deterministic_output(
            Some(ProviderPayloadPurpose::MainChatDirectAnswer),
            &[],
        ));
        assert!(non_streaming_ollama_requires_deterministic_output(
            Some(ProviderPayloadPurpose::MainChatArtifactDraft),
            &[],
        ));
        assert!(!non_streaming_ollama_requires_deterministic_output(
            Some(ProviderPayloadPurpose::ScheduledTaskStep),
            &[],
        ));
    }

    #[test]
    fn cloud_artifact_draft_keeps_local_schema_without_native_json_mode() {
        assert!(!cloud_provider_uses_native_structured_output(Some(
            ProviderPayloadPurpose::MainChatArtifactDraft
        )));
        assert!(cloud_provider_uses_native_structured_output(Some(
            ProviderPayloadPurpose::MainChatWorkPlan
        )));
        assert!(cloud_provider_uses_native_structured_output(Some(
            ProviderPayloadPurpose::MainChatEvidenceCheck
        )));
    }

    fn allow_network_policy() -> crate::config::NetworkPolicy {
        crate::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..crate::config::NetworkPolicy::default()
        }
    }

    fn governed_probe(
        scheduler: InferenceScheduler,
        policy: crate::config::NetworkPolicy,
        decision: crate::network_client::NetworkPolicyDecision,
    ) -> (
        InferenceScheduler,
        crate::network_client::ExplicitProviderProbeGrant,
    ) {
        let store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let scheduler = store.bind_explicit_provider_probe_scheduler(scheduler);
        let challenge = scheduler.explicit_provider_probe_challenge().unwrap();
        let grant = store
            .issue_explicit_provider_probe_grant(
                challenge,
                policy,
                &decision,
                decision.clone(),
                None,
            )
            .unwrap();
        (scheduler, grant)
    }

    fn canonical_cloud_subject_authorization(
        decision_id: &str,
        current_user_text: &str,
    ) -> ProviderPolicyAuthorization {
        let decision = crate::agent::PolicyStore::mvp_builtin().evaluate_context_policy(
            crate::agent::PolicyEvaluationRequest {
                topic: crate::agent::PolicyTopic::General,
                requested_route: crate::agent::ModelRoutePolicy::CloudAllowed,
            },
        );
        ProviderPolicyAuthorization::from_policy_store_context_decision(&decision, decision_id)
            .and_then(|authorization| {
                authorization.bind_policy_store_current_user_subject(current_user_text)
            })
            .expect("canonical PolicyStore cloud decision")
    }

    fn canonical_cloud_authorization(
        decision_id: &str,
        current_user_text: &str,
    ) -> ProviderPolicyAuthorization {
        canonical_cloud_subject_authorization(decision_id, current_user_text)
            .authorize_derived_payload(
                crate::llm::ProviderPayloadPurpose::ScheduledTaskGeneration,
                current_user_text,
                &[ChatMessage {
                    role: "user".into(),
                    content: current_user_text.into(),
                }],
                &[],
            )
            .expect("canonical PolicyStore cloud payload scope")
    }

    fn local_only_test_authorization(
        current_user_text: &str,
        messages: &[ChatMessage],
    ) -> ProviderPolicyAuthorization {
        ProviderPolicyAuthorization::local_only_fail_closed(ProviderLocalOnlyReason::TestFixture)
            .authorize_derived_payload(
                crate::llm::ProviderPayloadPurpose::ScheduledTaskGeneration,
                current_user_text,
                messages,
                &[],
            )
            .expect("local-only fixture payload scope")
    }

    fn scheduled_local_claim(description: &str) -> (Arc<TaskStore>, Arc<ScheduledTaskClaim>) {
        let store = Arc::new(TaskStore::new_in_memory().expect("scheduled task store"));
        let mut task = ScheduledTask::new(
            "scheduled provider admission",
            description,
            Some((chrono::Utc::now() - chrono::Duration::seconds(1)).to_rfc3339()),
            "medium",
        );
        task.seal_deterministic_local_provider_grant();
        store
            .create_task_idempotent(&task)
            .expect("create scheduled local task");
        let claim = store
            .claim_next_due(chrono::Utc::now(), chrono::Duration::seconds(30))
            .expect("claim due scheduled task")
            .expect("scheduled task is due");
        assert!(store
            .begin_claim_execution(&claim)
            .expect("begin scheduled execution"));
        (store, Arc::new(claim))
    }

    fn scheduled_authorization(
        claim: &ScheduledTaskClaim,
        messages: &[ChatMessage],
    ) -> ProviderPolicyAuthorization {
        ProviderPolicyAuthorization::from_scheduled_claim(claim)
            .and_then(|authorization| {
                authorization.authorize_derived_payload(
                    crate::llm::ProviderPayloadPurpose::ScheduledTaskStep,
                    &claim.task().description,
                    messages,
                    &[],
                )
            })
            .expect("scheduled exact payload authorization")
    }

    async fn prepare_scheduled_request(
        scheduler: &InferenceScheduler,
        claim: &ScheduledTaskClaim,
        request_id: &str,
    ) -> crate::llm::PreparedProviderRequest {
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: claim.task().description.clone(),
        }];
        scheduler
            .prepare_chat_request_with_authorization(
                messages.clone(),
                vec![],
                ContextManifest {
                    request_id: request_id.into(),
                    privacy_decision_id: claim.provider_grant().policy_decision_digest.clone(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::RuntimeCompiledMessages,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                scheduled_authorization(claim, &messages),
                allow_network_policy(),
                false,
            )
            .await
            .expect("prepare exact scheduled request")
    }

    async fn prepare_bound_scheduled_request(
        scheduler: &ScheduledInferenceScheduler,
        claim: &ScheduledTaskClaim,
        request_id: &str,
    ) -> crate::llm::PreparedProviderRequest {
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: claim.task().description.clone(),
        }];
        scheduler
            .prepare_scheduled_chat_request(
                messages.clone(),
                vec![],
                ContextManifest {
                    request_id: request_id.into(),
                    privacy_decision_id: claim.provider_grant().policy_decision_digest.clone(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::RuntimeCompiledMessages,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                scheduled_authorization(claim, &messages),
                allow_network_policy(),
                false,
            )
            .await
            .expect("prepare exact bound scheduled request")
    }

    fn terminal_receipt(
        attempt: &ProviderStartedAttempt,
        status: ProviderInvocationStatus,
    ) -> crate::llm::ProviderInvocationReceipt {
        let error = match status {
            ProviderInvocationStatus::Completed => None,
            ProviderInvocationStatus::Failed => Some("confirmed_provider_failure"),
            ProviderInvocationStatus::RemoteUnknown => Some("remote_state_unobserved"),
        };
        attempt.terminal_receipt(status, error)
    }

    #[test]
    fn scheduled_provider_truth_claim_binding_rejects_another_issued_claim() {
        let (_store, claim) = scheduled_local_claim("bind this exact scheduled claim");
        let binding = ScheduledProviderTruthClaimBinding::capture(&claim).unwrap();
        let (_other_store, other_claim) = scheduled_local_claim("another scheduled claim");
        assert!(binding.validate_claim(&other_claim).is_err());

        binding.validate_claim(&claim).unwrap();
    }

    #[tokio::test]
    async fn scheduled_provider_truth_caller_shaped_progress_cannot_mint_admission() {
        let (store, claim) = scheduled_local_claim("do not trust caller-shaped progress");
        let scheduler = InferenceScheduler::new(
            "fixture-local".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "unused-cloud".into(),
            "unused-embedding".into(),
            false,
        )
        .with_scripted_generation_response("fixture is not an adapter call");
        let generic_scheduler = scheduler.clone();
        let (scheduler, handle) = scheduler
            .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&claim))
            .unwrap();
        let prepared =
            prepare_bound_scheduled_request(&scheduler, &claim, "scheduled-shaped-progress").await;
        let shaped = ProviderInvocationProgress::Started {
            request_id: prepared.context_manifest.request_id.clone(),
            provider: prepared.provider_target.clone(),
            model: prepared.model_target.clone(),
            started_at: chrono::Utc::now(),
            policy_evidence: prepared.policy_receipt_evidence(),
        };

        let error = handle.take_for_progress(&shaped).unwrap_err();
        assert!(error.to_string().contains("caller-shaped progress"));
        let shaped_terminal =
            ProviderInvocationProgress::Completed(crate::llm::ProviderInvocationReceipt {
                request_id: prepared.context_manifest.request_id.clone(),
                provider: prepared.provider_target.clone(),
                model: prepared.model_target.clone(),
                status: ProviderInvocationStatus::Completed,
                started_at: chrono::Utc::now(),
                finished_at: chrono::Utc::now(),
                error_digest: None,
                simulated: false,
                policy_evidence: Some(prepared.policy_receipt_evidence()),
            });
        assert!(handle.take_for_progress(&shaped_terminal).is_err());

        let outcome = generic_scheduler.execute_prepared(prepared).await;
        assert!(outcome
            .result
            .unwrap_err()
            .contains("scheduled_policy_requires_scheduled_executor"));
        assert!(outcome.receipt.is_none());
        assert!(handle
            .take_remote_unknown_after_local_abort(
                ScheduledProviderLocalAbortCause::RuntimeFutureAborted,
            )
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn scheduled_provider_truth_scope_rejects_cross_claim_and_mutated_envelope() {
        let (first_store, first_claim) = scheduled_local_claim("first scheduled subject");
        let (_second_store, second_claim) = scheduled_local_claim("second scheduled subject");
        let scheduler = InferenceScheduler::new(
            "fixture-local".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "unused-cloud".into(),
            "unused-embedding".into(),
            false,
        )
        .with_scripted_generation_response("fixture");
        let (scheduler, _handle) = scheduler
            .bind_scheduled_provider_truth_scope(Arc::clone(&first_store), Arc::clone(&first_claim))
            .unwrap();
        let other = prepare_bound_scheduled_request(
            &scheduler,
            &second_claim,
            "scheduled-other-claim-request",
        )
        .await;
        let scope = scheduler.truth_scope();
        assert!(scope.capture_prepared(&other).is_err());

        let mut exact = prepare_bound_scheduled_request(
            &scheduler,
            &first_claim,
            "scheduled-mutated-envelope-request",
        )
        .await;
        exact.messages[0].content.push_str(" caller mutation");
        assert!(scope.capture_prepared(&exact).is_err());
    }

    #[tokio::test]
    async fn scheduled_provider_truth_terminal_state_machine_is_first_terminal_wins() {
        let (store, claim) = scheduled_local_claim("scheduled terminal ordering");
        let scheduler = InferenceScheduler::new(
            "fixture-local".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "unused-cloud".into(),
            "unused-embedding".into(),
            false,
        )
        .with_scripted_generation_response("fixture");
        let (scheduler, _handle) = scheduler
            .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&claim))
            .unwrap();
        let prepared_request =
            prepare_bound_scheduled_request(&scheduler, &claim, "scheduled-terminal-order").await;
        let claim_binding = ScheduledProviderTruthClaimBinding::capture(&claim).unwrap();
        let prepared =
            ScheduledPreparedProviderTruthBinding::capture(&claim_binding, &prepared_request)
                .unwrap();
        let attempt = ProviderStartedAttempt {
            request_id: prepared.request_id.clone(),
            provider: prepared.provider.clone(),
            model: prepared.model.clone(),
            started_at: chrono::Utc::now(),
            policy_evidence: prepared.policy_evidence.clone(),
        };
        let completed = terminal_receipt(&attempt, ProviderInvocationStatus::Completed);
        let unknown = terminal_receipt(&attempt, ProviderInvocationStatus::RemoteUnknown);

        let mut before_start = ScheduledProviderTruthAdmissionState::new(claim_binding.clone());
        assert!(before_start
            .register_terminal(&prepared, &completed)
            .unwrap_err()
            .to_string()
            .contains("before start"));

        let mut unknown_first = ScheduledProviderTruthAdmissionState::new(claim_binding.clone());
        unknown_first
            .register_started(prepared.clone(), attempt.clone())
            .unwrap();
        unknown_first
            .register_terminal(&prepared, &unknown)
            .unwrap();
        assert!(unknown_first
            .register_terminal(&prepared, &completed)
            .unwrap_err()
            .to_string()
            .contains("first terminal"));

        let mut completed_first = ScheduledProviderTruthAdmissionState::new(claim_binding);
        completed_first
            .register_started(prepared.clone(), attempt)
            .unwrap();
        completed_first
            .register_terminal(&prepared, &completed)
            .unwrap();
        assert!(completed_first
            .register_terminal(&prepared, &unknown)
            .unwrap_err()
            .to_string()
            .contains("first terminal"));
    }

    #[tokio::test]
    async fn scheduled_provider_truth_registry_is_bounded_and_fails_closed() {
        let (_store, claim) = scheduled_local_claim("bounded scheduled provider truth");
        let scheduler = InferenceScheduler::new(
            "fixture-local".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "unused-cloud".into(),
            "unused-embedding".into(),
            false,
        )
        .with_scripted_generation_response("fixture");
        let prepared_request =
            prepare_scheduled_request(&scheduler, &claim, "scheduled-bounded-base").await;
        let claim_binding = ScheduledProviderTruthClaimBinding::capture(&claim).unwrap();
        let base =
            ScheduledPreparedProviderTruthBinding::capture(&claim_binding, &prepared_request)
                .unwrap();
        let mut state = ScheduledProviderTruthAdmissionState::new(claim_binding);

        for index in 0..MAX_SCHEDULED_PROVIDER_TRUTH_ATTEMPTS {
            let mut prepared = base.clone();
            prepared.request_id = format!("scheduled-bounded-{index}");
            prepared.prepared_request_digest =
                crate::agent::metadata_safe::metadata_safe_text_digest(&prepared.request_id).1;
            let attempt = ProviderStartedAttempt {
                request_id: prepared.request_id.clone(),
                provider: prepared.provider.clone(),
                model: prepared.model.clone(),
                started_at: chrono::Utc::now(),
                policy_evidence: prepared.policy_evidence.clone(),
            };
            state.register_started(prepared, attempt).unwrap();
        }

        let mut overflow = base;
        overflow.request_id = "scheduled-bounded-overflow".into();
        overflow.prepared_request_digest =
            crate::agent::metadata_safe::metadata_safe_text_digest(&overflow.request_id).1;
        let overflow_attempt = ProviderStartedAttempt {
            request_id: overflow.request_id.clone(),
            provider: overflow.provider.clone(),
            model: overflow.model.clone(),
            started_at: chrono::Utc::now(),
            policy_evidence: overflow.policy_evidence.clone(),
        };
        assert!(state
            .register_started(overflow, overflow_attempt)
            .unwrap_err()
            .to_string()
            .contains("attempt limit"));
    }

    #[test]
    fn scheduled_provider_truth_terminal_shapes_fail_closed() {
        let evidence = test_stream_seed("scheduled-terminal-shape").policy_evidence;
        let started_at = chrono::Utc::now();
        let base = crate::llm::ProviderInvocationReceipt {
            request_id: "scheduled-terminal-shape".into(),
            provider: "ollama".into(),
            model: "local-model".into(),
            status: ProviderInvocationStatus::Completed,
            started_at,
            finished_at: started_at,
            error_digest: None,
            simulated: false,
            policy_evidence: Some(evidence),
        };

        let mut completed_with_error = base.clone();
        completed_with_error.error_digest = Some(format!("sha256:{}", "1".repeat(64)));
        assert!(
            super::ScheduledProviderTruthProgressKey::from_receipt(&completed_with_error).is_err()
        );

        let mut failed_without_error = base.clone();
        failed_without_error.status = ProviderInvocationStatus::Failed;
        assert!(
            super::ScheduledProviderTruthProgressKey::from_receipt(&failed_without_error).is_err()
        );

        let mut unknown_without_reason = base.clone();
        unknown_without_reason.status = ProviderInvocationStatus::RemoteUnknown;
        assert!(
            super::ScheduledProviderTruthProgressKey::from_receipt(&unknown_without_reason)
                .is_err()
        );

        let mut simulated = base;
        simulated.simulated = true;
        assert!(super::ScheduledProviderTruthProgressKey::from_receipt(&simulated).is_err());

        let mismatched_variant =
            ProviderInvocationProgress::Failed(crate::llm::ProviderInvocationReceipt {
                simulated: false,
                ..simulated
            });
        assert!(
            super::ScheduledProviderTruthProgressKey::from_progress(&mismatched_variant).is_err()
        );
    }

    // Provider tests mutate process-global endpoint credentials; the lock is
    // intentionally held through the complete async observation window.
    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test(flavor = "current_thread")]
    async fn scheduled_provider_truth_real_adapter_issues_start_and_completed_admissions() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", format!("http://{address}"));
        std::env::remove_var("OLLAMA_HOST");
        let server = tokio::spawn(serve_two_ollama_requests(listener));

        let (store, claim) = scheduled_local_claim("real scheduled adapter capability");
        let scheduler = InferenceScheduler::new(
            "qwen-local:latest".into(),
            true,
            "openai".into(),
            "http://127.0.0.1:9/v1".into(),
            "cloud-key-must-not-be-used".into(),
            "unused-cloud-model".into(),
            "unused-embedding".into(),
            false,
        );
        let (scheduler, handle) = scheduler
            .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&claim))
            .unwrap();
        let prepared =
            prepare_bound_scheduled_request(&scheduler, &claim, "scheduled-real-completed").await;

        let outcome = scheduler.execute_scheduled_provider_request(prepared).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        assert_eq!(outcome.result.as_deref(), Ok("local response"));
        server.await.unwrap();

        assert_eq!(
            outcome.receipt.as_ref().map(|receipt| receipt.status),
            Some(ProviderInvocationStatus::Completed)
        );
        let persisted = store
            .provider_receipts_for_attempt(claim.attempt_id())
            .unwrap();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].request_id, "scheduled-real-completed");
        assert_eq!(persisted[0].status, "completed");
        assert_eq!(
            persisted[0].provider_grant_id,
            claim.provider_grant().grant_id
        );
        assert_eq!(persisted[0].policy_evidence_state, "exact");
        assert!(handle
            .take_remote_unknown_after_local_abort(
                ScheduledProviderLocalAbortCause::ExecutionTimeout,
            )
            .unwrap()
            .is_empty());
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test(flavor = "current_thread")]
    async fn scheduled_provider_truth_local_abort_requires_real_in_flight_start() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", format!("http://{address}"));
        std::env::remove_var("OLLAMA_HOST");
        let chat_seen = std::sync::Arc::new(tokio::sync::Notify::new());
        let release_chat = std::sync::Arc::new(tokio::sync::Notify::new());
        let chat_seen_by_server = std::sync::Arc::clone(&chat_seen);
        let release_by_server = std::sync::Arc::clone(&release_chat);
        let server = tokio::spawn(async move {
            let (mut tags_socket, _) = listener.accept().await.unwrap();
            let mut tags_request = [0_u8; 16 * 1024];
            let _ = tags_socket.read(&mut tags_request).await.unwrap();
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
            let mut chat_request = [0_u8; 16 * 1024];
            let _ = chat_socket.read(&mut chat_request).await.unwrap();
            chat_seen_by_server.notify_one();
            release_by_server.notified().await;
        });

        let (store, claim) = scheduled_local_claim("abort a real scheduled provider future");
        let scheduler = InferenceScheduler::new(
            "qwen-local:latest".into(),
            true,
            "openai".into(),
            "http://127.0.0.1:9/v1".into(),
            "cloud-key-must-not-be-used".into(),
            "unused-cloud-model".into(),
            "unused-embedding".into(),
            false,
        );
        let (scheduler, handle) = scheduler
            .bind_scheduled_provider_truth_scope(Arc::clone(&store), Arc::clone(&claim))
            .unwrap();
        let prepared =
            prepare_bound_scheduled_request(&scheduler, &claim, "scheduled-real-aborted").await;
        let execution =
            tokio::spawn(
                async move { scheduler.execute_scheduled_provider_request(prepared).await },
            );
        // Coverage instrumentation can make the full local-provider path
        // several times slower; keep a bounded wait without using a
        // production latency threshold as a test-harness deadline.
        tokio::time::timeout(std::time::Duration::from_secs(10), chat_seen.notified())
            .await
            .expect("scheduled HTTP request crossed the loopback edge");
        let started = store
            .provider_receipts_for_attempt(claim.attempt_id())
            .unwrap();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].status, "started");

        let unknown = handle
            .take_remote_unknown_after_local_abort(
                ScheduledProviderLocalAbortCause::ExecutionTimeout,
            )
            .unwrap();
        assert_eq!(unknown.len(), 1);
        for admission in unknown {
            store.record_provider_truth(&claim, admission).unwrap();
        }
        let unknown = store
            .provider_receipts_for_attempt(claim.attempt_id())
            .unwrap();
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].status, "remote_unknown");
        assert!(unknown[0].error_digest.is_some());
        assert!(handle
            .take_remote_unknown_after_local_abort(
                ScheduledProviderLocalAbortCause::CancellationRequested,
            )
            .unwrap()
            .is_empty());

        execution.abort();
        let _ = execution.await;
        release_chat.notify_one();
        server.await.unwrap();
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
    }

    #[test]
    fn prepared_generation_requires_the_typed_receipt_outcome() {
        let removed_api = ["generate_", "prepared("].concat();
        let production_consumers = [(
            "agent/main_chat_agent_v1.rs",
            include_str!("agent/main_chat_agent_v1.rs") as &str,
        )];

        for (path, source) in production_consumers {
            assert!(
                !source.lines().any(|line| line.contains(&removed_api)),
                "{path} must consume execute_prepared and its receipt-bearing outcome"
            );
        }

        let scheduler_source = include_str!("scheduler.rs");
        assert!(
            !scheduler_source
                .lines()
                .any(|line| { line.contains("pub async fn") && line.contains(&removed_api) }),
            "the receipt-dropping convenience API must stay deleted"
        );
    }

    #[test]
    fn scheduled_scheduler_public_api_has_no_generic_or_streaming_bypass() {
        let source = include_str!("scheduler.rs").replace("\r\n", "\n");
        let start = source
            .find("impl ScheduledInferenceScheduler {")
            .expect("scheduled scheduler impl");
        let end = source[start..]
            .find("\n#[cfg(test)]\nmod tests")
            .map(|offset| start + offset)
            .expect("scheduled scheduler impl boundary");
        let scheduled_impl = &source[start..end];

        for forbidden in [
            "execute_prepared",
            "execute_prepared_with_observer",
            "execute_prepared_with_start_observer",
            "generate_prepared_stream",
            "generate_prepared_stream_with_start_observer",
        ] {
            assert!(
                !scheduled_impl.lines().any(|line| {
                    line.trim_start().starts_with("pub ") && line.contains(forbidden)
                }),
                "ScheduledInferenceScheduler exposed generic adapter entrypoint {forbidden}"
            );
        }
        assert!(scheduled_impl
            .lines()
            .any(|line| line.contains("pub async fn execute_scheduled_provider_request")));
        let qualified_deref = ["impl std::ops::", "Deref for ScheduledInferenceScheduler"].concat();
        let imported_deref = ["impl ", "Deref for ScheduledInferenceScheduler"].concat();
        assert!(!source.contains(&qualified_deref));
        assert!(!source.contains(&imported_deref));
    }

    fn test_stream_seed(request_id: &str) -> ProviderStartedAttempt {
        ProviderStartedAttempt {
            request_id: request_id.into(),
            provider: "capture-provider".into(),
            model: "capture-model".into(),
            started_at: chrono::Utc::now(),
            policy_evidence: ProviderPolicyReceiptEvidence {
                decision_id: "test-policy-decision".into(),
                policy_version: "test-policy-v1".into(),
                issuing_authority: crate::llm::ProviderPolicyAuthority::LocalOnlyFailClosed,
                effective_data_route: ProviderDataRoute::LocalOnly,
                effective_local_restriction: Some(crate::llm::ProviderLocalOnlyReason::TestFixture),
                subject_scope_digest: format!("sha256:{}", "0".repeat(64)),
                payload_purpose: Some(crate::llm::ProviderPayloadPurpose::FrozenRuntimeEvaluation),
                unfiltered_payload_digest: Some(format!("sha256:{}", "2".repeat(64))),
                context_manifest_digest: format!("sha256:{}", "1".repeat(64)),
                prepared_envelope_digest: Some(format!("sha256:{}", "3".repeat(64))),
                provider_config_generation: "test-provider-generation".into(),
                network_policy_decision_digest: format!("sha256:{}", "4".repeat(64)),
                selected_context_refs: Vec::new(),
                included_context_categories: Vec::new(),
                declared_payload_categories: vec![
                    crate::llm::ProviderPayloadCategory::FrozenEvaluationInput,
                ],
                policy_provenance_refs: Vec::new(),
                raw_life_model_included: false,
                raw_unbounded_memory_included: false,
            },
        }
    }

    fn test_stream_scheduler() -> InferenceScheduler {
        InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test-stream-binding".into(),
            "capture-model".into(),
            "unused-embedding-model".into(),
            false,
        )
    }

    async fn test_stream_adapter_binding(
        scheduler: &InferenceScheduler,
        request_id: &str,
    ) -> (ProviderStartedAttempt, ProviderInvocationTerminalBinding) {
        let user_text = format!("provider stream fixture {request_id}");
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: user_text.clone(),
        }];
        let privacy_decision_id = format!("stream-binding:{request_id}");
        let request = scheduler
            .prepare_chat_request_with_authorization(
                messages,
                Vec::new(),
                ContextManifest {
                    request_id: request_id.into(),
                    privacy_decision_id: privacy_decision_id.clone(),
                    selected_context_refs: Vec::new(),
                    included_context_categories: Vec::new(),
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                canonical_cloud_authorization(&privacy_decision_id, &user_text),
                allow_network_policy(),
                false,
            )
            .await
            .expect("prepare the exact stream adapter request");
        let execution_binding = scheduler
            .validate_prepared_execution_owner(&request)
            .expect("validate the exact stream adapter owner");
        let terminal_binding =
            ProviderInvocationTerminalBinding::capture(&request, &execution_binding)
                .expect("capture the production stream terminal binding");
        let seed = ProviderStartedAttempt {
            request_id: request.context_manifest.request_id.clone(),
            provider: request.provider_target.clone(),
            model: request.model_target.clone(),
            started_at: chrono::Utc::now(),
            policy_evidence: request.policy_receipt_evidence(),
        };
        scheduler
            .provider_receipt_collector
            .record_started(seed.clone())
            .expect("retain the same adapter-start proof production records before binding");
        (seed, terminal_binding)
    }

    #[tokio::test]
    async fn provider_stream_seam_yields_the_exact_completed_terminal_receipt() {
        let scheduler = test_stream_scheduler();
        let (seed, terminal_binding) =
            test_stream_adapter_binding(&scheduler, "stream-completed").await;
        let inner: crate::llm::StreamResult =
            Box::pin(futures::stream::iter(vec![Ok("token".to_string())]));
        let mut stream = scheduler
            .bind_prepared_provider_stream(Ok(inner), Some(seed), terminal_binding)
            .expect("bind receipt-retaining stream");

        assert!(matches!(
            stream.next().await,
            Some(PreparedProviderStreamEvent::Token(token)) if token == "token"
        ));
        let receipt = match stream.next().await {
            Some(PreparedProviderStreamEvent::Terminal(
                PreparedProviderStreamTerminal::Completed(receipt),
            )) => receipt,
            other => panic!("expected exact completed terminal receipt, got {other:?}"),
        };
        assert!(stream.next().await.is_none());

        let receipts = scheduler.provider_receipts_snapshot();
        assert_eq!(receipts, vec![*receipt]);
        assert_eq!(receipts[0].request_id, "stream-completed");
        assert_eq!(receipts[0].status, ProviderInvocationStatus::Completed);
        assert!(receipts[0].error_digest.is_none());
    }

    #[tokio::test]
    async fn provider_stream_confirmed_terminal_error_yields_failed_receipt() {
        let scheduler = test_stream_scheduler();
        let (seed, terminal_binding) =
            test_stream_adapter_binding(&scheduler, "stream-confirmed-failed").await;
        let inner: crate::llm::StreamResult = Box::pin(futures::stream::iter(vec![Err(
            crate::llm::confirmed_provider_terminal_failure(
                "provider_stream_reported_error",
                anyhow::anyhow!("provider_stream_reported_error"),
            ),
        )]));
        let mut stream = scheduler
            .bind_prepared_provider_stream(Ok(inner), Some(seed), terminal_binding)
            .expect("bind receipt-retaining stream");

        let receipt = match stream.next().await {
            Some(PreparedProviderStreamEvent::Terminal(
                PreparedProviderStreamTerminal::Failed { receipt, error },
            )) => {
                assert!(error.contains("provider_stream_reported_error"));
                receipt
            }
            other => panic!("expected confirmed failed terminal receipt, got {other:?}"),
        };
        assert_eq!(receipt.status, ProviderInvocationStatus::Failed);
        assert_eq!(scheduler.provider_receipts_snapshot(), vec![*receipt]);
    }

    #[tokio::test]
    async fn provider_stream_open_transport_error_after_start_yields_remote_unknown_receipt() {
        let scheduler = test_stream_scheduler();
        let (seed, terminal_binding) =
            test_stream_adapter_binding(&scheduler, "stream-open-remote-unknown").await;
        let mut stream = scheduler
            .bind_prepared_provider_stream(
                Err(anyhow::anyhow!("provider response headers timed out")),
                Some(seed),
                terminal_binding,
            )
            .expect("a post-start open error is a typed stream terminal, not a naked error");

        let receipt = match stream.next().await {
            Some(PreparedProviderStreamEvent::Terminal(
                PreparedProviderStreamTerminal::RemoteUnknown { receipt, error },
            )) => {
                assert_eq!(error, "provider response headers timed out");
                receipt
            }
            other => panic!("expected post-start remote-unknown terminal, got {other:?}"),
        };
        assert_eq!(receipt.status, ProviderInvocationStatus::RemoteUnknown);
        assert_eq!(scheduler.provider_receipts_snapshot(), vec![*receipt]);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn provider_stream_disconnect_yields_remote_unknown_terminal_receipt() {
        let scheduler = test_stream_scheduler();
        let (seed, terminal_binding) =
            test_stream_adapter_binding(&scheduler, "stream-failed").await;
        let inner: crate::llm::StreamResult = Box::pin(futures::stream::iter(vec![
            Ok("partial".to_string()),
            Err(anyhow::anyhow!("sensitive remote failure body")),
        ]));
        let mut stream = scheduler
            .bind_prepared_provider_stream(Ok(inner), Some(seed), terminal_binding)
            .expect("bind receipt-retaining stream");

        assert!(matches!(
            stream.next().await,
            Some(PreparedProviderStreamEvent::Token(token)) if token == "partial"
        ));
        let receipt = match stream.next().await {
            Some(PreparedProviderStreamEvent::Terminal(
                PreparedProviderStreamTerminal::RemoteUnknown { receipt, error },
            )) => {
                assert_eq!(error, "sensitive remote failure body");
                receipt
            }
            other => panic!("expected remote-unknown terminal receipt, got {other:?}"),
        };

        let receipts = scheduler.provider_receipts_snapshot();
        assert_eq!(receipts, vec![*receipt]);
        assert_eq!(receipts[0].request_id, "stream-failed");
        assert_eq!(receipts[0].status, ProviderInvocationStatus::RemoteUnknown);
        assert!(receipts[0]
            .error_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("sha256:")));
        assert!(!format!("{receipts:?}").contains("sensitive remote failure body"));
    }

    #[tokio::test]
    async fn dropping_started_provider_stream_retains_local_abort_unknown_receipt() {
        let scheduler = test_stream_scheduler();
        let (seed, terminal_binding) =
            test_stream_adapter_binding(&scheduler, "stream-dropped").await;
        let inner: crate::llm::StreamResult = Box::pin(futures::stream::pending());
        let stream = scheduler
            .bind_prepared_provider_stream(Ok(inner), Some(seed), terminal_binding)
            .expect("bind receipt-retaining stream");

        drop(stream);

        let receipts = scheduler.provider_receipts_snapshot();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].request_id, "stream-dropped");
        assert_eq!(receipts[0].status, ProviderInvocationStatus::RemoteUnknown);
        assert!(receipts[0].error_digest.is_some());
    }

    #[tokio::test]
    async fn cancellation_terminal_wins_over_late_stream_completion() {
        let scheduler = test_stream_scheduler();
        let (seed, terminal_binding) =
            test_stream_adapter_binding(&scheduler, "stream-cancel-wins").await;
        scheduler
            .provider_receipt_collector
            .record_started(seed.clone())
            .unwrap();
        let inner: crate::llm::StreamResult =
            Box::pin(futures::stream::iter(vec![Ok("late".to_string())]));
        let mut stream = scheduler
            .bind_prepared_provider_stream(Ok(inner), Some(seed), terminal_binding)
            .expect("bind started stream");

        scheduler
            .provider_receipt_collector
            .mark_in_flight_remote_unknown("local_cancel_won");

        let receipt = match stream.next().await {
            Some(PreparedProviderStreamEvent::Terminal(
                PreparedProviderStreamTerminal::RemoteUnknown { receipt, .. },
            )) => receipt,
            other => panic!("late completion must forward cancel-owned terminal: {other:?}"),
        };
        assert_eq!(receipt.status, ProviderInvocationStatus::RemoteUnknown);
        assert_eq!(scheduler.provider_receipts_snapshot(), vec![*receipt]);
        assert!(stream.next().await.is_none());
    }

    #[test]
    fn timeout_marks_the_exact_started_attempt_remote_unknown() {
        let collector = super::ProviderReceiptCollector::default();
        let attempt = test_stream_seed("timeout-started-attempt");
        collector.record_started(attempt.clone()).unwrap();

        let before = collector.summary();
        assert_eq!(before.started_attempt_count, 1);
        assert_eq!(before.in_flight_count, 1);
        assert_eq!(before.remote_unknown_count, 0);

        collector.mark_in_flight_remote_unknown("reasoning_bridge_timeout");
        let after = collector.summary();
        assert_eq!(after.in_flight_count, 0);
        assert_eq!(after.remote_unknown_count, 1);
        assert_eq!(after.retained_receipts.len(), 1);
        assert_eq!(
            after.retained_receipts[0].request_id,
            "timeout-started-attempt"
        );
        assert_eq!(
            after.retained_receipts[0].status,
            ProviderInvocationStatus::RemoteUnknown
        );
    }

    async fn serve_two_ollama_requests(listener: tokio::net::TcpListener) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        for (expected_path, response_body) in [
            (
                "/api/tags",
                r#"{"models":[{"name":"qwen-local:latest","size":1}]}"#,
            ),
            (
                "/api/chat",
                r#"{"message":{"role":"assistant","content":"local response"},"done":true}"#,
            ),
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 16 * 1024];
            let read = socket.read(&mut request).await.unwrap();
            let request_text = String::from_utf8_lossy(&request[..read]);
            assert!(
                request_text
                    .lines()
                    .next()
                    .is_some_and(|line| line.contains(expected_path)),
                "expected {expected_path}, got {request_text}"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    }

    async fn serve_one_json_response(
        listener: tokio::net::TcpListener,
        response_body: &'static str,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0_u8; 16 * 1024];
        let _ = socket.read(&mut request).await.unwrap();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    }

    #[tokio::test]
    async fn provider_route_fails_closed_without_an_available_provider() {
        let scheduler = InferenceScheduler::new(
            "qwen2.5".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            true,
        );
        let trace = scheduler
            .preview_chat_route(Some("typed_tool_contract"))
            .await;
        assert_eq!(trace.provider, "none");
        assert_eq!(trace.route_type, "blocked");
        assert!(trace.reason.starts_with("model_router_blocked:"));
    }

    #[tokio::test]
    async fn provider_route_uses_the_available_configured_cloud_adapter() {
        let scheduler = InferenceScheduler::new(
            "qwen2.5".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            true,
        );
        let trace = scheduler
            .preview_chat_route(Some("typed_tool_contract"))
            .await;
        assert_eq!(trace.provider, "openai");
        assert_eq!(trace.model, "gpt-4o-mini");
        assert_eq!(trace.route_type, "cloud");
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test(flavor = "current_thread")]
    async fn selected_local_model_is_the_canonical_available_route() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", format!("http://{address}"));
        std::env::remove_var("OLLAMA_HOST");
        let server = tokio::spawn(serve_one_json_response(
            listener,
            r#"{"models":[{"name":"openlife-selected-local:latest","size":1}]}"#,
        ));
        let scheduler = InferenceScheduler::new(
            "openlife-selected-local".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );

        let trace = scheduler.preview_chat_route(None).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        server.await.unwrap();

        assert_eq!(trace.provider, "ollama");
        assert_eq!(trace.model, "openlife-selected-local:latest");
        assert_eq!(trace.route_type, "local");
        assert_eq!(trace.reason, "user_selected_local_model_available");
        assert_eq!(trace.fallback_reason, None);
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test(flavor = "current_thread")]
    async fn unavailable_selected_local_model_never_falls_back_to_configured_cloud() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", "http://127.0.0.1:9");
        std::env::remove_var("OLLAMA_HOST");
        let scheduler = InferenceScheduler::new(
            "openlife-selected-local-unavailable".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "cloud-key-must-not-be-used".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );

        let trace = scheduler.preview_chat_route(None).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");

        assert_eq!(trace.provider, "none");
        assert_eq!(trace.route_type, "blocked");
        assert_eq!(trace.reason, "selected_local_provider_unavailable");
        assert_eq!(trace.fallback_reason, None);
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test(flavor = "current_thread")]
    async fn policy_allowed_local_first_uses_the_observed_loopback_provider() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", format!("http://{address}"));
        std::env::remove_var("OLLAMA_HOST");
        let server = tokio::spawn(serve_two_ollama_requests(listener));

        let scheduler = InferenceScheduler::new(
            "qwen-local:latest".into(),
            true,
            "openai".into(),
            "http://127.0.0.1:9/v1".into(),
            "sk-cloud-must-not-be-used".into(),
            "cloud-model".into(),
            "embedding-model".into(),
            false,
        );
        let prepared = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "ordinary local-first request".into(),
                }],
                vec![],
                ContextManifest {
                    request_id: uuid::Uuid::new_v4().to_string(),
                    privacy_decision_id: "policy-local-first".into(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                canonical_cloud_authorization("policy-local-first", "ordinary local-first request"),
                allow_network_policy(),
                false,
            )
            .await
            .unwrap();
        assert_eq!(prepared.provider_target, "ollama");
        assert_eq!(prepared.model_target, "qwen-local:latest");
        assert_eq!(
            prepared.provider_endpoint,
            format!("http://{address}/api/chat")
        );

        // Mutating the process-global discovery source after preparation must
        // not change this turn's exact adapter endpoint.
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        let outcome = scheduler.execute_prepared(prepared).await;
        server.await.unwrap();

        assert_eq!(outcome.result.unwrap(), "local response");
        let receipt = outcome.receipt.expect("local adapter receipt");
        assert_eq!(receipt.provider, "ollama");
        assert_eq!(receipt.status, ProviderInvocationStatus::Completed);
        assert_eq!(scheduler.provider_receipts_snapshot(), vec![receipt]);
    }

    #[tokio::test]
    async fn scripted_generation_remains_not_attempted_provider_truth() {
        let scheduler = InferenceScheduler::new(
            "scripted-local".into(),
            true,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "scripted-model".into(),
            "embedding-model".into(),
            false,
        )
        .with_scripted_generation_response("scripted response");
        let prepared = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "eval input".into(),
                }],
                vec![],
                ContextManifest {
                    request_id: "scripted-request".into(),
                    privacy_decision_id: "scripted-policy".into(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                canonical_cloud_authorization("scripted-policy", "eval input"),
                allow_network_policy(),
                false,
            )
            .await
            .unwrap();

        let mut start_observed = false;
        let mut stream = scheduler
            .generate_prepared_stream_with_start_observer(prepared.clone(), |_, _, _, _, _| {
                start_observed = true;
                Ok(())
            })
            .await
            .expect("scripted prepared stream");
        assert!(matches!(
            stream.next().await,
            Some(PreparedProviderStreamEvent::Token(token)) if token == "scripted response"
        ));
        assert!(matches!(
            stream.next().await,
            Some(PreparedProviderStreamEvent::Terminal(
                PreparedProviderStreamTerminal::NotAttempted
            ))
        ));
        assert!(stream.next().await.is_none());
        assert!(
            !start_observed,
            "scripted generation cannot claim adapter start"
        );
        assert!(scheduler.provider_receipts_snapshot().is_empty());

        let outcome = scheduler.execute_prepared(prepared).await;

        assert_eq!(outcome.result.unwrap(), "scripted response");
        assert!(outcome.receipt.is_none());
        assert!(scheduler.provider_receipts_snapshot().is_empty());
    }

    #[tokio::test]
    async fn caller_string_cannot_replace_cloud_policy_authorization() {
        let scheduler = InferenceScheduler::new(
            "unused-local".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "cloud-model".into(),
            "embedding-model".into(),
            false,
        )
        .with_scripted_generation_response("scripted response");

        let error = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "must not self-authorize".into(),
                }],
                vec![],
                ContextManifest {
                    request_id: "unverified-cloud-request".into(),
                    privacy_decision_id: "forged:policy_allowed".into(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                canonical_cloud_authorization(
                    "canonical-cloud-decision",
                    "must not self-authorize",
                ),
                allow_network_policy(),
                false,
            )
            .await
            .expect_err("a caller string is not the typed policy authorization");

        assert!(error.to_string().contains("policy authorization"));
    }

    #[tokio::test]
    async fn canonical_typed_cloud_decision_prepares_cloud_provider() {
        let authorization =
            canonical_cloud_subject_authorization("typed-cloud-decision", "typed authorization")
                .authorize_derived_payload(
                    crate::llm::ProviderPayloadPurpose::ScheduledTaskGeneration,
                    "typed authorization",
                    &[ChatMessage {
                        role: "user".into(),
                        content: "typed authorization".into(),
                    }],
                    &[BoundedContextBlock {
                        source_ref: "typed-cloud-context".into(),
                        category: "bounded_test_context".into(),
                        content: "bounded context".into(),
                    }],
                )
                .expect("exact typed cloud payload scope");
        let scheduler = InferenceScheduler::new(
            "unused-local".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "cloud-model".into(),
            "embedding-model".into(),
            false,
        )
        .with_scripted_generation_response("scripted response");

        let prepared = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "typed authorization".into(),
                }],
                vec![BoundedContextBlock {
                    source_ref: "typed-cloud-context".into(),
                    category: "bounded_test_context".into(),
                    content: "bounded context".into(),
                }],
                ContextManifest {
                    request_id: "typed-cloud-request".into(),
                    privacy_decision_id: authorization.decision_id().to_string(),
                    selected_context_refs: vec!["typed-cloud-context".into()],
                    included_context_categories: vec!["bounded_test_context".into()],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization,
                allow_network_policy(),
                false,
            )
            .await
            .unwrap();

        assert_eq!(prepared.provider_target, "openai");
        assert_eq!(prepared.data_route, ProviderDataRoute::PolicyAllowed);
        assert_eq!(
            prepared.policy_authorization().authority(),
            crate::llm::ProviderPolicyAuthority::PolicyStore
        );
        let replay_error = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "typed authorization".into(),
                }],
                vec![BoundedContextBlock {
                    source_ref: "typed-cloud-context".into(),
                    category: "bounded_test_context".into(),
                    content: "bounded context".into(),
                }],
                ContextManifest {
                    request_id: "typed-cloud-replay".into(),
                    privacy_decision_id: prepared.policy_authorization().decision_id().to_string(),
                    selected_context_refs: vec!["typed-cloud-context".into()],
                    included_context_categories: vec!["bounded_test_context".into()],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                prepared.policy_authorization().clone(),
                allow_network_policy(),
                false,
            )
            .await
            .expect_err("a prepared policy capability must not be replayed");
        assert!(replay_error.to_string().contains("already bound"));
        let mut tampered_message = prepared.clone();
        tampered_message.messages[0].content = "different message".into();
        assert!(tampered_message
            .validate()
            .unwrap_err()
            .to_string()
            .contains("envelope mismatch"));
        let message_outcome = scheduler.execute_prepared(tampered_message).await;
        assert!(message_outcome.result.is_err());
        assert!(message_outcome.receipt.is_none());
        let mut tampered_context = prepared.clone();
        tampered_context.context_blocks[0].content = "different context".into();
        assert!(tampered_context
            .validate()
            .unwrap_err()
            .to_string()
            .contains("envelope mismatch"));
        let context_outcome = scheduler.execute_prepared(tampered_context).await;
        assert!(context_outcome.result.is_err());
        assert!(context_outcome.receipt.is_none());
    }

    #[tokio::test]
    async fn authorization_for_message_a_cannot_prepare_message_b() {
        let decision = crate::agent::main_chat_agent_v1::AgentIngress::default().decide(
            "authorization-subject-session",
            "message A",
            None,
            crate::agent::AgentTaskKind::Conversation,
        );
        let authorization = ProviderPolicyAuthorization::from_main_chat_ingress(&decision)
            .expect("PolicyRouter-issued authorization");
        let transfer_error = authorization
            .clone()
            .authorize_derived_payload(
                crate::llm::ProviderPayloadPurpose::MainChatDirectAnswer,
                "message B",
                &[ChatMessage {
                    role: "user".into(),
                    content: "message B".into(),
                }],
                &[],
            )
            .expect_err("message B cannot be scoped with message A authorization");
        assert!(transfer_error.to_string().contains("subject mismatch"));
        let scheduler = InferenceScheduler::new(
            "unused-local".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "cloud-model".into(),
            "embedding-model".into(),
            false,
        )
        .with_scripted_generation_response("must not run");

        let error = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "message B".into(),
                }],
                Vec::new(),
                ContextManifest {
                    request_id: "message-transfer-attempt".into(),
                    privacy_decision_id: authorization.decision_id().to_string(),
                    selected_context_refs: Vec::new(),
                    included_context_categories: Vec::new(),
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization,
                allow_network_policy(),
                false,
            )
            .await
            .expect_err("authorization for message A must not authorize message B");

        assert!(error.to_string().contains("exact unfiltered payload scope"));
        assert!(scheduler.provider_receipts_snapshot().is_empty());
    }

    #[tokio::test]
    async fn exact_message_scope_rejects_added_system_or_context_payload() {
        let current_user_text = "same authorized user message";
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: current_user_text.into(),
        }];
        let authorization =
            canonical_cloud_authorization("no-extra-context-policy", current_user_text);
        let scheduler = InferenceScheduler::new(
            "unused-local".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "cloud-model".into(),
            "embedding-model".into(),
            false,
        )
        .with_scripted_generation_response("must not run");
        let injected_context = BoundedContextBlock {
            source_ref: "injected-hs".into(),
            category: "selected_life_guidance".into(),
            content: "raw serialized HS must not hitchhike".into(),
        };
        let rebound_error = authorization
            .clone()
            .authorize_derived_payload(
                crate::llm::ProviderPayloadPurpose::ScheduledTaskGeneration,
                current_user_text,
                &messages,
                std::slice::from_ref(&injected_context),
            )
            .expect_err("an exact provider payload scope must not be rebound");
        assert!(rebound_error.to_string().contains("cannot be rebound"));

        let error = scheduler
            .prepare_chat_request_with_authorization(
                messages,
                vec![injected_context],
                ContextManifest {
                    request_id: "extra-context-transfer".into(),
                    privacy_decision_id: authorization.decision_id().to_string(),
                    selected_context_refs: vec!["injected-hs".into()],
                    included_context_categories: vec!["selected_life_guidance".into()],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization,
                allow_network_policy(),
                false,
            )
            .await
            .expect_err("exact payload authorization must reject added context");

        assert!(error.to_string().contains("derived payload mismatch"));
        assert!(scheduler.provider_receipts_snapshot().is_empty());
    }

    #[tokio::test]
    async fn manifest_refs_and_categories_cannot_disagree_with_outbound_blocks() {
        let scheduler = InferenceScheduler::new(
            "unused-local".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "cloud-model".into(),
            "embedding-model".into(),
            false,
        )
        .with_scripted_generation_response("must not run");
        let authorization = canonical_cloud_authorization("manifest-truth", "manifest subject");
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "manifest subject".into(),
        }];
        let blocks = vec![BoundedContextBlock {
            source_ref: "actual-ref".into(),
            category: "actual-category".into(),
            content: "bounded context".into(),
        }];

        let refs_error = scheduler
            .prepare_chat_request_with_authorization(
                messages.clone(),
                blocks.clone(),
                ContextManifest {
                    request_id: "manifest-ref-tamper".into(),
                    privacy_decision_id: authorization.decision_id().to_string(),
                    selected_context_refs: vec!["forged-ref".into()],
                    included_context_categories: vec!["actual-category".into()],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization.clone(),
                allow_network_policy(),
                false,
            )
            .await
            .expect_err("forged manifest ref must fail before provider routing");
        assert!(refs_error.to_string().contains("selected refs"));

        let categories_error = scheduler
            .prepare_chat_request_with_authorization(
                messages,
                blocks,
                ContextManifest {
                    request_id: "manifest-category-tamper".into(),
                    privacy_decision_id: authorization.decision_id().to_string(),
                    selected_context_refs: vec!["actual-ref".into()],
                    included_context_categories: vec!["forged-category".into()],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization,
                allow_network_policy(),
                false,
            )
            .await
            .expect_err("forged manifest category must fail before provider routing");
        assert!(categories_error.to_string().contains("categories"));
        assert!(scheduler.provider_receipts_snapshot().is_empty());
    }

    #[tokio::test]
    async fn local_restriction_preserves_issuer_decision_provenance() {
        let decision = crate::agent::main_chat_agent_v1::AgentIngress::default().decide(
            "local-restriction-session",
            "keep this request scoped",
            None,
            crate::agent::AgentTaskKind::Conversation,
        );
        let issued = ProviderPolicyAuthorization::from_main_chat_ingress(&decision)
            .and_then(|authorization| {
                authorization.authorize_derived_payload(
                    crate::llm::ProviderPayloadPurpose::MainChatDirectAnswer,
                    "keep this request scoped",
                    &[ChatMessage {
                        role: "user".into(),
                        content: "keep this request scoped".into(),
                    }],
                    &[],
                )
            })
            .expect("PolicyRouter-issued authorization");
        let original_decision_id = issued.decision_id().to_string();
        let restricted = issued.restrict_to_local(ProviderLocalOnlyReason::CloudDisabled);
        assert_eq!(restricted.decision_id(), original_decision_id);
        assert_eq!(
            restricted.authority(),
            crate::llm::ProviderPolicyAuthority::MainChatPolicyRouter
        );
        assert_eq!(restricted.data_route(), ProviderDataRoute::LocalOnly);
        assert_eq!(
            restricted.effective_local_restriction(),
            Some(ProviderLocalOnlyReason::CloudDisabled)
        );

        let scheduler = InferenceScheduler::new(
            "fixture-local".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "cloud-model".into(),
            "embedding-model".into(),
            false,
        )
        .with_scripted_generation_response("local fixture");
        let prepared = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "keep this request scoped".into(),
                }],
                Vec::new(),
                ContextManifest {
                    request_id: "local-restriction-request".into(),
                    privacy_decision_id: original_decision_id.clone(),
                    selected_context_refs: Vec::new(),
                    included_context_categories: Vec::new(),
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                restricted,
                allow_network_policy(),
                false,
            )
            .await
            .expect("restriction should preserve a valid local subject scope");
        let evidence = prepared.policy_receipt_evidence();
        assert_eq!(evidence.decision_id, original_decision_id);
        assert_eq!(
            evidence.issuing_authority,
            crate::llm::ProviderPolicyAuthority::MainChatPolicyRouter
        );
        assert_eq!(
            evidence.effective_local_restriction,
            Some(ProviderLocalOnlyReason::CloudDisabled)
        );
    }

    #[tokio::test]
    async fn policy_store_outbound_receipt_retains_typed_provenance_without_raw_profile_data() {
        let user_text = "ordinary PolicyStore governed request";
        let task = crate::agent::AgentTask {
            kind: crate::agent::AgentTaskKind::Conversation,
            session_id: "hs-provider-trace-session".into(),
            user_text: user_text.into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: user_text.into(),
            }],
            layer: crate::layer::Layer::L2,
        };
        let policy_context = crate::agent::build_runtime_policy_context(
            &crate::agent::PolicyStore::mvp_builtin(),
            crate::agent::RuntimePolicyContextBuildInput {
                task: &task,
                sanitized_intent_summary: user_text.into(),
                privacy_topic: crate::agent::PolicyTopic::General,
                risk_level: crate::agent::RiskLevel::Low,
                tool_requirements: Vec::new(),
            },
        )
        .expect("PolicyStore context retains the route-policy capability");
        let authorization = policy_context
            .provider_authorization()
            .clone()
            .authorize_derived_payload(
                crate::llm::ProviderPayloadPurpose::ScheduledTaskGeneration,
                user_text,
                &task.messages,
                &[],
            )
            .expect("PolicyStore policy binds the exact outbound payload");
        let provenance = policy_context.policy_provenance_refs().to_vec();
        assert!(provenance.iter().any(|reference| {
            reference.kind() == crate::llm::ProviderPolicyProvenanceKind::PolicyStoreRouteDecision
        }));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let provider_base = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = tokio::spawn(serve_one_json_response(
            listener,
            r#"{"choices":[{"message":{"content":"governed response"}}]}"#,
        ));
        let scheduler = InferenceScheduler::new(
            "unused-local".into(),
            false,
            "openai".into(),
            provider_base,
            "test-key".into(),
            "hs-governed-model".into(),
            "embedding-model".into(),
            false,
        );
        let prepared = scheduler
            .prepare_chat_request_with_authorization(
                task.messages.clone(),
                Vec::new(),
                ContextManifest {
                    request_id: "hs-provider-trace-request".into(),
                    privacy_decision_id: authorization.decision_id().to_string(),
                    selected_context_refs: Vec::new(),
                    included_context_categories: Vec::new(),
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: provenance.clone(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization,
                allow_network_policy(),
                false,
            )
            .await
            .expect("PolicyStore-governed provider request should prepare");
        assert!(!prepared.context_manifest.raw_life_model_included);
        assert!(!prepared.context_manifest.raw_unbounded_memory_included);
        let prepared_endpoint = prepared.provider_endpoint.clone();
        let prepared_generation = prepared.provider_config_generation.clone();
        let prepared_network_policy = prepared.network_policy.clone();
        let prepared_network_policy_decision = prepared.network_policy_decision.clone();

        let outcome = scheduler.execute_prepared(prepared).await;
        server.await.unwrap();
        let proof = outcome
            .terminal_proof
            .as_ref()
            .expect("a real adapter terminal must issue a runtime-only proof");
        assert_eq!(proof.receipt(), outcome.receipt.as_ref().unwrap());
        proof
            .validate_runtime_binding(
                "openai",
                "hs-governed-model",
                &prepared_endpoint,
                &prepared_generation,
                &crate::llm::provider_credential_identity("test-key"),
                0,
                &prepared_network_policy,
                &prepared_network_policy_decision,
            )
            .expect("proof must retain the exact endpoint and config generation");
        assert_eq!(outcome.result.unwrap(), "governed response");
        let receipt = outcome.receipt.expect("real provider receipt");
        let evidence = receipt
            .policy_evidence
            .expect("typed provider policy evidence");
        assert_eq!(evidence.policy_provenance_refs, provenance);
        assert!(evidence.context_manifest_digest.starts_with("sha256:"));
        assert!(!evidence.raw_life_model_included);
        assert!(!evidence.raw_unbounded_memory_included);
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert!(!serialized.contains(user_text));
        assert!(!serialized.contains("bounded context"));
    }

    #[tokio::test]
    async fn local_only_capability_remains_usable_without_cloud_provenance() {
        let authorization = local_only_test_authorization("", &[]);
        let scheduler = InferenceScheduler::new(
            "fixture-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "cloud-model".into(),
            "embedding-model".into(),
            false,
        )
        .with_scripted_generation_response("scripted local response");

        let prepared = scheduler
            .prepare_chat_request_with_authorization(
                vec![],
                vec![],
                ContextManifest {
                    request_id: "typed-local-request".into(),
                    privacy_decision_id: authorization.decision_id().to_string(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization,
                allow_network_policy(),
                false,
            )
            .await
            .unwrap();

        assert_eq!(prepared.provider_target, "ollama");
        assert_eq!(prepared.data_route, ProviderDataRoute::LocalOnly);
    }

    #[tokio::test]
    async fn serialized_metadata_cannot_rehydrate_cloud_authorization() {
        let authorization = canonical_cloud_authorization(
            "non-replayable-cloud-decision",
            "non-replayable request",
        );
        let scheduler = InferenceScheduler::new(
            "unused-local".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "cloud-model".into(),
            "embedding-model".into(),
            false,
        )
        .with_scripted_generation_response("scripted response");
        let prepared = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "non-replayable request".into(),
                }],
                vec![],
                ContextManifest {
                    request_id: "non-replayable-cloud-request".into(),
                    privacy_decision_id: authorization.decision_id().to_string(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization,
                allow_network_policy(),
                false,
            )
            .await
            .unwrap();

        let serialized = serde_json::to_value(&prepared).unwrap();
        assert!(serialized.get("policy_authorization").is_none());
        let rehydrated: crate::llm::PreparedProviderRequest =
            serde_json::from_value(serialized).unwrap();
        let error = rehydrated
            .validate()
            .expect_err("serialized metadata cannot replay an in-process cloud capability");
        assert!(error.to_string().contains("policy authorization"));
        assert_eq!(
            rehydrated.policy_authorization().data_route(),
            ProviderDataRoute::LocalOnly
        );
    }

    #[tokio::test]
    async fn reasoning_only_provider_response_produces_a_failed_receipt() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let provider_base = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = tokio::spawn(serve_one_json_response(
            listener,
            r#"{"choices":[{"message":{"reasoning_content":"private chain","content":""}}]}"#,
        ));
        let scheduler = InferenceScheduler::new(
            "unused-local".into(),
            false,
            "openai".into(),
            provider_base,
            "test-key".into(),
            "reasoning-only-model".into(),
            "embedding-model".into(),
            false,
        );
        let prepared = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "answer with final content".into(),
                }],
                vec![],
                ContextManifest {
                    request_id: "reasoning-only-request".into(),
                    privacy_decision_id: "reasoning-only-policy".into(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                canonical_cloud_authorization("reasoning-only-policy", "answer with final content"),
                allow_network_policy(),
                false,
            )
            .await
            .unwrap();

        let outcome = scheduler.execute_prepared(prepared).await;

        assert!(outcome
            .result
            .as_ref()
            .is_err_and(|error| error.contains("provider_reasoning_without_final_content")));
        assert_eq!(
            outcome
                .terminal_proof
                .as_ref()
                .map(|proof| proof.receipt().status),
            Some(ProviderInvocationStatus::Failed),
            "confirmed adapter failures retain proof without becoming success"
        );
        let receipt = outcome.receipt.expect("provider dispatch receipt");
        assert_eq!(receipt.status, ProviderInvocationStatus::Failed);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn hanging_provider_records_local_adapter_start_without_inventing_terminal_truth() {
        use tokio::io::AsyncReadExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let provider_base = format!("http://{}/v1", listener.local_addr().unwrap());
        let release_provider = std::sync::Arc::new(tokio::sync::Notify::new());
        let release_for_server = std::sync::Arc::clone(&release_provider);
        let (request_observed_tx, request_observed_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 16 * 1024];
            let _ = socket.read(&mut buffer).await.unwrap();
            let _ = request_observed_tx.send(());
            release_for_server.notified().await;
        });
        let scheduler = InferenceScheduler::new(
            "unused-local".into(),
            false,
            "openai".into(),
            provider_base,
            "test-key".into(),
            "hanging-model".into(),
            "embedding-model".into(),
            false,
        );
        let prepared = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "wait for cancellation".into(),
                }],
                vec![],
                ContextManifest {
                    request_id: "hanging-provider-request".into(),
                    privacy_decision_id: "hanging-provider-policy".into(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                canonical_cloud_authorization("hanging-provider-policy", "wait for cancellation"),
                allow_network_policy(),
                false,
            )
            .await
            .unwrap();
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let execution = tokio::spawn(async move {
            scheduler
                .execute_prepared_with_observer(prepared, move |progress| {
                    progress_tx.send(progress).unwrap();
                    Ok(())
                })
                .await
        });

        let observation_watchdog = std::time::Duration::from_secs(5);
        tokio::time::timeout(observation_watchdog, request_observed_rx)
            .await
            .expect("server observes the HTTP attempt")
            .expect("request observation channel remains open");
        let progress = tokio::time::timeout(observation_watchdog, progress_rx.recv())
            .await
            .expect("local adapter start is observable before response headers")
            .expect("provider progress channel remains open");
        assert!(matches!(
            progress,
            ProviderInvocationProgress::Started {
                request_id,
                provider,
                model,
                ..
            } if request_id == "hanging-provider-request"
                && provider == "openai"
                && model == "hanging-model"
        ));
        assert!(!execution.is_finished());

        execution.abort();
        let _ = execution.await;
        assert!(
            progress_rx.try_recv().is_err(),
            "dropping the local future must not invent a provider terminal receipt"
        );
        release_provider.notify_one();
        let _ = server.await;
    }

    #[tokio::test]
    async fn model_router_uses_configured_cloud_key_without_prior_availability_probe() {
        let scheduler = InferenceScheduler::new(
            "qwen2.5".into(),
            false,
            "deepseek".into(),
            "https://api.deepseek.com".into(),
            "sk-test".into(),
            "deepseek-chat".into(),
            "text-embedding-3-small".into(),
            false,
        )
        .with_model_router(ModelRouter::new());

        let trace = scheduler.preview_chat_route(None).await;

        assert_eq!(trace.provider, "deepseek");
        assert_eq!(trace.model, "deepseek-chat");
        assert_eq!(trace.route_type, "cloud");
        assert_eq!(trace.provider_health_is_estimated, None);
    }

    #[tokio::test]
    async fn model_router_keeps_configured_cloud_provider_unavailable_without_key() {
        let scheduler = InferenceScheduler::new(
            "qwen2.5".into(),
            false,
            "deepseek".into(),
            "https://api.deepseek.com".into(),
            "".into(),
            "deepseek-chat".into(),
            "text-embedding-3-small".into(),
            false,
        )
        .with_model_router(ModelRouter::new());

        let trace = scheduler.preview_chat_route(None).await;

        assert_ne!(trace.provider, "deepseek");
        assert_ne!(trace.route_type, "cloud");
        assert!(
            trace.reason.contains("No available providers")
                || trace.reason.contains("no_backend_available")
                || trace.reason.contains("ollama_available_and_preferred"),
            "unexpected route reason: {}",
            trace.reason
        );
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test]
    async fn local_only_prepared_request_fails_closed_before_cloud_generation_without_local_provider(
    ) {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let scheduler = InferenceScheduler::new(
            "openlife-local-model-that-does-not-exist".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            true,
        );

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "provider mismatch".into(),
        }];
        let authorization = local_only_test_authorization("provider mismatch", &messages);
        let err = scheduler
            .prepare_chat_request_with_authorization(
                messages,
                vec![],
                ContextManifest {
                    request_id: "request-local-only".into(),
                    privacy_decision_id: authorization.decision_id().to_string(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                authorization,
                allow_network_policy(),
                false,
            )
            .await
            .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("local"));
    }

    #[tokio::test]
    async fn prepared_request_rejects_router_target_that_has_no_matching_adapter() {
        let mut router = ModelRouter::new();
        router.providers.insert(
            "deepseek".into(),
            ProviderAvailability {
                provider: "deepseek".into(),
                available: true,
                latency_ms: Some(1),
                models: vec!["deepseek-chat".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: false,
            },
        );
        let scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        )
        .with_model_router(router);

        let err = scheduler
            .prepare_chat_request_with_authorization(
                vec![ChatMessage {
                    role: "user".into(),
                    content: "provider mismatch".into(),
                }],
                vec![],
                ContextManifest {
                    request_id: "request-provider-mismatch".into(),
                    privacy_decision_id: "policy-provider-mismatch".into(),
                    selected_context_refs: vec![],
                    included_context_categories: vec![],
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                    ],
                    policy_provenance_refs: Vec::new(),
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                canonical_cloud_authorization("policy-provider-mismatch", "provider mismatch"),
                allow_network_policy(),
                false,
            )
            .await
            .unwrap_err();

        assert!(err
            .to_string()
            .contains("does not match the configured cloud adapter"));
    }

    #[tokio::test]
    async fn provider_start_observer_is_not_called_for_pre_dispatch_rejection() {
        let scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );
        let network_policy = allow_network_policy();
        let network_policy_decision = crate::network_client::resolve_network_policy_decision(
            &network_policy,
            &crate::llm::chat_completions_url("deepseek", &scheduler.openai_base),
            "provider.deepseek",
        )
        .unwrap();
        let request = crate::llm::PreparedProviderRequest {
            messages: vec![],
            context_blocks: vec![],
            context_manifest: ContextManifest {
                request_id: "request-pre-dispatch-rejection".into(),
                privacy_decision_id: "policy-pre-dispatch-rejection".into(),
                selected_context_refs: vec![],
                included_context_categories: vec![],
                declared_payload_categories: vec![
                    crate::llm::ProviderPayloadCategory::CurrentUserConversation,
                ],
                policy_provenance_refs: Vec::new(),
                raw_life_model_included: false,
                raw_unbounded_memory_included: false,
            },
            provider_target: "deepseek".into(),
            model_target: "deepseek-chat".into(),
            provider_endpoint: crate::llm::chat_completions_url("deepseek", &scheduler.openai_base),
            provider_config_generation: scheduler.provider_config_generation().to_string(),
            provider_credential_version: scheduler.provider_credential_version(),
            data_route: ProviderDataRoute::PolicyAllowed,
            policy_authorization: canonical_cloud_authorization(
                "policy-pre-dispatch-rejection",
                "pre-dispatch rejection",
            ),
            network_policy,
            network_policy_decision,
            tools_required: false,
            execution_binding: None,
        };
        let mut start_observed = false;

        let outcome = scheduler
            .execute_prepared_with_start_observer(request, |_, _, _, _, _| {
                start_observed = true;
                Ok(())
            })
            .await;

        assert!(outcome.result.is_err());
        assert!(
            outcome.receipt.is_none(),
            "a pre-dispatch rejection must project provider status as not_attempted"
        );
        assert!(
            !start_observed,
            "provider_started must describe an actual adapter dispatch, not a rejected plan"
        );
    }

    #[test]
    fn explicit_provider_probe_authority_is_exact_and_serialization_cannot_restore_it() {
        let scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );
        let policy = allow_network_policy();
        let endpoint = crate::llm::chat_completions_url("openai", &scheduler.openai_base);
        let decision = crate::network_client::resolve_network_policy_decision(
            &policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();
        let (scheduler, grant) = governed_probe(scheduler, policy, decision);
        let request = scheduler.prepare_explicit_provider_probe(grant).unwrap();

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "user");
        assert_eq!(request.messages[0].content, "ping");
        assert_eq!(
            request.policy_authorization().authority(),
            crate::llm::ProviderPolicyAuthority::ExplicitProviderProbePolicy
        );
        assert_eq!(
            request.policy_authorization().data_route(),
            ProviderDataRoute::PolicyAllowed
        );
        request.validate().unwrap();

        let mut message_tampered = request.clone();
        message_tampered.messages[0].content = "send a private workspace dump".into();
        assert!(message_tampered.validate().is_err());

        let mut model_tampered = request.clone();
        model_tampered.model_target = "different-model".into();
        assert!(model_tampered.validate().is_err());

        let mut endpoint_tampered = request.clone();
        endpoint_tampered.provider_endpoint = "https://api.openai.com/v2/responses".into();
        assert!(endpoint_tampered.validate().is_err());

        let mut credential_generation_tampered = request.clone();
        credential_generation_tampered.provider_credential_version = 99;
        assert!(credential_generation_tampered.validate().is_err());

        let mut network_tampered = request.clone();
        network_tampered.network_policy_decision.reason_code = "forged_allow".into();
        assert!(network_tampered.validate().is_err());

        let encoded = serde_json::to_string(&request).unwrap();
        let rehydrated: crate::llm::PreparedProviderRequest =
            serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            rehydrated.policy_authorization().authority(),
            crate::llm::ProviderPolicyAuthority::LocalOnlyFailClosed
        );
        assert!(rehydrated.validate().is_err());
    }

    #[test]
    fn unbound_scheduler_cannot_accept_even_a_canonical_store_grant() {
        let scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );
        let policy = allow_network_policy();
        let endpoint = crate::llm::chat_completions_url("openai", &scheduler.openai_base);
        let decision = crate::network_client::resolve_network_policy_decision(
            &policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();
        let store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let challenge = scheduler.explicit_provider_probe_challenge().unwrap();
        let grant = store
            .issue_explicit_provider_probe_grant(
                challenge,
                policy,
                &decision,
                decision.clone(),
                None,
            )
            .unwrap();

        let error = scheduler
            .prepare_explicit_provider_probe(grant)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("verifier is not bound by ToolPermissionStore"));
    }

    #[test]
    fn provider_model_is_final_before_prepared_request_binding() {
        let scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "deepseek".into(),
            "https://api.deepseek.com".into(),
            "sk-test".into(),
            "  deepseek-reasoner  ".into(),
            "text-embedding-3-small".into(),
            false,
        );
        assert_eq!(scheduler.chat_model, "deepseek-reasoner");
        assert_eq!(
            scheduler.provider_runtime_identity.model,
            "deepseek-reasoner"
        );
    }

    #[test]
    fn explicit_provider_probe_rejects_a_decision_for_another_endpoint() {
        let scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );
        let policy = allow_network_policy();
        let wrong_decision = crate::network_client::resolve_network_policy_decision(
            &policy,
            "https://api.openai.com/v2/responses",
            "provider.openai",
        )
        .unwrap();

        let store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let scheduler = store.bind_explicit_provider_probe_scheduler(scheduler);
        let challenge = scheduler.explicit_provider_probe_challenge().unwrap();
        let error = store
            .issue_explicit_provider_probe_grant(
                challenge,
                policy,
                &wrong_decision,
                wrong_decision.clone(),
                None,
            )
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("explicit_provider_probe_network_authority_mismatch"));
    }

    #[test]
    fn another_opaque_issuer_cannot_mint_for_the_target_scheduler() {
        let target = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );
        let other = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );
        let policy = allow_network_policy();
        let endpoint = crate::llm::chat_completions_url("openai", &other.openai_base);
        let decision = crate::network_client::resolve_network_policy_decision(
            &policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();
        let target_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let target = target_store.bind_explicit_provider_probe_scheduler(target);
        let (_other, foreign_grant) = governed_probe(other, policy, decision);

        let error = target
            .prepare_explicit_provider_probe(foreign_grant)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("explicit_provider_probe_issuer_mismatch"));
    }

    #[tokio::test]
    async fn prepared_provider_probe_cannot_cross_scheduler_generation_or_endpoint_path() {
        let scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        )
        .with_provider_credential_version(7);
        let policy = allow_network_policy();
        let endpoint = crate::llm::chat_completions_url("openai", &scheduler.openai_base);
        let decision = crate::network_client::resolve_network_policy_decision(
            &policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();
        let (scheduler, grant) = governed_probe(scheduler, policy, decision);
        let request = scheduler.prepare_explicit_provider_probe(grant).unwrap();
        let other_scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v2".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        )
        .with_provider_credential_version(7);

        let outcome = other_scheduler.execute_prepared(request).await;
        assert!(outcome.receipt.is_none());
        assert!(outcome
            .result
            .unwrap_err()
            .contains("another config generation"));
    }

    #[tokio::test]
    async fn prepared_provider_probe_never_drifts_to_mutated_scheduler_config() {
        let scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        )
        .with_provider_credential_version(9);
        let policy = allow_network_policy();
        let endpoint = crate::llm::chat_completions_url("openai", &scheduler.openai_base);
        let decision = crate::network_client::resolve_network_policy_decision(
            &policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();
        let (mut scheduler, grant) = governed_probe(scheduler, policy, decision);
        let request = scheduler.prepare_explicit_provider_probe(grant).unwrap();

        scheduler.openai_base = "https://api.openai.com/v2".into();
        scheduler.openai_key = "sk-replaced".into();
        let rejection = scheduler
            .validate_prepared_execution_owner(&request)
            .expect_err("mutated runtime must fail before proof capture or dispatch");
        assert!(rejection.is::<PreparedProviderGenerationMismatch>());
        let outcome = scheduler.execute_prepared(request).await;

        assert!(outcome.receipt.is_none());
        assert!(outcome.terminal_proof.is_none());
        assert!(outcome
            .result
            .unwrap_err()
            .contains("belongs to another config generation"));
    }

    #[test]
    fn provider_probe_rejects_key_mutation_before_prepare() {
        let scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-original".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        )
        .with_provider_credential_version(10);
        let policy = allow_network_policy();
        let endpoint = crate::llm::chat_completions_url("openai", &scheduler.openai_base);
        let decision = crate::network_client::resolve_network_policy_decision(
            &policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();
        let (mut scheduler, grant) = governed_probe(scheduler, policy, decision);
        scheduler.openai_key = "sk-mutated".into();

        let error = scheduler
            .prepare_explicit_provider_probe(grant)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("provider runtime identity changed"));
    }
}
