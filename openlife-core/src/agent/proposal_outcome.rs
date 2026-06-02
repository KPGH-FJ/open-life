use crate::agent::evidence_store::{
    EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceRecord, EvidenceSourceRef,
    EvidenceSourceType, EvidenceStore, EvidenceType,
};
use crate::agent::types::{AgentProposal, RiskLevel};
use anyhow::Result;
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const MATURATION_SOURCE_DETAIL_PREFIX: &str = "maturation:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaturationProposalOutcome {
    Accepted,
    Rejected,
    Edited,
}

impl MaturationProposalOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            MaturationProposalOutcome::Accepted => "accepted",
            MaturationProposalOutcome::Rejected => "rejected",
            MaturationProposalOutcome::Edited => "edited",
        }
    }

    fn is_negative(self) -> bool {
        matches!(self, MaturationProposalOutcome::Rejected)
    }
}

impl std::fmt::Display for MaturationProposalOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturationProposalOutcomeEvidenceReport {
    pub recorded: bool,
    pub maturation_lineage_present: bool,
    pub source_detail_maturation: bool,
    pub proposal_id: String,
    pub proposal_type: String,
    pub outcome: MaturationProposalOutcome,
    pub source_run_id: Option<String>,
    pub source_evidence_ids: Vec<String>,
    pub linked_agent_run_ids: Vec<String>,
    pub outcome_evidence_id: Option<String>,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub negative: bool,
    pub opposing: bool,
    pub blocking_reasons: Vec<String>,
}

pub fn evaluate_maturation_proposal_outcome_evidence(
    proposal: &AgentProposal,
    outcome: MaturationProposalOutcome,
    source_evidence_records: &[EvidenceRecord],
) -> MaturationProposalOutcomeEvidenceReport {
    let source_detail_maturation = proposal_has_maturation_source_detail(proposal);
    let source_evidence_ids = source_evidence_records
        .iter()
        .map(|record| record.id.clone())
        .collect::<Vec<_>>();
    let mut linked_agent_run_ids = Vec::new();
    if let Some(run_id) = non_empty(proposal.run_id.as_deref()) {
        push_unique(&mut linked_agent_run_ids, run_id.to_string());
    }
    for record in source_evidence_records {
        for run_id in &record.linked_agent_run_ids {
            if !run_id.trim().is_empty() {
                push_unique(&mut linked_agent_run_ids, run_id.clone());
            }
        }
        for source_ref in &record.source_refs {
            if source_ref.source_type == EvidenceSourceType::AgentRun
                && !source_ref.source_id.trim().is_empty()
            {
                push_unique(&mut linked_agent_run_ids, source_ref.source_id.clone());
            }
        }
    }

    let maturation_lineage_present = source_detail_maturation || !source_evidence_ids.is_empty();
    let mut blocking_reasons = Vec::new();
    if !maturation_lineage_present {
        blocking_reasons.push("maturation_lineage_missing".to_string());
    }

    MaturationProposalOutcomeEvidenceReport {
        recorded: false,
        maturation_lineage_present,
        source_detail_maturation,
        proposal_id: proposal.id.clone(),
        proposal_type: proposal.proposal_type.to_string(),
        outcome,
        source_run_id: linked_agent_run_ids.first().cloned(),
        source_evidence_ids,
        linked_agent_run_ids,
        outcome_evidence_id: None,
        metadata_safe: true,
        contains_raw_content: false,
        negative: outcome.is_negative(),
        opposing: outcome.is_negative(),
        blocking_reasons,
    }
}

pub fn record_maturation_proposal_outcome_evidence(
    evidence_store: &EvidenceStore,
    proposal: &AgentProposal,
    outcome: MaturationProposalOutcome,
) -> Result<MaturationProposalOutcomeEvidenceReport> {
    let source_evidence_records = evidence_store.query(EvidenceQuery {
        linked_proposal_id: Some(proposal.id.clone()),
        ..Default::default()
    })?;
    let mut report =
        evaluate_maturation_proposal_outcome_evidence(proposal, outcome, &source_evidence_records);

    if !report.maturation_lineage_present {
        return Ok(report);
    }

    let evidence =
        evidence_store.create_evidence(outcome_evidence_draft(proposal, outcome, &report))?;
    report.recorded = true;
    report.outcome_evidence_id = Some(evidence.id);
    Ok(report)
}

