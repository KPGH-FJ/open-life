use crate::agent::{
    bridge_life_signal_to_evidence, evaluate_lifemodel_backend_completion_readiness,
    extract_life_signals, EvidenceQuery, EvidenceSourceType, EvidenceStatus, EvidenceStore,
    EvidenceType, LifeDomain, LifeEventDraft, LifeEventPrivacyLevel, LifeEventSourceRef,
    LifeEventSourceType, LifeEventStore, LifeSignalBridgeInput, LifeSignalExtractorInput,
    LifeSignalPolarity, LifeSignalType, RiskLevel,
};

fn source_ref() -> LifeEventSourceRef {
    LifeEventSourceRef::from_digest(
        LifeEventSourceType::AgentRun,
        "run-low-energy-1",
        Some("plan_execute:weekly"),
        "sha256:event-source-digest",
    )
}

fn low_energy_event_draft() -> LifeEventDraft {
    LifeEventDraft::new(
        "preference.planning.low_energy",
        "User prefers low-pressure planning with small next steps.",
    )
    .with_source_run_id("run-low-energy-1")
    .with_metadata(serde_json::json!({
        "confidence": 0.86,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "sourceDigest": "sha256:event-source-digest"
    }))
}

#[test]
fn w124_backend_completion_readiness_reports_current_goal1_contract_and_blockers() {
    let report = evaluate_lifemodel_backend_completion_readiness();

    assert!(report.report_ready);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.default_chat_isolated);
    assert_eq!(report.default_chat_selected_adapter_path, "legacy_stream");
    assert!(!report.runtime_execution_allowed);
    assert!(!report.model_execution_allowed);
    assert!(!report.tool_execution_allowed);
    assert!(!report.business_writes_allowed);
    assert!(!report.tauri_command_required);
    assert!(report.governance_readiness.proposal_store_present);
    assert!(report.governance_readiness.evidence_store_present);
    assert!(report.governance_readiness.evidence_graph_present);
    assert!(report.governance_readiness.policy_store_present);
    assert!(report.governance_readiness.heuristic_store_present);
    assert!(!report
        .next_required_schemas
        .contains(&"maturation_engine_v1".to_string()));
    assert!(!report
        .next_required_schemas
        .contains(&"accepted_guidance_lifecycle".to_string()));
    assert!(!report
        .next_required_schemas
        .contains(&"governed_materialized_lifemodel_view_provenance".to_string()));
    assert!(!report
        .next_required_schemas
        .contains(&"version_diff_rollback_read_model".to_string()));
    assert!(report
        .next_required_schemas
        .contains(&"runtime_hs_packet_v2_guidance".to_string()));
    assert!(!report
        .next_required_schemas
        .contains(&"evidence_graph_v1".to_string()));
    assert!(!report
        .blockers
        .contains(&"maturation_engine_v1_missing".to_string()));
    assert!(!report
        .blockers
        .contains(&"evidence_graph_v1_missing".to_string()));
    assert!(!report
        .blockers
        .contains(&"accepted_guidance_lifecycle_missing".to_string()));
    assert!(!report
        .blockers
        .contains(&"materialized_lifemodel_view_provenance_missing".to_string()));
    assert!(!report
        .blockers
        .contains(&"version_diff_read_model_missing".to_string()));
    assert!(!report.master_spec_gate_blockers.iter().any(|gate| {
        gate.blockers
            .contains(&"evidence_timeline_read_model_missing".to_string())
    }));
    assert!(!report.master_spec_gate_blockers.iter().any(|gate| {
        gate.blockers
            .contains(&"accepted_guidance_lifecycle_missing".to_string())
            || gate
                .blockers
                .contains(&"materialized_lifemodel_view_provenance_missing".to_string())
            || gate
                .blockers
                .contains(&"version_diff_read_model_missing".to_string())
    }));
}

