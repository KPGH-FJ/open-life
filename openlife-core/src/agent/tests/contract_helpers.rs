//! Backend trace explainability contract helpers.
//!
//! These helpers provide lightweight field-level assertions on
//! `serde_json::Value` event payloads, matching the snake_case contract
//! consumed by the frontend typed explainability layer.
//!
//! All helpers are test-only — no production complexity is introduced.
//!
//! ## Typed reason validation
//!
//! The `assert_has_typed_reason` helper now validates that reason values
//! belong to the allowed enum sets defined by the production
//! `ExecutionBlockReason` / `ExecutionProposalReason` /
//! `ExecutionFailureKind` enums.  This prevents tests from accepting
//! `"not_a_real_enum_variant"` as valid, which would desynchronise the
//! backend from the frontend `typedContract.ts` parser.

use crate::agent::trace_payloads;

/// Assert the payload contains a non-empty string field.
pub fn assert_has_string(payload: &serde_json::Value, field: &str) {
    let val = payload
        .get(field)
        .unwrap_or_else(|| panic!("payload missing field '{}'", field));
    let s = val
        .as_str()
        .unwrap_or_else(|| panic!("payload field '{}' is not a string: {:?}", field, val));
    assert!(!s.is_empty(), "payload field '{}' is empty", field);
}

/// Assert the payload contains an array field with at least one element.
pub fn assert_has_array(payload: &serde_json::Value, field: &str) {
    let val = payload
        .get(field)
        .unwrap_or_else(|| panic!("payload missing field '{}'", field));
    let arr = val
        .as_array()
        .unwrap_or_else(|| panic!("payload field '{}' is not an array: {:?}", field, val));
    assert!(
        !arr.is_empty(),
        "payload field '{}' is an empty array",
        field
    );
}

/// Assert the payload contains an array field (may be empty).
pub fn assert_has_array_allow_empty(payload: &serde_json::Value, field: &str) {
    let val = payload
        .get(field)
        .unwrap_or_else(|| panic!("payload missing field '{}'", field));
    assert!(
        val.is_array(),
        "payload field '{}' is not an array: {:?}",
        field,
        val
    );
}

/// Assert the payload contains at least one of the given typed reason
/// fields with a value that belongs to the allowed enum set for that
/// field.
///
/// **Unlike the previous implementation**, this helper validates against
/// the production enum variant strings defined in
/// [`trace_payloads::allowed_block_reasons`],
/// [`trace_payloads::allowed_proposal_reasons`], and
/// [`trace_payloads::allowed_failure_kinds`].  Values like
/// `"not_a_real_enum_variant"`, empty strings, and `"null"` are
/// rejected — exactly matching the frontend `typedContract.ts` parser
/// behaviour.
///
/// # Panics
///
/// Panics if no candidate field carries a valid typed reason.
pub fn assert_has_typed_reason(payload: &serde_json::Value, candidates: &[&str]) {
    let found = candidates.iter().any(|field| {
        payload
            .get(field)
            .and_then(|v| v.as_str())
            .is_some_and(|s| trace_payloads::is_valid_typed_reason(field, s))
    });
    assert!(
        found,
        "payload must have at least one valid typed reason in {:?} (recognised enum variant required), got: {}",
        candidates,
        serde_json::to_string_pretty(payload).unwrap_or_default()
    );
}

/// Assert the payload does NOT contain any valid typed reason in the
/// given candidate fields.  Useful for verifying that malformed events
/// with invalid enum values are correctly rejected.
///
/// # Panics
///
/// Panics if any candidate field carries a valid typed reason.
pub fn assert_no_typed_reason(payload: &serde_json::Value, candidates: &[&str]) {
    let found = candidates.iter().any(|field| {
        payload
            .get(field)
            .and_then(|v| v.as_str())
            .is_some_and(|s| trace_payloads::is_valid_typed_reason(field, s))
    });
    assert!(
        !found,
        "payload must NOT have a valid typed reason in {:?}, but found one. Payload: {}",
        candidates,
        serde_json::to_string_pretty(payload).unwrap_or_default()
    );
}

/// Assert each array element in the given field has the specified sub-field.
pub fn assert_array_items_have_field(
    payload: &serde_json::Value,
    array_field: &str,
    item_field: &str,
) {
    let arr = payload
        .get(array_field)
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("payload missing array field '{}'", array_field));
    for (i, item) in arr.iter().enumerate() {
        assert!(
            item.get(item_field).is_some(),
            "array item {} in '{}' missing field '{}': {:?}",
            i,
            array_field,
            item_field,
            item
        );
    }
}

/// Assert the payload does NOT contain the given field.
pub fn assert_field_absent(payload: &serde_json::Value, field: &str) {
    assert!(
        payload.get(field).is_none(),
        "payload must not contain field '{}', but found: {:?}",
        field,
        payload.get(field)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_typed_reasons_pass() {
        let payload = serde_json::json!({
            "block_reason": "agent_spec_denied",
            "proposal_reason": null,
            "failure_kind": null,
        });
        assert_has_typed_reason(&payload, &["block_reason", "proposal_reason"]);
    }

    #[test]
    #[should_panic]
    fn test_invalid_block_reason_fails() {
        let payload = serde_json::json!({
            "block_reason": "not_a_real_enum_variant",
            "proposal_reason": null,
        });
        assert_has_typed_reason(&payload, &["block_reason", "proposal_reason"]);
    }

    #[test]
    #[should_panic]
    fn test_invalid_proposal_reason_fails() {
        let payload = serde_json::json!({
            "block_reason": null,
            "proposal_reason": "not_a_real_enum_variant",
        });
        assert_has_typed_reason(&payload, &["block_reason", "proposal_reason"]);
    }

    #[test]
    #[should_panic]
    fn test_invalid_failure_kind_fails() {
        let payload = serde_json::json!({
            "block_reason": null,
            "failure_kind": "not_a_real_enum_variant",
        });
        assert_has_typed_reason(&payload, &["block_reason", "failure_kind"]);
    }

    #[test]
    #[should_panic]
    fn test_null_reasons_fail() {
        let payload = serde_json::json!({
            "block_reason": null,
            "proposal_reason": null,
        });
        assert_has_typed_reason(&payload, &["block_reason", "proposal_reason"]);
    }

    #[test]
    #[should_panic]
    fn test_empty_reasons_fail() {
        let payload = serde_json::json!({
            "block_reason": "",
            "proposal_reason": "",
        });
        assert_has_typed_reason(&payload, &["block_reason", "proposal_reason"]);
    }

    #[test]
    fn test_assert_no_typed_reason_with_invalid_enum() {
        // malformedAndUnknownRun-like payload: invalid enum variant
        let payload = serde_json::json!({
            "block_reason": "not_a_real_enum_variant",
            "proposal_reason": null,
        });
        assert_no_typed_reason(&payload, &["block_reason", "proposal_reason"]);
    }

    #[test]
    fn test_assert_no_typed_reason_with_null() {
        let payload = serde_json::json!({
            "block_reason": null,
            "proposal_reason": null,
        });
        assert_no_typed_reason(&payload, &["block_reason", "proposal_reason"]);
    }
}
