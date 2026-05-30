use crate::agent::{
    HeuristicQuery, HeuristicStore, LifeEventDraft, LifeModelMaturationService, MaturationInput,
    ProposalStatus, ProposalStore, ProposalType, RiskLevel, RuntimeOutput,
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
            "User wants to finish the LifeModel maturation MVP.",
            0.86,
        ),
        draft(
            "goal.short_term",
            "User wants to finish the LifeModel maturation MVP.",
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
