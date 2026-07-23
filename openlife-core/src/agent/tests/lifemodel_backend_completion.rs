use crate::agent::{
    bridge_life_signal_to_evidence, evaluate_lifemodel_backend_completion_readiness,
    extract_life_signals, EvidenceQuery, EvidenceSourceType, EvidenceStatus, EvidenceStore,
    EvidenceType, LifeDomain, LifeEventDraft, LifeEventPrivacyLevel, LifeEventSourceRef,
    LifeEventSourceType, LifeEventSourceVerification, LifeEventStore, LifeSignalBridgeInput,
    LifeSignalExtractorInput, LifeSignalPolarity, LifeSignalType, RiskLevel,
};
use crate::agent::{AgentRun, AgentRunStore};
use crate::llm::ChatMessage;
use crate::memory::MemoryStore;
use anyhow::{anyhow, Result};
use std::sync::{Arc, Barrier, LazyLock};

static VERIFIED_TEST_AGENT_RUN_OWNER: LazyLock<AgentRunStore> =
    LazyLock::new(|| AgentRunStore::new_in_memory().expect("test AgentRun owner"));

trait VerifiedTestLifeEventCreate {
    fn create_event(
        &self,
        draft: LifeEventDraft,
        source_refs: Vec<LifeEventSourceRef>,
        domain: LifeDomain,
        risk_level: RiskLevel,
        privacy_level: LifeEventPrivacyLevel,
    ) -> Result<crate::agent::LifeEvent>;
}

