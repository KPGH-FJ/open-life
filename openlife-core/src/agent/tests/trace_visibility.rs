use crate::agent::{
    ensure_low_energy_rule_trace_visibility, evaluate_accepted_low_energy_rule_selection,
    evaluate_low_energy_rule_trace_visibility, propose_low_energy_collaboration_rule_candidate,
    AcceptedLowEnergyRuleSelectionInput, AgentProposal, AgentTaskKind, EvidenceDraft,
    EvidencePrivacyLevel, EvidenceQuery, EvidenceRecord, EvidenceSourceRef, EvidenceSourceType,
    EvidenceStore, EvidenceType, LowEnergyCollaborationRuleCandidateInput,
    LowEnergyRuleTraceVisibilityInput, MaturationProposalOutcome, ModelRoutePolicy, PolicyTopic,
    ProposalSource, ProposalStatus, ProposalStore, ProposalType, RiskLevel, RuntimeHSPacket,
    SelectedPolicyRef, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY,
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
            agent_task_id: Some("task-w78-local-only".into()),
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
        "RAW_TOOL_PAYLOAD_SECRET",
        "RAW_LIFEMODEL_TEXT_SECRET",
        "RAW_MEMORY_TEXT_SECRET",
        "RAW_EDITED_PAYLOAD_SECRET",
        "reviewer raw note",
    ] {
        assert!(
            !serialized.contains(raw),
            "serialized W78 output leaked raw marker {raw}: {serialized}"
        );
    }
}

#[test]
fn trace_visibility_selected_w77_report_generates_metadata_safe_visibility_report() {
    let (candidate, outcome_evidence_id, run_id) =
        accepted_w76_candidate_fixture("proposal-w78-selected", "run-w78-selected");
    let candidate_id = candidate.id.clone();
    let candidate_digest = candidate.after["candidateRuleDigest"]
        .as_str()
        .unwrap()
        .to_string();
    let selection_report = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(candidate),
    );
    let trace_payload = serde_json::json!({
        "schema": "test.w78.traceMetadataPayload.v1",
        "metadataSafe": true,
        "containsRawContent": false,
        "selectedCandidateProposalId": candidate_id,
        "candidateRuleDigest": candidate_digest,
        "lineageCounts": {
            "outcomeEvidence": 1,
            "proposal": 1,
            "agentRun": 1
        },
        "defaultChatUnchanged": true,
        "ordinaryChatEntrypointAttached": false,
        "runtimeExecuted": false,
        "modelCalled": false,
        "toolCalled": false,
        "lifeModelWritten": false,
        "memoryWritten": false,
        "heuristicActivated": false,
        "agentRunWritten": false
    });

    let report = evaluate_low_energy_rule_trace_visibility(
        LowEnergyRuleTraceVisibilityInput::for_selection_report(selection_report)
            .with_trace_payload(trace_payload),
    );

    assert!(report.trace_visibility_ready);
    assert!(report.selected_rule_visible);
    assert!(report.runtime_hs_packet_guidance_visible);
    assert!(report.evidence_lineage_visible);
    assert!(report.proposal_lineage_visible);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.default_chat_unchanged);
    assert!(!report.ordinary_chat_entrypoint_attached);
    assert!(!report.runtime_executed);
    assert!(!report.model_called);
    assert!(!report.tool_called);
    assert!(!report.life_model_written);
    assert!(!report.memory_written);
    assert!(!report.heuristic_activated);
    assert!(!report.agent_run_written);
    assert!(report.privacy_policy_preserved);
    assert!(report.local_only_policy_preserved);
    assert_eq!(
        report
            .trace_metadata
            .selected_candidate_proposal_id
            .as_deref(),
        Some(candidate_id.as_str())
    );
    assert_eq!(
        report
            .trace_metadata
            .selected_candidate_rule_digest
            .as_deref(),
        Some(candidate_digest.as_str())
    );
    assert_eq!(report.trace_metadata.evidence_lineage.count, 1);
    assert_eq!(
        report.trace_metadata.evidence_lineage.items[0].id,
        outcome_evidence_id
    );
    assert_eq!(report.trace_metadata.proposal_lineage.count, 1);
    assert_eq!(
        report.trace_metadata.proposal_lineage.items[0].id,
        "proposal-w78-selected"
    );
    assert_eq!(report.trace_metadata.agent_run_lineage.count, 1);
    assert_eq!(report.trace_metadata.agent_run_lineage.items[0].id, run_id);
    assert_eq!(report.wrote_evidence_count, 0);
    assert_eq!(report.wrote_proposal_count, 0);
    assert_eq!(report.wrote_life_model_count, 0);
    assert_eq!(report.wrote_memory_count, 0);
    assert_eq!(report.wrote_heuristic_count, 0);
    assert_eq!(report.wrote_agent_run_count, 0);
    assert_eq!(report.wrote_chat_message_count, 0);
    assert_eq!(report.wrote_mcp_audit_count, 0);
    assert_eq!(report.wrote_external_count, 0);
    assert!(!report.ran_runtime);
    assert!(!report.ran_model);
    assert!(!report.ran_tool);

    let serialized = serde_json::to_string(&report).unwrap();
    assert_no_raw_content(&serialized);
}

