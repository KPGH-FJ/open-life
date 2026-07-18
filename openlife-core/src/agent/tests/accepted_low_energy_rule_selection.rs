use crate::agent::{
    evaluate_accepted_low_energy_rule_selection, propose_low_energy_collaboration_rule_candidate,
    AcceptedLowEnergyRuleSelectionInput, AgentProposal, AgentTaskKind, EvidenceDraft,
    EvidencePrivacyLevel, EvidenceQuery, EvidenceRecord, EvidenceSourceRef, EvidenceSourceType,
    EvidenceStore, EvidenceType, LowEnergyCollaborationRuleCandidateInput,
    MaturationProposalOutcome, ModelRoutePolicy, PolicyTopic, ProposalSource, ProposalStatus,
    ProposalStore, ProposalType, RiskLevel, RuntimeHSPacket, SelectedPolicyRef,
    BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY,
};

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

fn accepted_w76_candidate_fixture(
    proposal_id: &str,
    run_id: &str,
) -> (AgentProposal, String, String) {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let maturation_proposal = maturation_proposal(proposal_id, run_id);
    create_source_evidence(&evidence_store, &maturation_proposal, run_id);
    let outcome = record_outcome(
        &evidence_store,
        &maturation_proposal,
        MaturationProposalOutcome::Accepted,
    );
    let outcome_evidence_id = outcome.id.clone();

    let report = propose_low_energy_collaboration_rule_candidate(
        LowEnergyCollaborationRuleCandidateInput::for_outcome_evidence(vec![outcome]),
        &proposal_store,
    )
    .unwrap();
    let mut candidate = proposal_store
        .get_proposal(report.candidate_proposal_id.as_deref().unwrap())
        .unwrap()
        .unwrap();
    candidate.accept();
    (candidate, outcome_evidence_id, run_id.to_string())
}

fn local_only_packet(run_id: &str) -> RuntimeHSPacket {
    RuntimeHSPacket {
        selected_policies: vec![SelectedPolicyRef {
            policy_id: BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
            reason: "sensitive_topic_route".into(),
            route: Some(ModelRoutePolicy::LocalOnly),
            digest: "policy-digest".into(),
        }],
        selected_heuristics: Vec::new(),
        guidance_refs: Vec::new(),
        estimated_tokens: 0,
        audit: crate::agent::HSSelectionAudit {
            agent_task_id: Some("task-w77-local-only".into()),
            agent_run_id: Some(run_id.into()),
            input_digest: "input-digest".into(),
            selected_policy_ids: vec![BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into()],
            selected_heuristic_ids: Vec::new(),
            selected_guidance_ids: Vec::new(),
            selected_guidance_refs: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 0,
            token_budget: 128,
        },
        provider_authorization: crate::llm::ProviderPolicyAuthorization::local_only_fail_closed(
            crate::llm::ProviderLocalOnlyReason::TestFixture,
        ),
    }
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
            "serialized W77 output leaked raw marker {raw}: {serialized}"
        );
    }
}

#[test]
fn accepted_low_energy_rule_selection_accepted_candidate_generates_metadata_safe_packet_proof() {
    let (candidate, outcome_evidence_id, run_id) =
        accepted_w76_candidate_fixture("proposal-w77-accepted", "run-w77-accepted");
    let candidate_id = candidate.id.clone();
    let candidate_digest = candidate.after["candidateRuleDigest"]
        .as_str()
        .unwrap()
        .to_string();

    let report = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(candidate),
    );

    assert!(report.selected);
    assert!(report.planning_task_only);
    assert!(report.low_energy_domain_only);
    assert!(!report.privacy_policy_relaxed);
    assert_eq!(
        report.selected_candidate_proposal_id.as_deref(),
        Some(candidate_id.as_str())
    );
    assert_eq!(
        report.selected_candidate_rule_digest.as_deref(),
        Some(candidate_digest.as_str())
    );
    assert_eq!(
        report.source_outcome_evidence_ids,
        vec![outcome_evidence_id]
    );
    assert_eq!(report.source_proposal_ids, vec!["proposal-w77-accepted"]);
    assert_eq!(report.source_agent_run_ids, vec![run_id]);
    assert!(report
        .selected_guidance_summary
        .as_deref()
        .unwrap()
        .contains("low-pressure planning"));
    assert_eq!(
        report.hs_packet_audit_proof.selected_guidance_summary,
        report.selected_guidance_summary
    );
    assert_eq!(
        report.hs_packet_audit_proof.selected_candidate_rule_digest,
        report.selected_candidate_rule_digest
    );
    assert!(report.hs_packet_audit_proof.metadata_safe);
    assert!(!report.ran_runtime);
    assert!(!report.ran_model);
    assert!(!report.ran_tool);
    assert_eq!(report.wrote_life_model_count, 0);
    assert_eq!(report.wrote_memory_count, 0);
    assert_eq!(report.wrote_heuristic_count, 0);
    assert_eq!(report.wrote_proposal_count, 0);
    assert_eq!(report.wrote_evidence_count, 0);

    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn accepted_low_energy_rule_selection_pending_rejected_or_non_w76_candidate_fails_closed() {
    let (accepted_candidate, _, _) =
        accepted_w76_candidate_fixture("proposal-w77-status", "run-w77-status");
    let mut pending_candidate = accepted_candidate.clone();
    pending_candidate.status = ProposalStatus::Pending;
    let mut rejected_candidate = accepted_candidate.clone();
    rejected_candidate.reject();
    let mut non_w76_candidate = AgentProposal::new(
        ProposalType::PreferenceUpdate,
        "/preferences",
        serde_json::json!({ "summary": "safe but ordinary proposal" }),
        "metadata-safe ordinary proposal",
        0.9,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    non_w76_candidate.accept();

    let pending = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(pending_candidate),
    );
    let rejected = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(rejected_candidate),
    );
    let non_w76 = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(non_w76_candidate),
    );

    assert!(!pending.selected);
    assert!(pending
        .blocking_reasons
        .contains(&"candidate_proposal_not_accepted".to_string()));
    assert!(!rejected.selected);
    assert!(rejected
        .blocking_reasons
        .contains(&"candidate_proposal_not_accepted".to_string()));
    assert!(!non_w76.selected);
    assert!(non_w76
        .blocking_reasons
        .contains(&"candidate_proposal_not_w76_low_energy_rule_candidate".to_string()));
}

