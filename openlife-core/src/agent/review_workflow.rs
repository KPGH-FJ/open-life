use crate::agent::proposal_store::ProposalStore;
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
    pub(crate) fn validate(&self) -> Result<()> {
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

    pub(crate) fn proposal(&self) -> &AgentProposal {
        &self.proposal
    }

    pub(crate) fn proposal_snapshot_digest(&self) -> &str {
        &self.proposal_snapshot_digest
    }

    pub(crate) fn dispatch_claim_digest(&self) -> &str {
        &self.dispatch_claim_digest
    }
}

impl ClaimedReviewAcceptanceSnapshot {
    pub(crate) fn validate(&self) -> Result<()> {
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

    pub(crate) fn proposal(&self) -> &AgentProposal {
        &self.proposal
    }

    pub(crate) fn proposal_snapshot_digest(&self) -> &str {
        &self.proposal_snapshot_digest
    }

    pub(crate) fn dispatch_claim_digest(&self) -> &str {
        &self.dispatch_claim_digest
    }
}

pub struct ReviewWorkflow<'a> {
    proposal_store: &'a ProposalStore,
}

impl<'a> ReviewWorkflow<'a> {
    pub fn new(proposal_store: &'a ProposalStore) -> Self {
        Self { proposal_store }
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
        let request = DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::MainChat,
            DurableWriteSubject::Memory,
            proposal,
            "Observation-derived Memory proposal is pending Review Center approval.",
        )
        .with_evidence_refs(evidence_refs);
        self.submit_with_admission(request, admission)
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
