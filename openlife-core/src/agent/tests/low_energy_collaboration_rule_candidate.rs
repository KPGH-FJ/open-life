use crate::agent::{
    evaluate_low_energy_collaboration_rule_candidate,
    propose_low_energy_collaboration_rule_candidate, AgentProposal, EvidenceDraft,
    EvidencePrivacyLevel, EvidenceQuery, EvidenceRecord, EvidenceSourceRef, EvidenceSourceType,
    EvidenceStore, EvidenceType, HeuristicQuery, HeuristicStore,
    LowEnergyCollaborationRuleCandidateInput, MaturationProposalOutcome, ProposalSource,
    ProposalStatus, ProposalStore, ProposalType, RiskLevel,
};
use crate::life_model::LifeModel;
use crate::memory::MemoryStore;

fn maturation_proposal(id: &str, run_id: &str) -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::PreferenceUpdate,
        "/preferences",
        serde_json::json!({
            "summary": "metadata-safe low energy planning preference",
            "rawMemoryText": "RAW_MEMORY_TEXT_SECRET",
        }),
        "RAW_PROMPT_SECRET RAW_ASSISTANT_OUTPUT_SECRET reviewer raw note",
        0.88,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    proposal.id = id.to_string();
    proposal.run_id = Some(run_id.to_string());
    proposal.source_detail = Some("maturation:preference.planning.low_energy".into());
    proposal
}

fn create_source_evidence(store: &EvidenceStore, proposal: &AgentProposal, run_id: &str) -> String {
    let source_ref = EvidenceSourceRef::from_digest(
        EvidenceSourceType::AgentRun,
        run_id,
        Some("maturation_candidate"),
        "low-energy-candidate-digest",
    );
    store
        .create_evidence(
            EvidenceDraft::new(
                EvidenceType::Preference,
                proposal.affected_path.clone(),
                0.88,
                proposal.risk_level,
                EvidencePrivacyLevel::Internal,
            )
            .with_summary("low-energy planning maturation candidate")
            .with_source_ref(source_ref)
            .with_linked_proposal(proposal.id.clone())
            .with_linked_agent_run(run_id),
        )
        .unwrap()
        .id
}

fn record_outcome(
    store: &EvidenceStore,
    proposal: &AgentProposal,
    outcome: MaturationProposalOutcome,
) -> EvidenceRecord {
    crate::agent::record_maturation_proposal_outcome_evidence(store, proposal, outcome).unwrap();
    store
        .query(EvidenceQuery {
            evidence_type: Some(EvidenceType::ProposalOutcome),
            linked_proposal_id: Some(proposal.id.clone()),
            ..Default::default()
        })
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
}

fn accepted_outcome(store: &EvidenceStore, id: &str, run_id: &str) -> (EvidenceRecord, String) {
    let proposal = maturation_proposal(id, run_id);
    let source_evidence_id = create_source_evidence(store, &proposal, run_id);
    (
        record_outcome(store, &proposal, MaturationProposalOutcome::Accepted),
        source_evidence_id,
    )
}

fn assert_no_raw_content(serialized: &str) {
    for raw in [
        "RAW_PROMPT_SECRET",
        "RAW_ASSISTANT_OUTPUT_SECRET",
        "RAW_MEMORY_TEXT_SECRET",
        "RAW_TOOL_PAYLOAD_SECRET",
        "RAW_EDITED_PAYLOAD_SECRET",
        "reviewer raw note",
    ] {
        assert!(
            !serialized.contains(raw),
            "serialized W76 output leaked raw marker {raw}: {serialized}"
        );
    }
}

