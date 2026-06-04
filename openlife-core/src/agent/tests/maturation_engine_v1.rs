use crate::agent::{
    evaluate_maturation_engine_v1, record_maturation_proposal_outcome_evidence, AgentProposal,
    EvidenceDraft, EvidenceGraphInput, EvidencePrivacyLevel, EvidenceQuery, EvidenceSourceRef,
    EvidenceSourceType, EvidenceStore, EvidenceType, MaturationCandidateDomain,
    MaturationEngineV1Input, MaturationProposalOutcome, ProposalSource, ProposalStore,
    ProposalType, RiskLevel,
};
use chrono::{Duration, TimeZone, Utc};

fn source_ref(id: &str) -> EvidenceSourceRef {
    EvidenceSourceRef::from_digest(
        EvidenceSourceType::AgentRun,
        id,
        Some("metadata_safe_goal3_test"),
        format!("{id}-digest"),
    )
}

fn low_risk_evidence(path: &str, run_id: &str, confidence: f32) -> EvidenceDraft {
    EvidenceDraft::new(
        EvidenceType::Preference,
        path,
        confidence,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary("metadata safe low-risk preference evidence")
    .with_source_ref(source_ref(run_id))
    .with_linked_agent_run(run_id)
}

fn maturation_proposal(id: &str, path: &str, run_id: &str) -> AgentProposal {
    let mut proposal = AgentProposal::new(
        ProposalType::PreferenceUpdate,
        path,
        serde_json::json!({
            "summary": "metadata safe maturation candidate",
            "rawMemoryText": "RAW_MEMORY_TEXT_SECRET",
        }),
        "RAW_PROMPT_SECRET RAW_ASSISTANT_OUTPUT_SECRET reviewer raw note",
        0.86,
        RiskLevel::Low,
        ProposalSource::FeedbackEvolution,
    );
    proposal.id = id.to_string();
    proposal.run_id = Some(run_id.to_string());
    proposal.source_detail = Some("maturation:preference.communication".into());
    proposal
}

fn graph_input(store: &EvidenceStore) -> EvidenceGraphInput {
    EvidenceGraphInput::new(
        store.query(EvidenceQuery::default()).unwrap(),
        Utc.with_ymd_and_hms(2026, 6, 4, 12, 0, 0).unwrap(),
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
        "raw user text",
        "assistant output",
    ] {
        assert!(
            !serialized.contains(raw),
            "serialized Goal 3 output leaked raw marker {raw}: {serialized}"
        );
    }
}

#[test]
fn w131_generates_low_risk_candidates_from_graph_clusters_across_supported_domains() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let paths = [
        (
            "/preferences/planning/structure",
            "run-plan",
            MaturationCandidateDomain::PlanningPreference,
        ),
        (
            "/state/energy_pattern/morning",
            "run-energy",
            MaturationCandidateDomain::EnergyPattern,
        ),
        (
            "/preferences/work_style/deep_work",
            "run-work",
            MaturationCandidateDomain::WorkStyle,
        ),
        (
            "/preferences/communication/concise",
            "run-comm",
            MaturationCandidateDomain::CommunicationPreference,
        ),
    ];
    for (path, run_id, _) in paths {
        store
            .create_evidence(low_risk_evidence(path, run_id, 0.82))
            .unwrap();
    }

    let first = evaluate_maturation_engine_v1(MaturationEngineV1Input::from_graph_input(
        graph_input(&store),
    ));
    let repeated = evaluate_maturation_engine_v1(MaturationEngineV1Input::from_graph_input(
        graph_input(&store),
    ));

    assert!(first.engine_ready);
    assert!(first.candidate_generation_ready);
    assert_eq!(first.candidate_count, 4);
    assert_eq!(first.suppressed_candidate_count, 0);
    assert_eq!(first.high_risk_cluster_count, 0);
    assert_eq!(first.candidates, repeated.candidates);
    for (_, run_id, domain) in paths {
        let candidate = first
            .candidates
            .iter()
            .find(|candidate| candidate.domain == domain)
            .unwrap();
        assert_eq!(candidate.risk_level, RiskLevel::Low);
        assert!(candidate.proposal_required);
        assert!(candidate.candidate_only);
        assert!(candidate.support_evidence_ids.len() >= 1);
        assert_eq!(candidate.linked_agent_run_ids, vec![run_id.to_string()]);
        assert!(candidate.source_cluster_id.starts_with("egc_"));
        assert_eq!(candidate.source_cluster_hash.len(), 64);
    }
    assert_eq!(store.query(EvidenceQuery::default()).unwrap().len(), 4);
    assert_eq!(
        ProposalStore::new_in_memory()
            .unwrap()
            .pending_count()
            .unwrap(),
        0
    );
    assert_no_raw_content(&serde_json::to_string(&first).unwrap());
}

