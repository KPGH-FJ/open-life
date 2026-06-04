use crate::agent::evidence_store::{
    EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceRecord, EvidenceSourceRef,
    EvidenceSourceType, EvidenceStore, EvidenceType,
};
use crate::agent::maturation_domain::{
    classify_supported_maturation_domain, high_risk_maturation_path_or_source_detail,
    is_maturation_source_detail, maturation_source_event_type,
};
use crate::agent::types::{AgentProposal, RiskLevel};
use anyhow::Result;
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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

    fn evidence_kind(self) -> &'static str {
        match self {
            MaturationProposalOutcome::Accepted => "positive",
            MaturationProposalOutcome::Edited => "corrective",
            MaturationProposalOutcome::Rejected => "negative",
        }
    }

    fn is_positive(self) -> bool {
        matches!(self, MaturationProposalOutcome::Accepted)
    }

    fn is_corrective(self) -> bool {
        matches!(self, MaturationProposalOutcome::Edited)
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
    pub supported_low_risk_domain: bool,
    pub high_risk_domain: bool,
    pub candidate_domain: Option<String>,
    pub proposal_id: String,
    pub proposal_type: String,
    pub outcome: MaturationProposalOutcome,
    pub outcome_evidence_kind: String,
    pub source_run_id: Option<String>,
    pub source_evidence_ids: Vec<String>,
    pub linked_agent_run_ids: Vec<String>,
    pub outcome_evidence_id: Option<String>,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub positive: bool,
    pub corrective: bool,
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
    let candidate_domain = supported_low_risk_domain_for_proposal(proposal);
    let high_risk_domain = proposal_high_risk_for_outcome(proposal);
    let supported_low_risk_domain = candidate_domain.is_some() && !high_risk_domain;
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
    if high_risk_domain {
        blocking_reasons.push("high_risk_maturation_outcome_domain".to_string());
    }
    if !supported_low_risk_domain && !high_risk_domain {
        blocking_reasons.push("unsupported_maturation_outcome_domain".to_string());
    }

    MaturationProposalOutcomeEvidenceReport {
        recorded: false,
        maturation_lineage_present,
        source_detail_maturation,
        supported_low_risk_domain,
        high_risk_domain,
        candidate_domain,
        proposal_id: proposal.id.clone(),
        proposal_type: proposal.proposal_type.to_string(),
        outcome,
        outcome_evidence_kind: outcome.evidence_kind().to_string(),
        source_run_id: linked_agent_run_ids.first().cloned(),
        source_evidence_ids,
        linked_agent_run_ids,
        outcome_evidence_id: None,
        metadata_safe: true,
        contains_raw_content: false,
        positive: outcome.is_positive(),
        corrective: outcome.is_corrective(),
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
    let linked_records = evidence_store.query(EvidenceQuery {
        linked_proposal_id: Some(proposal.id.clone()),
        ..Default::default()
    })?;
    let source_evidence_records = linked_records
        .iter()
        .filter(|record| record.evidence_type != EvidenceType::ProposalOutcome)
        .cloned()
        .collect::<Vec<_>>();
    let mut report =
        evaluate_maturation_proposal_outcome_evidence(proposal, outcome, &source_evidence_records);

    if !report.maturation_lineage_present
        || report.high_risk_domain
        || !report.supported_low_risk_domain
    {
        return Ok(report);
    }

    if let Some(existing) = existing_outcome_evidence(&linked_records, proposal, outcome) {
        report.outcome_evidence_id = Some(existing.id.clone());
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
        "schema": "w132.maturationProposalOutcomeEvidence.v1",
        "previousSchema": "w75.maturationProposalOutcomeEvidence.v1",
        "w75": true,
        "w132": true,
        "outcome": outcome.as_str(),
        "outcomeEvidenceKind": outcome.evidence_kind(),
        "proposalId": proposal.id,
        "proposalType": proposal.proposal_type.to_string(),
        "proposalSource": proposal.source.to_string(),
        "proposalStatus": proposal.status.to_string(),
        "sourceDetailMaturation": report.source_detail_maturation,
        "supportedLowRiskDomain": report.supported_low_risk_domain,
        "candidateDomain": report.candidate_domain,
        "sourceEventTypeDigest": maturation_source_event_type_digest(proposal),
        "sourceRunId": report.source_run_id,
        "sourceEvidenceIds": report.source_evidence_ids,
        "linkedAgentRunIds": report.linked_agent_run_ids,
        "proposalDigest": proposal_digest,
        "accepted": outcome == MaturationProposalOutcome::Accepted,
        "rejected": outcome == MaturationProposalOutcome::Rejected,
        "edited": outcome == MaturationProposalOutcome::Edited,
        "positive": outcome.is_positive(),
        "corrective": outcome.is_corrective(),
        "negative": outcome.is_negative(),
        "opposing": outcome.is_negative(),
        "editedPayloadDigest": if outcome.is_corrective() { Some(value_digest(&proposal.after)) } else { None },
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
        .is_some_and(is_maturation_source_detail)
}

fn supported_low_risk_domain_for_proposal(proposal: &AgentProposal) -> Option<String> {
    classify_supported_maturation_domain(&proposal.affected_path, proposal.source_detail.as_deref())
        .map(|domain| domain.as_str().to_string())
}

fn proposal_high_risk_for_outcome(proposal: &AgentProposal) -> bool {
    matches!(proposal.risk_level, RiskLevel::High | RiskLevel::Critical)
        || high_risk_maturation_path_or_source_detail(
            &proposal.affected_path,
            proposal.source_detail.as_deref(),
        )
}

fn maturation_source_event_type_digest(proposal: &AgentProposal) -> Option<String> {
    proposal
        .source_detail
        .as_deref()
        .and_then(maturation_source_event_type)
        .map(|event_type| sha256_hex(event_type.as_bytes()))
}

fn existing_outcome_evidence<'a>(
    linked_records: &'a [EvidenceRecord],
    proposal: &AgentProposal,
    outcome: MaturationProposalOutcome,
) -> Option<&'a EvidenceRecord> {
    linked_records.iter().find(|record| {
        record.evidence_type == EvidenceType::ProposalOutcome
            && record
                .linked_proposal_ids
                .iter()
                .any(|proposal_id| proposal_id == &proposal.id)
            && record.run_metadata["proposalId"] == proposal.id
            && record.run_metadata["outcome"] == outcome.as_str()
    })
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