impl VerifiedTestLifeEventCreate for LifeEventStore {
    fn create_event(
        &self,
        draft: LifeEventDraft,
        source_refs: Vec<LifeEventSourceRef>,
        domain: LifeDomain,
        risk_level: RiskLevel,
        privacy_level: LifeEventPrivacyLevel,
    ) -> Result<crate::agent::LifeEvent> {
        let source = source_refs
            .first()
            .ok_or_else(|| anyhow!("life event blocked: source_lineage_missing"))?;
        if source.source_type != LifeEventSourceType::AgentRun || source_refs.len() != 1 {
            anyhow::bail!("test LifeEvent helper requires one canonical AgentRun source");
        }
        if VERIFIED_TEST_AGENT_RUN_OWNER
            .get_run(&source.source_id)?
            .is_none()
        {
            let mut run = AgentRun::new_chat_run("life-event-test-owner", "");
            run.id = source.source_id.clone();
            if let Err(error) = VERIFIED_TEST_AGENT_RUN_OWNER.create_run(&run) {
                if VERIFIED_TEST_AGENT_RUN_OWNER
                    .get_run(&source.source_id)?
                    .is_none()
                {
                    return Err(error);
                }
            }
        }
        self.bind_canonical_agent_run_store(&VERIFIED_TEST_AGENT_RUN_OWNER)?;
        VERIFIED_TEST_AGENT_RUN_OWNER.create_life_event_from_active_run(
            self,
            &source.source_id,
            source.source_detail.as_deref(),
            draft,
            domain,
            risk_level,
            privacy_level,
        )
    }
}

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
fn backend_completion_readiness_reports_w149_contract_freeze_complete() {
    let report = evaluate_lifemodel_backend_completion_readiness();

    assert!(report.report_ready);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.default_chat_isolated);
    assert_eq!(
        report.default_chat_selected_adapter_path,
        "main_chat_kernel"
    );
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
    assert!(report.next_required_schemas.is_empty());
    assert!(report.blockers.is_empty());
    assert!(report.master_spec_gate_blockers.is_empty());
    assert!(!report
        .next_required_schemas
        .contains(&"runtime_hs_packet_v2_guidance".to_string()));
    assert!(!report
        .next_required_schemas
        .contains(&"runtime_guidance_impact_read_model".to_string()));
    assert!(!report
        .next_required_schemas
        .contains(&"model_router_privacy_hs_hardening".to_string()));
    assert!(!report
        .next_required_schemas
        .contains(&"action_executor_hs_tool_governance".to_string()));
    assert!(!report
        .next_required_schemas
        .contains(&"ui_read_model_contracts".to_string()));
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
    assert!(!event.contains_raw_content);
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
fn life_event_summary_receipt_binds_equal_length_body_and_store_key() {
    let key = crate::agent::AgentRunReceiptKey::from_bytes([0x41; 32]).unwrap();
    let first_store = LifeEventStore::new_in_memory_with_receipt_key(key.clone()).unwrap();
    let second_store = LifeEventStore::new_in_memory_with_receipt_key(key.clone()).unwrap();
    let first = first_store
        .create_event(
            LifeEventDraft::new("receipt.body.binding", "AAAA"),
            vec![source_ref()],
            LifeDomain::Other,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();
    let second = second_store
        .create_event(
            LifeEventDraft::new("receipt.body.binding", "BBBB"),
            vec![source_ref()],
            LifeDomain::Other,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();
    assert_ne!(first.summary, second.summary);
    assert_ne!(first.payload_digest, second.payload_digest);

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("life-events-keyed.db");
    let persistent = LifeEventStore::new_with_receipt_key(&path, key).unwrap();
    persistent
        .create_event(
            LifeEventDraft::new("receipt.key.binding", "canonical body"),
            vec![source_ref()],
            LifeDomain::Other,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();
    drop(persistent);
    let wrong_key = crate::agent::AgentRunReceiptKey::from_bytes([0x42; 32]).unwrap();
    assert!(LifeEventStore::new_with_receipt_key(&path, wrong_key).is_err());
}

#[test]
fn life_event_requires_a_current_row_in_the_bound_canonical_agent_run_owner() {
    let owner = AgentRunStore::new_in_memory().unwrap();
    let event_store = LifeEventStore::new_in_memory().unwrap();
    event_store.bind_canonical_agent_run_store(&owner).unwrap();

    let error = owner
        .create_life_event_from_active_run(
            &event_store,
            "run-does-not-exist",
            Some("test:missing-owner-row"),
            LifeEventDraft::new("receipt.body.binding", "transient body")
                .with_source_run_id("run-does-not-exist"),
            LifeDomain::Other,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .expect_err("a receipt-shaped run id cannot substitute for a canonical owner row")
        .to_string();

    assert!(error.contains("canonical_agent_run_life_event_source_missing"));
    assert!(event_store.query_events(None, None).unwrap().is_empty());
}

#[test]
fn life_event_rejects_same_run_id_transplanted_from_a_different_agent_run_store() {
    let first_owner = AgentRunStore::new_in_memory().unwrap();
    let second_owner = AgentRunStore::new_in_memory().unwrap();
    let event_store = LifeEventStore::new_in_memory().unwrap();
    let run_id = "run-cross-store-life-event-transplant";
    for owner in [&first_owner, &second_owner] {
        let mut run = AgentRun::new_chat_run("cross-store-life-event", "");
        run.id = run_id.into();
        owner.create_run(&run).unwrap();
    }
    event_store
        .bind_canonical_agent_run_store(&first_owner)
        .unwrap();
    first_owner
        .create_life_event_from_active_run(
            &event_store,
            run_id,
            Some("cross_store_sentinel_test"),
            LifeEventDraft::new("receipt.body.binding", "first transient body")
                .with_source_run_id(run_id),
            LifeDomain::Other,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();

    let error = second_owner
        .create_life_event_from_active_run(
            &event_store,
            run_id,
            Some("cross_store_sentinel_test"),
            LifeEventDraft::new("receipt.key.binding", "second transient body")
                .with_source_run_id(run_id),
            LifeDomain::Other,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .expect_err("the LifeEvent store must remain bound to one exact canonical owner")
        .to_string();

    assert!(error.contains("life_event_canonical_source_owner_not_bound"));
    assert_eq!(event_store.query_events(None, None).unwrap().len(), 1);
    assert!(event_store
        .bind_canonical_agent_run_store(&second_owner)
        .unwrap_err()
        .to_string()
        .contains("identity_conflict"));
}

#[test]
fn current_life_event_rows_reject_non_exact_enums_and_noncanonical_json() {
    let key = crate::agent::AgentRunReceiptKey::from_bytes([0x43; 32]).unwrap();
    let corruptions = [
        "UPDATE life_events SET domain = ' low_energy_planning'",
        "UPDATE life_events SET risk_level = 'LOW'",
        "UPDATE life_events SET privacy_level = 'internal '",
        "UPDATE life_events SET source_type = 'Agent_run'",
        "UPDATE life_events SET source_refs_json = ' ' || source_refs_json",
    ];

    for (index, corruption) in corruptions.into_iter().enumerate() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(format!("strict-life-event-{index}.db"));
        let store = LifeEventStore::new_with_receipt_key(&path, key.clone()).unwrap();
        store
            .create_event(
                low_energy_event_draft(),
                vec![source_ref()],
                LifeDomain::LowEnergyPlanning,
                RiskLevel::Low,
                LifeEventPrivacyLevel::Internal,
            )
            .unwrap();
        drop(store);
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(corruption, [])
            .unwrap();

        let error = LifeEventStore::new_with_receipt_key(&path, key.clone())
            .err()
            .expect("corrupt current encoding must fail startup")
            .to_string();
        assert!(
            error.contains("enum_not_exact")
                || error.contains("noncanonical")
                || error.contains("invalid minimized LifeEvent receipt"),
            "unexpected rejection for {corruption}: {error}"
        );
    }
}

#[test]
fn agent_run_delete_and_life_event_create_are_linearized_by_the_canonical_owner() {
    let owner = Arc::new(AgentRunStore::new_in_memory().unwrap());
    let event_store = Arc::new(LifeEventStore::new_in_memory().unwrap());
    let run_id = "run-life-event-delete-create-race";
    let mut run = AgentRun::new_chat_run("life-event-delete-create-race", "");
    run.id = run_id.into();
    owner.create_run(&run).unwrap();
    event_store.bind_canonical_agent_run_store(&owner).unwrap();
    let barrier = Arc::new(Barrier::new(3));

    let create_owner = Arc::clone(&owner);
    let create_store = Arc::clone(&event_store);
    let create_barrier = Arc::clone(&barrier);
    let create = std::thread::spawn(move || {
        create_barrier.wait();
        create_owner.create_life_event_from_active_run(
            &create_store,
            run_id,
            Some("candidate:test:delete-create-race"),
            LifeEventDraft::new("receipt.body.binding", "transient race body")
                .with_source_run_id(run_id),
            LifeDomain::Other,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
    });
    let delete_owner = Arc::clone(&owner);
    let delete_barrier = Arc::clone(&barrier);
    let delete = std::thread::spawn(move || {
        delete_barrier.wait();
        delete_owner.delete_run_with_tombstone(run_id, Some("concurrent test delete"))
    });
    barrier.wait();

    let created = create.join().unwrap();
    let deleted = delete.join().unwrap();
    assert!(deleted.is_ok());
    match created {
        Ok(event) => {
            assert_eq!(event.source_id, run_id);
            assert_eq!(event_store.query_events(None, None).unwrap().len(), 1);
        }
        Err(error) => {
            assert!(
                error
                    .to_string()
                    .contains("canonical_agent_run_life_event_source_missing"),
                "unexpected create rejection: {error}"
            );
            assert!(event_store.query_events(None, None).unwrap().is_empty());
        }
    }
}

#[test]
fn canonical_source_tombstone_hides_life_event_projection_and_replays_idempotently() {
    let store = LifeEventStore::new_in_memory().unwrap();
    let event = store
        .create_event(
            low_energy_event_draft(),
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();
    assert_eq!(store.query_events(None, None).unwrap().len(), 1);

    assert_eq!(
        store
            .project_agent_run_canonical_head(
                "delete-event",
                1,
                "run-low-energy-1",
                Some("delete-tombstone"),
                &["delete-tombstone".into()],
            )
            .unwrap(),
        2
    );
    assert_eq!(
        store
            .project_agent_run_canonical_head(
                "delete-event",
                1,
                "run-low-energy-1",
                Some("delete-tombstone"),
                &["delete-tombstone".into()],
            )
            .unwrap(),
        0
    );
    assert!(store.query_events(None, None).unwrap().is_empty());
    assert!(store
        .create_event(
            low_energy_event_draft(),
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .is_err());

    store
        .project_agent_run_canonical_head(
            "restore-event",
            2,
            "run-low-energy-1",
            None,
            &["delete-tombstone".into()],
        )
        .unwrap();
    assert_eq!(store.query_events(None, None).unwrap()[0].id, event.id);
    assert!(store
        .project_agent_run_canonical_head(
            "late-delete-event",
            1,
            "run-low-energy-1",
            Some("delete-tombstone"),
            &["delete-tombstone".into()],
        )
        .unwrap_err()
        .to_string()
        .contains("ahead of canonical source"));
    assert_eq!(store.query_events(None, None).unwrap()[0].id, event.id);
}

#[test]
fn w125_life_event_store_replaces_raw_metadata_with_receipts_and_blocks_missing_lineage() {
    let store = LifeEventStore::new_in_memory().unwrap();
    let raw_event = low_energy_event_draft().with_metadata(serde_json::json!({
        "confidence": 0.88,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "rawPrompt": "raw user text should not be stored",
        "rawAssistantOutput": "assistant output should not be stored",
        "toolPayload": {"body": "tool output should not be stored"},
        "rawEvidencePreview": "evidence preview should become a keyed receipt"
    }));

    let minimized = store
        .create_event(
            raw_event,
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();
    let serialized = serde_json::to_string(&minimized).unwrap();
    for forbidden in [
        "raw user text should not be stored",
        "assistant output should not be stored",
        "tool output should not be stored",
        "evidence preview should become a keyed receipt",
    ] {
        assert!(!serialized.contains(forbidden), "{serialized}");
    }
    assert!(minimized.summary.starts_with("hmac-sha256:"));
    assert!(minimized.payload_digest.starts_with("hmac-sha256:"));
    assert!(minimized
        .source_refs
        .iter()
        .all(|source| source.digest.starts_with("hmac-sha256:")));
    assert!(minimized
        .metadata
        .get("rawEvidenceReceipt")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|receipt| receipt.starts_with("hmac-sha256:")));
    assert!(minimized.metadata.get("rawEvidencePreview").is_none());

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
    assert_eq!(store.query_events(None, None).unwrap().len(), 1);
}

#[test]
fn life_event_v2_default_denies_unregistered_types_refs_and_metadata() {
    const PRIVATE_METADATA: &str = "PRIVATE_LIFE_EVENT_METADATA_SENTINEL";
    let store = LifeEventStore::new_in_memory().unwrap();
    let mut metadata = serde_json::json!({
        "confidence": 0.86,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "unknownPrivateField": PRIVATE_METADATA,
    });
    metadata.as_object_mut().unwrap().insert(
        PRIVATE_METADATA.into(),
        serde_json::json!({"body": PRIVATE_METADATA}),
    );
    let event = store
        .create_event(
            low_energy_event_draft().with_metadata(metadata),
            vec![source_ref()],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap();
    let serialized = serde_json::to_string(&event).unwrap();
    assert!(!serialized.contains(PRIVATE_METADATA));
    assert!(event.metadata.get("unknownPrivateField").is_none());
    assert!(event
        .metadata
        .get("defaultDeniedMetadataReceipt")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value.starts_with("hmac-sha256:")));

    let unknown_type = store
        .create_event(
            LifeEventDraft::new("PRIVATE UNREGISTERED EVENT TYPE", "transient body"),
            vec![source_ref()],
            LifeDomain::Other,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap_err()
        .to_string();
    assert!(unknown_type.contains("event_type_unregistered"));

    let forged_source = LifeEventSourceRef::from_digest(
        LifeEventSourceType::AgentRun,
        "memory://PRIVATE-FORGED-LIFE-EVENT-URI",
        Some("plan_execute:weekly"),
        "sha256:event-source-digest",
    );
    let forged_source_error = store
        .create_event(
            low_energy_event_draft(),
            vec![forged_source],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap_err()
        .to_string();
    assert!(forged_source_error.contains("source_id_invalid"));

    let forged_detail = LifeEventSourceRef::from_digest(
        LifeEventSourceType::AgentRun,
        "run-low-energy-2",
        Some("https://PRIVATE-FORGED-SOURCE-DETAIL"),
        "sha256:event-source-digest",
    );
    let forged_detail_error = store
        .create_event(
            low_energy_event_draft(),
            vec![forged_detail],
            LifeDomain::LowEnergyPlanning,
            RiskLevel::Low,
            LifeEventPrivacyLevel::Internal,
        )
        .unwrap_err()
        .to_string();
    assert!(forged_detail_error.contains("source_detail_invalid"));
}

#[test]
fn life_event_v2_migration_physically_purges_legacy_free_text_fields() {
    const LEGACY_SENTINEL: &str = "LEGACY_LIFE_EVENT_FREE_TEXT_SENTINEL";
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("life-event-v1-physical-purge.db");
    let now = chrono::Utc::now();
    let source_refs = vec![LifeEventSourceRef::from_digest(
        LifeEventSourceType::AgentRun,
        format!("memory://{LEGACY_SENTINEL}"),
        Some(LEGACY_SENTINEL),
        format!("hmac-sha256:{}", "a".repeat(64)),
    )];
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE life_events (
                id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                source_refs_json TEXT NOT NULL,
                occurred_at TEXT NOT NULL,
                domain TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                privacy_level TEXT NOT NULL,
                summary TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                metadata_safe_summary_json TEXT,
                contains_raw_content INTEGER NOT NULL,
                dedupe_key TEXT NOT NULL,
                created_at TEXT NOT NULL,
                payload_minimized_version INTEGER NOT NULL DEFAULT 1
             );",
        )
        .unwrap();
        let mut legacy_metadata = serde_json::json!({"unknown": LEGACY_SENTINEL});
        legacy_metadata
            .as_object_mut()
            .unwrap()
            .insert(LEGACY_SENTINEL.into(), LEGACY_SENTINEL.into());
        conn.execute(
            "INSERT INTO life_events VALUES (
                'legacy-life-event', ?1, 'agent_run', ?2, ?3, ?4,
                'other', 'low', 'internal', ?1, ?5, ?6, NULL, 0,
                'legacy-life-event-dedupe', ?4, 1
             )",
            rusqlite::params![
                LEGACY_SENTINEL,
                format!("memory://{LEGACY_SENTINEL}"),
                serde_json::to_string(&source_refs).unwrap(),
                now.to_rfc3339(),
                format!("hmac-sha256:{}", "b".repeat(64)),
                legacy_metadata.to_string(),
            ],
        )
        .unwrap();
    }

    let key = crate::agent::AgentRunReceiptKey::from_bytes([0x71; 32]).unwrap();
    let migrated = LifeEventStore::new_with_receipt_key(&path, key).unwrap();
    let event = migrated.query_events(None, None).unwrap().remove(0);
    assert_eq!(event.event_type, "legacy.unregistered");
    assert!(event.source_id.starts_with("legacy-source:hmac-sha256:"));
    assert!(event.source_refs[0]
        .source_detail
        .as_deref()
        .is_some_and(|value| value.starts_with("source_detail:bytes=")));
    assert_eq!(
        event.source_refs[0].verification,
        LifeEventSourceVerification::LegacyUnverified
    );
    assert!(!event.has_canonical_source_authority());
    let extracted = extract_life_signals(LifeSignalExtractorInput::new(vec![event]));
    assert!(extracted.accepted_signals.is_empty());
    assert!(extracted.dropped_signals[0]
        .reasons
        .contains(&"source_lineage_legacy_unverified".to_string()));
    drop(migrated);

    let raw = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = raw
        .query_row(
            "SELECT payload_minimized_version FROM life_events
             WHERE id = 'legacy-life-event'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 3);
    drop(raw);
    for candidate in [
        path.clone(),
        std::path::PathBuf::from(format!("{}-wal", path.display())),
        std::path::PathBuf::from(format!("{}-shm", path.display())),
    ] {
        if candidate.exists() {
            let bytes = std::fs::read(&candidate).unwrap();
            assert!(!bytes
                .windows(LEGACY_SENTINEL.len())
                .any(|window| window == LEGACY_SENTINEL.as_bytes()));
        }
    }
}

#[test]
fn life_event_v2_pending_physical_purge_recovers_on_writable_reopen() {
    const CRASH_SENTINEL: &str = "LIFE_EVENT_V2_POST_COMMIT_CRASH_SENTINEL";
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("life-event-v2-crash-window.db");
    let key = crate::agent::AgentRunReceiptKey::from_bytes([0x72; 32]).unwrap();
    let store = LifeEventStore::new_with_receipt_key(&path, key.clone()).unwrap();
    drop(store);

    let raw = rusqlite::Connection::open(&path).unwrap();
    raw.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE retired_life_event_v1_pages(body TEXT NOT NULL);",
    )
    .unwrap();
    raw.execute(
        "INSERT INTO retired_life_event_v1_pages(body) VALUES (?1)",
        [CRASH_SENTINEL],
    )
    .unwrap();
    raw.execute_batch("DROP TABLE retired_life_event_v1_pages;")
        .unwrap();
    raw.execute(
        "UPDATE life_event_store_metadata SET value = 'pending'
         WHERE key = 'life_event_v2_physical_purge_complete'",
        [],
    )
    .unwrap();
    drop(raw);

    let read_only_error =
        LifeEventStore::open_read_only_existing_with_receipt_key(&path, key.clone())
            .err()
            .expect("LifeEvent read-only startup must fail while purge is pending")
            .to_string();
    assert!(read_only_error.contains("physical_purge_incomplete"));
    let recovered = LifeEventStore::new_with_receipt_key(&path, key).unwrap();
    drop(recovered);
    for candidate in [
        path.clone(),
        std::path::PathBuf::from(format!("{}-wal", path.display())),
        std::path::PathBuf::from(format!("{}-shm", path.display())),
    ] {
        if candidate.exists() {
            let bytes = std::fs::read(&candidate).unwrap();
            assert!(!bytes
                .windows(CRASH_SENTINEL.len())
                .any(|window| window == CRASH_SENTINEL.as_bytes()));
        }
    }
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
struct GovernedLifeEventFixture {
    memory: MemoryStore,
    owner: AgentRunStore,
    event_store: LifeEventStore,
    commit: crate::memory::CanonicalConversationMessageCommit,
    run: AgentRun,
    user_text: String,
    candidate_id: String,
}

impl GovernedLifeEventFixture {
    fn new(suffix: &str, user_text: &str) -> Self {
        let session_id = format!("life-event-authority-{suffix}");
        let memory = MemoryStore::new_in_memory().unwrap();
        memory
            .create_chat_session(&session_id, "Authority")
            .unwrap();
        let commit = memory
            .save_message_idempotent_with_proof(
                &session_id,
                &ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                },
                &format!("life-event-authority-message-{suffix}"),
            )
            .unwrap();
        let candidate_id =
            crate::agent::main_chat_memory_candidate::plan_main_chat_memory_routing(user_text)
                .life_event_candidate_ids
                .into_iter()
                .next()
                .expect("deterministic LifeEvent candidate");
        let owner = AgentRunStore::new_in_memory().unwrap();
        owner.bind_canonical_memory_store(&memory).unwrap();
        let mut run = AgentRun::new_chat_run(&session_id, user_text);
        run.id = format!("run-authority-{suffix}");
        run.input_ref = Some(commit.proof().canonical_ref().to_string());
        memory
            .create_agent_run_from_active_conversation_message(&owner, &run, commit.proof())
            .unwrap();
        let event_store = LifeEventStore::new_in_memory().unwrap();
        event_store.bind_canonical_agent_run_store(&owner).unwrap();
        Self {
            memory,
            owner,
            event_store,
            commit,
            run,
            user_text: user_text.into(),
            candidate_id,
        }
    }

    fn create(&self, operation_id: &str) -> Result<crate::agent::LifeEvent> {
        self.memory
            .create_low_risk_life_event_from_active_user_message(
                &self.owner,
                &self.event_store,
                self.commit.proof(),
                &self.candidate_id,
                &self.run.id,
                operation_id,
            )
    }

    fn policy_proof(
        &self,
    ) -> crate::agent::main_chat_memory_candidate::DeterministicLifeEventPolicyProof {
        crate::agent::main_chat_memory_candidate::issue_deterministic_life_event_policy_proof(
            self.commit.proof(),
            &self.user_text,
            &self.candidate_id,
        )
        .unwrap()
    }
}

#[test]
fn governed_life_event_core_seam_replays_one_operation_as_one_row() {
    let fixture = GovernedLifeEventFixture::new("replay", "今天午饭吃了牛肉面，下午犯困");
    let operation_id = uuid::Uuid::new_v4().to_string();
    let first = fixture.create(&operation_id).unwrap();
    let replay = fixture.create(&operation_id).unwrap();

    assert_eq!(first.id, replay.id);
    assert_eq!(
        fixture.event_store.query_events(None, None).unwrap().len(),
        1
    );
    assert_eq!(first.risk_level, RiskLevel::Low);
    assert_eq!(first.privacy_level, LifeEventPrivacyLevel::Internal);
    assert_eq!(
        first.source_refs[0]
            .canonical_owner
            .as_ref()
            .and_then(|owner| owner.canonical_revision),
        Some(1)
    );
}

#[test]
fn concurrent_governed_life_event_replay_commits_exactly_one_row() {
    let fixture = Arc::new(GovernedLifeEventFixture::new(
        "concurrent-replay",
        "今天午饭吃了牛肉面，下午犯困",
    ));
    let operation_id = uuid::Uuid::new_v4().to_string();
    let barrier = Arc::new(Barrier::new(3));

    let spawn_create = |fixture: Arc<GovernedLifeEventFixture>, barrier: Arc<Barrier>| {
        let operation_id = operation_id.clone();
        std::thread::spawn(move || {
            barrier.wait();
            fixture.create(&operation_id)
        })
    };
    let first = spawn_create(Arc::clone(&fixture), Arc::clone(&barrier));
    let second = spawn_create(Arc::clone(&fixture), Arc::clone(&barrier));
    barrier.wait();

    let first = first.join().unwrap().unwrap();
    let second = second.join().unwrap().unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(
        fixture.event_store.query_events(None, None).unwrap().len(),
        1
    );
}

#[test]
fn governed_life_event_operation_id_cannot_be_rebound_to_another_candidate() {
    let user_text = "今天午饭吃了牛肉面。昨晚睡了5小时";
    let fixture = GovernedLifeEventFixture::new("operation-conflict", user_text);
    let candidate_ids =
        crate::agent::main_chat_memory_candidate::plan_main_chat_memory_routing(user_text)
            .life_event_candidate_ids;
    assert!(
        candidate_ids.len() >= 2,
        "fixture must yield two exact candidates"
    );
    let operation_id = uuid::Uuid::new_v4().to_string();

    fixture
        .memory
        .create_low_risk_life_event_from_active_user_message(
            &fixture.owner,
            &fixture.event_store,
            fixture.commit.proof(),
            &candidate_ids[0],
            &fixture.run.id,
            &operation_id,
        )
        .unwrap();
    let error = fixture
        .memory
        .create_low_risk_life_event_from_active_user_message(
            &fixture.owner,
            &fixture.event_store,
            fixture.commit.proof(),
            &candidate_ids[1],
            &fixture.run.id,
            &operation_id,
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("life_event_create_operation_binding_conflict"));
    assert_eq!(
        fixture.event_store.query_events(None, None).unwrap().len(),
        1
    );
}

#[test]
fn governed_life_event_rejects_non_uuid_v4_operation_identity_before_write() {
    let fixture = GovernedLifeEventFixture::new("operation-id", "今天午饭吃了牛肉面，下午犯困");
    for invalid in [
        "not-a-uuid".to_string(),
        uuid::Uuid::nil().to_string(),
        uuid::Uuid::new_v4().to_string().to_uppercase(),
    ] {
        let error = fixture.create(&invalid).unwrap_err().to_string();
        assert!(error.contains("life_event_create_operation_id"));
    }
    assert!(fixture
        .event_store
        .query_events(None, None)
        .unwrap()
        .is_empty());
}

#[test]
fn unrelated_candidate_id_cannot_self_authorize_against_a_canonical_message() {
    let fixture = GovernedLifeEventFixture::new("candidate", "今天午饭吃了牛肉面，下午犯困");
    let unrelated = crate::agent::main_chat_memory_candidate::plan_main_chat_memory_routing(
        "今天睡了 5 小时，上午头痛",
    )
    .life_event_candidate_ids
    .into_iter()
    .next()
    .unwrap();
    let error = fixture
        .memory
        .create_low_risk_life_event_from_active_user_message(
            &fixture.owner,
            &fixture.event_store,
            fixture.commit.proof(),
            &unrelated,
            &fixture.run.id,
            &uuid::Uuid::new_v4().to_string(),
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("life_event_policy_candidate_missing"));
    assert!(fixture
        .event_store
        .query_events(None, None)
        .unwrap()
        .is_empty());
}

#[test]
fn canonical_user_message_cannot_authorize_an_unrelated_agent_run_input() {
    let fixture = GovernedLifeEventFixture::new("message-binding", "今天午饭吃了牛肉面，下午犯困");
    let second = fixture
        .memory
        .save_message_idempotent_with_proof(
            fixture.commit.proof().session_id(),
            &ChatMessage {
                role: "user".into(),
                content: fixture.user_text.clone(),
            },
            "life-event-authority-second-message",
        )
        .unwrap();
    let candidate_id =
        crate::agent::main_chat_memory_candidate::plan_main_chat_memory_routing(&fixture.user_text)
            .life_event_candidate_ids
            .into_iter()
            .next()
            .unwrap();

    let error = fixture
        .memory
        .create_low_risk_life_event_from_active_user_message(
            &fixture.owner,
            &fixture.event_store,
            second.proof(),
            &candidate_id,
            &fixture.run.id,
            &uuid::Uuid::new_v4().to_string(),
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("life_event_create_execution_message_ref_mismatch"));
    assert!(fixture
        .event_store
        .query_events(None, None)
        .unwrap()
        .is_empty());
}

#[test]
fn same_message_ref_from_an_unbound_memory_store_cannot_authorize_the_run() {
    let fixture = GovernedLifeEventFixture::new("memory-store", "今天午饭吃了牛肉面，下午犯困");
    let unbound_memory = MemoryStore::new_in_memory().unwrap();
    unbound_memory
        .create_chat_session(fixture.commit.proof().session_id(), "Unbound")
        .unwrap();
    let unbound_commit = unbound_memory
        .save_message_idempotent_with_proof(
            fixture.commit.proof().session_id(),
            &ChatMessage {
                role: "user".into(),
                content: fixture.user_text.clone(),
            },
            "life-event-unbound-memory-message",
        )
        .unwrap();
    assert_eq!(
        unbound_commit.proof().canonical_ref(),
        fixture.commit.proof().canonical_ref(),
        "counterfactual requires the same caller-visible canonical ref"
    );

    let error = unbound_memory
        .create_low_risk_life_event_from_active_user_message(
            &fixture.owner,
            &fixture.event_store,
            unbound_commit.proof(),
            &fixture.candidate_id,
            &fixture.run.id,
            &uuid::Uuid::new_v4().to_string(),
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("life_event_create_execution_message_store_mismatch"));
    assert!(fixture
        .event_store
        .query_events(None, None)
        .unwrap()
        .is_empty());
}

#[test]
fn agent_run_revision_and_stale_permit_resist_a_b_a_updates() {
    let fixture = GovernedLifeEventFixture::new("aba", "今天午饭吃了牛肉面，下午犯困");
    let execution = fixture
        .owner
        .issue_life_event_execution_proof_for_test(
            &fixture.run.id,
            fixture.commit.proof().session_id(),
        )
        .unwrap();
    let (stale, draft) =
        crate::agent::lifemodel_backend_completion::issue_life_event_create_permit(
            fixture.commit.proof(),
            fixture.policy_proof(),
            &execution,
            &uuid::Uuid::new_v4().to_string(),
        )
        .unwrap();
    assert_eq!(
        fixture
            .owner
            .canonical_revision_for_test(&fixture.run.id)
            .unwrap(),
        1
    );

    let mut state_b = fixture.owner.get_run(&fixture.run.id).unwrap().unwrap();
    state_b.step_count = 1;
    fixture.owner.update_run(&state_b).unwrap();
    let mut state_a = fixture.owner.get_run(&fixture.run.id).unwrap().unwrap();
    state_a.step_count = 0;
    fixture.owner.update_run(&state_a).unwrap();
    assert_eq!(
        fixture
            .owner
            .canonical_revision_for_test(&fixture.run.id)
            .unwrap(),
        3
    );

    let error = fixture
        .owner
        .commit_prepared_life_event_for_test(&fixture.event_store, stale, draft)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("life_event_create_permit_owner_revision_stale"),
        "{error}"
    );
    assert!(fixture
        .event_store
        .query_events(None, None)
        .unwrap()
        .is_empty());
}

#[test]
fn deterministic_life_event_policy_rejects_medium_and_high_risk_or_sensitivity() {
    use crate::agent::lifemodel_backend_completion::LifeEventSensitivity;
    let fixture = GovernedLifeEventFixture::new("risk", "今天午饭吃了牛肉面，下午犯困");
    let execution = fixture
        .owner
        .issue_life_event_execution_proof_for_test(
            &fixture.run.id,
            fixture.commit.proof().session_id(),
        )
        .unwrap();

    for (risk, sensitivity, expected) in [
        (
            RiskLevel::Medium,
            LifeEventSensitivity::Low,
            "risk_requires_review",
        ),
        (
            RiskLevel::High,
            LifeEventSensitivity::Low,
            "risk_requires_review",
        ),
        (
            RiskLevel::Low,
            LifeEventSensitivity::Medium,
            "sensitivity_requires_review",
        ),
        (
            RiskLevel::Low,
            LifeEventSensitivity::High,
            "sensitivity_requires_review",
        ),
    ] {
        let policy = fixture
            .policy_proof()
            .with_policy_for_test(risk, sensitivity);
        let error = crate::agent::lifemodel_backend_completion::issue_life_event_create_permit(
            fixture.commit.proof(),
            policy,
            &execution,
            &uuid::Uuid::new_v4().to_string(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(expected), "unexpected rejection: {error}");
    }
}

#[test]
fn stale_permit_stays_invalid_and_physical_recreate_cannot_bypass_tombstone() {
    let fixture = GovernedLifeEventFixture::new("generation", "今天午饭吃了牛肉面，下午犯困");
    let execution = fixture
        .owner
        .issue_life_event_execution_proof_for_test(
            &fixture.run.id,
            fixture.commit.proof().session_id(),
        )
        .unwrap();
    let (stale_after_restore, draft_after_restore) =
        crate::agent::lifemodel_backend_completion::issue_life_event_create_permit(
            fixture.commit.proof(),
            fixture.policy_proof(),
            &execution,
            &uuid::Uuid::new_v4().to_string(),
        )
        .unwrap();
    let (stale_after_recreate, draft_after_recreate) =
        crate::agent::lifemodel_backend_completion::issue_life_event_create_permit(
            fixture.commit.proof(),
            fixture.policy_proof(),
            &execution,
            &uuid::Uuid::new_v4().to_string(),
        )
        .unwrap();
    fixture
        .owner
        .delete_run_with_tombstone(&fixture.run.id, Some("generation test"))
        .unwrap();
    fixture
        .owner
        .restore_run_with_receipt(&fixture.run.id)
        .unwrap();
    assert_eq!(
        fixture
            .owner
            .canonical_revision_for_test(&fixture.run.id)
            .unwrap(),
        3
    );
    let error = fixture
        .owner
        .commit_prepared_life_event_for_test(
            &fixture.event_store,
            stale_after_restore,
            draft_after_restore,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("owner_revision_stale"));

    fixture
        .owner
        .delete_run_with_tombstone(&fixture.run.id, Some("physical generation test"))
        .unwrap();
    assert_eq!(fixture.owner.cleanup_old_deleted_runs(-1).unwrap(), 1);
    let revision_after_delete = fixture
        .owner
        .canonical_revision_for_test(&fixture.run.id)
        .unwrap();
    assert!(revision_after_delete >= 5);
    let recreate_error = fixture
        .memory
        .create_agent_run_from_active_conversation_message(
            &fixture.owner,
            &fixture.run,
            fixture.commit.proof(),
        )
        .unwrap_err()
        .to_string();
    assert!(
        recreate_error.contains("agent_run_create_canonical_tombstone_active"),
        "{recreate_error}"
    );
    let error = fixture
        .owner
        .commit_prepared_life_event_for_test(
            &fixture.event_store,
            stale_after_recreate,
            draft_after_recreate,
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("canonical_agent_run_life_event_source_missing"),
        "{error}"
    );
    assert!(fixture
        .event_store
        .query_events(None, None)
        .unwrap()
        .is_empty());
}