#[test]
fn accepted_low_energy_rule_selection_non_planning_task_fails_closed() {
    let (candidate, _, _) =
        accepted_w76_candidate_fixture("proposal-w77-non-planning", "run-w77-non-planning");
    let mut input = AcceptedLowEnergyRuleSelectionInput::for_candidate(candidate);
    input.target_task_kind = AgentTaskKind::Conversation;
    input.planning_intent_present = false;

    let report = evaluate_accepted_low_energy_rule_selection(input);

    assert!(!report.selected);
    assert!(report
        .blocking_reasons
        .contains(&"non_planning_task".to_string()));
}

#[test]
fn accepted_low_energy_rule_selection_non_low_energy_domain_fails_closed() {
    let (candidate, _, _) =
        accepted_w76_candidate_fixture("proposal-w77-non-domain", "run-w77-non-domain");
    let mut input = AcceptedLowEnergyRuleSelectionInput::for_candidate(candidate);
    input.target_domain = "identity_values".into();

    let report = evaluate_accepted_low_energy_rule_selection(input);

    assert!(!report.selected);
    assert!(report
        .blocking_reasons
        .contains(&"non_low_energy_planning_domain".to_string()));
}

#[test]
fn accepted_low_energy_rule_selection_preserves_local_only_privacy_policy() {
    let (candidate, _, run_id) =
        accepted_w76_candidate_fixture("proposal-w77-privacy", "run-w77-privacy");
    let mut input = AcceptedLowEnergyRuleSelectionInput::for_candidate(candidate);
    input.privacy_topic = PolicyTopic::Health;
    input.current_route_policy = ModelRoutePolicy::LocalOnly;
    input.existing_hs_packet = Some(local_only_packet(&run_id));

    let report = evaluate_accepted_low_energy_rule_selection(input);

    assert!(report.selected);
    assert!(!report.privacy_policy_relaxed);
    assert_eq!(
        report.hs_packet_audit_proof.enforced_route_policy,
        ModelRoutePolicy::LocalOnly
    );
    assert!(report
        .hs_packet_audit_proof
        .selected_policy_ids
        .contains(&BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.to_string()));
    assert_eq!(
        report.hs_packet_audit_proof.selected_guidance_summary,
        report.selected_guidance_summary
    );
}

#[test]
fn accepted_low_energy_rule_selection_does_not_write_lifemodel_memory_or_active_heuristic() {
    let (candidate, _, _) =
        accepted_w76_candidate_fixture("proposal-w77-side-effects", "run-w77-side-effects");

    let report = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(candidate),
    );

    assert!(report.selected);
    assert_eq!(report.wrote_life_model_count, 0);
    assert_eq!(report.wrote_memory_count, 0);
    assert_eq!(report.wrote_heuristic_count, 0);
    assert_eq!(report.wrote_chat_message_count, 0);
    assert_eq!(report.wrote_agent_run_count, 0);
    assert_eq!(report.wrote_mcp_audit_count, 0);
    assert_eq!(report.wrote_external_count, 0);
    assert!(!report.ran_runtime);
    assert!(!report.ran_model);
    assert!(!report.ran_tool);
}