#[test]
fn low_energy_collaboration_accepted_outcome_evidence_generates_candidate_proposal() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let (outcome, source_evidence_id) =
        accepted_outcome(&evidence_store, "proposal-w76-accepted", "run-w76-accepted");

    let input = LowEnergyCollaborationRuleCandidateInput::for_outcome_evidence(vec![outcome]);
    let report = evaluate_low_energy_collaboration_rule_candidate(input.clone());

    assert!(report.ready);
    assert!(report.reviewable_candidate_ready);
    assert_eq!(report.accepted_outcome_evidence_ids.len(), 1);
    assert_eq!(report.source_evidence_ids, vec![source_evidence_id.clone()]);
    assert_eq!(
        report.linked_proposal_ids,
        vec!["proposal-w76-accepted".to_string()]
    );
    assert_eq!(
        report.linked_agent_run_ids,
        vec!["run-w76-accepted".to_string()]
    );
    assert!(report.candidate_only);
    assert!(!report.activates_heuristic);
    assert!(!report.writes_active_rule);
    assert!(!report.ran_runtime);
    assert!(!report.ran_model);
    assert!(!report.ran_tool);
    assert_eq!(proposal_store.pending_count().unwrap(), 0);

    let proposed = propose_low_energy_collaboration_rule_candidate(input, &proposal_store).unwrap();
    assert!(proposed.ready);
    assert_eq!(proposed.wrote_proposal_count, 1);
    let candidate_id = proposed.candidate_proposal_id.clone().unwrap();
    let candidate = proposal_store.get_proposal(&candidate_id).unwrap().unwrap();
    assert_eq!(candidate.status, ProposalStatus::Pending);
    assert_eq!(candidate.proposal_type, ProposalType::Unsupported);
    assert_eq!(
        candidate.source_detail.as_deref(),
        Some("maturation:low_energy_collaboration_rule_candidate")
    );
    assert_eq!(
        candidate.after["sourceLineage"]["acceptedOutcomeEvidenceIds"],
        serde_json::json!(proposed.accepted_outcome_evidence_ids)
    );
    assert_eq!(
        candidate.after["sourceLineage"]["sourceEvidenceIds"],
        serde_json::json!([source_evidence_id])
    );
    assert_eq!(candidate.after["candidateOnly"], true);
    assert_eq!(candidate.after["activatesHeuristic"], false);
    assert_eq!(candidate.after["writesActiveRule"], false);

    assert_no_raw_content(&serde_json::to_string(&(proposed, candidate)).unwrap());
}

#[test]
fn low_energy_collaboration_rejected_or_opposing_outcome_blocks_repeated_candidate() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let (accepted, _) =
        accepted_outcome(&evidence_store, "proposal-w76-support", "run-w76-support");
    let rejected_proposal = maturation_proposal("proposal-w76-rejected", "run-w76-rejected");
    create_source_evidence(&evidence_store, &rejected_proposal, "run-w76-rejected");
    let rejected = record_outcome(
        &evidence_store,
        &rejected_proposal,
        MaturationProposalOutcome::Rejected,
    );

    let input =
        LowEnergyCollaborationRuleCandidateInput::for_outcome_evidence(vec![accepted, rejected]);
    let report = propose_low_energy_collaboration_rule_candidate(input, &proposal_store).unwrap();

    assert!(!report.ready);
    assert!(report.weakened_by_opposing_outcome);
    assert!(report
        .blocking_reasons
        .contains(&"opposing_outcome_evidence_blocks_candidate".to_string()));
    assert_eq!(report.rejected_outcome_evidence_ids.len(), 1);
    assert_eq!(report.opposing_outcome_evidence_ids.len(), 1);
    assert_eq!(report.wrote_proposal_count, 0);
    assert!(report.candidate_proposal_id.is_none());
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn low_energy_collaboration_edited_outcome_uses_safe_digest_lineage_without_raw_payload() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let mut proposal = maturation_proposal("proposal-w76-edited", "run-w76-edited");
    proposal.after = serde_json::json!({
        "summary": "safe edited summary",
        "content": "RAW_EDITED_PAYLOAD_SECRET",
        "toolPayload": "RAW_TOOL_PAYLOAD_SECRET",
    });
    create_source_evidence(&evidence_store, &proposal, "run-w76-edited");
    let edited = record_outcome(
        &evidence_store,
        &proposal,
        MaturationProposalOutcome::Edited,
    );

    let input = LowEnergyCollaborationRuleCandidateInput::for_outcome_evidence(vec![edited]);
    let report = propose_low_energy_collaboration_rule_candidate(input, &proposal_store).unwrap();

    assert!(report.ready);
    assert_eq!(report.edited_outcome_evidence_ids.len(), 1);
    assert_eq!(report.wrote_proposal_count, 1);
    let candidate = proposal_store
        .get_proposal(report.candidate_proposal_id.as_deref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        candidate.after["sourceLineage"]["editedOutcomeEvidenceIds"],
        serde_json::json!(report.edited_outcome_evidence_ids)
    );
    assert_eq!(candidate.after["editedPayloadIncluded"], false);
    assert_no_raw_content(&serde_json::to_string(&(report, candidate)).unwrap());
}

