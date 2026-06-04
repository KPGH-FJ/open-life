use crate::agent::{
    evaluate_maturation_proposal_outcome_evidence, record_maturation_proposal_outcome_evidence,
    AgentProposal, EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceSourceRef,
    EvidenceSourceType, EvidenceStore, EvidenceType, MaturationProposalOutcome, ProposalSource,
    ProposalType, RiskLevel,
};

fn maturation_proposal(id: &str) -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::PreferenceUpdate,
        "/preferences/communication_style",
        serde_json::json!({
            "summary": "metadata safe proposal payload",
            "rawMemoryText": "RAW_MEMORY_TEXT_SECRET",
        }),
        "RAW_PROMPT_SECRET RAW_ASSISTANT_OUTPUT_SECRET unredacted reviewer note",
        0.84,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    proposal.id = id.to_string();
    proposal.run_id = Some("run-maturation-outcome".into());
    proposal.source_detail = Some("maturation:preference.communication".into());
    proposal
}

fn create_source_evidence(store: &EvidenceStore, proposal: &AgentProposal) -> String {
    let source_ref = EvidenceSourceRef::from_digest(
        EvidenceSourceType::AgentRun,
        "run-maturation-outcome",
        Some("maturation_candidate"),
        "candidate-digest-only",
    );
    store
        .create_evidence(
            EvidenceDraft::new(
                EvidenceType::Preference,
                proposal.affected_path.clone(),
                0.84,
                proposal.risk_level,
                EvidencePrivacyLevel::Internal,
            )
            .with_summary("preference maturation candidate")
            .with_source_ref(source_ref)
            .with_linked_proposal(proposal.id.clone())
            .with_linked_agent_run("run-maturation-outcome"),
        )
        .unwrap()
        .id
}

fn proposal_outcome_records(store: &EvidenceStore) -> Vec<crate::agent::EvidenceRecord> {
    store
        .query(EvidenceQuery {
            evidence_type: Some(EvidenceType::ProposalOutcome),
            ..Default::default()
        })
        .unwrap()
}

fn assert_no_raw_content(serialized: &str) {
    for raw in [
        "RAW_PROMPT_SECRET",
        "RAW_ASSISTANT_OUTPUT_SECRET",
        "RAW_MEMORY_TEXT_SECRET",
        "RAW_TOOL_PAYLOAD_SECRET",
        "RAW_EDITED_PAYLOAD_SECRET",
        "unredacted reviewer note",
    ] {
        assert!(
            !serialized.contains(raw),
            "serialized output leaked raw marker {raw}: {serialized}"
        );
    }
}

#[test]
fn proposal_outcome_accept_maturation_creates_metadata_safe_evidence_and_links_lineage() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let proposal = maturation_proposal("proposal-outcome-accepted");
    let source_evidence_id = create_source_evidence(&store, &proposal);

    let report = record_maturation_proposal_outcome_evidence(
        &store,
        &proposal,
        MaturationProposalOutcome::Accepted,
    )
    .unwrap();

    assert!(report.recorded);
    assert_eq!(report.outcome, MaturationProposalOutcome::Accepted);
    assert_eq!(report.proposal_id, proposal.id);
    assert_eq!(report.source_evidence_ids, vec![source_evidence_id.clone()]);
    assert_eq!(
        report.source_run_id.as_deref(),
        Some("run-maturation-outcome")
    );

    let records = proposal_outcome_records(&store);
    assert_eq!(records.len(), 1);
    let evidence = &records[0];
    assert_eq!(evidence.evidence_type, EvidenceType::ProposalOutcome);
    assert_eq!(evidence.linked_proposal_ids, vec![proposal.id]);
    assert_eq!(
        evidence.linked_agent_run_ids,
        vec!["run-maturation-outcome"]
    );
    assert_eq!(evidence.run_metadata["outcome"], "accepted");
    assert_eq!(
        evidence.run_metadata["sourceEvidenceIds"],
        serde_json::json!([source_evidence_id])
    );
    assert_eq!(
        evidence.run_metadata["sourceRunId"],
        serde_json::json!("run-maturation-outcome")
    );

    let serialized = serde_json::to_string(&(report, evidence)).unwrap();
    assert_no_raw_content(&serialized);
}

#[test]
fn proposal_outcome_reject_maturation_creates_negative_opposing_evidence() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let proposal = maturation_proposal("proposal-outcome-rejected");
    let source_evidence_id = create_source_evidence(&store, &proposal);

    let report = record_maturation_proposal_outcome_evidence(
        &store,
        &proposal,
        MaturationProposalOutcome::Rejected,
    )
    .unwrap();

    assert!(report.recorded);
    assert_eq!(report.outcome, MaturationProposalOutcome::Rejected);
    let records = proposal_outcome_records(&store);
    assert_eq!(records.len(), 1);
    let evidence = &records[0];
    assert_eq!(evidence.run_metadata["outcome"], "rejected");
    assert_eq!(evidence.run_metadata["negative"], true);
    assert_eq!(evidence.run_metadata["opposing"], true);
    assert_eq!(evidence.opposing_refs, vec![source_evidence_id]);
}