#[test]
fn w131_high_risk_domains_fail_closed_without_candidates() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let high_risk_paths = [
        "/identity/values/core",
        "/relationships/family",
        "/state/health/sleep",
        "/finance/risk_profile",
        "/privacy/location",
        "/goals/long_term_direction",
    ];
    for (idx, path) in high_risk_paths.iter().enumerate() {
        store
            .create_evidence(
                EvidenceDraft::new(
                    EvidenceType::Preference,
                    *path,
                    0.9,
                    RiskLevel::High,
                    EvidencePrivacyLevel::StrictlyLocal,
                )
                .with_summary("metadata safe high-risk evidence")
                .with_source_ref(source_ref(&format!("run-high-risk-{idx}"))),
            )
            .unwrap();
    }

    let report = evaluate_maturation_engine_v1(MaturationEngineV1Input::from_graph_input(
        graph_input(&store),
    ));

    assert!(!report.engine_ready);
    assert!(!report.candidate_generation_ready);
    assert_eq!(report.candidate_count, 0);
    assert_eq!(report.high_risk_cluster_count, high_risk_paths.len());
    assert!(report
        .blocking_reasons
        .contains(&"high_risk_domain_cluster_present".to_string()));
    assert!(report
        .suppressed_candidates
        .iter()
        .all(|suppression| suppression
            .reasons
            .contains(&"high_risk_domain".to_string())));
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn w131_mixed_graph_keeps_low_risk_candidates_and_suppresses_high_risk_clusters() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let low_risk = store
        .create_evidence(low_risk_evidence(
            "/preferences/communication/concise",
            "run-mixed-low-risk",
            0.86,
        ))
        .unwrap();
    let high_risk = store
        .create_evidence(
            EvidenceDraft::new(
                EvidenceType::Preference,
                "/identity/values/core",
                0.91,
                RiskLevel::High,
                EvidencePrivacyLevel::StrictlyLocal,
            )
            .with_summary("metadata safe high-risk evidence")
            .with_source_ref(source_ref("run-mixed-high-risk")),
        )
        .unwrap();

    let report = evaluate_maturation_engine_v1(MaturationEngineV1Input::from_graph_input(
        graph_input(&store),
    ));

    assert!(report.engine_ready);
    assert!(report.candidate_generation_ready);
    assert_eq!(report.candidate_count, 1);
    assert_eq!(report.candidates[0].support_evidence_ids, vec![low_risk.id]);
    assert_eq!(
        report.candidates[0].domain,
        MaturationCandidateDomain::CommunicationPreference
    );
    assert_eq!(report.high_risk_cluster_count, 1);
    assert!(report
        .suppressed_candidates
        .iter()
        .any(
            |candidate| candidate.support_evidence_ids.contains(&high_risk.id)
                && candidate.reasons.contains(&"high_risk_domain".to_string())
        ));
    assert!(!report
        .blocking_reasons
        .contains(&"high_risk_domain_cluster_present".to_string()));
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn w132_outcomes_create_positive_corrective_and_negative_evidence_with_lineage() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let accepted = maturation_proposal(
        "proposal-w132-accepted",
        "/preferences/planning/structure",
        "run-w132-accepted",
    );
    let edited = maturation_proposal(
        "proposal-w132-edited",
        "/preferences/communication/concise",
        "run-w132-edited",
    );
    let rejected = maturation_proposal(
        "proposal-w132-rejected",
        "/preferences/work_style/deep_work",
        "run-w132-rejected",
    );
    for proposal in [&accepted, &edited, &rejected] {
        store
            .create_evidence(
                low_risk_evidence(
                    &proposal.affected_path,
                    proposal.run_id.as_deref().unwrap(),
                    0.84,
                )
                .with_linked_proposal(proposal.id.clone()),
            )
            .unwrap();
    }

    let accepted_report = record_maturation_proposal_outcome_evidence(
        &store,
        &accepted,
        MaturationProposalOutcome::Accepted,
    )
    .unwrap();
    let edited_report = record_maturation_proposal_outcome_evidence(
        &store,
        &edited,
        MaturationProposalOutcome::Edited,
    )
    .unwrap();
    let rejected_report = record_maturation_proposal_outcome_evidence(
        &store,
        &rejected,
        MaturationProposalOutcome::Rejected,
    )
    .unwrap();

    assert!(accepted_report.positive);
    assert!(!accepted_report.corrective);
    assert!(!accepted_report.negative);
    assert!(edited_report.corrective);
    assert!(!edited_report.negative);
    assert!(rejected_report.negative);
    assert!(rejected_report.opposing);

    let records = store
        .query(EvidenceQuery {
            evidence_type: Some(EvidenceType::ProposalOutcome),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(records.len(), 3);
    let positive = records
        .iter()
        .find(|record| record.linked_proposal_ids.contains(&accepted.id))
        .unwrap();
    let corrective = records
        .iter()
        .find(|record| record.linked_proposal_ids.contains(&edited.id))
        .unwrap();
    let negative = records
        .iter()
        .find(|record| record.linked_proposal_ids.contains(&rejected.id))
        .unwrap();
    assert_eq!(positive.run_metadata["outcomeEvidenceKind"], "positive");
    assert_eq!(corrective.run_metadata["outcomeEvidenceKind"], "corrective");
    assert_eq!(corrective.run_metadata["editedPayloadIncluded"], false);
    assert_eq!(negative.run_metadata["outcomeEvidenceKind"], "negative");
    assert_eq!(negative.opposing_refs, rejected_report.source_evidence_ids);
    assert_eq!(positive.linked_agent_run_ids, vec!["run-w132-accepted"]);
    assert_eq!(corrective.linked_agent_run_ids, vec!["run-w132-edited"]);
    assert_eq!(negative.linked_agent_run_ids, vec!["run-w132-rejected"]);
    assert_no_raw_content(
        &serde_json::to_string(&(accepted_report, edited_report, rejected_report, records))
            .unwrap(),
    );
}

#[test]
fn w132_high_risk_outcome_domain_is_metadata_safe_noop() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let mut proposal = maturation_proposal(
        "proposal-w132-high-risk",
        "/identity/values/core",
        "run-w132-high-risk",
    );
    proposal.risk_level = RiskLevel::Low;
    proposal.source_detail = Some("maturation:preference.communication".to_string());
    store
        .create_evidence(
            low_risk_evidence(
                "/preferences/communication/concise",
                proposal.run_id.as_deref().unwrap(),
                0.84,
            )
            .with_linked_proposal(proposal.id.clone()),
        )
        .unwrap();

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
    assert!(store
        .query(EvidenceQuery {
            evidence_type: Some(EvidenceType::ProposalOutcome),
            ..Default::default()
        })
        .unwrap()
        .is_empty());
}