#[test]
fn w125_life_event_store_accepts_metadata_safe_event_and_does_not_store_raw_content() {
    let store = LifeEventStore::new_in_memory().unwrap();
    let raw_prompt = "raw prompt: contact jane@example.com with SECRET-123";

    let event = store
        .create_event(
            low_energy_event_draft(),
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();

    assert!(event.id.starts_with("le_"));
    assert_eq!(event.domain, LifeDomain::LowEnergyPlanning);
    assert_eq!(event.risk_level, RiskLevel::Low);
    assert_eq!(event.source_refs.len(), 1);
    assert_eq!(event.source_refs[0].source_id, "run-low-energy-1");
    assert_eq!(event.contains_raw_content, false);
    assert_ne!(event.payload_digest, raw_prompt);
    assert!(event.metadata_safe_summary.is_some());
    assert!(event.dedupe_key.starts_with("life_event:"));

    let serialized = serde_json::to_string(&event).unwrap();
    for forbidden in ["raw prompt", "jane@example.com", "SECRET-123"] {
        assert!(
            !serialized.contains(forbidden),
            "LifeEvent leaked raw marker {forbidden}: {serialized}"
        );
    }

    let queried = store
        .query_events(Some(LifeDomain::LowEnergyPlanning), Some(10))
        .unwrap();
    assert_eq!(queried.len(), 1);
    assert_eq!(queried[0].id, event.id);
}

#[test]
fn w125_life_event_store_blocks_raw_content_in_metadata_and_missing_lineage() {
    let store = LifeEventStore::new_in_memory().unwrap();
    let raw_event = low_energy_event_draft().with_metadata(serde_json::json!({
        "confidence": 0.88,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "rawPrompt": "raw user text should not be stored",
        "rawAssistantOutput": "assistant output should not be stored",
        "toolPayload": {"body": "tool output should not be stored"}
    }));

    let blocked = store
        .create_event(
            raw_event,
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap_err()
        .to_string();

    assert!(blocked.contains("life event blocked"));
    assert!(blocked.contains("raw_content_present"));

    let missing_lineage = store
        .create_event(
            low_energy_event_draft(),
            Vec::new(),
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap_err()
        .to_string();

    assert!(missing_lineage.contains("source_lineage_missing"));
    assert_eq!(store.query_events(None, None).unwrap().len(), 0);
}

#[test]
fn w125_life_event_store_preserves_high_risk_privacy_metadata_without_promoting_truth() {
    let store = LifeEventStore::new_in_memory().unwrap();
    let event = store
        .create_event(
            LifeEventDraft::new(
                "identity.preference.corrected",
                "User corrected a long-term identity preference.",
            )
            .with_source_run_id("run-low-energy-1")
            .with_metadata(serde_json::json!({
                "sourceDigest": "sha256:event-source-digest",
                "proposalRequired": true
            })),
            vec![source_ref()],
            LifeDomain::Identity,
            RiskLevel::High,
            LifeEventPrivacyLevel::StrictlyLocal,
        )
        .unwrap();

    assert_eq!(event.domain, LifeDomain::Identity);
    assert_eq!(event.risk_level, RiskLevel::High);
    assert_eq!(event.privacy_level, LifeEventPrivacyLevel::StrictlyLocal);
    assert!(!event.contains_raw_content);
    assert!(event.metadata_safe_summary.is_some());

    let report = extract_life_signals(LifeSignalExtractorInput::new(vec![event]));

    assert!(report.accepted_signals.is_empty());
    assert_eq!(report.dropped_signals.len(), 1);
    assert!(report.dropped_signals[0]
        .reasons
        .contains(&"high_risk_event".to_string()));
    assert!(report.dropped_signals[0]
        .reasons
        .contains(&"event_privacy_not_allowed".to_string()));
}

#[test]
fn w126_deterministic_extractor_emits_only_low_risk_planning_signal() {
    let event_store = LifeEventStore::new_in_memory().unwrap();
    let event = event_store
        .create_event(
            low_energy_event_draft(),
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();

    let signals = extract_life_signals(LifeSignalExtractorInput::new(vec![event.clone()]));

    assert_eq!(signals.accepted_signals.len(), 1);
    assert!(signals.dropped_signals.is_empty());
    let signal = &signals.accepted_signals[0];
    assert!(signal.id.starts_with("ls_"));
    assert_eq!(
        signal.signal_type,
        LifeSignalType::PlanningIntensityPreference
    );
    assert_eq!(signal.domain, LifeDomain::LowEnergyPlanning);
    assert_eq!(signal.polarity, LifeSignalPolarity::Supporting);
    assert!(signal.confidence >= 0.75);
    assert_eq!(signal.source_event_ids, vec![event.id]);
    assert_eq!(signal.extractor_id, "deterministic.low_energy_planning");
    assert_eq!(signal.extractor_version, "1");
    assert!(signal.uncertainty_reasons.is_empty());
    assert!(signal.dedupe_key.starts_with("signal:low_energy_planning:"));

    let serialized = serde_json::to_string(signal).unwrap();
    for forbidden in [
        "raw prompt",
        "assistant output",
        "tool payload",
        "SECRET-123",
    ] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn w126_extractor_drops_high_risk_raw_or_unsupported_events() {
    let event_store = LifeEventStore::new_in_memory().unwrap();
    let supported = event_store
        .create_event(
            low_energy_event_draft(),
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();

    let mut high_risk = supported.clone();
    high_risk.id = "le_high_risk".into();
    high_risk.domain = LifeDomain::Identity;
    high_risk.risk_level = RiskLevel::High;

    let mut raw = supported.clone();
    raw.id = "le_raw".into();
    raw.contains_raw_content = true;

    let mut unsupported = supported;
    unsupported.id = "le_unsupported".into();
    unsupported.domain = LifeDomain::Health;

    let report = extract_life_signals(LifeSignalExtractorInput::new(vec![
        high_risk,
        raw,
        unsupported,
    ]));

    assert!(report.accepted_signals.is_empty());
    assert_eq!(report.dropped_signals.len(), 3);
    assert!(report
        .dropped_signals
        .iter()
        .any(|drop| drop.reasons.contains(&"high_risk_event".to_string())));
    assert!(report.dropped_signals.iter().any(|drop| drop
        .reasons
        .contains(&"event_contains_raw_content".to_string())));
    assert!(report
        .dropped_signals
        .iter()
        .any(|drop| drop.reasons.contains(&"unsupported_domain".to_string())));
}

#[test]
fn w127_bridge_writes_candidate_evidence_for_safe_signal_with_lineage_only() {
    let event_store = LifeEventStore::new_in_memory().unwrap();
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let event = event_store
        .create_event(
            low_energy_event_draft(),
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();
    let signal = extract_life_signals(LifeSignalExtractorInput::new(vec![event.clone()]))
        .accepted_signals
        .remove(0);

    let report = bridge_life_signal_to_evidence(
        LifeSignalBridgeInput::new(signal, vec![event]),
        &evidence_store,
    )
    .unwrap();

    assert!(report.bridged);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert_eq!(report.wrote_evidence_count, 1);
    assert_eq!(report.wrote_life_model_count, 0);
    assert_eq!(report.wrote_memory_count, 0);
    assert_eq!(report.wrote_heuristic_count, 0);
    assert_eq!(report.wrote_chat_message_count, 0);
    assert_eq!(report.wrote_agent_run_count, 0);
    assert_eq!(report.wrote_mcp_audit_count, 0);
    assert_eq!(report.wrote_external_count, 0);
    assert_eq!(report.evidence_ids.len(), 1);

    let records = evidence_store
        .query(EvidenceQuery {
            evidence_type: Some(EvidenceType::Preference),
            status: Some(EvidenceStatus::Candidate),
            limit: Some(10),
            ..EvidenceQuery::default()
        })
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, report.evidence_ids[0]);
    assert_eq!(records[0].source_refs.len(), 2);
    assert!(records[0]
        .source_refs
        .iter()
        .any(|source| source.source_type == EvidenceSourceType::RunMetadata));
    assert!(records[0]
        .source_refs
        .iter()
        .any(|source| source.source_type == EvidenceSourceType::AgentRun));
    assert_eq!(records[0].linked_agent_run_ids, vec!["run-low-energy-1"]);
}

#[test]
fn w127_bridge_fails_closed_without_writing_for_unsafe_signals() {
    let event_store = LifeEventStore::new_in_memory().unwrap();
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let event = event_store
        .create_event(
            low_energy_event_draft(),
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();
    let safe_signal = extract_life_signals(LifeSignalExtractorInput::new(vec![event.clone()]))
        .accepted_signals
        .remove(0);

    let mut low_confidence = safe_signal.clone();
    low_confidence.confidence = 0.2;
    let report = bridge_life_signal_to_evidence(
        LifeSignalBridgeInput::new(low_confidence, vec![event.clone()]),
        &evidence_store,
    )
    .unwrap();
    assert!(!report.bridged);
    assert_eq!(report.wrote_evidence_count, 0);
    assert!(report
        .blocking_reasons
        .contains(&"signal_confidence_too_low".to_string()));

    let mut high_risk = safe_signal.clone();
    high_risk.risk_level = RiskLevel::High;
    let report = bridge_life_signal_to_evidence(
        LifeSignalBridgeInput::new(high_risk, vec![event.clone()]),
        &evidence_store,
    )
    .unwrap();
    assert!(!report.bridged);
    assert!(report
        .blocking_reasons
        .contains(&"signal_risk_not_allowed".to_string()));

    let mut raw = safe_signal.clone();
    raw.contains_raw_content = true;
    let report = bridge_life_signal_to_evidence(
        LifeSignalBridgeInput::new(raw, vec![event.clone()]),
        &evidence_store,
    )
    .unwrap();
    assert!(!report.bridged);
    assert!(report
        .blocking_reasons
        .contains(&"signal_contains_raw_content".to_string()));

    let mut missing_lineage = safe_signal;
    missing_lineage.source_event_ids.clear();
    let report = bridge_life_signal_to_evidence(
        LifeSignalBridgeInput::new(missing_lineage, vec![event]),
        &evidence_store,
    )
    .unwrap();
    assert!(!report.bridged);
    assert!(report
        .blocking_reasons
        .contains(&"source_event_lineage_missing".to_string()));

    assert_eq!(
        evidence_store
            .query(EvidenceQuery::default())
            .unwrap()
            .len(),
        0
    );
}
