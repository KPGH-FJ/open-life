use crate::agent::{
    AgentRunStore, EvidenceQuery, EvidenceStore, HeuristicQuery, HeuristicStore, LifeEventDraft,
    LifeModelMaturationNonDefaultInvocationInput, ProposalStatus, ProposalStore, RuntimeOutput,
};
use crate::life_model::LifeModel;
use crate::memory::MemoryStore;

fn clean_candidate() -> LifeEventDraft {
    LifeEventDraft::new(
        "preference.planning.low_energy",
        "User prefers low-energy planning with low-pressure next steps.",
    )
    .with_source_run_id("run-w74-clean")
    .with_metadata(serde_json::json!({
        "confidence": 0.88,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "sourceDigest": "sha256:w74-clean-source",
    }))
}

fn runtime_output_for(candidate: LifeEventDraft) -> RuntimeOutput {
    RuntimeOutput {
        run_id: candidate.source_run_id.clone(),
        user_output: "ordinary assistant output must remain outside W74 report".into(),
        life_event_candidates: vec![candidate],
        ..RuntimeOutput::default()
    }
}

fn evidence_count(store: &EvidenceStore) -> usize {
    store
        .query(EvidenceQuery {
            limit: Some(100),
            ..EvidenceQuery::default()
        })
        .unwrap()
        .len()
}

#[test]
fn lifemodel_maturation_non_default_invocation_clean_candidate_writes_evidence_and_pending_proposal_only(
) {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();

    let report = LifeModelMaturationNonDefaultInvocationInput::for_runtime_output(
        runtime_output_for(clean_candidate()),
    )
    .run(&evidence_store, &proposal_store)
    .unwrap();

    assert!(report.invocation_ready);
    assert!(report.readiness_report.ready);
    assert!(report.non_default_invocation);
    assert!(report.default_chat_unchanged);
    assert!(report.ordinary_chat_entrypoint_unchanged);
    assert_eq!(report.source_run_id.as_deref(), Some("run-w74-clean"));
    assert_eq!(report.wrote_evidence_count, 1);
    assert_eq!(report.wrote_proposal_count, 1);
    assert_eq!(report.evidence_ids.len(), 1);
    assert_eq!(report.proposal_ids.len(), 1);
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
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.blocking_reasons.is_empty());

    let proposal = proposal_store
        .get_proposal(&report.proposal_ids[0])
        .unwrap()
        .unwrap();
    assert_eq!(proposal.status, ProposalStatus::Pending);
    assert_eq!(proposal.run_id.as_deref(), Some("run-w74-clean"));

    let evidence = evidence_store
        .get_evidence(&report.evidence_ids[0])
        .unwrap()
        .unwrap();
    assert_eq!(evidence.linked_proposal_ids, vec![proposal.id]);
    assert_eq!(evidence.linked_agent_run_ids, vec!["run-w74-clean"]);
}