#[test]
fn w133_suppresses_conflicted_decayed_and_cooldown_candidates_deterministically() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let now = Utc.with_ymd_and_hms(2026, 6, 4, 12, 0, 0).unwrap();
    let keep = store
        .create_evidence(low_risk_evidence(
            "/preferences/communication/concise",
            "run-keep",
            0.88,
        ))
        .unwrap();
    let old = store
        .create_evidence(low_risk_evidence(
            "/preferences/planning/old_structure",
            "run-old",
            0.74,
        ))
        .unwrap();
    let conflict = store
        .create_evidence(low_risk_evidence(
            "/preferences/work_style/deep_work",
            "run-conflict",
            0.82,
        ))
        .unwrap();
    store
        .contradict_evidence(&conflict.id, "manual-opposition", Some("metadata safe"))
        .unwrap();
    let rejected_proposal = maturation_proposal(
        "proposal-w133-rejected",
        "/state/energy_pattern/morning",
        "run-rejected",
    );
    let rejected_source = store
        .create_evidence(
            low_risk_evidence(
                &rejected_proposal.affected_path,
                "run-rejected-source",
                0.83,
            )
            .with_linked_proposal(rejected_proposal.id.clone()),
        )
        .unwrap();
    record_maturation_proposal_outcome_evidence(
        &store,
        &rejected_proposal,
        MaturationProposalOutcome::Rejected,
    )
    .unwrap();

    let mut records = store.query(EvidenceQuery::default()).unwrap();
    for record in &mut records {
        if record.id == old.id {
            record.last_observed_at = now - Duration::days(240);
        }
    }

    let first = evaluate_maturation_engine_v1(MaturationEngineV1Input::from_graph_input(
        EvidenceGraphInput::new(records.clone(), now),
    ));
    let repeated = evaluate_maturation_engine_v1(MaturationEngineV1Input::from_graph_input(
        EvidenceGraphInput::new(records, now),
    ));

    assert_eq!(first.candidates, repeated.candidates);
    assert_eq!(first.suppressed_candidates, repeated.suppressed_candidates);
    assert_eq!(first.candidate_count, 1);
    assert_eq!(first.candidates[0].support_evidence_ids, vec![keep.id]);
    assert!(first.suppressed_candidate_count >= 3);
    let reasons = first
        .suppressed_candidates
        .iter()
        .flat_map(|suppression| suppression.reasons.clone())
        .collect::<Vec<_>>();
    assert!(reasons.contains(&"supporting_evidence_decayed".to_string()));
    assert!(reasons.contains(&"cluster_conflict_active".to_string()));
    assert!(reasons.contains(&"rejected_similar_cooldown_active".to_string()));
    assert!(first.suppressed_candidates.iter().any(|suppression| {
        suppression.rejected_evidence_ids.len() == 1
            && suppression
                .support_evidence_ids
                .contains(&rejected_source.id)
    }));
    assert_no_raw_content(&serde_json::to_string(&first).unwrap());
}
