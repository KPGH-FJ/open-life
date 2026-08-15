use crate::agent::{
    build_evidence_timeline, evaluate_evidence_graph, EvidenceDraft, EvidenceGraphInput,
    EvidenceGraphLinkKind, EvidencePolarity, EvidencePrivacyLevel, EvidenceQuery,
    EvidenceSourceRef, EvidenceSourceType, EvidenceStore, EvidenceType, RiskLevel,
};
use chrono::{Duration, TimeZone, Utc};

const PATH: &str = "/preferences/planning/low_energy_intensity";

fn source_ref(source_type: EvidenceSourceType, id: &str) -> EvidenceSourceRef {
    EvidenceSourceRef::from_digest(
        source_type,
        id,
        Some("metadata_safe_test"),
        format!("{id}-digest"),
    )
}

fn preference_draft(id: &str, confidence: f32) -> EvidenceDraft {
    EvidenceDraft::new(
        EvidenceType::Preference,
        PATH,
        confidence,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary("metadata safe planning preference")
    .with_source_ref(source_ref(EvidenceSourceType::WorkRun, id))
    .with_linked_work_run(id)
}

fn rejected_outcome_draft(source_evidence_id: &str, proposal_id: &str) -> EvidenceDraft {
    let mut draft = EvidenceDraft::new(
        EvidenceType::ProposalOutcome,
        PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary("metadata safe rejected proposal outcome")
    .with_source_ref(source_ref(EvidenceSourceType::Proposal, proposal_id))
    .with_linked_proposal(proposal_id)
    .with_linked_work_run("run-rejected-outcome");
    draft.opposing_refs = vec![source_evidence_id.to_string()];
    draft.run_metadata = serde_json::json!({
        "schema": "w75.maturationProposalOutcomeEvidence.v1",
        "outcome": "rejected",
        "proposalId": proposal_id,
        "negative": true,
        "opposing": true,
        "metadataSafe": true,
        "containsRawContent": false
    });
    draft
}

#[test]
fn w128_evidence_graph_clusters_support_and_opposition_with_source_weights() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let first = store
        .create_evidence(preference_draft("run-support-1", 0.82))
        .unwrap();
    let second = store
        .create_evidence(preference_draft("run-support-2", 0.76))
        .unwrap();
    let rejected = store
        .create_evidence(rejected_outcome_draft(&first.id, "proposal-rejected-1"))
        .unwrap();
    let now = Utc.with_ymd_and_hms(2026, 6, 4, 12, 0, 0).unwrap();
    let records = store.query(EvidenceQuery::default()).unwrap();

    let graph = evaluate_evidence_graph(EvidenceGraphInput::new(records, now));

    assert!(graph.graph_ready);
    assert!(graph.metadata_safe);
    assert!(!graph.contains_raw_content);
    assert_eq!(graph.record_count, 3);
    assert_eq!(graph.clusters.len(), 1);
    assert_eq!(graph.clusters[0].affected_path, PATH);
    assert!(graph.clusters[0].evidence_ids.contains(&first.id));
    assert!(graph.clusters[0].evidence_ids.contains(&second.id));
    assert!(graph.clusters[0].evidence_ids.contains(&rejected.id));
    assert!(graph.clusters[0].source_weight_total > 0.0);
    assert!(graph.clusters[0]
        .source_weights
        .iter()
        .any(|weight| weight.source_type == "work_run" && weight.ref_count == 2));
    assert!(graph
        .links
        .iter()
        .any(|link| link.kind == EvidenceGraphLinkKind::Support));
    assert!(graph.links.iter().any(|link| {
        link.kind == EvidenceGraphLinkKind::Opposition
            && link.from_evidence_id == rejected.id
            && link.to_evidence_id == first.id
    }));
    assert!(graph.timeline.items.iter().any(|item| {
        item.evidence_id == rejected.id && item.polarity == EvidencePolarity::Opposing
    }));
}

#[test]
fn w129_conflict_decay_and_rejected_similar_cooldown_use_injected_now() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let old_support = store
        .create_evidence(preference_draft("run-old-support", 0.90))
        .unwrap();
    let contradicted = store
        .contradict_evidence(
            &old_support.id,
            "ev-manual-opposition",
            Some("metadata safe reason"),
        )
        .unwrap();
    let rejected = store
        .create_evidence(rejected_outcome_draft(
            &old_support.id,
            "proposal-rejected-cooldown",
        ))
        .unwrap();
    let now = Utc.with_ymd_and_hms(2026, 6, 4, 12, 0, 0).unwrap();
    let mut records = store.query(EvidenceQuery::default()).unwrap();
    for record in &mut records {
        if record.id == contradicted.id {
            record.last_observed_at = now - Duration::days(150);
        }
        if record.id == rejected.id {
            record.last_observed_at = now - Duration::days(2);
        }
    }

    let graph = evaluate_evidence_graph(EvidenceGraphInput::new(records.clone(), now));
    let repeated = evaluate_evidence_graph(EvidenceGraphInput::new(records, now));

    assert_eq!(graph.timeline.items, repeated.timeline.items);
    let old_item = graph
        .timeline
        .items
        .iter()
        .find(|item| item.evidence_id == contradicted.id)
        .unwrap();
    assert!(old_item.decay_state.decayed);
    assert!(old_item.decay_state.effective_confidence < old_item.confidence);
    assert!(old_item.conflict_state.conflicted);
    assert!(old_item
        .conflict_state
        .reasons
        .contains(&"opposing_refs_present".to_string()));
    assert!(old_item
        .conflict_state
        .reasons
        .contains(&"evidence_status_contradicted".to_string()));
    assert!(old_item
        .conflict_state
        .reasons
        .contains(&"same_affected_path_cluster_opposition".to_string()));
    assert!(old_item.cooldown_state.active);
    assert_eq!(
        old_item.cooldown_state.reason.as_deref(),
        Some("recent_rejected_similar_proposal_outcome")
    );

    let cluster = &graph.clusters[0];
    assert!(cluster.conflict_state.conflicted);
    assert!(cluster.cooldown_state.active);
    assert!(cluster
        .cooldown_state
        .rejected_evidence_ids
        .contains(&rejected.id));
}

