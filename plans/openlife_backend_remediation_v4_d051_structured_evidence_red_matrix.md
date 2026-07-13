# BR4-D051 Structured Memory Evidence RED Matrix

> Status: frozen RED contract, tests only
> Baseline: `3c6f4d7f9fe8c6617f213c8b70cf900b6454e780`
> Fixture: `openlife-core/tests/fixtures/d051_structured_memory_evidence_cases.json`

This file freezes the D051 failure contract before production implementation.
It does not claim the route exists, does not grant live-provider credit, and
does not amend the frozen 40 product scenarios.

## Authority boundary

- The current authenticated user request may ask for a conditional,
  low/medium-risk, Review-only Memory candidate.
- The exact tool observation remains `untrusted_tool_observation`.
- A draft is eligible only when it comes from the already-required final
  AgentLoop provider response and that same request's completed,
  non-simulated receipt and `ContextManifest` bind the exact observation.
- Model-provided subject, assertion, modality, confidence, or prose never
  authorizes a write. They may only reject a draft fail-closed.
- Admission can create a pending Review proposal. It cannot directly write
  Memory or canonical LifeModel-HS truth.
- Scripted replies prove only local contract parsing/validation. They cannot
  create product proposals or count as external live-provider evidence.

## Frozen limits

| Boundary | Limit | Over-limit result |
| --- | ---: | --- |
| Drafts in one final envelope | 4 | `rejected / draft_limit_exceeded` |
| One transient observation | 16 KiB | `rejected / observation_limit_exceeded` |
| One exact evidence slice | 2 KiB | `rejected / evidence_slice_limit_exceeded` |
| Admitted candidates | exactly 1 | zero is `no_candidate`; multiple is `rejected` |

Observation bodies remain transient. Tests require references, exact byte
ranges, SHA-256 digests, categories and receipt state; they do not authorize a
new body copy in AgentRun, event, receipt, audit, or proposal metadata.

## State matrix

| Condition | Evidence status | Final delivery | Proposal | Answer |
| --- | --- | --- | ---: | --- |
| Exact same-final request, receipt, manifest and slice | `proposal_staged` | `completed_with_pending_items` | 1 pending Review item | preserved |
| Explicit empty `memory_evidence` array | `no_candidate` | `completed` | 0 | preserved |
| Provider/extractor/parse unavailable | `unavailable` with typed reason | `completed_with_partial_evidence` | 0 | preserved |
| Binding, structural, limit or ambiguity rejection | `rejected` with typed reason | `completed_with_partial_evidence` | 0 | preserved |
| Cancel before Review commit | `cancelled` | `cancelled` | 0, including after late output | not synthesized |
| Scripted valid envelope | `candidate_admitted`, local contract only | not product-credited | 0 | test-only |

`completed` is forbidden for unavailable/rejected evidence because that would
hide the unfulfilled explicit Memory-review part of the request.

## Frozen tests

### Existing authority deletion RED

- `d051_legacy_implicit_stable_fact_authority_is_deleted`
  - Plain observation text must no longer become a Memory proposal through
    `is_supported_stable_user_fact_expression`.
  - This replaces the obsolete unit expectation that made the handwritten
    heuristic authoritative. It is not a waiver for a frozen product scenario.

### Core binding and validation RED

- `d051_same_existing_final_provider_receipt_and_exact_observation_are_required`
- `d051_legacy_implicit_and_preview_authorization_routes_are_absent`
- `d051_any_range_digest_receipt_request_response_context_epoch_or_user_drift_fails_closed`
- `d051_model_subject_assertion_modality_and_confidence_can_only_reject`
- `d051_only_current_authenticated_explicit_low_or_medium_review_requests_are_eligible`
- `d051_structural_filter_is_only_range_based_for_quote_code_and_json_boundaries`
- `d051_bounds_and_candidate_cardinality_are_fail_closed`
- `d051_provider_extractor_and_parse_unavailable_are_typed_and_never_create_proposals`
- `d051_scripted_envelope_is_local_contract_credit_only`

The structural scanner is restricted to an O(n) boundary pass for quoted
strings, Markdown blockquotes, inline/fenced code, and JSON object/array/string
ranges. It must not inspect prompt-injection keywords, infer semantic subject,
or become a second classifier. The plain prompt-like fixture freezes this:
identical plain evidence is not rejected merely because its words resemble an
instruction.

### TurnRuntime RED

- `d051_runtime_uses_the_existing_final_provider_request_without_a_third_call`
- `d051_provider_extractor_or_parse_unavailable_is_partial_not_fake_completed`
- `d051_buffered_and_streaming_have_identical_evidence_truth`
- `d051_cancel_fences_review_commit_and_late_provider_output`
- `d051_scripted_envelope_never_receives_external_live_credit_or_product_commit`

The positive AgentLoop fixture requires two provider calls: the existing tool
planning call and the existing post-observation final call. A third extraction
call is a failure. Buffered and streaming must bind the same final provider
receipt and project identical Memory evidence status/reason/proposal facts.

## Expected initial RED

The baseline fails before implementation for two independent reasons:

1. The legacy implicit stable-fact heuristic still authorizes plain
   observation text.
2. The expected structured evidence and runtime test-support seams do not yet
   exist, so the frozen contract suite has an unresolved-import compile RED.

Production work may begin only after this tests-only commit is reviewed. The
implementation must make these tests green without changing fixture outcomes,
adding a provider call, restoring preview authorization, or granting scripted
live credit.
