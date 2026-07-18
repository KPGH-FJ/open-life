use crate::agent::proposal_store::{
    proposal_terminal_relation_storage_request_digest, ProposalStore, ProposalTerminalRelationKind,
    ProposalTerminalRelationRecord, ProposalTerminalRelationStoreOutcome,
    TerminalOwnerOriginBinding,
};
use crate::agent::store::AgentRunTerminalRelationTargetIntentAdmission;
use crate::agent::types::{AgentProposal, ProposalStatus, ProposalType, RiskLevel};
use crate::agent::{CanonicalWriteAdmission, CanonicalWriteAdmissionRequest};
use anyhow::{anyhow, Result};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurableWriteSource {
    MainChat,
    Builder,
    Calibration,
    ToolPermission,
    PlanExecute,
    Maturation,
    Proactive,
    ManualOverride,
    SkillRuntime,
    NetworkConsent,
    TestFixture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurableWriteSubject {
    Memory,
    LifeModel,
    ToolPermission,
    ExternalWrite,
    FileWrite,
    Calendar,
    Email,
    PlanStep,
    MaturationCandidate,
}

impl DurableWriteSubject {
    pub fn from_proposal_type(proposal_type: ProposalType) -> Self {
        match proposal_type {
            ProposalType::MemoryWrite | ProposalType::MemoryArchive => Self::Memory,
            ProposalType::GoalUpdate
            | ProposalType::StateUpdate
            | ProposalType::PreferenceUpdate
            | ProposalType::CapabilityUpdate
            | ProposalType::LifeModelUpdate => Self::LifeModel,
            ProposalType::ToolPermission | ProposalType::PluginPermission => Self::ToolPermission,
            ProposalType::ScheduledTask | ProposalType::ScheduleCheckin => Self::Calendar,
            ProposalType::DataExport | ProposalType::ExternalWriteAction => Self::ExternalWrite,
            ProposalType::ModelPolicyChange | ProposalType::Unsupported => Self::LifeModel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalDeliveryWordingContract {
    PendingProposalOnly,
    ApprovalRequiredBeforeDurableWrite,
    TestFixtureOnly,
}

impl FinalDeliveryWordingContract {
    pub fn pending_message(self) -> &'static str {
        match self {
            Self::PendingProposalOnly => {
                "Proposal is pending Review Center approval; no durable write has been applied."
            }
            Self::ApprovalRequiredBeforeDurableWrite => {
                "Review Center approval is required before any durable write is applied."
            }
            Self::TestFixtureOnly => {
                "Test fixture proposal was seeded for local verification only."
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableWriteRequest {
    pub source: DurableWriteSource,
    pub subject: DurableWriteSubject,
    pub risk_level: RiskLevel,
    pub user_visible_summary: String,
    pub evidence_refs: Vec<String>,
    pub idempotency_key: String,
    pub requires_approval: bool,
    pub final_delivery_wording_contract: FinalDeliveryWordingContract,
    pub proposal: AgentProposal,
    pub existing_proposal_id: Option<String>,
}

impl DurableWriteRequest {
    pub fn from_agent_proposal(
        source: DurableWriteSource,
        subject: DurableWriteSubject,
        proposal: AgentProposal,
        user_visible_summary: impl Into<String>,
    ) -> Self {
        let risk_level = proposal.risk_level;
        let idempotency_key = default_idempotency_key(source, subject, &proposal);
        Self {
            source,
            subject,
            risk_level,
            user_visible_summary: user_visible_summary.into(),
            evidence_refs: Vec::new(),
            idempotency_key,
            requires_approval: true,
            final_delivery_wording_contract: FinalDeliveryWordingContract::PendingProposalOnly,
            proposal,
            existing_proposal_id: None,
        }
    }

    pub fn with_evidence_refs(mut self, evidence_refs: Vec<String>) -> Self {
        self.evidence_refs = evidence_refs;
        self
    }

    pub fn with_existing_proposal_id(mut self, proposal_id: Option<String>) -> Self {
        self.existing_proposal_id = proposal_id;
        self
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = idempotency_key.into();
        self
    }

    pub fn with_requires_approval(mut self, requires_approval: bool) -> Self {
        self.requires_approval = requires_approval;
        self
    }

    pub fn with_final_delivery_wording_contract(
        mut self,
        contract: FinalDeliveryWordingContract,
    ) -> Self {
        self.final_delivery_wording_contract = contract;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurableWriteDecisionKind {
    CreatePendingProposal,
    ReusePendingProposal,
    UpdatePendingProposal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableWriteDecision {
    pub kind: DurableWriteDecisionKind,
    pub proposal_id: String,
    pub proposal_status: ProposalStatus,
    pub durable_write_completed: bool,
    pub idempotency_key: String,
    pub requires_approval: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewWorkflowOutcome {
    pub proposal: AgentProposal,
    pub decision: DurableWriteDecision,
    pub user_visible_status: String,
    pub user_visible_summary: String,
    pub final_delivery_message: String,
    pub evidence_refs: Vec<String>,
}

impl ReviewWorkflowOutcome {
    pub fn proposal_id(&self) -> &str {
        &self.proposal.id
    }

    pub fn durable_write_completed(&self) -> bool {
        self.decision.durable_write_completed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalOwnerReviewOriginAuthority {
    BoundToVerifiedEpochAdmission,
}

/// Non-serializable authority tying a Review item to the exact terminal epoch
/// opened from a TaskSession-verified canonical user-message admission.
#[derive(Debug, Clone)]
pub struct TerminalOwnerReviewOriginProof {
    task_session_id: String,
    run_id: String,
    epoch_id: String,
    epoch_generation: u64,
    admission_id: String,
    canonical_user_message_ref: String,
    canonical_user_message_digest: String,
    canonical_store_identity: String,
    authority: TerminalOwnerReviewOriginAuthority,
}

#[derive(Debug, Clone)]
pub(crate) enum ProposalTerminalRelationSubmitOutcome {
    CreatedOwned {
        review: ReviewWorkflowOutcome,
        relation: ProposalTerminalRelationRecord,
    },
    ReplayedSameOrigin {
        review: ReviewWorkflowOutcome,
        relation: ProposalTerminalRelationRecord,
    },
    ReusedForeignNonBlocking {
        review: ReviewWorkflowOutcome,
    },
}

/// Product-facing result of the atomic Proposal + terminal relation boundary.
/// The canonical relation row remains private to ProposalStore; callers only
/// learn whether this exact origin owns a projection that must be applied.
#[derive(Debug, Clone)]
pub struct TerminalOwnerReviewSubmission {
    review: ReviewWorkflowOutcome,
    owned_relation: bool,
}

impl TerminalOwnerReviewSubmission {
    pub fn review(&self) -> &ReviewWorkflowOutcome {
        &self.review
    }

    pub fn owns_terminal_relation(&self) -> bool {
        self.owned_relation
    }
}

impl ProposalTerminalRelationSubmitOutcome {
    pub(crate) fn review(&self) -> &ReviewWorkflowOutcome {
        match self {
            Self::CreatedOwned { review, .. }
            | Self::ReplayedSameOrigin { review, .. }
            | Self::ReusedForeignNonBlocking { review } => review,
        }
    }

    pub(crate) fn owned_relation(&self) -> Option<&ProposalTerminalRelationRecord> {
        match self {
            Self::CreatedOwned { relation, .. } | Self::ReplayedSameOrigin { relation, .. } => {
                Some(relation)
            }
            Self::ReusedForeignNonBlocking { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProposalTerminalRelationStorageWriteAuthority {
    IssuedByReviewWorkflowAfterPolicyAdmission,
}

/// Opaque, non-serializable capability proving that ReviewWorkflow completed
/// the approval, wording, identity, and cancellation checks for this exact
/// typed relation write. ProposalStore validates the bound request digest and
/// cannot be called with caller-shaped strings alone.
#[derive(Debug)]
pub(super) struct ProposalTerminalRelationStorageWriteProof {
    request_digest: String,
    authority: ProposalTerminalRelationStorageWriteAuthority,
}

impl ProposalTerminalRelationStorageWriteProof {
    fn issue(request_digest: String) -> Self {
        Self {
            request_digest,
            authority:
                ProposalTerminalRelationStorageWriteAuthority::IssuedByReviewWorkflowAfterPolicyAdmission,
        }
    }

    pub(super) fn validate_for(&self, expected_request_digest: &str) -> Result<()> {
        if self.authority
            != ProposalTerminalRelationStorageWriteAuthority::IssuedByReviewWorkflowAfterPolicyAdmission
            || self.request_digest != expected_request_digest
        {
            anyhow::bail!("proposal_terminal_relation_storage_write_authority_invalid");
        }
        Ok(())
    }
}

impl crate::agent::main_chat_agent_v1::TerminalOwnerEpochAdmission {
    /// Consume the opaque TaskSession-store admission after the event store
    /// has durably opened (or reloaded) the exact epoch. A caller cannot mint
    /// a second origin from the same admission or deserialize this authority.
    pub fn into_opened_epoch_review_origin(
        self,
        epoch_id: String,
        epoch_generation: u64,
    ) -> Result<TerminalOwnerReviewOriginProof> {
        self.validate()?;
        if epoch_id.trim().is_empty() || epoch_generation == 0 {
            anyhow::bail!("terminal owner epoch origin is invalid");
        }
        Ok(TerminalOwnerReviewOriginProof {
            task_session_id: self.task_session_id().to_string(),
            run_id: self.run_id().to_string(),
            epoch_id,
            epoch_generation,
            admission_id: self.admission_id().to_string(),
            canonical_user_message_ref: self.canonical_user_message_ref().to_string(),
            canonical_user_message_digest: self.canonical_user_message_digest().to_string(),
            canonical_store_identity: self.canonical_store_identity().to_string(),
            authority: TerminalOwnerReviewOriginAuthority::BoundToVerifiedEpochAdmission,
        })
    }
}

impl TerminalOwnerReviewOriginProof {
    pub fn validate(&self) -> Result<()> {
        if self.authority != TerminalOwnerReviewOriginAuthority::BoundToVerifiedEpochAdmission
            || self.task_session_id.trim().is_empty()
            || self.run_id.trim().is_empty()
            || self.epoch_id.trim().is_empty()
            || self.epoch_generation == 0
            || self.admission_id.trim().is_empty()
            || self.canonical_user_message_ref.trim().is_empty()
            || self.canonical_user_message_digest.trim().is_empty()
            || self.canonical_store_identity.trim().is_empty()
        {
            anyhow::bail!("terminal owner review origin authority is invalid");
        }
        Ok(())
    }

    pub fn task_session_id(&self) -> &str {
        &self.task_session_id
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn epoch_id(&self) -> &str {
        &self.epoch_id
    }

    pub fn epoch_generation(&self) -> u64 {
        self.epoch_generation
    }

    pub fn admission_id(&self) -> &str {
        &self.admission_id
    }

    pub fn canonical_user_message_ref(&self) -> &str {
        &self.canonical_user_message_ref
    }

    pub fn canonical_user_message_digest(&self) -> &str {
        &self.canonical_user_message_digest
    }

    pub fn canonical_store_identity(&self) -> &str {
        &self.canonical_store_identity
    }
}

#[cfg(test)]
pub(super) fn terminal_owner_review_origin_fixture(label: &str) -> TerminalOwnerReviewOriginProof {
    TerminalOwnerReviewOriginProof {
        task_session_id: format!("task:{label}"),
        run_id: format!("run:{label}"),
        epoch_id: format!("epoch:{label}"),
        epoch_generation: 1,
        admission_id: format!("admission:{label}"),
        canonical_user_message_ref: format!("message:{label}"),
        canonical_user_message_digest: format!("sha256:{:0>64}", label.len()),
        canonical_store_identity: format!("canonical-store:{label}"),
        authority: TerminalOwnerReviewOriginAuthority::BoundToVerifiedEpochAdmission,
    }
}

#[cfg(test)]
pub(super) fn terminal_owner_review_origin_fixture_for_run(
    task_session_id: &str,
    run_id: &str,
    canonical_user_message_ref: &str,
    canonical_user_message_digest: &str,
    canonical_store_identity: &str,
) -> TerminalOwnerReviewOriginProof {
    TerminalOwnerReviewOriginProof {
        task_session_id: task_session_id.to_string(),
        run_id: run_id.to_string(),
        epoch_id: format!("epoch:{run_id}"),
        epoch_generation: 1,
        admission_id: format!("admission:{run_id}"),
        canonical_user_message_ref: canonical_user_message_ref.to_string(),
        canonical_user_message_digest: canonical_user_message_digest.to_string(),
        canonical_store_identity: canonical_store_identity.to_string(),
        authority: TerminalOwnerReviewOriginAuthority::BoundToVerifiedEpochAdmission,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewAcceptanceAuthorityProof {
    ClaimedByReviewWorkflow,
}

/// Non-serializable proof that the exact Proposal snapshot currently owns the
/// Review Center acceptance dispatch claim.
///
/// The Proposal remains pending until its effect receipt is durable; this
/// object proves the user's accepted action and exact snapshot without
/// misreporting the effect as completed. Deserialized Proposal/claim strings
/// cannot construct this type because the authority proof is private.
#[derive(Debug, Clone)]
pub struct ClaimedReviewAcceptanceSnapshot {
    proposal: AgentProposal,
    proposal_snapshot_digest: String,
    dispatch_claim_digest: String,
    terminal_owner_origin: Option<TerminalOwnerOriginBinding>,
    authority_proof: ReviewAcceptanceAuthorityProof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterializedReviewAcceptanceAuthorityProof {
    ReloadedFromCanonicalReviewWorkflow,
}

/// Non-serializable restart proof that ProposalStore durably confirmed the
/// effect for the exact pre-dispatch Proposal snapshot and claim.
#[derive(Debug, Clone)]
pub struct MaterializedReviewAcceptanceSnapshot {
    proposal: AgentProposal,
    proposal_snapshot_digest: String,
    dispatch_claim_digest: String,
    authority_proof: MaterializedReviewAcceptanceAuthorityProof,
}

impl MaterializedReviewAcceptanceSnapshot {
    pub fn validate(&self) -> Result<()> {
        if self.authority_proof
            != MaterializedReviewAcceptanceAuthorityProof::ReloadedFromCanonicalReviewWorkflow
        {
            anyhow::bail!("materialized review acceptance authority proof is unavailable");
        }
        if self.proposal_snapshot_digest.trim().is_empty()
            || self.dispatch_claim_digest.trim().is_empty()
        {
            anyhow::bail!("materialized review acceptance provenance is incomplete");
        }
        Ok(())
    }

    pub fn proposal(&self) -> &AgentProposal {
        &self.proposal
    }

    pub fn proposal_snapshot_digest(&self) -> &str {
        &self.proposal_snapshot_digest
    }

    pub fn dispatch_claim_digest(&self) -> &str {
        &self.dispatch_claim_digest
    }
}

impl ClaimedReviewAcceptanceSnapshot {
    pub fn validate(&self) -> Result<()> {
        if self.authority_proof != ReviewAcceptanceAuthorityProof::ClaimedByReviewWorkflow {
            anyhow::bail!("review acceptance authority proof is unavailable");
        }
        if !matches!(
            self.proposal.status,
            ProposalStatus::Pending | ProposalStatus::Postponed | ProposalStatus::Edited
        ) || self.proposal.is_expired()
        {
            anyhow::bail!("review acceptance snapshot is not active");
        }
        if review_proposal_snapshot_digest(&self.proposal)? != self.proposal_snapshot_digest {
            anyhow::bail!("review acceptance snapshot changed after claim");
        }
        if self.dispatch_claim_digest.trim().is_empty() {
            anyhow::bail!("review acceptance dispatch claim digest is missing");
        }
        Ok(())
    }

    pub fn proposal(&self) -> &AgentProposal {
        &self.proposal
    }

    pub fn proposal_snapshot_digest(&self) -> &str {
        &self.proposal_snapshot_digest
    }

    pub fn dispatch_claim_digest(&self) -> &str {
        &self.dispatch_claim_digest
    }

    pub fn terminal_owner_origin(&self) -> Option<&TerminalOwnerOriginBinding> {
        self.terminal_owner_origin.as_ref()
    }
}

pub struct ReviewWorkflow<'a> {
    proposal_store: &'a ProposalStore,
}

impl<'a> ReviewWorkflow<'a> {
    pub fn new(proposal_store: &'a ProposalStore) -> Self {
        Self { proposal_store }
    }

    /// Single atomic Review relation boundary. ProposalStore commits the
    /// Proposal, immutable origin, typed relation, and metadata-only outbox in
    /// one IMMEDIATE transaction. The outbox only expresses projection intent;
    /// it does not claim a cross-database AgentRun link is already complete.
    pub(crate) fn submit_with_terminal_owner_relation(
        &self,
        mut request: DurableWriteRequest,
        origin: &TerminalOwnerReviewOriginProof,
        relation_kind: ProposalTerminalRelationKind,
        admission: &dyn CanonicalWriteAdmission,
        agent_run_target: &AgentRunTerminalRelationTargetIntentAdmission,
    ) -> Result<ProposalTerminalRelationSubmitOutcome> {
        origin.validate()?;
        if relation_kind == ProposalTerminalRelationKind::LegacyUnclassified {
            anyhow::bail!("legacy_unclassified_relation_requires_migration");
        }
        agent_run_target.validate_for(origin, relation_kind)?;
        validate_pending_wording(&request)?;
        if !request.requires_approval {
            anyhow::bail!("terminal_relation_submission_requires_approval");
        }
        if request.existing_proposal_id.is_some() {
            anyhow::bail!("terminal relation submission forbids caller-selected replacement");
        }
        request.proposal.status = ProposalStatus::Pending;
        request.proposal.resolved_at = None;
        request.proposal.run_id = None;
        request.proposal.source_detail = None;

        let storage_request_digest = proposal_terminal_relation_storage_request_digest(
            &request.proposal,
            &request.idempotency_key,
            origin.task_session_id(),
            origin.run_id(),
            origin.epoch_id(),
            origin.epoch_generation(),
            origin.admission_id(),
            origin.canonical_user_message_ref(),
            origin.canonical_user_message_digest(),
            origin.canonical_store_identity(),
            relation_kind,
            agent_run_target.target_binding_digest(),
            agent_run_target.agent_run_store_identity_digest(),
            agent_run_target.owner_revision(),
            agent_run_target.status_at_issue(),
        )?;
        let permit = admission
            .acquire(CanonicalWriteAdmissionRequest::new(
                "proposal_terminal_relation",
                format!(
                    "proposal_relation:{}",
                    sha256_hex(request.idempotency_key.as_bytes())
                ),
            ))
            .map_err(anyhow::Error::from)?;
        let write_proof = ProposalTerminalRelationStorageWriteProof::issue(storage_request_digest);
        let store_outcome = self
            .proposal_store
            .create_or_reuse_active_review_proposal_with_terminal_relation(
                &write_proof,
                &request.proposal,
                &request.idempotency_key,
                origin.task_session_id(),
                origin.run_id(),
                origin.epoch_id(),
                origin.epoch_generation(),
                origin.admission_id(),
                origin.canonical_user_message_ref(),
                origin.canonical_user_message_digest(),
                origin.canonical_store_identity(),
                relation_kind,
                agent_run_target.target_binding_digest(),
                agent_run_target.agent_run_store_identity_digest(),
                agent_run_target.owner_revision(),
                agent_run_target.status_at_issue(),
            );

        match store_outcome {
            Ok(ProposalTerminalRelationStoreOutcome::CreatedOwned { proposal, relation }) => {
                permit.finish_committed();
                Ok(ProposalTerminalRelationSubmitOutcome::CreatedOwned {
                    review: outcome(
                        request,
                        proposal,
                        DurableWriteDecisionKind::CreatePendingProposal,
                        "created_pending_review_proposal_with_terminal_relation",
                    ),
                    relation,
                })
            }
            Ok(ProposalTerminalRelationStoreOutcome::ReplayedSameOrigin { proposal, relation }) => {
                permit.finish_noop();
                Ok(ProposalTerminalRelationSubmitOutcome::ReplayedSameOrigin {
                    review: outcome(
                        request,
                        proposal,
                        DurableWriteDecisionKind::ReusePendingProposal,
                        "replayed_pending_review_proposal_with_terminal_relation",
                    ),
                    relation,
                })
            }
            Ok(ProposalTerminalRelationStoreOutcome::ReusedForeignNonBlocking { proposal }) => {
                permit.finish_noop();
                Ok(
                    ProposalTerminalRelationSubmitOutcome::ReusedForeignNonBlocking {
                        review: outcome(
                            request,
                            proposal,
                            DurableWriteDecisionKind::ReusePendingProposal,
                            "reused_foreign_review_proposal_without_current_origin_link",
                        ),
                    },
                )
            }
            Err(error) => {
                permit.finish_failed();
                Err(error)
            }
        }
    }

    /// Public product seam for submitting a Review item that has an explicit
    /// lifecycle relationship to the currently open terminal owner. The
    /// detailed relation record is intentionally not exposed across the Core
    /// boundary; ProposalStore remains its sole canonical owner.
    pub fn submit_product_with_terminal_owner_relation(
        &self,
        request: DurableWriteRequest,
        origin: &TerminalOwnerReviewOriginProof,
        relation_kind: ProposalTerminalRelationKind,
        admission: &dyn CanonicalWriteAdmission,
        agent_run_target: &AgentRunTerminalRelationTargetIntentAdmission,
    ) -> Result<TerminalOwnerReviewSubmission> {
        let outcome = self.submit_with_terminal_owner_relation(
            request,
            origin,
            relation_kind,
            admission,
            agent_run_target,
        )?;
        Ok(TerminalOwnerReviewSubmission {
            review: outcome.review().clone(),
            owned_relation: outcome.owned_relation().is_some(),
        })
    }

    pub fn submit_with_terminal_owner_origin(
        &self,
        mut request: DurableWriteRequest,
        origin: &TerminalOwnerReviewOriginProof,
    ) -> Result<ReviewWorkflowOutcome> {
        origin.validate()?;
        validate_pending_wording(&request)?;
        if request.requires_approval {
            request.proposal.status = ProposalStatus::Pending;
            request.proposal.resolved_at = None;
        }
        if request.existing_proposal_id.is_some() {
            anyhow::bail!("terminal owner review does not permit caller-selected replacement");
        }
        request.proposal.run_id = None;
        request.proposal.source_detail = None;
        let proposal = request.proposal.clone();
        let (proposal, created) = self
            .proposal_store
            .create_or_reuse_active_review_proposal_with_terminal_origin(
                &proposal,
                &request.idempotency_key,
                origin.task_session_id(),
                origin.run_id(),
                origin.epoch_id(),
                origin.epoch_generation(),
                origin.admission_id(),
                origin.canonical_user_message_ref(),
                origin.canonical_user_message_digest(),
            )?;
        Ok(outcome(
            request,
            proposal,
            if created {
                DurableWriteDecisionKind::CreatePendingProposal
            } else {
                DurableWriteDecisionKind::ReusePendingProposal
            },
            if created {
                "created_pending_review_proposal"
            } else {
                "reused_pending_review_proposal"
            },
        ))
    }

    /// Consume one exact PolicyRouter grant after a successful canonical read
    /// observation. ReviewWorkflow constructs the Proposal itself so a caller
    /// cannot substitute request prose, a different candidate, or different
    /// evidence-shaped strings after policy admission.
    pub fn submit_conditional_observation_memory_review(
        &self,
        grant: crate::agent::main_chat_agent_v1::PolicyConditionalObservationReviewGrant,
        admission: &dyn CanonicalWriteAdmission,
    ) -> Result<ReviewWorkflowOutcome> {
        let request = Self::prepare_conditional_observation_memory_review(grant);
        self.submit_with_admission(request, admission)
    }

    /// Consume the exact one-shot policy grant into a canonical Review
    /// request without persisting it. Product runtimes can then submit that
    /// request through the typed terminal-relation gateway instead of first
    /// creating an untyped Proposal and binding it later.
    pub fn prepare_conditional_observation_memory_review(
        grant: crate::agent::main_chat_agent_v1::PolicyConditionalObservationReviewGrant,
    ) -> DurableWriteRequest {
        let evidence_refs = vec![
            format!("main_chat_operation:{}", grant.operation_id()),
            format!(
                "canonical_user_message:{}:{}",
                grant.source_user_message_id(),
                grant.source_user_message_digest()
            ),
            format!("agent_run:{}", grant.run_id()),
            format!("agent_action:{}", grant.action_id()),
            format!("agent_observation:{}", grant.observation_id()),
            format!("bound_output_receipt:{}", grant.output_receipt_digest()),
            format!("tool_execution_receipt:{}", grant.tool_receipt_id()),
            format!("policy_conditional_grant:{}", grant.policy_grant_id()),
        ];
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.pending.observation_review",
            serde_json::json!({
                "content": grant.candidate_body(),
                "scope": "global",
                "category": "fact",
                "riskLevel": "medium",
                "sensitivity": "internal",
                "candidateKind": "semantic_user_fact",
                "source": "main_chat_observation",
                "operationId": grant.operation_id(),
                "sourceUserMessageId": grant.source_user_message_id(),
                "sourceUserMessageDigest": grant.source_user_message_digest(),
                "sourceRunId": grant.run_id(),
                "sourceActionId": grant.action_id(),
                "sourceObservationId": grant.observation_id(),
                "sourceOutputReceiptDigest": grant.output_receipt_digest(),
                "sourceToolReceiptId": grant.tool_receipt_id(),
                "candidateDigest": grant.candidate_digest(),
                "policyGrantId": grant.policy_grant_id(),
                "policyContractDigest": grant.policy_contract_digest(),
                "reviewPath": "mailbox",
                "acceptedDurableTruthWritten": false,
                "directWritesExecuted": false,
            }),
            "A useful supported Memory candidate was derived from an admitted current-turn observation and requires review.",
            0.86,
            RiskLevel::Medium,
            crate::agent::ProposalSource::ChatConversation,
        );
        proposal.run_id = Some(grant.run_id().to_string());
        proposal.source_detail = Some(grant.operation_id().to_string());
        // ReviewWorkflow's canonical idempotency key covers the full
        // observation-bound `after` payload above. Do not introduce a second
        // key namespace that ProposalStore cannot reconstruct.
        DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::MainChat,
            DurableWriteSubject::Memory,
            proposal,
            "Observation-derived Memory proposal is pending Review Center approval.",
        )
        .with_evidence_refs(evidence_refs)
    }

    pub fn submit(&self, mut request: DurableWriteRequest) -> Result<ReviewWorkflowOutcome> {
        validate_pending_wording(&request)?;
        if request.requires_approval {
            request.proposal.status = ProposalStatus::Pending;
            request.proposal.resolved_at = None;
        }

        if let Some(existing_id) = request.existing_proposal_id.as_deref() {
            if let Some(mut existing) = self.proposal_store.get_proposal(existing_id)? {
                if existing.status == ProposalStatus::Pending {
                    let mut replacement = request.proposal.clone();
                    replacement.id = existing.id.clone();
                    replacement.created_at = existing.created_at;
                    replacement.status = ProposalStatus::Pending;
                    replacement.resolved_at = None;
                    existing = replacement;
                    if !self.proposal_store.update_active_review_proposal(
                        &existing,
                        existing_id,
                        &request.idempotency_key,
                    )? {
                        anyhow::bail!("linked pending review proposal changed before update");
                    }
                    return Ok(outcome(
                        request,
                        existing,
                        DurableWriteDecisionKind::UpdatePendingProposal,
                        "linked_existing_pending_proposal",
                    ));
                }
            }
        }

        let proposal = request.proposal.clone();
        let (proposal, created) = self
            .proposal_store
            .create_or_reuse_active_review_proposal(&proposal, &request.idempotency_key)?;
        Ok(outcome(
            request,
            proposal,
            if created {
                DurableWriteDecisionKind::CreatePendingProposal
            } else {
                DurableWriteDecisionKind::ReusePendingProposal
            },
            if created {
                "created_pending_review_proposal"
            } else {
                "reused_existing_active_review_proposal"
            },
        ))
    }

    /// Issue an ephemeral acceptance proof only after ProposalStore confirms
    /// the exact id + dispatch claim is the sole claimed Review Center action.
    pub fn claimed_acceptance_snapshot(
        &self,
        proposal_id: &str,
        dispatch_claim_id: &str,
    ) -> Result<ClaimedReviewAcceptanceSnapshot> {
        if proposal_id.trim().is_empty() || dispatch_claim_id.trim().is_empty() {
            anyhow::bail!("review acceptance claim is incomplete");
        }
        let (proposal, persisted_snapshot_digest) = self
            .proposal_store
            .claimed_dispatch_proposal(proposal_id, dispatch_claim_id)?
            .ok_or_else(|| anyhow!("review acceptance dispatch claim is not canonical"))?;
        let computed_snapshot_digest = review_proposal_snapshot_digest(&proposal)?;
        if persisted_snapshot_digest != computed_snapshot_digest {
            anyhow::bail!("review acceptance snapshot digest is not canonical");
        }
        let snapshot = ClaimedReviewAcceptanceSnapshot {
            proposal_snapshot_digest: persisted_snapshot_digest,
            dispatch_claim_digest: format!("sha256:{}", sha256_hex(dispatch_claim_id.as_bytes())),
            terminal_owner_origin: self
                .proposal_store
                .terminal_owner_origin_binding(proposal_id)?,
            proposal,
            authority_proof: ReviewAcceptanceAuthorityProof::ClaimedByReviewWorkflow,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Re-establish runtime authority after restart from canonical
    /// ProposalStore evidence. The returned proof cannot be serialized or
    /// constructed from a TaskStore row.
    pub fn materialized_acceptance_snapshot(
        &self,
        proposal_id: &str,
    ) -> Result<MaterializedReviewAcceptanceSnapshot> {
        let (proposal, dispatch_claim_id, proposal_snapshot_digest, dispatch_state) = self
            .proposal_store
            .materialized_dispatch_proposal(proposal_id)?
            .ok_or_else(|| anyhow!("materialized review acceptance is not canonical"))?;
        match dispatch_state.as_str() {
            "confirmed_projection_pending" => {
                if !matches!(
                    proposal.status,
                    ProposalStatus::Pending | ProposalStatus::Postponed | ProposalStatus::Edited
                ) || review_proposal_snapshot_digest(&proposal)? != proposal_snapshot_digest
                {
                    anyhow::bail!("projection-pending review snapshot changed after dispatch");
                }
            }
            "confirmed" => {
                if proposal.status != ProposalStatus::Accepted || proposal.resolved_at.is_none() {
                    anyhow::bail!("confirmed review effect lacks its accepted projection");
                }
            }
            _ => anyhow::bail!("review effect is not durably materialized"),
        }
        let snapshot = MaterializedReviewAcceptanceSnapshot {
            proposal,
            proposal_snapshot_digest,
            dispatch_claim_digest: format!("sha256:{}", sha256_hex(dispatch_claim_id.as_bytes())),
            authority_proof:
                MaterializedReviewAcceptanceAuthorityProof::ReloadedFromCanonicalReviewWorkflow,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Canonical proposal admission plus ReviewWorkflow submission as one
    /// authority. Product callers must use this entrypoint when cancellation
    /// can race a durable Proposal create/reuse/update.
    pub fn submit_with_admission(
        &self,
        request: DurableWriteRequest,
        admission: &dyn CanonicalWriteAdmission,
    ) -> Result<ReviewWorkflowOutcome> {
        let canonical_proposal_id = request
            .existing_proposal_id
            .as_deref()
            .unwrap_or(&request.proposal.id);
        let permit = admission
            .acquire(CanonicalWriteAdmissionRequest::new(
                "proposal",
                format!("proposal:{canonical_proposal_id}"),
            ))
            .map_err(anyhow::Error::from)?;
        match self.submit(request) {
            Ok(outcome) => {
                match outcome.decision.kind {
                    DurableWriteDecisionKind::CreatePendingProposal
                    | DurableWriteDecisionKind::UpdatePendingProposal => {
                        permit.finish_committed();
                    }
                    DurableWriteDecisionKind::ReusePendingProposal => {
                        permit.finish_noop();
                    }
                }
                Ok(outcome)
            }
            Err(error) => {
                permit.finish_failed();
                Err(error)
            }
        }
    }
}

pub fn proposal_status_semantics(status: ProposalStatus) -> &'static str {
    match status {
        ProposalStatus::Pending => "pending_review_no_durable_write_applied",
        ProposalStatus::Accepted => "accepted_and_eligible_for_durable_write_application",
        ProposalStatus::Rejected => "rejected_no_durable_write_applied",
        ProposalStatus::Edited => "edited_pending_or_applied_only_after_explicit_acceptance",
        ProposalStatus::Postponed => "postponed_no_durable_write_applied",
        ProposalStatus::Expired => "expired_no_durable_write_applied",
    }
}

fn outcome(
    request: DurableWriteRequest,
    proposal: AgentProposal,
    kind: DurableWriteDecisionKind,
    reason: &str,
) -> ReviewWorkflowOutcome {
    ReviewWorkflowOutcome {
        decision: DurableWriteDecision {
            kind,
            proposal_id: proposal.id.clone(),
            proposal_status: proposal.status,
            durable_write_completed: false,
            idempotency_key: request.idempotency_key,
            requires_approval: request.requires_approval,
            reason: reason.into(),
        },
        user_visible_status: proposal_status_semantics(proposal.status).into(),
        user_visible_summary: request.user_visible_summary,
        final_delivery_message: request
            .final_delivery_wording_contract
            .pending_message()
            .into(),
        evidence_refs: request.evidence_refs,
        proposal,
    }
}

fn validate_pending_wording(request: &DurableWriteRequest) -> Result<()> {
    for value in [
        request.user_visible_summary.as_str(),
        request.final_delivery_wording_contract.pending_message(),
    ] {
        if contains_forbidden_pending_completion_claim(value) {
            return Err(anyhow!(
                "pending proposal wording must not claim durable completion"
            ));
        }
    }
    Ok(())
}

fn contains_forbidden_pending_completion_claim(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("remembered")
        || lower.contains("updated")
        || lower.contains("completed")
        || value.contains("已记住")
        || value.contains("已更新")
        || value.contains("已完成")
}

fn default_idempotency_key(
    source: DurableWriteSource,
    subject: DurableWriteSubject,
    proposal: &AgentProposal,
) -> String {
    let payload = json!({
        "schema": "openlife.reviewWorkflow.idempotency.v1",
        "source": source,
        "subject": subject,
        "proposalType": proposal.proposal_type,
        "proposalSource": proposal.source,
        "sourceDetail": proposal.source_detail,
        "affectedPath": proposal.affected_path,
        "before": proposal.before,
        "after": proposal.after,
    });
    let serialized = serde_json::to_vec(&payload).unwrap_or_default();
    format!("review_workflow:{}", sha256_hex(&serialized))
}

pub(crate) fn review_proposal_snapshot_digest(proposal: &AgentProposal) -> Result<String> {
    let payload = json!({
        "schema": "openlife.reviewWorkflow.acceptanceSnapshot.v1",
        "proposal": proposal,
    });
    let serialized = serde_json::to_vec(&payload)
        .map_err(|error| anyhow!("review acceptance snapshot is not canonical JSON: {error}"))?;
    Ok(format!("sha256:{}", sha256_hex(&serialized)))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let hash = digest(&SHA256, bytes);
    hash.as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{ProposalSource, ProposalType};

    struct RelationTestPermit;

    impl crate::agent::CanonicalWritePermit for RelationTestPermit {
        fn finish_committed(self: Box<Self>) {}
        fn finish_failed(self: Box<Self>) {}
        fn finish_noop(self: Box<Self>) {}
    }

    struct RelationTestAdmission {
        reject: bool,
    }

    impl CanonicalWriteAdmission for RelationTestAdmission {
        fn acquire(
            &self,
            _request: CanonicalWriteAdmissionRequest,
        ) -> std::result::Result<
            Box<dyn crate::agent::CanonicalWritePermit>,
            crate::agent::CanonicalWriteAdmissionRejection,
        > {
            if self.reject {
                Err(crate::agent::CanonicalWriteAdmissionRejection::new(
                    "test_cancelled",
                ))
            } else {
                Ok(Box::new(RelationTestPermit))
            }
        }
    }

    #[derive(Clone)]
    struct TrackingRelationAdmission {
        reject: bool,
        acquire_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        terminal_outcomes: std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
    }

    impl TrackingRelationAdmission {
        fn new(reject: bool) -> Self {
            Self {
                reject,
                acquire_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                terminal_outcomes: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn acquire_count(&self) -> usize {
            self.acquire_count.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn outcomes(&self) -> Vec<&'static str> {
            self.terminal_outcomes.lock().unwrap().clone()
        }
    }

    struct TrackingRelationPermit {
        terminal_outcomes: std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
    }

    impl crate::agent::CanonicalWritePermit for TrackingRelationPermit {
        fn finish_committed(self: Box<Self>) {
            self.terminal_outcomes.lock().unwrap().push("committed");
        }

        fn finish_failed(self: Box<Self>) {
            self.terminal_outcomes.lock().unwrap().push("failed");
        }

        fn finish_noop(self: Box<Self>) {
            self.terminal_outcomes.lock().unwrap().push("noop");
        }
    }

    impl CanonicalWriteAdmission for TrackingRelationAdmission {
        fn acquire(
            &self,
            _request: CanonicalWriteAdmissionRequest,
        ) -> std::result::Result<
            Box<dyn crate::agent::CanonicalWritePermit>,
            crate::agent::CanonicalWriteAdmissionRejection,
        > {
            self.acquire_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.reject {
                Err(crate::agent::CanonicalWriteAdmissionRejection::new(
                    "tracking_rejected",
                ))
            } else {
                Ok(Box::new(TrackingRelationPermit {
                    terminal_outcomes: std::sync::Arc::clone(&self.terminal_outcomes),
                }))
            }
        }
    }

    fn relation_origin(label: &str) -> TerminalOwnerReviewOriginProof {
        TerminalOwnerReviewOriginProof {
            task_session_id: format!("task:{label}"),
            run_id: format!("run:{label}"),
            epoch_id: format!("epoch:{label}"),
            epoch_generation: 1,
            admission_id: format!("admission:{label}"),
            canonical_user_message_ref: format!("message:{label}"),
            canonical_user_message_digest: format!("sha256:{:0>64}", label.len()),
            canonical_store_identity: format!("canonical-store:{label}"),
            authority: TerminalOwnerReviewOriginAuthority::BoundToVerifiedEpochAdmission,
        }
    }

    fn relation_link(
        origin: &TerminalOwnerReviewOriginProof,
    ) -> AgentRunTerminalRelationTargetIntentAdmission {
        crate::agent::store::agent_run_terminal_relation_target_fixture(origin)
    }

    fn relation_request(path: &str, key: &str) -> DurableWriteRequest {
        DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::MainChat,
            DurableWriteSubject::Memory,
            proposal(path),
            "Memory proposal is ready for Review Center approval.",
        )
        .with_idempotency_key(key)
    }

    fn submit_relation(
        store: &ProposalStore,
        request: DurableWriteRequest,
        origin: TerminalOwnerReviewOriginProof,
        relation_kind: ProposalTerminalRelationKind,
    ) -> Result<ProposalTerminalRelationSubmitOutcome> {
        submit_relation_with_admission(
            store,
            request,
            origin,
            relation_kind,
            &RelationTestAdmission { reject: false },
        )
    }

    fn submit_relation_with_admission(
        store: &ProposalStore,
        request: DurableWriteRequest,
        origin: TerminalOwnerReviewOriginProof,
        relation_kind: ProposalTerminalRelationKind,
        admission: &dyn CanonicalWriteAdmission,
    ) -> Result<ProposalTerminalRelationSubmitOutcome> {
        let link = relation_link(&origin);
        ReviewWorkflow::new(store).submit_with_terminal_owner_relation(
            request,
            &origin,
            relation_kind,
            admission,
            &link,
        )
    }

    fn proposal(path: &str) -> AgentProposal {
        AgentProposal::new(
            ProposalType::MemoryWrite,
            path,
            serde_json::json!({"content": "review me"}),
            "User requested a reviewable memory proposal.",
            0.8,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        )
    }

    #[test]
    fn proposal_relation_nonblocking_memory_is_owned_without_blocking_semantics() {
        let store = ProposalStore::new_in_memory().unwrap();
        let outcome = submit_relation(
            &store,
            relation_request("memory.pending.nonblocking", "relation:nonblocking"),
            relation_origin("nonblocking"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect("non-blocking relation submit must be implemented");

        assert!(matches!(
            outcome,
            ProposalTerminalRelationSubmitOutcome::CreatedOwned { .. }
        ));
        assert_eq!(
            outcome.owned_relation().unwrap().relation_kind(),
            ProposalTerminalRelationKind::NonBlockingSuccessor
        );
        assert_eq!(outcome.review().proposal.status, ProposalStatus::Pending);
    }

    #[test]
    fn proposal_relation_effect_blocking_is_explicit_not_proposal_type_inference() {
        let store = ProposalStore::new_in_memory().unwrap();
        let outcome = submit_relation(
            &store,
            relation_request("memory.pending.effect", "relation:effect"),
            relation_origin("effect"),
            ProposalTerminalRelationKind::EffectBlockingPrerequisite,
        )
        .expect("effect-blocking relation submit must be implemented");

        assert_eq!(
            outcome.owned_relation().unwrap().relation_kind(),
            ProposalTerminalRelationKind::EffectBlockingPrerequisite
        );
        assert_eq!(
            outcome.review().proposal.proposal_type,
            ProposalType::MemoryWrite
        );
    }

    #[test]
    fn proposal_relation_action_resume_never_claims_effect_completion() {
        let store = ProposalStore::new_in_memory().unwrap();
        let outcome = submit_relation(
            &store,
            relation_request("permission.pending.resume", "relation:resume"),
            relation_origin("resume"),
            ProposalTerminalRelationKind::ActionResumePrerequisite,
        )
        .expect("action-resume relation submit must be implemented");

        assert_eq!(
            outcome.owned_relation().unwrap().relation_kind(),
            ProposalTerminalRelationKind::ActionResumePrerequisite
        );
        assert!(!outcome.review().durable_write_completed());
    }

    #[test]
    fn proposal_relation_same_origin_exact_submit_replays_same_relation_and_outbox() {
        let store = ProposalStore::new_in_memory().unwrap();
        let origin = relation_origin("replay");
        let first = submit_relation(
            &store,
            relation_request("memory.pending.replay", "relation:replay"),
            relation_origin("replay"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect("first exact relation submit must be implemented");
        let replay = submit_relation(
            &store,
            relation_request("memory.pending.replay", "relation:replay"),
            origin,
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect("exact relation replay must be implemented");

        assert!(matches!(
            replay,
            ProposalTerminalRelationSubmitOutcome::ReplayedSameOrigin { .. }
        ));
        assert_eq!(first.review().proposal_id(), replay.review().proposal_id());
        assert_eq!(
            first.owned_relation().unwrap().link_outbox_event_id(),
            replay.owned_relation().unwrap().link_outbox_event_id()
        );
    }

    #[test]
    fn proposal_relation_store_issues_id_and_replay_ignores_caller_uuid() {
        let store = ProposalStore::new_in_memory().unwrap();
        let mut first_request = relation_request("memory.pending.store-id", "relation:store-id");
        let first_caller_id = uuid::Uuid::new_v4().to_string();
        first_request.proposal.id = first_caller_id.clone();
        let first = submit_relation(
            &store,
            first_request,
            relation_origin("store-id"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .unwrap();
        let canonical_id = first.review().proposal_id().to_string();
        assert_ne!(canonical_id, first_caller_id);
        assert_eq!(
            uuid::Uuid::parse_str(&canonical_id).unwrap().to_string(),
            canonical_id
        );
        assert_eq!(first.owned_relation().unwrap().proposal_id(), canonical_id);

        let mut replay_request = relation_request("memory.pending.store-id", "relation:store-id");
        let replay_caller_id = uuid::Uuid::new_v4().to_string();
        replay_request.proposal.id = replay_caller_id.clone();
        let replay = submit_relation(
            &store,
            replay_request,
            relation_origin("store-id"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .unwrap();

        assert_ne!(canonical_id, replay_caller_id);
        assert_eq!(replay.review().proposal_id(), canonical_id);
        assert_eq!(replay.owned_relation().unwrap().proposal_id(), canonical_id);
    }

    #[test]
    fn proposal_relation_raw_store_writer_has_one_production_caller() {
        let symbol = [
            "create_or_reuse_active_review_proposal_with_",
            "terminal_relation",
        ]
        .concat();
        let store_source = include_str!("proposal_store.rs");
        let workflow_source = include_str!("review_workflow.rs");
        assert_eq!(store_source.matches(&symbol).count(), 1);
        assert_eq!(workflow_source.matches(&symbol).count(), 1);
    }

    #[test]
    fn proposal_relation_foreign_reuse_never_rebinds_or_blocks_current_origin() {
        let store = ProposalStore::new_in_memory().unwrap();
        let first = submit_relation(
            &store,
            relation_request("memory.pending.foreign", "relation:foreign"),
            relation_origin("foreign-owner"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect("foreign owner seed must be implemented");
        let foreign = submit_relation(
            &store,
            relation_request("memory.pending.foreign", "relation:foreign"),
            relation_origin("foreign-caller"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect("foreign idempotency reuse must return a typed outcome");

        assert!(matches!(
            foreign,
            ProposalTerminalRelationSubmitOutcome::ReusedForeignNonBlocking { .. }
        ));
        assert_eq!(first.review().proposal_id(), foreign.review().proposal_id());
        assert!(foreign.owned_relation().is_none());
    }

    #[test]
    fn repeated_foreign_effect_blocking_memory_reuses_original_owner_without_rebinding() {
        let store = ProposalStore::new_in_memory().unwrap();
        let first = submit_relation(
            &store,
            relation_request(
                "memory.pending.foreign-effect-blocking",
                "relation:foreign-effect-blocking",
            ),
            relation_origin("foreign-effect-blocking-owner"),
            ProposalTerminalRelationKind::EffectBlockingPrerequisite,
        )
        .expect("first sensitive Memory review owns the blocking relation");
        let repeated = submit_relation(
            &store,
            relation_request(
                "memory.pending.foreign-effect-blocking",
                "relation:foreign-effect-blocking",
            ),
            relation_origin("foreign-effect-blocking-repeat"),
            ProposalTerminalRelationKind::EffectBlockingPrerequisite,
        )
        .expect("the repeated fact reuses the original pending review");

        assert!(matches!(
            repeated,
            ProposalTerminalRelationSubmitOutcome::ReusedForeignNonBlocking { .. }
        ));
        assert_eq!(
            first.review().proposal_id(),
            repeated.review().proposal_id()
        );
        assert!(repeated.owned_relation().is_none());
        let retained = store
            .terminal_owner_relation(first.review().proposal_id())
            .unwrap()
            .expect("original blocking relation remains canonical");
        assert_eq!(
            retained.relation_kind(),
            ProposalTerminalRelationKind::EffectBlockingPrerequisite
        );
    }

    #[test]
    fn proposal_relation_foreign_blocking_collision_fails_closed_without_downgrade() {
        for (index, relation_kind) in [
            ProposalTerminalRelationKind::EffectBlockingPrerequisite,
            ProposalTerminalRelationKind::ActionResumePrerequisite,
        ]
        .into_iter()
        .enumerate()
        {
            let store = ProposalStore::new_in_memory().unwrap();
            let key = format!("relation:foreign-blocking:{index}");
            let path = format!("memory.pending.foreign-blocking-{index}");
            let owner = submit_relation(
                &store,
                relation_request(&path, &key),
                relation_origin(&format!("foreign-blocking-owner-{index}")),
                ProposalTerminalRelationKind::NonBlockingSuccessor,
            )
            .unwrap();
            let error = submit_relation(
                &store,
                relation_request(&path, &key),
                relation_origin(&format!("foreign-blocking-caller-{index}")),
                relation_kind,
            )
            .expect_err("a foreign blocking relation must never be silently downgraded");
            assert_eq!(
                error.to_string(),
                "proposal_terminal_relation_foreign_blocking_collision"
            );
            assert_eq!(store.pending_count().unwrap(), 1);
            let relation = store
                .terminal_owner_relation(owner.review().proposal_id())
                .unwrap()
                .unwrap();
            assert_eq!(
                relation.relation_kind(),
                ProposalTerminalRelationKind::NonBlockingSuccessor
            );
        }
    }

    #[test]
    fn proposal_relation_kind_drift_same_origin_is_conflict_no_write() {
        let store = ProposalStore::new_in_memory().unwrap();
        let origin = relation_origin("drift");
        submit_relation(
            &store,
            relation_request("memory.pending.drift", "relation:drift"),
            relation_origin("drift"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect("relation seed must be implemented");

        let error = submit_relation(
            &store,
            relation_request("memory.pending.drift", "relation:drift"),
            origin,
            ProposalTerminalRelationKind::EffectBlockingPrerequisite,
        )
        .expect_err("relation-kind drift must fail closed");
        assert_eq!(error.to_string(), "proposal_terminal_relation_kind_drift");
    }

    #[test]
    fn proposal_relation_legacy_unclassified_fails_closed_without_type_guess() {
        let store = ProposalStore::new_in_memory().unwrap();
        let error = submit_relation(
            &store,
            relation_request("memory.pending.legacy", "relation:legacy"),
            relation_origin("legacy"),
            ProposalTerminalRelationKind::LegacyUnclassified,
        )
        .expect_err("new submissions must reject LegacyUnclassified");

        assert_eq!(
            error.to_string(),
            "legacy_unclassified_relation_requires_migration"
        );
        assert_eq!(store.pending_count().unwrap(), 0);
    }

    #[test]
    fn proposal_relation_outbox_record_is_metadata_only() {
        let store = ProposalStore::new_in_memory().unwrap();
        let outcome = submit_relation(
            &store,
            relation_request("memory.pending.metadata", "relation:metadata"),
            relation_origin("metadata"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect("metadata-only relation submit must be implemented");

        let record = outcome.owned_relation().unwrap();
        assert!(record.relation_digest().starts_with("sha256:"));
        assert!(record
            .link_outbox_event_id()
            .is_some_and(|event_id| event_id.starts_with("outbox:")));
        assert!(!record.relation_digest().contains("review me"));
        assert!(!record
            .link_outbox_event_id()
            .is_some_and(|event_id| event_id.contains("review me")));
        assert!(record.created_at() <= chrono::Utc::now());
    }

    #[test]
    fn proposal_relation_cancel_wins_before_atomic_submit_writes_nothing() {
        let store = ProposalStore::new_in_memory().unwrap();
        let origin = relation_origin("cancel");
        let link = relation_link(&origin);
        let error = ReviewWorkflow::new(&store)
            .submit_with_terminal_owner_relation(
                relation_request("memory.pending.cancel", "relation:cancel"),
                &origin,
                ProposalTerminalRelationKind::EffectBlockingPrerequisite,
                &RelationTestAdmission { reject: true },
                &link,
            )
            .expect_err("cancelled admission must fail before the transaction");

        assert_eq!(
            error.to_string(),
            "canonical_write_admission_rejected:test_cancelled"
        );
        assert_eq!(store.pending_count().unwrap(), 0);
    }

    #[test]
    fn proposal_relation_write_permit_records_exactly_one_terminal_outcome() {
        let store = ProposalStore::new_in_memory().unwrap();
        let key = "relation:permit-created";
        let path = "memory.pending.permit-created";

        let created_admission = TrackingRelationAdmission::new(false);
        let created = submit_relation_with_admission(
            &store,
            relation_request(path, key),
            relation_origin("permit-owner"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
            &created_admission,
        )
        .unwrap();
        assert!(matches!(
            created,
            ProposalTerminalRelationSubmitOutcome::CreatedOwned { .. }
        ));
        assert_eq!(created_admission.acquire_count(), 1);
        assert_eq!(created_admission.outcomes(), vec!["committed"]);

        let replay_admission = TrackingRelationAdmission::new(false);
        let replay = submit_relation_with_admission(
            &store,
            relation_request(path, key),
            relation_origin("permit-owner"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
            &replay_admission,
        )
        .unwrap();
        assert!(matches!(
            replay,
            ProposalTerminalRelationSubmitOutcome::ReplayedSameOrigin { .. }
        ));
        assert_eq!(replay_admission.acquire_count(), 1);
        assert_eq!(replay_admission.outcomes(), vec!["noop"]);

        let foreign_admission = TrackingRelationAdmission::new(false);
        let foreign = submit_relation_with_admission(
            &store,
            relation_request(path, key),
            relation_origin("permit-foreign"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
            &foreign_admission,
        )
        .unwrap();
        assert!(matches!(
            foreign,
            ProposalTerminalRelationSubmitOutcome::ReusedForeignNonBlocking { .. }
        ));
        assert_eq!(foreign_admission.acquire_count(), 1);
        assert_eq!(foreign_admission.outcomes(), vec!["noop"]);
        assert_eq!(store.pending_count().unwrap(), 1);

        let failed_store = ProposalStore::new_in_memory().unwrap();
        let failed_key = "relation:permit-failed";
        failed_store
            .fail_next_terminal_relation_commit_for_test(failed_key)
            .unwrap();
        let failed_admission = TrackingRelationAdmission::new(false);
        let error = submit_relation_with_admission(
            &failed_store,
            relation_request("memory.pending.permit-failed", failed_key),
            relation_origin("permit-failed"),
            ProposalTerminalRelationKind::EffectBlockingPrerequisite,
            &failed_admission,
        )
        .expect_err("a store failure must terminalize the acquired permit as failed");
        assert_eq!(
            error.to_string(),
            "proposal_terminal_relation_commit_failpoint"
        );
        assert_eq!(failed_admission.acquire_count(), 1);
        assert_eq!(failed_admission.outcomes(), vec!["failed"]);
        assert_eq!(failed_store.pending_count().unwrap(), 0);

        let rejected_store = ProposalStore::new_in_memory().unwrap();
        let rejected_admission = TrackingRelationAdmission::new(true);
        let error = submit_relation_with_admission(
            &rejected_store,
            relation_request("memory.pending.permit-rejected", "relation:permit-rejected"),
            relation_origin("permit-rejected"),
            ProposalTerminalRelationKind::ActionResumePrerequisite,
            &rejected_admission,
        )
        .expect_err("admission rejection must not create a write permit or rows");
        assert_eq!(
            error.to_string(),
            "canonical_write_admission_rejected:tracking_rejected"
        );
        assert_eq!(rejected_admission.acquire_count(), 1);
        assert!(rejected_admission.outcomes().is_empty());
        assert_eq!(rejected_store.pending_count().unwrap(), 0);
    }

    #[test]
    fn proposal_relation_requires_an_actual_review_boundary() {
        let store = ProposalStore::new_in_memory().unwrap();
        let error = submit_relation(
            &store,
            relation_request("memory.pending.no-review", "relation:no-review")
                .with_requires_approval(false),
            relation_origin("no-review"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect_err("terminal relation saga must not admit a non-review write");

        assert_eq!(
            error.to_string(),
            "terminal_relation_submission_requires_approval"
        );
        assert_eq!(store.pending_count().unwrap(), 0);
    }

    #[test]
    fn proposal_relation_same_fact_key_allows_non_authoritative_candidate_metadata_refresh() {
        let store = ProposalStore::new_in_memory().unwrap();
        let origin = relation_origin("fact-refresh");
        let mut first = relation_request("memory.pending.fact", "memory_fact:stable");
        first.proposal.after["candidateId"] = json!("candidate:first");
        first.proposal.after["content"] = json!("same semantic fact");
        let created = submit_relation(
            &store,
            first,
            relation_origin("fact-refresh"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .unwrap();

        let mut refreshed = relation_request("memory.pending.fact", "memory_fact:stable");
        refreshed.proposal.after["candidateId"] = json!("candidate:second");
        refreshed.proposal.after["content"] = json!("same semantic fact");
        let replay = submit_relation(
            &store,
            refreshed,
            origin,
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .unwrap();

        assert!(matches!(
            replay,
            ProposalTerminalRelationSubmitOutcome::ReplayedSameOrigin { .. }
        ));
        assert_eq!(
            created.review().proposal_id(),
            replay.review().proposal_id()
        );
    }

    #[test]
    fn proposal_relation_policy_identity_drift_conflicts_for_same_origin() {
        let store = ProposalStore::new_in_memory().unwrap();
        let origin = relation_origin("identity-drift");
        submit_relation(
            &store,
            relation_request("memory.pending.identity", "relation:identity-drift"),
            relation_origin("identity-drift"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .unwrap();
        let error = submit_relation(
            &store,
            relation_request("memory.pending.different", "relation:identity-drift"),
            origin,
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect_err("policy-bound affected path drift must fail closed");

        assert_eq!(
            error.to_string(),
            "proposal_terminal_relation_identity_drift"
        );
        assert_eq!(store.pending_count().unwrap(), 1);
    }

    #[test]
    fn proposal_relation_foreign_identity_drift_is_not_silently_deduplicated() {
        let store = ProposalStore::new_in_memory().unwrap();
        submit_relation(
            &store,
            relation_request("memory.pending.foreign-a", "relation:foreign-drift"),
            relation_origin("foreign-drift-owner"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .unwrap();
        let error = submit_relation(
            &store,
            relation_request("memory.pending.foreign-b", "relation:foreign-drift"),
            relation_origin("foreign-drift-caller"),
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .expect_err("foreign policy identity drift must not gain dedup credit");

        assert_eq!(
            error.to_string(),
            "proposal_terminal_relation_identity_drift"
        );
        assert_eq!(store.pending_count().unwrap(), 1);
    }

    #[test]
    fn proposal_relation_placeholder_is_absent_after_implementation() {
        let retired_marker = ["proposal_terminal_relation_contract", "_not_implemented"].concat();
        let review_source = include_str!("review_workflow.rs");
        let store_source = include_str!("proposal_store.rs");
        assert!(!review_source.contains(&retired_marker));
        assert!(!store_source.contains(&retired_marker));
    }

    #[test]
    fn review_workflow_creates_pending_proposal_without_durable_completion_claim() {
        let store = ProposalStore::new_in_memory().unwrap();
        let outcome = ReviewWorkflow::new(&store)
            .submit(DurableWriteRequest::from_agent_proposal(
                DurableWriteSource::MainChat,
                DurableWriteSubject::Memory,
                proposal("memory.pending.chat"),
                "Memory proposal is ready for Review Center approval.",
            ))
            .unwrap();

        assert_eq!(
            outcome.decision.kind,
            DurableWriteDecisionKind::CreatePendingProposal
        );
        assert_eq!(outcome.proposal.status, ProposalStatus::Pending);
        assert!(!outcome.durable_write_completed());
        assert!(outcome
            .final_delivery_message
            .contains("pending Review Center approval"));
        assert_eq!(store.pending_count().unwrap(), 1);
    }

    #[test]
    fn review_workflow_reuses_pending_proposal_by_idempotency_key() {
        let store = ProposalStore::new_in_memory().unwrap();
        let first = DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::MainChat,
            DurableWriteSubject::Memory,
            proposal("memory.pending.chat"),
            "Memory proposal is ready for Review Center approval.",
        );
        let second = first.clone();

        let created = ReviewWorkflow::new(&store).submit(first).unwrap();
        let reused = ReviewWorkflow::new(&store).submit(second).unwrap();

        assert_eq!(created.proposal_id(), reused.proposal_id());
        assert_eq!(
            reused.decision.kind,
            DurableWriteDecisionKind::ReusePendingProposal
        );
        assert_eq!(store.pending_count().unwrap(), 1);
    }

    #[test]
    fn review_workflow_persists_custom_idempotency_across_restart() {
        let path = std::env::temp_dir().join(format!(
            "openlife-review-workflow-idempotency-{}.db",
            uuid::Uuid::new_v4()
        ));
        let key = "memory_fact_v1:sha256:stable-fact-key";
        let created_id = {
            let store = ProposalStore::new(&path).unwrap();
            let mut first = proposal("memory.pending.chat");
            first.source_detail = Some("main_chat_session:first".into());
            first.after["candidateId"] = json!("candidate:first");
            ReviewWorkflow::new(&store)
                .submit(
                    DurableWriteRequest::from_agent_proposal(
                        DurableWriteSource::MainChat,
                        DurableWriteSubject::Memory,
                        first,
                        "Memory proposal is ready for Review Center approval.",
                    )
                    .with_idempotency_key(key),
                )
                .unwrap()
                .proposal
                .id
        };

        let store = ProposalStore::new(&path).unwrap();
        let mut second = proposal("memory.pending.chat");
        second.source_detail = Some("main_chat_session:second".into());
        second.after["candidateId"] = json!("candidate:second");
        let reused = ReviewWorkflow::new(&store)
            .submit(
                DurableWriteRequest::from_agent_proposal(
                    DurableWriteSource::MainChat,
                    DurableWriteSubject::Memory,
                    second,
                    "Memory proposal is ready for Review Center approval.",
                )
                .with_idempotency_key(key),
            )
            .unwrap();

        assert_eq!(reused.proposal.id, created_id);
        assert_eq!(
            reused.decision.kind,
            DurableWriteDecisionKind::ReusePendingProposal
        );
        assert_eq!(store.pending_count().unwrap(), 1);
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn review_workflow_concurrent_custom_key_has_one_canonical_owner() {
        let store = ProposalStore::new_in_memory().unwrap();
        let workers = 12;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(workers));
        let mut handles = Vec::new();
        for index in 0..workers {
            let store = store.clone();
            let barrier = barrier.clone();
            handles.push(std::thread::spawn(move || {
                let mut candidate = proposal("memory.pending.concurrent");
                candidate.after["candidateId"] = json!(format!("candidate:{index}"));
                barrier.wait();
                ReviewWorkflow::new(&store)
                    .submit(
                        DurableWriteRequest::from_agent_proposal(
                            DurableWriteSource::MainChat,
                            DurableWriteSubject::Memory,
                            candidate,
                            "Memory proposal is ready for Review Center approval.",
                        )
                        .with_idempotency_key("memory_fact_v1:sha256:concurrent-owner"),
                    )
                    .unwrap()
                    .proposal
                    .id
            }));
        }

        let ids = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 1);
        assert_eq!(store.pending_count().unwrap(), 1);
    }

    #[test]
    fn review_workflow_rejects_pending_proposal_completion_wording() {
        let store = ProposalStore::new_in_memory().unwrap();
        let err = ReviewWorkflow::new(&store)
            .submit(DurableWriteRequest::from_agent_proposal(
                DurableWriteSource::MainChat,
                DurableWriteSubject::Memory,
                proposal("memory.pending.chat"),
                "已完成记忆更新。",
            ))
            .unwrap_err();

        assert!(err
            .to_string()
            .contains("pending proposal wording must not claim durable completion"));
        assert_eq!(store.pending_count().unwrap(), 0);
    }

    #[test]
    fn acceptance_snapshot_requires_the_exact_canonical_dispatch_claim() {
        let store = ProposalStore::new_in_memory().unwrap();
        let proposal = proposal("memory.acceptance.snapshot");
        let proposal_id = proposal.id.clone();
        store.create_proposal(&proposal).unwrap();
        assert!(ReviewWorkflow::new(&store)
            .claimed_acceptance_snapshot(&proposal_id, "forged-claim")
            .is_err());

        let claim_id = store.claim_dispatch(&proposal_id).unwrap().unwrap();
        let snapshot = ReviewWorkflow::new(&store)
            .claimed_acceptance_snapshot(&proposal_id, &claim_id)
            .unwrap();

        snapshot.validate().unwrap();
        assert_eq!(snapshot.proposal().id, proposal_id);
        assert!(snapshot.proposal_snapshot_digest().starts_with("sha256:"));
        assert!(snapshot.dispatch_claim_digest().starts_with("sha256:"));
        assert!(!snapshot.dispatch_claim_digest().contains(&claim_id));
    }
}
