use super::terminal_owner_successor::{
    commit_attested_terminal_owner_successor_fixture_for_test,
    issue_turn_owner_writer_claim_fixture_for_test,
    stage_terminal_owner_bound_proposal_fixture_for_test,
    verify_attested_terminal_owner_successor_fixture_for_test,
    verify_terminal_owner_bound_proposal_fixture_for_test, TerminalOwnerKindV1,
    TerminalOwnerSuccessorCauseV1, TerminalOwnerSuccessorReceiptViewV1,
    TerminalOwnerSuccessorTestFactsV1,
};
use super::{AgentProposal, ProposalSource, ProposalStore, ProposalType, RiskLevel};

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn canonical_facts() -> TerminalOwnerSuccessorTestFactsV1 {
    TerminalOwnerSuccessorTestFactsV1::new_for_test(
        "task_session_store_v1",
        "018f3298-46ff-7f10-a2fb-83d4b75b2c79",
        "018f3298-46ff-7f10-a2fb-83d4b75b2c80",
        7,
        "018f3298-46ff-7f10-a2fb-83d4b75b2c81",
        TerminalOwnerKindV1::TaskSession,
        "018f3298-46ff-7f10-a2fb-83d4b75b2c80",
        TerminalOwnerSuccessorCauseV1::ProposalEffect,
        "018f3298-46ff-7f10-a2fb-83d4b75b2c82",
        41,
        digest('a'),
        42,
        digest('b'),
        "2026-07-13T07:00:00Z",
    )
}

// This freezes a public read contract, not a public authority-minting API.
// The cfg(test) issuer simulates two separate mechanical steps only:
//
// 1. EventStore issues a sealed writer claim binding the turn/epoch/final.
// 2. The canonical owner store persists its mutation and transition receipt in
//    that owner's single local transaction, including the claim binding and
//    attestation. EventStore later confirms only receipt ref + digest.
//
// It does not claim one transaction spans separate SQLite databases, and it
// must not copy canonical owner payload into TurnEventStore.
#[test]
fn attested_successor_receipt_validates_one_exact_monotonic_owner_transition() {
    let facts = canonical_facts();
    let claim =
        issue_turn_owner_writer_claim_fixture_for_test(facts.writer_claim_request_for_test())
            .expect("EventStore test seam issues one turn/epoch/final-bound writer claim");
    let issued = commit_attested_terminal_owner_successor_fixture_for_test(claim, facts.clone())
        .expect("the owner-store test seam commits one claim-bound transition receipt");
    let expected = facts.expectation_for_test();
    let view = verify_attested_terminal_owner_successor_fixture_for_test(
        issued.attested_bytes(),
        &expected,
    )
    .expect("the verifier revalidates the exact claim-bound owner transition");

    let _: &TerminalOwnerSuccessorReceiptViewV1 = &view;
    assert_eq!(view.schema_version(), "terminal_owner_successor_v1");
    assert_eq!(view.owner_store_id(), "task_session_store_v1");
    assert_eq!(view.operation_id(), facts.operation_id_for_test());
    assert_eq!(view.epoch_generation(), 7);
    assert_eq!(view.final_event_id(), facts.final_event_id_for_test());
    assert_eq!(view.owner_kind(), TerminalOwnerKindV1::TaskSession);
    assert_eq!(view.owner_id(), facts.operation_id_for_test());
    assert_eq!(view.cause(), TerminalOwnerSuccessorCauseV1::ProposalEffect);
    assert_eq!(view.cause_ref(), "018f3298-46ff-7f10-a2fb-83d4b75b2c82");
    assert_eq!(view.before_revision(), 41);
    assert_eq!(view.before_digest(), digest('a'));
    assert_eq!(view.after_revision(), 42);
    assert_eq!(view.after_digest(), digest('b'));
    assert!(!view.owner_receipt_ref().trim().is_empty());
    assert!(view.owner_receipt_digest().starts_with("sha256:"));
    serde_json::to_value(&view).expect("the public view is serializable for read models");
}

#[test]
fn successor_verifier_rejects_revision_digest_and_identity_mismatches() {
    let facts = canonical_facts();
    let claim =
        issue_turn_owner_writer_claim_fixture_for_test(facts.writer_claim_request_for_test())
            .expect("issue exact writer claim fixture");
    let issued = commit_attested_terminal_owner_successor_fixture_for_test(claim, facts.clone())
        .expect("commit exact attested owner-store fixture");
    let bytes = issued.attested_bytes();
    let expected = facts.expectation_for_test();

    for (label, counterfactual) in [
        (
            "same_prior_digest_wrong_prior_revision",
            expected.clone().with_before_revision_for_test(40),
        ),
        (
            "wrong_prior_digest",
            expected.clone().with_before_digest_for_test(digest('d')),
        ),
        (
            "same_digest_wrong_revision",
            expected.clone().with_after_revision_for_test(43),
        ),
        (
            "wrong_after_digest",
            expected.clone().with_after_digest_for_test(digest('c')),
        ),
        (
            "wrong_final_event",
            expected
                .clone()
                .with_final_event_id_for_test("018f3298-46ff-7f10-a2fb-83d4b75b2c91"),
        ),
        (
            "wrong_proposal",
            expected
                .clone()
                .with_cause_ref_for_test("018f3298-46ff-7f10-a2fb-83d4b75b2c92"),
        ),
        (
            "wrong_owner_kind",
            expected
                .clone()
                .with_owner_kind_for_test(TerminalOwnerKindV1::AgentRun),
        ),
    ] {
        assert!(
            verify_attested_terminal_owner_successor_fixture_for_test(bytes, &counterfactual)
                .is_err(),
            "counterfactual {label} must not validate the receipt"
        );
    }

    let non_monotonic = facts.clone().with_after_revision_for_test(41);
    let non_monotonic_claim = issue_turn_owner_writer_claim_fixture_for_test(
        non_monotonic.writer_claim_request_for_test(),
    )
    .expect("issue claim before testing owner-store monotonicity rejection");
    assert!(
        commit_attested_terminal_owner_successor_fixture_for_test(
            non_monotonic_claim,
            non_monotonic,
        )
        .is_err(),
        "a successor revision must be strictly greater than its prior revision"
    );

    let mut forged_bytes = bytes.to_vec();
    let last = forged_bytes
        .last_mut()
        .expect("attested receipt bytes are non-empty");
    *last ^= 1;
    assert!(
        verify_attested_terminal_owner_successor_fixture_for_test(&forged_bytes, &expected)
            .is_err(),
        "a forged public view or generic payload append cannot substitute for the owner-store attestation"
    );
}