#[test]
fn lifemodel_maturation_non_default_invocation_blocked_readiness_writes_no_evidence_or_proposal() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let candidate = clean_candidate().with_metadata(serde_json::json!({
        "confidence": 0.2,
        "proposal_only": true,
        "domain": "low_energy_planning",
    }));

    let report = LifeModelMaturationNonDefaultInvocationInput::for_runtime_output(
        runtime_output_for(candidate),
    )
    .run(&evidence_store, &proposal_store)
    .unwrap();

    assert!(!report.invocation_ready);
    assert!(!report.readiness_report.ready);
    assert!(report
        .blocking_reasons
        .contains(&"candidate_confidence_too_low".to_string()));
    assert_eq!(report.wrote_evidence_count, 0);
    assert_eq!(report.wrote_proposal_count, 0);
    assert!(report.evidence_ids.is_empty());
    assert!(report.proposal_ids.is_empty());
    assert_eq!(evidence_count(&evidence_store), 0);
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn lifemodel_maturation_non_default_invocation_raw_metadata_writes_no_stores() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let candidate = clean_candidate().with_metadata(serde_json::json!({
        "confidence": 0.91,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "rawPrompt": "raw prompt jane@example.com",
        "rawAssistantOutput": "assistant output SECRET-123",
        "rawMemoryContext": "memory context private body",
        "toolPayload": "tool payload sk-test-secret"
    }));

    let report = LifeModelMaturationNonDefaultInvocationInput::for_runtime_output(
        runtime_output_for(candidate),
    )
    .run(&evidence_store, &proposal_store)
    .unwrap();

    assert!(!report.invocation_ready);
    assert!(report.contains_raw_content);
    assert!(report
        .blocking_reasons
        .contains(&"candidate_metadata_contains_raw_content".to_string()));
    assert_eq!(evidence_count(&evidence_store), 0);
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn lifemodel_maturation_non_default_invocation_unsupported_domain_writes_no_stores() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let candidate = LifeEventDraft::new(
        "identity.values",
        "User says independence is a core life value.",
    )
    .with_source_run_id("run-w74-identity")
    .with_metadata(serde_json::json!({
        "confidence": 0.9,
        "proposal_only": true,
    }));

    let report = LifeModelMaturationNonDefaultInvocationInput::for_runtime_output(
        runtime_output_for(candidate),
    )
    .run(&evidence_store, &proposal_store)
    .unwrap();

    assert!(!report.invocation_ready);
    assert!(report
        .blocking_reasons
        .contains(&"candidate_type_outside_low_energy_planning_domain".to_string()));
    assert_eq!(evidence_count(&evidence_store), 0);
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn lifemodel_maturation_non_default_invocation_proposal_only_false_writes_no_stores() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let candidate = clean_candidate().with_metadata(serde_json::json!({
        "confidence": 0.9,
        "proposal_only": false,
        "domain": "low_energy_planning",
    }));

    let report = LifeModelMaturationNonDefaultInvocationInput::for_runtime_output(
        runtime_output_for(candidate),
    )
    .run(&evidence_store, &proposal_store)
    .unwrap();

    assert!(!report.invocation_ready);
    assert!(report
        .blocking_reasons
        .contains(&"proposal_only_false".to_string()));
    assert_eq!(evidence_count(&evidence_store), 0);
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn lifemodel_maturation_non_default_invocation_candidate_without_source_run_id_writes_no_stores() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let candidate = LifeEventDraft::new(
        "preference.planning.low_energy",
        "User prefers low-energy planning with low-pressure next steps.",
    )
    .with_metadata(serde_json::json!({
        "confidence": 0.88,
        "proposal_only": true,
        "domain": "low_energy_planning",
    }));

    let report = LifeModelMaturationNonDefaultInvocationInput::for_runtime_output(
        runtime_output_for(candidate),
    )
    .run(&evidence_store, &proposal_store)
    .unwrap();

    assert!(!report.invocation_ready);
    assert!(report
        .blocking_reasons
        .contains(&"source_lineage_missing".to_string()));
    assert_eq!(evidence_count(&evidence_store), 0);
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn lifemodel_maturation_non_default_invocation_side_effect_count_allows_only_evidence_and_proposals(
) {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let memory_store = MemoryStore::new_in_memory().unwrap();
    let agent_run_store = AgentRunStore::new_in_memory().unwrap();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    let mut life_model = LifeModel::default();
    life_model.state.current_focus = "before".into();

    let report = LifeModelMaturationNonDefaultInvocationInput::for_runtime_output(
        runtime_output_for(clean_candidate()),
    )
    .run(&evidence_store, &proposal_store)
    .unwrap();

    assert_eq!(report.wrote_evidence_count, 1);
    assert_eq!(report.wrote_proposal_count, 1);
    assert_eq!(report.wrote_life_model_count, 0);
    assert_eq!(report.wrote_memory_count, 0);
    assert_eq!(report.wrote_heuristic_count, 0);
    assert_eq!(report.wrote_chat_message_count, 0);
    assert_eq!(report.wrote_agent_run_count, 0);
    assert_eq!(report.wrote_mcp_audit_count, 0);
    assert_eq!(report.wrote_external_count, 0);
    assert_eq!(life_model.state.current_focus, "before");
    assert!(memory_store.export_all_messages().unwrap().is_empty());
    assert_eq!(agent_run_store.run_count().unwrap(), 0);
    assert!(heuristic_store
        .query(HeuristicQuery::default())
        .unwrap()
        .is_empty());
}

#[test]
fn lifemodel_maturation_non_default_invocation_fails_closed_for_direct_writes_or_chat_migration() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let mut input = LifeModelMaturationNonDefaultInvocationInput::for_runtime_output(
        runtime_output_for(clean_candidate()),
    );
    input.require_direct_life_model_write = true;
    input.require_direct_memory_write = true;
    input.require_heuristic_activation = true;
    input.default_chat_selected_adapter_path = "controlled_adapter".into();
    input.ordinary_chat_auto_maturation_enabled = true;

    let report = input.run(&evidence_store, &proposal_store).unwrap();

    assert!(!report.invocation_ready);
    for reason in [
        "direct_lifemodel_write_required",
        "direct_memory_write_required",
        "heuristic_activation_required",
        "default_chat_route_migration_assumed",
        "ordinary_chat_auto_maturation_assumed",
    ] {
        assert!(
            report.blocking_reasons.contains(&reason.to_string()),
            "missing blocker {reason}"
        );
    }
    assert_eq!(evidence_count(&evidence_store), 0);
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn lifemodel_maturation_non_default_invocation_serialization_omits_raw_prompt_output_memory_tool_and_secrets(
) {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let candidate = clean_candidate().with_metadata(serde_json::json!({
        "confidence": 0.91,
        "proposal_only": true,
        "domain": "low_energy_planning",
        "rawPrompt": "raw prompt jane@example.com",
        "rawAssistantOutput": "assistant output SECRET-123",
        "rawMemoryContext": "memory context private body",
        "toolPayload": "tool payload sk-test-secret"
    }));

    let report = LifeModelMaturationNonDefaultInvocationInput::for_runtime_output(
        runtime_output_for(candidate),
    )
    .run(&evidence_store, &proposal_store)
    .unwrap();
    let serialized = serde_json::to_string(&report).unwrap();
    let debug_dump = format!("{report:?}");

    for forbidden in [
        "raw prompt",
        "assistant output",
        "memory context",
        "tool payload",
        "jane@example.com",
        "SECRET-123",
        "sk-test-secret",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "serialized invocation report leaked {forbidden}"
        );
        assert!(
            !debug_dump.contains(forbidden),
            "debug invocation report leaked {forbidden}"
        );
    }
}
