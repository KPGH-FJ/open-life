# OpenLife Roadshow H2 Governance Evidence

Date: 2026-07-14

## Decision

- H2 backend implementation and local mechanical gates: **GREEN**.
- H2 independent read-only review: **PENDING**.
- H2 product UI consumption and desktop trial: **PENDING**. The backend emits a
  typed `ReviewBatch`, but this slice does not claim that the current UI has
  completed a batch-level product trial.
- Roadshow release: **NO-GO**. V1-V4, cumulative integration, reliability, live
  Web, and product trial remain incomplete.
- This evidence verifies only the roadshow governance subset. It does not
  globally close BR4-D051, BR4-D052, or BR4-D053.

## Root fixes

- `e6fd50f` makes active ReviewWorkflow admission atomic with a database-backed
  review idempotency key instead of a scan-then-insert race.
- `6f24f32` gives Memory review a canonical fact key, reuses an active pending
  review item, and suppresses a new review after accepted canonical truth
  already exists.
- `d1e20a9` projects ReviewItems into typed domain/session `ReviewBatch` values.
  A batch has no decision or dispatch authority; child Proposals remain the
  only review/effect owners.
- `10e40cc` fixes the frozen MEM-04 misroute where a declarative habit containing
  the word `安排` was incorrectly treated as a PlanExecute command. It also
  binds explicit Memory durable-change projection to the canonical Memory
  owner and removes the obsolete fake ToolGateway evidence expectation.

## Verified governance facts

| Contract | Mechanical evidence | Result |
| --- | --- | --- |
| ordinary answer/read journeys create no Proposal or durable effect | exact frozen ORD-01 through ORD-08 via real `send_message_with_state` | 8/8, zero Proposals |
| current authenticated user can explicitly commit only an exact low/medium reversible Memory fact | exact frozen MEM-01, MEM-02, ZH-02 | 3/3 |
| explicit Memory is a domain write, not a fake tool call | same exact tests; ToolCall count is zero and canonical Memory receipt is present | passed |
| explicit Memory result matches canonical truth | task/run-bound canonical Memory row -> AgentState FinalDelivery -> TurnRuntime FinalDelivery | passed |
| missing canonical Memory owner fails closed | missing lifecycle-store frozen test | passed |
| inferred Memory does not replace or block the answer | exact frozen MEM-04 | passed |
| repeated inferred fact does not create proposal fatigue | MEM-04 repeated in 10 distinct sessions against one state | 1 pending Proposal, 1 Memory ReviewBatch |
| deferred review does not mutate accepted Memory | canonical lifecycle row count before/after the ten MEM-04 turns | delta 0 |
| quoted Web/tool content cannot authorize Memory | exact frozen MEM-05 and MEM-06 | 2/2, zero Proposals and zero writes |
| ReviewBatch is presentation-only | `openlife-core` review-item tests | 10/10 |
| ReviewWorkflow admission is atomic | ReviewWorkflow and ProposalStore focused tests | 7/7 and 16/16 |
| broad ordinary Main Chat behavior remains intact | `cargo test -p openlife-tauri 'main_chat_kernel_' -- --nocapture --test-threads=1` | 53/53 |
| single TurnRuntime/module authority remains intact | `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture --test-threads=1` | 29/29 |
| Rust lint build completes | `cargo clippy -p openlife-tauri --lib --tests` | completed with existing warnings |

The explicit Memory projection does not trust assistant text or the transient
generation object as canonical truth. The minimized transcript carries only a
receipt reference and boolean outcome. The backend reloads that Memory owner,
requires the same task/run binding, and only then emits a durable change. A
pending Proposal remains `completed_with_pending_items`; it is never described
as an applied Memory change.

## Frozen-suite boundary

The latest complete frozen mechanics module is **11/15**, not globally green.
The four remaining red tests are deliberately retained:

1. `high_risk_exact_prompts_use_real_send_and_stop_before_any_effect` — TOOL-02
   email intent is still misrouted as DirectAnswer instead of confirmation.
2. `run_01_exact_prompt_cancels_with_remote_unknown_and_no_late_commit` — the
   fixture observes `not_attempted` where the frozen expectation requires
   `remote_unknown`.
3. `run_03_tool_gateway_allows_one_dispatch_and_one_counting_effect` — the
   fixture lacks a `BoundContentReceipt` issuer.
4. `run_04_exact_prompt_uses_real_send_and_stream_with_independent_uuidv4_ids`
   — durable event cardinality is 2 instead of the frozen 6.

These failures belong to the remaining runtime/tool vertical work. They do not
invalidate the focused H2 facts, and they prevent any roadshow or full-suite
GREEN claim.

## Independent review and non-claims

- No reviewer independent from this implementation has yet retraced H2 source
  and evidence. H2 therefore cannot be marked accepted/closed.
- The product UI has a compatible typed `ReviewBatch` contract, but batch-level
  UI behavior and proposal-fatigue dogfood are not yet product-trial evidence.
- External live-provider and live-Web credit are not awarded by scripted
  provider responses or local fixtures.
- The broad `openlife-core` Main Chat Agent v1 module remains 46/54 because of
  eight pre-existing MCP, receipt-issuer, minimized-fixture, and legacy/runtime
  eval failures. The focused Main Chat kernel gate is 53/53; these are different
  denominators and are not conflated.
- Clippy completed but reported existing warnings. This slice does not claim a
  warning-free repository.
