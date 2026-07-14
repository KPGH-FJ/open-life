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

## Downstream frozen-mechanics closure

The full frozen mechanics module exposed four downstream runtime/tool issues
after the focused H2 gates were green. They were resolved without changing the
frozen scenario file or rubric:

- `ee99709` recognizes Chinese email action requests through a bounded semantic
  combination while retaining history, draft-only, summary, and instruction
  counterexamples.
- `5388afe` records the local provider adapter-start edge before the HTTP future
  can be cancelled, so an in-flight request is `remote_unknown` rather than
  `not_attempted`; pre-dispatch rejection remains `not_attempted`.
- `4ed3c05` binds RUN-03 to real UUIDv4 canonical AgentRun owners and the real
  bound-content receipt issuer. It does not weaken ToolGateway receipt checks.
- `115335f` corrects the RUN-04 evaluator to the frozen rubric: two repeated
  turns have distinct task owners and transcripts, while each turn retains the
  D050 one-operation/one-message/one-task/one-run recovery identity.

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
| full frozen mechanics module | `cargo test -p openlife-tauri backend_remediation_frozen_scenario_tests -- --nocapture --test-threads=1` | 15/15 |
| Rust lint build completes | `cargo clippy -p openlife-tauri --lib --tests` | completed with existing warnings |

The explicit Memory projection does not trust assistant text or the transient
generation object as canonical truth. The minimized transcript carries only a
receipt reference and boolean outcome. The backend reloads that Memory owner,
requires the same task/run binding, and only then emits a durable change. A
pending Proposal remains `completed_with_pending_items`; it is never described
as an applied Memory change.

## Frozen-suite boundary

The complete frozen mechanics module is now **15/15**. This closes the local
mechanical failures previously observed in TOOL-02 and RUN-01/03/04. It does
not add external-live, product-UI, soak, independent-review, or global BR4
closure credit. In particular, a green frozen module is not a roadshow release
decision.

## Independent review and non-claims

- No reviewer independent from this implementation has yet retraced H2 source
  and evidence. H2 therefore cannot be marked accepted/closed.
- The product UI has a compatible typed `ReviewBatch` contract, but batch-level
  UI behavior and proposal-fatigue dogfood are not yet product-trial evidence.
- External live-provider and live-Web credit are not awarded by scripted
  provider responses or local fixtures.
- The last recorded broad `openlife-core` Main Chat Agent v1 module result is
  46/54 because of eight pre-existing MCP, receipt-issuer, minimized-fixture,
  and legacy/runtime eval failures. It was not rerun by the downstream frozen
  mechanics closure. The focused Main Chat kernel gate is 53/53; these are
  different denominators and are not conflated.
- Clippy completed but reported existing warnings. This slice does not claim a
  warning-free repository.
