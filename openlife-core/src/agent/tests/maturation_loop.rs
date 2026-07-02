use crate::agent::{
    EvidenceQuery, EvidenceStore, EvidenceType, GovernanceDecisionKind, HeuristicQuery,
    HeuristicStore, LifeEventDraft, LifeModelGovernor, LifeModelMaturationService, MaturationInput,
    MaturationService, ProposalStatus, ProposalStore, ProposalType, RiskLevel, RuntimeOutput,
};
use crate::life_model::LifeModel;

fn draft(event_type: &str, summary: &str, confidence: f64) -> LifeEventDraft {
    LifeEventDraft::new(event_type, summary)
        .with_source_run_id("run-maturation")
        .with_metadata(serde_json::json!({ "confidence": confidence }))
}

fn input_with_candidates(candidates: Vec<LifeEventDraft>) -> MaturationInput {
    MaturationInput {
        run_id: Some("run-maturation".into()),
        user_text: "raw user input must not be copied into proposal payloads".into(),
        assistant_output: "raw assistant output must not be copied into proposal payloads".into(),
        life_event_candidates: candidates,
        accepted_proposal_ids: Vec::new(),
        rejected_proposal_ids: Vec::new(),
    }
}

#[test]
fn life_event_draft_converts_to_lifemodel_proposal_candidate() {
    let service = LifeModelMaturationService::default();
    let output = service.mature(input_with_candidates(vec![draft(
        "preference.communication",
        "User prefers concise planning updates.",
        0.82,
    )]));

    assert_eq!(output.proposal_candidates.len(), 1);
    let candidate = &output.proposal_candidates[0];
    assert_eq!(candidate.proposal_type, ProposalType::PreferenceUpdate);
    assert_eq!(candidate.affected_path, "/preferences/communication_style");
    assert_eq!(candidate.risk_level, RiskLevel::Low);
    assert!(candidate.proposal_only);
    assert_eq!(candidate.source_run_id.as_deref(), Some("run-maturation"));
    assert_eq!(
        candidate
            .payload
            .get("summary")
            .and_then(|value| value.as_str()),
        Some("User prefers concise planning updates.")
    );
    assert!(!candidate
        .payload
        .to_string()
        .contains("raw user input must not be copied"));
}

#[test]
fn high_risk_identity_candidate_is_proposal_only() {
    let service = LifeModelMaturationService::default();
    let output = service.mature(input_with_candidates(vec![draft(
        "identity.values",
        "User says integrity is a core life value.",
        0.91,
    )]));

    assert_eq!(output.proposal_candidates.len(), 1);
    let candidate = &output.proposal_candidates[0];
    assert_eq!(candidate.proposal_type, ProposalType::LifeModelUpdate);
    assert_eq!(candidate.affected_path, "/identity/values");
    assert_eq!(candidate.risk_level, RiskLevel::High);
    assert!(candidate.proposal_only);
    assert!(candidate.confidence >= 0.9);
    assert!(!candidate.reason.trim().is_empty());

    let proposal = candidate.to_agent_proposal();
    assert_eq!(proposal.status, ProposalStatus::Pending);
    assert_eq!(proposal.risk_level, RiskLevel::High);
    assert_eq!(proposal.run_id.as_deref(), Some("run-maturation"));
}

#[test]
fn maturation_does_not_persist_to_lifemodel_or_hs_store() {
    let mut life_model = LifeModel::default();
    life_model.state.current_focus = "before".into();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();

    let service = LifeModelMaturationService::default();
    let output = service.mature(input_with_candidates(vec![draft(
        "state.current_focus",
        "User is currently focused on shipping the W4 maturation loop.",
        0.8,
    )]));

    assert_eq!(output.proposal_candidates.len(), 1);
    assert_eq!(life_model.state.current_focus, "before");
    assert!(heuristic_store
        .query(HeuristicQuery::default())
        .unwrap()
        .is_empty());
    assert!(proposal_store
        .list_pending_proposals(10)
        .unwrap()
        .is_empty());
}

#[test]
fn duplicate_candidates_are_deduplicated_within_run() {
    let service = LifeModelMaturationService::default();
    let output = service.mature(input_with_candidates(vec![
        draft(
            "goal.short_term",
            "User wants to finish the LifeModel maturation capability release.",
            0.86,
        ),
        draft(
            "goal.short_term",
            "User wants to finish the LifeModel maturation capability release.",
            0.86,
        ),
    ]));

    assert_eq!(output.proposal_candidates.len(), 1);
    assert_eq!(
        output.proposal_candidates[0].proposal_type,
        ProposalType::GoalUpdate
    );
}

#[test]
fn low_confidence_or_empty_candidates_are_dropped() {
    let service = LifeModelMaturationService::default();
    let output = service.mature(input_with_candidates(vec![
        draft("preference.communication", "", 0.92),
        draft("goal.short_term", "ok", 0.92),
        draft(
            "state.current_focus",
            "User is focused on a later follow-up.",
            0.2,
        ),
    ]));

    assert!(output.proposal_candidates.is_empty());
    assert_eq!(output.warnings.len(), 3);
}