#[test]
fn low_energy_collaboration_preserves_source_proposal_and_agent_run_lineage() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let (accepted, source_evidence_id) =
        accepted_outcome(&evidence_store, "proposal-w76-lineage", "run-w76-lineage");

    let report = evaluate_low_energy_collaboration_rule_candidate(
        LowEnergyCollaborationRuleCandidateInput::for_outcome_evidence(vec![accepted]),
    );

    assert_eq!(report.source_evidence_ids, vec![source_evidence_id]);
    assert_eq!(
        report.linked_proposal_ids,
        vec!["proposal-w76-lineage".to_string()]
    );
    assert_eq!(
        report.linked_agent_run_ids,
        vec!["run-w76-lineage".to_string()]
    );
}

#[test]
fn low_energy_collaboration_non_low_energy_domain_fails_closed() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let (outcome, _) = accepted_outcome(
        &evidence_store,
        "proposal-w76-non-domain",
        "run-w76-non-domain",
    );
    let mut input = LowEnergyCollaborationRuleCandidateInput::for_outcome_evidence(vec![outcome]);
    input.target_domain = "identity_values".into();

    let report = evaluate_low_energy_collaboration_rule_candidate(input);

    assert!(!report.ready);
    assert!(report
        .blocking_reasons
        .contains(&"non_low_energy_planning_domain".to_string()));
}

#[test]
fn low_energy_collaboration_non_low_energy_outcome_evidence_fails_closed() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let mut proposal = AgentProposal::new(
        ProposalType::StateUpdate,
        "/state/energy_pattern/morning",
        serde_json::json!({ "summary": "safe energy pattern candidate" }),
        "metadata-safe energy pattern proposal",
        0.9,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    proposal.id = "proposal-w76-energy-pattern".into();
    proposal.run_id = Some("run-w76-energy-pattern".into());
    proposal.source_detail = Some("maturation:state.energy_pattern".into());
    create_source_evidence(&evidence_store, &proposal, "run-w76-energy-pattern");
    let outcome = record_outcome(
        &evidence_store,
        &proposal,
        MaturationProposalOutcome::Accepted,
    );

    let report = evaluate_low_energy_collaboration_rule_candidate(
        LowEnergyCollaborationRuleCandidateInput::for_outcome_evidence(vec![outcome]),
    );

    assert!(!report.ready);
    assert!(report
        .blocking_reasons
        .contains(&"outcome_evidence_outside_low_energy_collaboration_scope".to_string()));
}

#[test]
fn low_energy_collaboration_candidate_does_not_write_lifemodel_memory_or_active_heuristic() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let memory_store = MemoryStore::new_in_memory().unwrap();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    let mut life_model = LifeModel::default();
    life_model.state.current_focus = "before-w76".into();
    let (outcome, _) = accepted_outcome(
        &evidence_store,
        "proposal-w76-side-effects",
        "run-w76-side-effects",
    );

    let report = propose_low_energy_collaboration_rule_candidate(
        LowEnergyCollaborationRuleCandidateInput::for_outcome_evidence(vec![outcome]),
        &proposal_store,
    )
    .unwrap();

    assert!(report.ready);
    assert_eq!(report.wrote_proposal_count, 1);
    assert_eq!(report.wrote_evidence_count, 0);
    assert_eq!(report.wrote_life_model_count, 0);
    assert_eq!(report.wrote_memory_count, 0);
    assert_eq!(report.wrote_heuristic_count, 0);
    assert!(!report.ran_runtime);
    assert!(!report.ran_model);
    assert!(!report.ran_tool);
    assert_eq!(life_model.state.current_focus, "before-w76");
    assert!(memory_store.export_all_messages().unwrap().is_empty());
    assert!(heuristic_store
        .query(HeuristicQuery::default())
        .unwrap()
        .is_empty());
}
