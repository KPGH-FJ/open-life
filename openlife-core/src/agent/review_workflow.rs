use crate::agent::proposal_store::ProposalStore;
use crate::agent::types::{AgentProposal, ProposalStatus, ProposalType, RiskLevel};
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

pub struct ReviewWorkflow<'a> {
    proposal_store: &'a ProposalStore,
}

impl<'a> ReviewWorkflow<'a> {
    pub fn new(proposal_store: &'a ProposalStore) -> Self {
        Self { proposal_store }
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
                    self.proposal_store.update_proposal(&existing)?;
                    return Ok(outcome(
                        request,
                        existing,
                        DurableWriteDecisionKind::UpdatePendingProposal,
                        "linked_existing_pending_proposal",
                    ));
                }
            }
        }

        if let Some(existing) = self.find_existing_pending(&request)? {
            return Ok(outcome(
                request,
                existing,
                DurableWriteDecisionKind::ReusePendingProposal,
                "reused_existing_pending_proposal",
            ));
        }

        let proposal = request.proposal.clone();
        self.proposal_store.create_proposal(&proposal)?;
        Ok(outcome(
            request,
            proposal,
            DurableWriteDecisionKind::CreatePendingProposal,
            "created_pending_review_proposal",
        ))
    }

    fn find_existing_pending(
        &self,
        request: &DurableWriteRequest,
    ) -> Result<Option<AgentProposal>> {
        let existing = self.proposal_store.list_all_proposals(10_000, 0)?;
        Ok(existing.into_iter().find(|proposal| {
            proposal.status == ProposalStatus::Pending
                && default_idempotency_key(request.source, request.subject, proposal)
                    == request.idempotency_key
        }))
    }
}

pub fn proposal_status_semantics(status: ProposalStatus) -> &'static str {
    match status {
        ProposalStatus::Pending => "pending_review_no_durable_write_applied",
        ProposalStatus::Accepted => "accepted_and_eligible_for_durable_write_application",
        ProposalStatus::Rejected => "rejected_no_durable_write_applied",
        ProposalStatus::Edited => "edited_pending_or_applied_only_after_explicit_acceptance",
        ProposalStatus::Postponed => "postponed_no_durable_write_applied",
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
}