#[test]
fn memory_candidate_uses_memory_proposal_not_lifemodel_patch() {
    let service = LifeModelMaturationService::default();
    let output = service.mature(input_with_candidates(vec![draft(
        "memory.write",
        "Remember that the user prefers proposal-first LifeModel updates.",
        0.88,
    )]));

    assert_eq!(output.proposal_candidates.len(), 1);
    let candidate = &output.proposal_candidates[0];
    assert_eq!(candidate.proposal_type, ProposalType::MemoryWrite);
    assert_eq!(candidate.affected_path, "memory.candidates");

    let proposal = candidate.to_agent_proposal();
    assert_eq!(proposal.proposal_type, ProposalType::MemoryWrite);
    assert_ne!(proposal.proposal_type, ProposalType::LifeModelUpdate);
}

#[test]
fn runtime_output_can_feed_maturation_without_changing_runtime_execution() {
    let runtime_output = RuntimeOutput {
        run_id: Some("run-runtime-output".into()),
        user_output: "Assistant answer remains normal runtime output.".into(),
        life_event_candidates: vec![LifeEventDraft::new(
            "goal.short_term",
            "User wants a callable maturation post-processing loop.",
        )
        .with_metadata(serde_json::json!({ "confidence": 0.84 }))],
        ..RuntimeOutput::default()
    };

    let maturation_input =
        MaturationInput::from_runtime_output("User asks for W4", &runtime_output, vec![], vec![]);
    let service = LifeModelMaturationService::default();
    let output = service.mature(maturation_input);

    assert_eq!(runtime_output.proposal_ids.len(), 0);
    assert_eq!(runtime_output.life_event_candidates.len(), 1);
    assert_eq!(output.proposal_candidates.len(), 1);
    assert_eq!(
        output.proposal_candidates[0].source_run_id.as_deref(),
        Some("run-runtime-output")
    );
}

#[test]
fn v1_runtime_output_matures_candidates_into_evidence_and_proposals() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let service = MaturationService::default();
    let runtime_output = RuntimeOutput {
        run_id: Some("run-v1".into()),
        user_output: "normal chat answer stays unchanged".into(),
        life_event_candidates: vec![LifeEventDraft::new(
            "preference.communication",
            "User prefers concise maturation status updates.",
        )
        .with_metadata(serde_json::json!({ "confidence": 0.86 }))],
        ..RuntimeOutput::default()
    };

    let report = service
        .mature_runtime_output(&runtime_output, &evidence_store, &proposal_store)
        .unwrap();

    assert_eq!(report.source_run_id.as_deref(), Some("run-v1"));
    assert_eq!(report.candidate_count, 1);
    assert_eq!(report.evidence_ids.len(), 1);
    assert_eq!(report.proposal_ids.len(), 1);
    assert_eq!(report.governance_summary.proposal_only_count, 1);

    let proposal = proposal_store
        .get_proposal(&report.proposal_ids[0])
        .unwrap()
        .unwrap();
    assert_eq!(proposal.proposal_type, ProposalType::PreferenceUpdate);
    assert_eq!(proposal.run_id.as_deref(), Some("run-v1"));
    assert_eq!(proposal.status, ProposalStatus::Pending);

    let evidence = evidence_store
        .get_evidence(&report.evidence_ids[0])
        .unwrap()
        .unwrap();
    assert_eq!(evidence.evidence_type, EvidenceType::Preference);
    assert_eq!(evidence.linked_proposal_ids, vec![proposal.id]);
    assert_eq!(evidence.linked_agent_run_ids, vec!["run-v1"]);
    assert_eq!(
        evidence.run_metadata["sourceRunId"],
        serde_json::json!("run-v1")
    );
    assert!(evidence.run_metadata["candidateDigest"].is_string());
}

#[test]
fn v1_high_risk_lifemodel_candidate_is_pending_proposal_only() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let mut life_model = LifeModel::default();
    life_model.identity.name = "before".into();
    let service = MaturationService::default();
    let runtime_output = RuntimeOutput {
        run_id: Some("run-high-risk".into()),
        life_event_candidates: vec![LifeEventDraft::new(
            "identity.values",
            "User says independence is a core life value.",
        )
        .with_metadata(serde_json::json!({ "confidence": 0.93 }))],
        ..RuntimeOutput::default()
    };

    let report = service
        .mature_runtime_output(&runtime_output, &evidence_store, &proposal_store)
        .unwrap();

    assert_eq!(life_model.identity.name, "before");
    assert_eq!(report.proposal_ids.len(), 1);
    assert_eq!(report.governance_summary.confirm_required_count, 1);
    assert_eq!(
        report.governance_summary.decisions[0].decision_kind,
        GovernanceDecisionKind::RequireConfirmation
    );

    let proposal = proposal_store
        .get_proposal(&report.proposal_ids[0])
        .unwrap()
        .unwrap();
    assert_eq!(proposal.proposal_type, ProposalType::LifeModelUpdate);
    assert_eq!(proposal.risk_level, RiskLevel::High);
    assert_eq!(proposal.status, ProposalStatus::Pending);
}

