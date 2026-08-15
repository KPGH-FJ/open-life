use crate::agent::evidence_store::{
    EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceSourceRef, EvidenceSourceType,
    EvidenceStatus, EvidenceStore, EvidenceType,
};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};

fn test_draft() -> EvidenceDraft {
    EvidenceDraft::new(
        EvidenceType::Preference,
        "/preferences/communication",
        0.82,
        RiskLevel::Medium,
        EvidencePrivacyLevel::Sensitive,
    )
    .with_summary("User prefers concise collaboration summaries")
    .with_source_ref(EvidenceSourceRef::from_payload(
        EvidenceSourceType::ChatMessage,
        "message-1",
        Some("session:session-1"),
        "raw sensitive chat payload with private details",
    ))
}

#[test]
fn evidence_create_query_and_digest_do_not_require_raw_payload() {
    let store = EvidenceStore::new_in_memory().unwrap();

    let record = store.create_evidence(test_draft()).unwrap();

    assert!(record.id.starts_with("ev_"));
    assert_eq!(record.status, EvidenceStatus::Candidate);
    assert_eq!(record.support_count, 1);
    assert_eq!(record.source_refs.len(), 1);
    assert_eq!(record.source_refs[0].source_id, "message-1");
    assert_ne!(
        record.source_refs[0].digest,
        "raw sensitive chat payload with private details"
    );

    let serialized = serde_json::to_string(&record).unwrap();
    assert!(!serialized.contains("raw sensitive chat payload"));

    let queried = store
        .query(EvidenceQuery {
            affected_path: Some("/preferences/communication".into()),
            status: Some(EvidenceStatus::Candidate),
            limit: Some(10),
            ..EvidenceQuery::default()
        })
        .unwrap();

    assert_eq!(queried.len(), 1);
    assert_eq!(queried[0].id, record.id);
}

#[test]
fn evidence_lifecycle_weaken_archive_contradict_and_tombstone() {
    let store = EvidenceStore::new_in_memory().unwrap();

    let record = store.create_evidence(test_draft()).unwrap();
    let weakened = store
        .weaken_evidence(&record.id, 0.22, Some("new negative signal"))
        .unwrap();
    assert_eq!(weakened.status, EvidenceStatus::Weakened);
    assert!((weakened.confidence - 0.60).abs() < 0.001);

    let contradicted = store
        .contradict_evidence(&record.id, "evidence-opposes-1", Some("user rejected it"))
        .unwrap();
    assert_eq!(contradicted.status, EvidenceStatus::Contradicted);
    assert_eq!(contradicted.opposing_refs, vec!["evidence-opposes-1"]);

    let archived = store
        .archive_evidence(&record.id, Some("superseded by later evidence"))
        .unwrap();
    assert_eq!(archived.status, EvidenceStatus::Archived);

    let tombstone_reason = "forget private detail and prevent relearning";
    let tombstoned = store
        .tombstone_evidence(&record.id, tombstone_reason, Some("fact-digest-1"))
        .unwrap();
    assert_eq!(tombstoned.status, EvidenceStatus::Tombstoned);
    assert_eq!(
        tombstoned
            .tombstone
            .as_ref()
            .and_then(|meta| meta.prevent_relearning_digest.as_deref()),
        Some("fact-digest-1")
    );
    let serialized = serde_json::to_string(&tombstoned).unwrap();
    assert!(!serialized.contains(tombstone_reason));
}

#[test]
fn proposal_output_maps_to_candidate_evidence_without_raw_payload_or_fact_acceptance() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let raw_memory = "user private medication schedule";
    let mut proposal = AgentProposal::new(
        ProposalType::MemoryWrite,
        "/memory/explicit",
        serde_json::json!({
            "content": raw_memory,
            "source": "chat_explicit",
        }),
        &format!("User explicitly asked OpenLife to remember: {}", raw_memory),
        0.95,
        RiskLevel::Medium,
        ProposalSource::FeedbackEvolution,
    );
    proposal.source_detail = Some("session:session-1".into());

    let draft = EvidenceDraft::from_proposal_candidate(&proposal);
    let record = store.create_evidence(draft).unwrap();

    assert_eq!(record.status, EvidenceStatus::Candidate);
    assert_eq!(record.evidence_type, EvidenceType::Memory);
    assert_eq!(record.linked_proposal_ids, vec![proposal.id]);
    assert!(record
        .summary
        .as_deref()
        .unwrap_or_default()
        .contains("memory_write"));

    let serialized = serde_json::to_string(&record).unwrap();
    assert!(!serialized.contains(raw_memory));
    assert!(!serialized.contains("medication"));
}

#[test]
fn evidence_can_link_proposals_work_runs_and_run_metadata() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let record = store.create_evidence(test_draft()).unwrap();

    let linked = store.link_proposal(&record.id, "proposal-1").unwrap();
    assert_eq!(linked.linked_proposal_ids, vec!["proposal-1"]);

    let linked = store.link_work_run(&record.id, "run-1").unwrap();
    assert_eq!(linked.linked_work_run_ids, vec!["run-1"]);

    let linked = store
        .merge_run_metadata(
            &record.id,
            serde_json::json!({
                "selector": "chat_proposal_mapper",
                "run_id": "run-1"
            }),
        )
        .unwrap();
    assert_eq!(linked.run_metadata["selector"], "chat_proposal_mapper");
}
