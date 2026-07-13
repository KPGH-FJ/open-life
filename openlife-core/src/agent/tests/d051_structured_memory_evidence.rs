use crate::agent::structured_memory_evidence::{
    evaluate_frozen_contract_case, MemoryEvidenceStatus, StructuredEvidenceCredit,
};
use serde_json::Value;

const FIXTURE: &str =
    include_str!("../../../tests/fixtures/d051_structured_memory_evidence_cases.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("D051 structured-memory fixture must be valid JSON")
}

fn case(case_id: &str) -> Value {
    fixture()["cases"]
        .as_array()
        .expect("D051 cases array")
        .iter()
        .find(|candidate| candidate["id"] == case_id)
        .unwrap_or_else(|| panic!("missing frozen D051 case: {case_id}"))
        .clone()
}

fn expected_string(case: &Value, field: &str) -> String {
    case[field]
        .as_str()
        .unwrap_or_else(|| panic!("D051 case missing string field {field}: {case}"))
        .to_string()
}

#[test]
fn d051_legacy_implicit_and_preview_authorization_routes_are_absent() {
    let candidate_source = include_str!("../main_chat_memory_candidate.rs");
    assert!(!candidate_source.contains("is_supported_stable_user_fact_expression"));
    assert!(!candidate_source.contains("stable_fact_supports_future_rule"));

    let kernel_source = include_str!("../../../../src-tauri/src/main_chat_kernel.rs");
    let conditional_stage = kernel_source
        .split("async fn stage_conditional_observation_memory_review(")
        .nth(1)
        .and_then(|tail| tail.split("async fn create_kernel_write_proposal(").next())
        .expect("conditional observation review stage source slice");
    assert!(!conditional_stage.contains(".get(\"preview\")"));
    assert!(!conditional_stage.contains("observed_body"));
    assert!(!conditional_stage.contains("extract_main_chat_memory_candidates"));
}

#[test]
fn d051_same_existing_final_provider_receipt_and_exact_observation_are_required() {
    let case = case("positive_same_final_exact_observation");
    let outcome = evaluate_frozen_contract_case(&fixture(), &case)
        .expect("exact same-final structured evidence contract");

    assert_eq!(outcome.status, MemoryEvidenceStatus::ProposalStaged);
    assert_eq!(outcome.reason_code, "same_final_provider_evidence_admitted");
    assert_eq!(outcome.source_kind, "untrusted_tool_observation");
    assert_eq!(outcome.proposal_count, 1);
    assert!(outcome.answer_continued);
    assert!(outcome.same_final_provider_receipt_bound);
    assert!(outcome.exact_observation_manifest_bound);
    assert_eq!(outcome.provider_request_count, 2);
    assert_eq!(outcome.final_provider_request_ordinal, Some(2));
    assert_eq!(outcome.exact_manifest_matches, 1);
    assert_eq!(outcome.late_proposal_count, 0);
}

#[test]
fn d051_any_range_digest_receipt_request_response_context_epoch_or_user_drift_fails_closed() {
    for case_id in [
        "wrong_evidence_range",
        "wrong_evidence_digest",
        "wrong_provider_receipt",
        "provider_receipt_not_completed",
        "wrong_provider_request",
        "wrong_provider_response",
        "wrong_context_manifest",
        "wrong_execution_epoch",
        "wrong_current_user_ref",
        "wrong_current_user_digest",
        "wrong_policy_version",
        "wrong_observation_ref",
    ] {
        let case = case(case_id);
        let outcome = evaluate_frozen_contract_case(&fixture(), &case)
            .unwrap_or_else(|error| panic!("{case_id} must produce a typed outcome: {error}"));

        assert!(
            matches!(
                outcome.status,
                MemoryEvidenceStatus::Rejected | MemoryEvidenceStatus::Unavailable
            ),
            "{case_id}"
        );
        assert_eq!(
            outcome.reason_code,
            expected_string(&case, "expectedReasonCode"),
            "{case_id}"
        );
        assert_eq!(outcome.proposal_count, 0, "{case_id}");
        assert!(outcome.answer_continued, "{case_id}");
    }
}

#[test]
fn d051_model_subject_assertion_modality_and_confidence_can_only_reject() {
    for case_id in [
        "model_subject_not_current_user",
        "model_assertion_not_asserted_fact",
        "model_modality_not_asserted",
        "model_confidence_below_threshold",
    ] {
        let case = case(case_id);
        let outcome = evaluate_frozen_contract_case(&fixture(), &case)
            .unwrap_or_else(|error| panic!("{case_id} typed model-filter outcome: {error}"));

        assert_eq!(outcome.status, MemoryEvidenceStatus::Rejected, "{case_id}");
        assert_eq!(
            outcome.reason_code,
            expected_string(&case, "expectedReasonCode"),
            "{case_id}"
        );
        assert_eq!(outcome.proposal_count, 0, "{case_id}");
        assert!(outcome.answer_continued, "{case_id}");
    }
}