#[test]
fn trace_visibility_blocked_or_non_selected_w77_report_fails_closed() {
    let (accepted_candidate, _, _) =
        accepted_w76_candidate_fixture("proposal-w78-blocked", "run-w78-blocked");
    let mut pending_candidate = accepted_candidate.clone();
    pending_candidate.status = ProposalStatus::Pending;
    let pending_selection = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(pending_candidate),
    );
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
    let non_w76_selection = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(non_w76_candidate),
    );

    let pending = evaluate_low_energy_rule_trace_visibility(
        LowEnergyRuleTraceVisibilityInput::for_selection_report(pending_selection.clone()),
    );
    let non_w76 = evaluate_low_energy_rule_trace_visibility(
        LowEnergyRuleTraceVisibilityInput::for_selection_report(non_w76_selection),
    );

    assert!(!pending.trace_visibility_ready);
    assert!(pending
        .blocking_reasons
        .contains(&"w77_selection_not_selected".to_string()));
    assert!(pending
        .blocking_reasons
        .contains(&"candidate_proposal_not_accepted".to_string()));
    assert!(ensure_low_energy_rule_trace_visibility(
        LowEnergyRuleTraceVisibilityInput::for_selection_report(pending_selection)
    )
    .is_err());

    assert!(!non_w76.trace_visibility_ready);
    assert!(non_w76
        .blocking_reasons
        .contains(&"candidate_proposal_not_w76_low_energy_rule_candidate".to_string()));
}

