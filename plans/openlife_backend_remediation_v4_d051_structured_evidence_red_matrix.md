# BR4-D051 Structured Memory Evidence RED Matrix — Review Revision 2

> Status: frozen RED contract, tests only; production seam intentionally absent
> Source baseline: `3c6f4d7f9fe8c6617f213c8b70cf900b6454e780`
> Supersedes the synthetic test design in tests-only commit `94d2b4b`

This revision closes the independent review finding that the first RED matrix
could be made green by implementing a test-only interpreter or runtime helper.
There is no outcome fixture and no expected status, reason, mutation or credit
value enters the system under test. Expected values exist only in assertions.

The suite now constructs production types, executes the real Main Chat runtime
and reads product truth from real durable stores. It remains a RED contract: it
does not implement the missing production admission seam, does not amend the
frozen 40 product scenarios and does not claim external-live-provider credit.

## Authority boundary

- Only the current authenticated user message may explicitly request a
  conditional, low/medium-risk, Review-only Memory candidate.
- The exact tool observation remains `untrusted_tool_observation` and has no
  proposal authority by itself.
- A draft is eligible only when it comes from the sole post-observation AgentLoop
  provider response and that same request's completed, non-simulated provider
  receipt and `ContextManifest` bind exactly one canonical observation.
- Model-provided subject, assertion, modality, confidence or prose can reject a
  draft fail-closed; none can authorize a write.
- Admission may stage one pending Review proposal. It may not write canonical
  Memory or canonical LifeModel-HS truth.
- Localhost HTTP capture proves a real serialization/network/runtime contract,
  but `externalLiveProviderCredit` must remain `false`.

## No-fake-green evidence graph

### Core admission graph

The Core matrix constructs production-shaped inputs and passes them to the
future single admission function `admit_structured_memory_evidence`:

1. `AgentIngress::decide` and its real `PolicyDecision`;
2. a typed, explicitly simulated `ToolExecutor` action/observation fixture;
3. the production `AgentRunStore` implementation for persistence and reload;
4. production-shaped `BoundContentReceipt` and test-only
   `ToolExecutionReceipt` fixtures;
5. `InferenceScheduler` output as a real `PreparedProviderRequest` with an
   exact `BoundedContextBlock` and `ContextManifest`;
6. a typed `ProviderInvocationReceipt` fixture, with simulated receipts covered
   by a mandatory negative counterfactual;
7. raw final response bytes parsed into a typed structured-evidence envelope.

The only Core support added to production source is under `#[cfg(test)]`. It
builds a typed simulated graph and persisted minimization record; it is not
adapter/runtime evidence and receives only `typed_core_contract` credit. It
does not classify, admit, mutate, supply expected outcomes or emulate the
missing production seam. The Tauri runtime suite, not this Core fixture, owns
real ToolGateway, HTTP and durable-event proof.

### Runtime graph

Both product transports execute their actual shared owner:

```text
send_message_with_operation_state
  -> OpenLifeTurnRuntime::run_buffered

start_stream_message_with_operation_state
  -> OpenLifeTurnRuntime::run_streaming
```

A real local TCP HTTP server captures every provider request and returns
OpenAI-compatible buffered or SSE bytes. Before each target turn, an ordinary
direct-answer control turn must reach that same server and complete, excluding
API-key, network-policy, runtime-config and listener-fixture errors. The server
then extracts the exact
`agent-run://.../observation/...` reference from the actual final request and
uses it in the final response; OpenLife never receives a placeholder.

Assertions read:

- final truth from `MainChatAgentEventStore`;
- proposal truth from `ProposalStore`;
- minimized runtime truth from `AgentRunStore`;
- execution traces from `ActionQueueStore` and `TaskSessionStore`;
- audit truth from `McpAuditStore`;
- canonical before/after truth from `MemoryLifecycleStore`, LifeModel and the
  HS asset-authority registry.

The full observation sentinel must be absent from every serialized durable
artifact above. Its presence in the transient captured HTTP request is
intentional and is not persistence credit.

## Frozen limits

| Boundary | Limit | Over-limit result |
| --- | ---: | --- |
| Drafts in one final envelope | 4 | `rejected / draft_limit_exceeded` |
| One transient observation | 16 KiB | `rejected / observation_limit_exceeded` |
| One exact evidence slice | 2 KiB | `rejected / evidence_slice_limit_exceeded` |
| Admitted candidates | exactly 1 | zero is `no_candidate`; multiple is `rejected` |

The tests construct actual 16 KiB/16 KiB + 1 observations and 2 KiB/2 KiB + 1
evidence slices. They also exercise reversed, out-of-range and UTF-8-splitting
byte ranges without pre-slicing invalid input in test code.

Observation bodies remain transient. Durable artifacts may keep references,
digests, categories and states, but must not duplicate the raw body in
AgentRun, event, receipt, audit or proposal metadata.

## Frozen state matrix