fn outcome_evidence_draft(
    proposal: &AgentProposal,
    outcome: MaturationProposalOutcome,
    report: &MaturationProposalOutcomeEvidenceReport,
) -> EvidenceDraft {
    let proposal_digest = proposal_digest(proposal);
    let mut draft = EvidenceDraft::new(
        EvidenceType::ProposalOutcome,
        proposal.affected_path.clone(),
        1.0,
        proposal.risk_level,
        privacy_from_risk(proposal.risk_level),
    )
    .with_summary(format!("maturation proposal {} outcome", outcome.as_str()))
    .with_source_ref(EvidenceSourceRef::from_digest(
        EvidenceSourceType::Proposal,
        proposal.id.clone(),
        Some("maturation_proposal_outcome"),
        proposal_digest.clone(),
    ))
    .with_linked_proposal(proposal.id.clone());

    for run_id in &report.linked_agent_run_ids {
        draft = draft.with_linked_agent_run(run_id.clone());
    }
    if outcome.is_negative() {
        draft.opposing_refs = if report.source_evidence_ids.is_empty() {
            vec![format!("proposal:{}", proposal.id)]
        } else {
            report.source_evidence_ids.clone()
        };
    }

    draft.run_metadata = outcome_metadata(proposal, outcome, report, proposal_digest);
    draft
}

fn outcome_metadata(
    proposal: &AgentProposal,
    outcome: MaturationProposalOutcome,
    report: &MaturationProposalOutcomeEvidenceReport,
    proposal_digest: String,
) -> Value {
    json!({
        "schema": "w75.maturationProposalOutcomeEvidence.v1",
        "w75": true,
        "outcome": outcome.as_str(),
        "proposalId": proposal.id,
        "proposalType": proposal.proposal_type.to_string(),
        "proposalSource": proposal.source.to_string(),
        "proposalStatus": proposal.status.to_string(),
        "sourceDetailMaturation": report.source_detail_maturation,
        "sourceEventTypeDigest": maturation_source_event_type_digest(proposal),
        "sourceRunId": report.source_run_id,
        "sourceEvidenceIds": report.source_evidence_ids,
        "linkedAgentRunIds": report.linked_agent_run_ids,
        "proposalDigest": proposal_digest,
        "accepted": outcome == MaturationProposalOutcome::Accepted,
        "rejected": outcome == MaturationProposalOutcome::Rejected,
        "edited": outcome == MaturationProposalOutcome::Edited,
        "negative": outcome.is_negative(),
        "opposing": outcome.is_negative(),
        "metadataSafe": true,
        "containsRawContent": false,
        "rawPromptIncluded": false,
        "assistantOutputIncluded": false,
        "memoryRawTextIncluded": false,
        "toolPayloadIncluded": false,
        "reviewerNoteIncluded": false,
        "editedPayloadIncluded": false
    })
}

fn proposal_has_maturation_source_detail(proposal: &AgentProposal) -> bool {
    proposal
        .source_detail
        .as_deref()
        .is_some_and(|detail| detail.starts_with(MATURATION_SOURCE_DETAIL_PREFIX))
}

fn maturation_source_event_type_digest(proposal: &AgentProposal) -> Option<String> {
    proposal
        .source_detail
        .as_deref()
        .and_then(|detail| detail.strip_prefix(MATURATION_SOURCE_DETAIL_PREFIX))
        .filter(|event_type| !event_type.trim().is_empty())
        .map(|event_type| sha256_hex(event_type.as_bytes()))
}

fn proposal_digest(proposal: &AgentProposal) -> String {
    let before_digest = proposal
        .before
        .as_ref()
        .map(value_digest)
        .unwrap_or_else(|| sha256_hex(b"null"));
    let after_digest = value_digest(&proposal.after);
    let reason_digest = sha256_hex(proposal.reason.as_bytes());
    sha256_hex(
        json!({
            "proposalId": proposal.id,
            "runId": proposal.run_id,
            "proposalType": proposal.proposal_type.to_string(),
            "source": proposal.source.to_string(),
            "sourceDetailMaturation": proposal_has_maturation_source_detail(proposal),
            "affectedPath": proposal.affected_path,
            "beforeDigest": before_digest,
            "afterDigest": after_digest,
            "reasonDigest": reason_digest,
            "confidence": proposal.confidence,
            "riskLevel": proposal.risk_level.to_string(),
        })
        .to_string()
        .as_bytes(),
    )
}

fn value_digest(value: &Value) -> String {
    sha256_hex(value.to_string().as_bytes())
}

fn privacy_from_risk(risk: RiskLevel) -> EvidencePrivacyLevel {
    match risk {
        RiskLevel::Low => EvidencePrivacyLevel::Internal,
        RiskLevel::Medium => EvidencePrivacyLevel::Sensitive,
        RiskLevel::High | RiskLevel::Critical => EvidencePrivacyLevel::StrictlyLocal,
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let hash = digest(&SHA256, bytes);
    let bytes = hash.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}