#[test]
fn trace_visibility_unsafe_trace_payload_fails_closed_without_echoing_raw_payload() {
    let (candidate, _, _) = accepted_w76_candidate_fixture("proposal-w78-unsafe", "run-w78-unsafe");
    let selection_report = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(candidate),
    );
    let unsafe_trace_payload = serde_json::json!({
        "rawPrompt": "RAW_PROMPT_SECRET",
        "rawAssistantOutput": "RAW_ASSISTANT_OUTPUT_SECRET",
        "rawToolPayload": "RAW_TOOL_PAYLOAD_SECRET",
        "rawLifeModelText": "RAW_LIFEMODEL_TEXT_SECRET",
        "rawMemoryText": "RAW_MEMORY_TEXT_SECRET",
        "modelRoutePolicy": "cloud_allowed",
        "defaultChatRoute": "controlled_adapter",
        "runtimeExecuted": true,
        "modelCalled": true,
        "toolCalled": true,
        "heuristicActivated": true
    });

    let report = evaluate_low_energy_rule_trace_visibility(
        LowEnergyRuleTraceVisibilityInput::for_selection_report(selection_report)
            .with_trace_payload(unsafe_trace_payload),
    );

    assert!(!report.trace_visibility_ready);
    assert!(!report.metadata_safe);
    assert!(report.contains_raw_content);
    assert!(report
        .blocking_reasons
        .contains(&"trace_payload_contains_raw_content".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"trace_payload_relaxes_privacy_or_route_policy".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"trace_payload_implies_default_chat_route_cutover".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"trace_payload_implies_runtime_execution".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"trace_payload_implies_model_call".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"trace_payload_implies_tool_call".to_string()));
    assert!(report
        .blocking_reasons
        .contains(&"trace_payload_implies_heuristic_activation".to_string()));

    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn trace_visibility_lineage_debug_dump_exposes_only_ids_hashes_counts_status_and_type() {
    let (candidate, _, _) =
        accepted_w76_candidate_fixture("proposal-w78-lineage", "run-w78-lineage");
    let selection_report = evaluate_accepted_low_energy_rule_selection(
        AcceptedLowEnergyRuleSelectionInput::for_candidate(candidate),
    );

    let report = evaluate_low_energy_rule_trace_visibility(
        LowEnergyRuleTraceVisibilityInput::for_selection_report(selection_report),
    );
    let serialized_lineage = serde_json::to_value(&serde_json::json!({
        "evidence": report.trace_metadata.evidence_lineage,
        "proposal": report.trace_metadata.proposal_lineage,
    }))
    .unwrap();
    let allowed_summary_keys = ["items", "count", "idsHash"];
    let allowed_item_keys = ["id", "idHash", "recordType", "status"];

    for key in serialized_lineage["evidence"].as_object().unwrap().keys() {
        assert!(allowed_summary_keys.contains(&key.as_str()), "{key}");
    }
    for key in serialized_lineage["proposal"].as_object().unwrap().keys() {
        assert!(allowed_summary_keys.contains(&key.as_str()), "{key}");
    }
    for lineage_key in ["evidence", "proposal"] {
        for item in serialized_lineage[lineage_key]["items"].as_array().unwrap() {
            for key in item.as_object().unwrap().keys() {
                assert!(allowed_item_keys.contains(&key.as_str()), "{key}");
            }
        }
    }
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn trace_visibility_preserves_local_only_privacy_policy() {
    let (candidate, _, run_id) =
        accepted_w76_candidate_fixture("proposal-w78-privacy", "run-w78-privacy");
    let mut input = AcceptedLowEnergyRuleSelectionInput::for_candidate(candidate);
    input.privacy_topic = PolicyTopic::Health;
    input.current_route_policy = ModelRoutePolicy::LocalOnly;
    input.existing_hs_packet = Some(local_only_packet(&run_id));
    let selection_report = evaluate_accepted_low_energy_rule_selection(input);

    let report = evaluate_low_energy_rule_trace_visibility(
        LowEnergyRuleTraceVisibilityInput::for_selection_report(selection_report),
    );

    assert!(report.trace_visibility_ready);
    assert!(report.privacy_policy_preserved);
    assert!(report.local_only_policy_preserved);
    assert_eq!(
        report.trace_metadata.enforced_route_policy,
        ModelRoutePolicy::LocalOnly
    );
    assert!(report
        .trace_metadata
        .selected_policy_ids
        .contains(&BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.to_string()));
}

#[test]
fn trace_visibility_non_planning_or_non_low_energy_selection_report_fails_closed() {
    let (planning_candidate, _, _) =
        accepted_w76_candidate_fixture("proposal-w78-non-planning", "run-w78-non-planning");
    let mut non_planning_input =
        AcceptedLowEnergyRuleSelectionInput::for_candidate(planning_candidate);
    non_planning_input.target_task_kind = AgentTaskKind::Conversation;
    non_planning_input.planning_intent_present = false;
    let non_planning_selection = evaluate_accepted_low_energy_rule_selection(non_planning_input);

    let (domain_candidate, _, _) =
        accepted_w76_candidate_fixture("proposal-w78-non-domain", "run-w78-non-domain");
    let mut non_domain_input = AcceptedLowEnergyRuleSelectionInput::for_candidate(domain_candidate);
    non_domain_input.target_domain = "identity_values".into();
    let non_domain_selection = evaluate_accepted_low_energy_rule_selection(non_domain_input);

    let non_planning = evaluate_low_energy_rule_trace_visibility(
        LowEnergyRuleTraceVisibilityInput::for_selection_report(non_planning_selection),
    );
    let non_domain = evaluate_low_energy_rule_trace_visibility(
        LowEnergyRuleTraceVisibilityInput::for_selection_report(non_domain_selection),
    );

    assert!(!non_planning.trace_visibility_ready);
    assert!(non_planning
        .blocking_reasons
        .contains(&"non_planning_task".to_string()));
    assert!(!non_domain.trace_visibility_ready);
    assert!(non_domain
        .blocking_reasons
        .contains(&"non_low_energy_planning_domain".to_string()));
}