| Condition | Evidence status | Final delivery | Proposal | Canonical state |
| --- | --- | --- | ---: | --- |
| Exact same-final request, receipt, manifest and slice | `proposal_staged` | `completed_with_pending_items` | 1 pending Review item | unchanged |
| Explicit empty `memory_evidence` array | `no_candidate` | `completed` | 0 | unchanged |
| Extractor/schema/parse unavailable | `unavailable` with typed reason | `completed_with_partial_evidence` | 0 | unchanged |
| Binding, structural, limit or ambiguity rejection | `rejected` with typed reason | `completed_with_partial_evidence` | 0 | unchanged |
| Cancel before Review commit | `cancelled` | `cancelled` or `interrupted` | 0 after late output | unchanged |

`completed` is forbidden for unavailable/rejected evidence when the current
user explicitly requested the conditional Memory review, because it would hide
the unfulfilled part of the request.

## Frozen tests

### Behavior and single-authority deletion RED

- `d051_product_behavior_not_symbol_names_removes_legacy_implicit_authority`
  proves that plain untrusted observation prose has no Memory Proposal route;
  it does not rely on a function-name scan.
- `d051_authority_inventory_requires_one_production_admission_seam_and_no_raw_preview_router`
  requires one definition and one product caller, and inspects the product
  conditional-review stage for absence of raw-body, preview and heuristic
  routing.
- The existing
  `d051_legacy_implicit_stable_fact_authority_is_deleted` remains the direct
  regression contract for removal of handwritten stable-fact authority.

### Core binding and validation RED

- `d051_same_existing_final_provider_receipt_and_exact_observation_are_required`
- `d051_provider_request_response_receipt_and_manifest_counterfactuals_fail_closed`
- `d051_canonical_observation_receipt_and_owner_counterfactuals_fail_closed`
- `d051_current_authenticated_explicit_low_medium_review_lane_is_required`
- `d051_model_subject_assertion_modality_and_confidence_can_only_reject`
- `d051_structural_boundaries_are_ranges_not_prompt_keyword_classification`
- `d051_real_byte_limits_and_utf8_ranges_fail_closed_without_panics`
- `d051_candidate_cardinality_and_extractor_unavailability_are_typed`

The counterfactuals mutate one real input at a time: simulated receipt, request
ID, response digest, zero/two manifest matches, missing canonical observation
receipt, transplanted owner graph, request source, explicitness, policy grant,
model claims, byte boundary or candidate cardinality.

The structural scanner is restricted to an O(n) boundary pass for quoted
strings, Markdown blockquotes, inline/fenced code and JSON object/array/string
ranges. It must not inspect prompt-injection keywords or become a semantic
classifier. A prompt-like plain-text case freezes that distinction.

### TurnRuntime RED

- `d051_buffered_runtime_uses_real_http_event_proposal_and_canonical_stores`
- `d051_buffered_and_streaming_project_identical_durable_evidence_truth`
- `d051_missing_structured_envelope_cannot_fall_back_to_observation_heuristics`
- `d051_concurrent_same_operation_has_one_provider_execution_and_one_proposal`
- `d051_real_cancel_barrier_releases_late_provider_output_without_durable_commit`

For the exact one-candidate `file.read` scenario, capture must show one ordinary
provider control request followed by exactly one target request: the
post-observation final response that also carries the structured evidence
envelope. Read authorization remains deterministic in `PolicyRouter`; the
existing bounded read decision and ToolGateway execution remain
non-provider-backed. Adding a provider planning or extraction call is
forbidden. There is no ranking request in this exact case. This does not forbid
an already-governed ranking call in a different multi-candidate scenario.

Retry uses the same operation UUID and must reuse one durable final receipt and
one Proposal without another provider request. Concurrent calls use the same
operation UUID and require one execution owner; the competing caller may
safely reuse the durable final or receive a typed owner/in-progress
disposition. Cancellation blocks the real final provider response at a
barrier, invokes the real cancellation registry, then releases late response
bytes and proves no late proposal or canonical commit.

## Mechanical RED evidence

The revised Core contract compiles cleanly up to the intentionally missing
production module:

```text
error[E0432]: unresolved import `crate::agent::structured_memory_evidence`
error[E0433]: could not find `structured_memory_evidence` in `agent`
```

The real buffered runtime first proves the provider harness with the ordinary
control turn (`ProviderInvocationState::Completed`, capture count 1). The
unchanged target prompt then executes the actual governed file read, and the
durable event store proves its local, response-observed, successful ToolGateway
receipt. Current production stops at `main_chat_kernel_read_tool_synthesis`
without invoking the provider for the target. The focused test therefore fails
on the exact network-edge count:

```text
left:  1  # control request only
right: 2  # control + one post-observation target final
```

The current result independently records `provider_invocation_status =
not_attempted` for the target. This is the intended RED: the old product route
short-circuits before the same-final provider evidence seam. It is not an API
key, permission, network, test parser, fixture-outcome or mock-store failure.

## Credit and remaining verification boundary

This matrix can mechanically prove typed binding, local HTTP transport,
durable truth, duplicate fencing and cancellation fencing. It cannot award
external-live-provider credit. A later product-trial gate must independently
exercise an external provider and verify captured network, receipt, durable
event, ProposalStore and canonical before/after truth. Fixture, scripted or
localhost success can never satisfy that gate.

Production work may begin only after this revised tests-only commit is
reviewed. Implementation must make it green without changing assertions,
adding any provider call beyond the one post-observation final request,
restoring heuristic/preview authorization, persisting the raw observation body
or granting localhost external-live credit.