#[test]
fn proposal_outcome_edit_maturation_does_not_leak_raw_edited_payload() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let mut proposal = maturation_proposal("proposal-outcome-edited");
    proposal.after = serde_json::json!({
        "summary": "edited safe summary",
        "content": "RAW_EDITED_PAYLOAD_SECRET",
        "toolPayload": "RAW_TOOL_PAYLOAD_SECRET",
    });
    create_source_evidence(&store, &proposal);

    let report = record_maturation_proposal_outcome_evidence(
        &store,
        &proposal,
        MaturationProposalOutcome::Edited,
    )
    .unwrap();

    assert!(report.recorded);
    let records = proposal_outcome_records(&store);
    assert_eq!(records.len(), 1);
    let evidence = &records[0];
    assert_eq!(evidence.run_metadata["outcome"], "edited");
    assert_eq!(evidence.run_metadata["editedPayloadIncluded"], false);

    let serialized = serde_json::to_string(&(report, evidence)).unwrap();
    assert_no_raw_content(&serialized);
}

#[test]
fn proposal_outcome_unsupported_maturation_domain_is_metadata_safe_noop() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let mut proposal = AgentProposal::new(
        ProposalType::GoalUpdate,
        "/goals/short_term",
        serde_json::json!({ "summary": "unsupported metadata safe proposal" }),
        "RAW_PROMPT_SECRET RAW_ASSISTANT_OUTPUT_SECRET",
        0.78,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    proposal.id = "proposal-outcome-unsupported-domain".to_string();
    proposal.run_id = Some("run-maturation-outcome-unsupported".into());
    proposal.source_detail = Some("maturation:goal.short_term".into());
    create_source_evidence(&store, &proposal);

    let report = record_maturation_proposal_outcome_evidence(
        &store,
        &proposal,
        MaturationProposalOutcome::Accepted,
    )
    .unwrap();

    assert!(!report.recorded);
    assert!(!report.high_risk_domain);
    assert!(!report.supported_low_risk_domain);
    assert_eq!(report.candidate_domain, None);
    assert!(report
        .blocking_reasons
        .contains(&"unsupported_maturation_outcome_domain".to_string()));
    assert!(proposal_outcome_records(&store).is_empty());
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn proposal_outcome_high_risk_path_with_low_risk_source_detail_is_metadata_safe_noop() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let mut proposal = maturation_proposal("proposal-outcome-high-risk-path");
    proposal.affected_path = "/identity/name".to_string();
    proposal.risk_level = RiskLevel::Low;
    create_source_evidence(&store, &proposal);

    let report = record_maturation_proposal_outcome_evidence(
        &store,
        &proposal,
        MaturationProposalOutcome::Accepted,
    )
    .unwrap();

    assert!(!report.recorded);
    assert!(report.high_risk_domain);
    assert!(!report.supported_low_risk_domain);
    assert!(report
        .blocking_reasons
        .contains(&"high_risk_maturation_outcome_domain".to_string()));
    assert!(proposal_outcome_records(&store).is_empty());
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn proposal_outcome_repeated_same_outcome_does_not_create_duplicate_evidence() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let proposal = maturation_proposal("proposal-outcome-idempotent");
    create_source_evidence(&store, &proposal);

    let first = record_maturation_proposal_outcome_evidence(
        &store,
        &proposal,
        MaturationProposalOutcome::Accepted,
    )
    .unwrap();
    let second = record_maturation_proposal_outcome_evidence(
        &store,
        &proposal,
        MaturationProposalOutcome::Accepted,
    )
    .unwrap();

    assert!(first.recorded);
    assert!(!second.recorded);
    assert_eq!(second.outcome_evidence_id, first.outcome_evidence_id);
    let records = proposal_outcome_records(&store);
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].id,
        first
            .outcome_evidence_id
            .expect("first outcome evidence id")
    );
}

#[test]
fn proposal_outcome_non_maturation_proposal_is_noop() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let proposal = AgentProposal::new(
        ProposalType::PreferenceUpdate,
        "/preferences/communication_style",
        serde_json::json!({ "summary": "ordinary proposal" }),
        "ordinary manual proposal",
        0.7,
        RiskLevel::Low,
        ProposalSource::Manual,
    );

    let report = record_maturation_proposal_outcome_evidence(
        &store,
        &proposal,
        MaturationProposalOutcome::Accepted,
    )
    .unwrap();

    assert!(!report.recorded);
    assert!(!report.maturation_lineage_present);
    assert!(proposal_outcome_records(&store).is_empty());
}

#[test]
fn proposal_outcome_serialized_and_debug_reports_are_metadata_safe() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let mut proposal = maturation_proposal("proposal-outcome-safe-report");
    proposal.after = serde_json::json!({
        "content": "RAW_MEMORY_TEXT_SECRET",
        "tool_payload": "RAW_TOOL_PAYLOAD_SECRET",
    });
    let source_evidence_id = create_source_evidence(&store, &proposal);
    let source_records = store
        .query(EvidenceQuery {
            linked_proposal_id: Some(proposal.id.clone()),
            ..Default::default()
        })
        .unwrap();

    let report = evaluate_maturation_proposal_outcome_evidence(
        &proposal,
        MaturationProposalOutcome::Edited,
        &source_records,
    );

    assert!(report.metadata_safe);
    assert_eq!(report.source_evidence_ids, vec![source_evidence_id]);
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
    assert_no_raw_content(&format!("{report:?}"));
}