#[test]
fn proposal_turn_origin_is_staged_typed_round_tripped_and_not_forged_from_free_text() {
    let store = ProposalStore::new_in_memory().expect("create isolated ProposalStore");
    let operation_id = "018f3298-46ff-7f10-a2fb-83d4b75b2ca0";
    let mut proposal = AgentProposal::new(
        ProposalType::LifeModelUpdate,
        "state.current_focus",
        serde_json::json!("typed origin target"),
        "D055 typed Proposal origin staging fixture",
        1.0,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    proposal.run_id = Some("018f3298-46ff-7f10-a2fb-83d4b75b2ca1".into());
    let staged =
        stage_terminal_owner_bound_proposal_fixture_for_test(&store, proposal, operation_id, 7)
            .expect("the test-only ReviewWorkflow staging seam signs one typed immutable origin");
    let binding = store
        .terminal_owner_origin_binding(staged.proposal_id())
        .expect("load the typed binding from ProposalStore")
        .expect("the staged Proposal has one typed origin binding");
    assert_eq!(binding.operation_id(), operation_id);
    assert_eq!(binding.epoch_generation(), 7);
    verify_terminal_owner_bound_proposal_fixture_for_test(
        &store,
        staged.proposal_id(),
        operation_id,
        7,
    )
    .expect("the exact ProposalStore round-trip revalidates the immutable origin");

    let mut forged = AgentProposal::new(
        ProposalType::LifeModelUpdate,
        "state.current_focus",
        serde_json::json!({
            "value": "forged origin",
            "originatingTaskSessionId": operation_id,
            "operationId": operation_id,
            "epochGeneration": 7,
        }),
        "untrusted caller-shaped origin must not authorize turn admission",
        1.0,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    forged.source_detail = Some(format!("main_chat_agent_task_session:{operation_id}"));
    let forged_id = forged.id.clone();
    store
        .create_proposal(&forged)
        .expect("persist the legacy/free-text counterfactual Proposal");
    assert!(
        store
            .terminal_owner_origin_binding(&forged_id)
            .expect("query forged Proposal binding")
            .is_none(),
        "source_detail or after JSON must not manufacture typed turn origin"
    );
    assert!(
        verify_terminal_owner_bound_proposal_fixture_for_test(&store, &forged_id, operation_id, 7,)
            .is_err(),
        "a free-text origin must not obtain turn admission or successor authority"
    );
}

#[test]
fn public_successor_view_has_no_public_raw_fact_issuer() {
    let source = include_str!("terminal_owner_successor.rs");
    let view_marker = "pub struct TerminalOwnerSuccessorReceiptViewV1";
    let view_offset = source
        .find(view_marker)
        .expect("production exposes the typed read-only successor receipt view");
    let derive_window = &source[view_offset.saturating_sub(320)..view_offset];
    let view_body_start = source[view_offset..]
        .find('{')
        .map(|offset| view_offset + offset + 1)
        .expect("public successor view has a struct body");
    let view_body_end = source[view_body_start..]
        .find('}')
        .map(|offset| view_body_start + offset)
        .expect("public successor view closes its struct body");
    let view_body = &source[view_body_start..view_body_end];
    assert!(
        !derive_window.contains("Deserialize"),
        "the public receipt view must not deserialize untrusted caller-shaped authority"
    );
    assert!(
        !view_body
            .lines()
            .any(|line| line.trim_start().starts_with("pub ")),
        "the public receipt view fields must stay private behind read-only getters"
    );
    assert!(
        !source.contains("impl TerminalOwnerSuccessorReceiptViewV1 {\n    pub fn new")
            && !source.contains("impl TerminalOwnerSuccessorReceiptViewV1 {\n    pub fn try_new"),
        "the public receipt view must not expose a raw-fact authority constructor"
    );
    assert!(
        !source.contains("source_detail")
            && !source.contains("originatingTaskSessionId")
            && !source.contains("originating_task_session_id"),
        "terminal owner admission must not parse free-text Proposal fields as authority"
    );
}