#[test]
fn w130_evidence_timeline_is_metadata_safe_and_exposes_required_fields() {
    let store = EvidenceStore::new_in_memory().unwrap();
    let mut draft = preference_draft("run-raw-source", 0.70).with_summary(
        "RAW_PROMPT_SECRET raw user text assistant output tool payload LifeModel raw",
    );
    draft.linked_proposal_ids = vec!["proposal-raw-source".into()];
    draft.run_metadata = serde_json::json!({
        "rawPrompt": "RAW_PROMPT_SECRET",
        "rawUserText": "RAW_USER_TEXT_SECRET",
        "rawAssistantOutput": "RAW_ASSISTANT_OUTPUT_SECRET",
        "toolPayload": "RAW_TOOL_PAYLOAD_SECRET",
        "lifeModelRawContent": "RAW_LIFEMODEL_SECRET",
        "metadataSafe": false
    });
    let record = store.create_evidence(draft).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 6, 4, 12, 0, 0).unwrap();
    let timeline = build_evidence_timeline(EvidenceGraphInput::new(
        store.query(EvidenceQuery::default()).unwrap(),
        now,
    ));

    assert!(timeline.metadata_safe);
    assert!(!timeline.contains_raw_content);
    assert_eq!(timeline.items.len(), 1);
    let item = &timeline.items[0];
    assert_eq!(item.evidence_id, record.id);
    assert_eq!(item.evidence_type, "preference");
    assert_eq!(item.affected_path, PATH);
    assert_eq!(item.status, "candidate");
    assert_eq!(item.risk_level, "low");
    assert_eq!(item.privacy_level, "internal");
    assert_eq!(item.polarity, EvidencePolarity::Supporting);
    assert_eq!(item.linked_proposal_ids, vec!["proposal-raw-source"]);
    assert_eq!(item.linked_work_run_ids, vec!["run-raw-source"]);
    assert!(item.cluster_id.starts_with("egc_"));
    assert_eq!(item.cluster_hash.len(), 64);
    assert_eq!(item.source_ref_count, 1);
    assert_eq!(item.support_link_count, 0);
    assert_eq!(item.opposition_link_count, 0);
    assert!(!item.conflict_state.conflicted);
    assert!(!item.cooldown_state.active);
    assert_eq!(item.decay_state.generated_at, now);
    assert_eq!(item.created_at, record.created_at);
    assert_eq!(item.updated_at, record.updated_at);
    assert_eq!(item.last_observed_at, record.last_observed_at);

    let serialized = serde_json::to_string(&timeline).unwrap();
    for forbidden in [
        "RAW_PROMPT_SECRET",
        "RAW_USER_TEXT_SECRET",
        "RAW_ASSISTANT_OUTPUT_SECRET",
        "RAW_TOOL_PAYLOAD_SECRET",
        "RAW_LIFEMODEL_SECRET",
        "raw user text",
        "assistant output",
        "tool payload",
        "LifeModel raw",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "timeline leaked raw marker {forbidden}: {serialized}"
        );
    }
}