#[test]
fn d051_only_current_authenticated_explicit_low_or_medium_review_requests_are_eligible() {
    for case_id in [
        "current_request_not_explicit",
        "current_request_not_authenticated_user",
        "policy_risk_above_medium",
        "review_only_lane_missing",
    ] {
        let case = case(case_id);
        let outcome = evaluate_frozen_contract_case(&fixture(), &case)
            .unwrap_or_else(|error| panic!("{case_id} typed policy outcome: {error}"));

        assert_eq!(outcome.status, MemoryEvidenceStatus::Rejected, "{case_id}");
        assert_eq!(
            outcome.reason_code,
            expected_string(&case, "expectedReasonCode"),
            "{case_id}"
        );
        assert_eq!(outcome.proposal_count, 0, "{case_id}");
        assert!(outcome.answer_continued, "{case_id}");
    }
}

#[test]
fn d051_structural_filter_is_only_range_based_for_quote_code_and_json_boundaries() {
    for case_id in [
        "quoted_string_range",
        "markdown_blockquote_range",
        "inline_code_range",
        "fenced_code_range",
        "json_string_range",
    ] {
        let case = case(case_id);
        let outcome = evaluate_frozen_contract_case(&fixture(), &case)
            .unwrap_or_else(|error| panic!("{case_id} typed structural outcome: {error}"));

        assert_eq!(outcome.status, MemoryEvidenceStatus::Rejected, "{case_id}");
        assert_eq!(
            outcome.reason_code, "evidence_inside_untrusted_structure",
            "{case_id}"
        );
        assert_eq!(outcome.proposal_count, 0, "{case_id}");
    }

    let plain = case("plain_prompt_like_text_uses_no_keyword_classifier");
    let plain_outcome = evaluate_frozen_contract_case(&fixture(), &plain)
        .expect("plain exact slice must not be judged by prompt-like keywords");
    assert_eq!(plain_outcome.status, MemoryEvidenceStatus::ProposalStaged);
    assert_eq!(plain_outcome.proposal_count, 1);
    assert_eq!(plain_outcome.structural_scan_kind, "boundary_ranges_only");
}

#[test]
fn d051_bounds_and_candidate_cardinality_are_fail_closed() {
    for case_id in [
        "more_than_four_drafts",
        "observation_over_sixteen_kib",
        "slice_over_two_kib",
        "multiple_candidates",
    ] {
        let case = case(case_id);
        let outcome = evaluate_frozen_contract_case(&fixture(), &case)
            .unwrap_or_else(|error| panic!("{case_id} typed limit outcome: {error}"));
        assert_eq!(outcome.status, MemoryEvidenceStatus::Rejected, "{case_id}");
        assert_eq!(
            outcome.reason_code,
            expected_string(&case, "expectedReasonCode"),
            "{case_id}"
        );
        assert_eq!(outcome.proposal_count, 0, "{case_id}");
    }

    let empty = case("zero_candidates");
    let empty_outcome = evaluate_frozen_contract_case(&fixture(), &empty)
        .expect("an explicit empty evidence array is a typed no-candidate outcome");
    assert_eq!(empty_outcome.status, MemoryEvidenceStatus::NoCandidate);
    assert_eq!(empty_outcome.reason_code, "provider_returned_no_candidate");
    assert_eq!(empty_outcome.proposal_count, 0);
    assert!(empty_outcome.answer_continued);
}

#[test]
fn d051_provider_extractor_and_parse_unavailable_are_typed_and_never_create_proposals() {
    for case_id in [
        "provider_unavailable",
        "extractor_unavailable",
        "extractor_parse_unavailable",
    ] {
        let case = case(case_id);
        let outcome = evaluate_frozen_contract_case(&fixture(), &case)
            .unwrap_or_else(|error| panic!("{case_id} typed unavailable outcome: {error}"));

        assert_eq!(
            outcome.status,
            MemoryEvidenceStatus::Unavailable,
            "{case_id}"
        );
        assert_eq!(
            outcome.reason_code,
            expected_string(&case, "expectedReasonCode"),
            "{case_id}"
        );
        assert_eq!(
            outcome.final_delivery_status,
            "completed_with_partial_evidence"
        );
        assert_eq!(outcome.proposal_count, 0, "{case_id}");
        assert!(outcome.answer_continued, "{case_id}");
        assert_eq!(outcome.provider_request_count, 2, "{case_id}");
    }
}

#[test]
fn d051_scripted_envelope_is_local_contract_credit_only() {
    let case = case("scripted_positive_local_contract_only");
    let outcome = evaluate_frozen_contract_case(&fixture(), &case)
        .expect("scripted structured envelope contract");

    assert_eq!(outcome.status, MemoryEvidenceStatus::CandidateAdmitted);
    assert_eq!(outcome.credit, StructuredEvidenceCredit::LocalContractOnly);
    assert!(!outcome.external_live_credit);
    assert_eq!(outcome.proposal_count, 0);
}