#[test]
fn v1_memory_candidate_creates_memory_write_proposal() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let runtime_output = RuntimeOutput {
        run_id: Some("run-memory".into()),
        life_event_candidates: vec![LifeEventDraft::new(
            "memory.write",
            "Remember that the user prefers proposal-first maturation.",
        )
        .with_metadata(serde_json::json!({ "confidence": 0.9 }))],
        ..RuntimeOutput::default()
    };

    let report = MaturationService::default()
        .mature_runtime_output(&runtime_output, &evidence_store, &proposal_store)
        .unwrap();

    let proposal = proposal_store
        .get_proposal(&report.proposal_ids[0])
        .unwrap()
        .unwrap();
    assert_eq!(proposal.proposal_type, ProposalType::MemoryWrite);
    assert_ne!(proposal.proposal_type, ProposalType::LifeModelUpdate);
    assert_eq!(proposal.affected_path, "memory.candidates");
}

#[test]
fn v1_drop_reasons_are_structured_for_low_confidence_duplicate_and_empty_candidates() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let runtime_output = RuntimeOutput {
        run_id: Some("run-drops".into()),
        life_event_candidates: vec![
            LifeEventDraft::new("preference.communication", "")
                .with_metadata(serde_json::json!({ "confidence": 0.91 })),
            LifeEventDraft::new(
                "state.current_focus",
                "User is focused on a later follow-up.",
            )
            .with_metadata(serde_json::json!({ "confidence": 0.2 })),
            LifeEventDraft::new("goal.short_term", "User wants a V1 maturation service.")
                .with_metadata(serde_json::json!({ "confidence": 0.88 })),
            LifeEventDraft::new("goal.short_term", "User wants a V1 maturation service.")
                .with_metadata(serde_json::json!({ "confidence": 0.88 })),
        ],
        ..RuntimeOutput::default()
    };

    let report = MaturationService::default()
        .mature_runtime_output(&runtime_output, &evidence_store, &proposal_store)
        .unwrap();

    assert_eq!(report.candidate_count, 4);
    assert_eq!(report.proposal_ids.len(), 1);
    assert_eq!(report.dropped_reasons.len(), 3);
    let codes: Vec<&str> = report
        .dropped_reasons
        .iter()
        .map(|reason| reason.reason_code.as_str())
        .collect();
    assert!(codes.contains(&"empty_candidate"));
    assert!(codes.contains(&"low_confidence"));
    assert!(codes.contains(&"duplicate_candidate"));
    assert!(report
        .dropped_reasons
        .iter()
        .all(|reason| reason.candidate_digest.len() == 64));
}

#[test]
fn v1_evidence_and_audit_are_metadata_safe() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let raw_prompt = "raw prompt: email alice@example.com and paste file body SECRET-123";
    let runtime_output = RuntimeOutput {
        run_id: Some("run-safe".into()),
        user_output: raw_prompt.into(),
        life_event_candidates: vec![LifeEventDraft::new(
            "memory.write",
            "Remember alice@example.com and raw memory context SECRET-123.",
        )
        .with_metadata(serde_json::json!({
            "confidence": 0.92,
            "rawPrompt": raw_prompt,
            "rawMemoryContext": "private memory context SECRET-123"
        }))],
        ..RuntimeOutput::default()
    };

    let report = MaturationService::default()
        .mature_runtime_output(&runtime_output, &evidence_store, &proposal_store)
        .unwrap();

    let evidence = evidence_store
        .query(EvidenceQuery {
            linked_agent_run_id: Some("run-safe".into()),
            limit: Some(10),
            ..EvidenceQuery::default()
        })
        .unwrap();
    assert_eq!(evidence.len(), 1);

    let serialized_evidence = serde_json::to_string(&evidence).unwrap();
    let serialized_report = serde_json::to_string(&report).unwrap();
    for forbidden in [
        "alice@example.com",
        "SECRET-123",
        "raw prompt",
        "raw memory context",
        "private memory context",
    ] {
        assert!(!serialized_evidence.contains(forbidden));
        assert!(!serialized_report.contains(forbidden));
    }
}

#[test]
fn v1_governor_blocked_candidate_creates_audit_without_proposal() {
    let evidence_store = EvidenceStore::new_in_memory().unwrap();
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let service = MaturationService::with_governor(LifeModelGovernor::default());
    let runtime_output = RuntimeOutput {
        run_id: Some("run-blocked".into()),
        life_event_candidates: vec![LifeEventDraft::new(
            "preference.communication",
            "User prefers concise updates.",
        )
        .with_metadata(serde_json::json!({
            "confidence": 0.86,
            "proposal_only": false
        }))],
        ..RuntimeOutput::default()
    };

    let report = service
        .mature_runtime_output(&runtime_output, &evidence_store, &proposal_store)
        .unwrap();

    assert!(report.proposal_ids.is_empty());
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
    assert_eq!(report.evidence_ids.len(), 1);
    assert_eq!(report.governance_summary.blocked_count, 1);
    assert_eq!(
        report.governance_summary.decisions[0].decision_kind,
        GovernanceDecisionKind::Block
    );
    assert!(!report.governance_summary.decisions[0].proposal_only);
}
