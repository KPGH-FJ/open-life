use crate::main_chat_d051_test_support::{
    run_d051_runtime_case, D051RuntimeMode, D051RuntimeTestOutcome,
};
use serde_json::Value;

const FIXTURE: &str =
    include_str!("../../openlife-core/tests/fixtures/d051_structured_memory_evidence_cases.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("D051 structured-memory fixture must be valid JSON")
}

fn case(case_id: &str) -> Value {
    fixture()["cases"]
        .as_array()
        .expect("D051 cases array")
        .iter()
        .find(|candidate| candidate["id"] == case_id)
        .unwrap_or_else(|| panic!("missing frozen D051 runtime case: {case_id}"))
        .clone()
}

fn memory_status(outcome: &D051RuntimeTestOutcome) -> Option<&str> {
    outcome.terminal["finalDelivery"]["memoryEvidenceStatus"].as_str()
}

fn memory_reason(outcome: &D051RuntimeTestOutcome) -> Option<&str> {
    outcome.terminal["finalDelivery"]["memoryEvidenceReason"].as_str()
}

fn delivery_status(outcome: &D051RuntimeTestOutcome) -> Option<&str> {
    outcome.terminal["finalDelivery"]["status"].as_str()
}

fn receipt_memory_status(outcome: &D051RuntimeTestOutcome) -> Option<&str> {
    outcome.execution_receipt["memoryEvidenceStatus"].as_str()
}

fn receipt_memory_reason(outcome: &D051RuntimeTestOutcome) -> Option<&str> {
    outcome.execution_receipt["memoryEvidenceReason"].as_str()
}

#[tokio::test]
async fn d051_runtime_uses_the_existing_final_provider_request_without_a_third_call() {
    let case = case("positive_same_final_exact_observation");
    let outcome = run_d051_runtime_case(&fixture(), &case, D051RuntimeMode::Buffered)
        .await
        .expect("buffered D051 positive runtime case");

    assert_eq!(memory_status(&outcome), Some("proposal_staged"));
    assert_eq!(receipt_memory_status(&outcome), Some("proposal_staged"));
    assert_eq!(
        memory_reason(&outcome),
        Some("same_final_provider_evidence_admitted")
    );
    assert_eq!(
        receipt_memory_reason(&outcome),
        Some("same_final_provider_evidence_admitted")
    );
    assert_eq!(
        delivery_status(&outcome),
        Some("completed_with_pending_items")
    );
    assert_eq!(outcome.proposal_count, 1);
    assert_eq!(outcome.provider_request_count, 2);
    assert_eq!(outcome.final_provider_request_ordinal, Some(2));
    assert_eq!(outcome.exact_observation_manifest_matches, 1);
    assert!(outcome.final_provider_receipt_matches_manifest);
    assert!(outcome.canonical_answer_preserved);
    assert_eq!(outcome.late_proposal_count, 0);
}

#[tokio::test]
async fn d051_provider_extractor_or_parse_unavailable_is_partial_not_fake_completed() {
    for case_id in [
        "provider_unavailable",
        "extractor_unavailable",
        "extractor_parse_unavailable",
    ] {
        let case = case(case_id);
        let outcome = run_d051_runtime_case(&fixture(), &case, D051RuntimeMode::Buffered)
            .await
            .unwrap_or_else(|error| panic!("{case_id} runtime outcome: {error}"));

        assert_eq!(memory_status(&outcome), Some("unavailable"), "{case_id}");
        assert_eq!(
            receipt_memory_status(&outcome),
            Some("unavailable"),
            "{case_id}"
        );
        assert_eq!(
            memory_reason(&outcome),
            case["expectedReasonCode"].as_str(),
            "{case_id}"
        );
        assert_eq!(
            receipt_memory_reason(&outcome),
            case["expectedReasonCode"].as_str(),
            "{case_id}"
        );
        assert_eq!(
            delivery_status(&outcome),
            Some("completed_with_partial_evidence"),
            "{case_id}"
        );
        assert_eq!(outcome.proposal_count, 0, "{case_id}");
        assert!(outcome.canonical_answer_preserved, "{case_id}");
        assert_eq!(outcome.provider_request_count, 2, "{case_id}");
        assert_eq!(outcome.late_proposal_count, 0, "{case_id}");
    }
}

#[tokio::test]
async fn d051_buffered_and_streaming_have_identical_evidence_truth() {
    let case = case("positive_same_final_exact_observation");
    let buffered = run_d051_runtime_case(&fixture(), &case, D051RuntimeMode::Buffered)
        .await
        .expect("buffered D051 runtime case");
    let streaming = run_d051_runtime_case(&fixture(), &case, D051RuntimeMode::Streaming)
        .await
        .expect("streaming D051 runtime case");

    assert_eq!(memory_status(&buffered), memory_status(&streaming));
    assert_eq!(memory_reason(&buffered), memory_reason(&streaming));
    assert_eq!(
        receipt_memory_status(&buffered),
        receipt_memory_status(&streaming)
    );
    assert_eq!(
        receipt_memory_reason(&buffered),
        receipt_memory_reason(&streaming)
    );
    assert_eq!(delivery_status(&buffered), delivery_status(&streaming));
    assert_eq!(buffered.proposal_count, streaming.proposal_count);
    assert_eq!(
        buffered.exact_observation_manifest_matches,
        streaming.exact_observation_manifest_matches
    );
    assert_eq!(buffered.provider_request_count, 2);
    assert_eq!(streaming.provider_request_count, 2);
    assert!(buffered.final_provider_receipt_matches_manifest);
    assert!(streaming.final_provider_receipt_matches_manifest);
}

#[tokio::test]
async fn d051_cancel_fences_review_commit_and_late_provider_output() {
    let case = case("cancel_before_review_commit");
    let outcome = run_d051_runtime_case(&fixture(), &case, D051RuntimeMode::Streaming)
        .await
        .expect("cancelled D051 runtime case");

    assert_eq!(memory_status(&outcome), Some("cancelled"));
    assert_eq!(receipt_memory_status(&outcome), Some("cancelled"));
    assert_eq!(
        memory_reason(&outcome),
        Some("turn_cancelled_before_review_commit")
    );
    assert_eq!(delivery_status(&outcome), Some("cancelled"));
    assert_eq!(outcome.proposal_count, 0);
    assert_eq!(outcome.late_proposal_count, 0);
    assert!(!outcome.review_commit_after_cancel_observed);
    assert!(outcome.late_provider_output_was_released);
}

#[tokio::test]
async fn d051_scripted_envelope_never_receives_external_live_credit_or_product_commit() {
    let case = case("scripted_positive_local_contract_only");
    let outcome = run_d051_runtime_case(&fixture(), &case, D051RuntimeMode::ScriptedContract)
        .await
        .expect("scripted D051 contract case");

    assert_eq!(memory_status(&outcome), Some("candidate_admitted"));
    assert_eq!(outcome.evidence_credit, "local_contract_only");
    assert!(!outcome.external_live_credit);
    assert_eq!(outcome.proposal_count, 0);
    assert_eq!(outcome.late_proposal_count, 0);
}
