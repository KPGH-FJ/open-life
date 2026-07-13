# OpenLife BR4-D055 terminal-owner RED matrix

> Status: executable RED-oracle contract; production implementation not started
> Scope: tests, test-only barriers, and evidence commands only
> Authority: subordinate to `AGENTS.md`, the Phase7 single-system contract, and
> `plans/openlife_backend_remediation_v4_discovered_findings.json`

## Purpose

BR4-D055 is not closed by rereading mutable stores or by accepting a
caller-shaped receipt. The target is one durable terminalization epoch, one
production terminal-owner write gateway, and owner-local transition receipts
that remain verifiable after reopening the real file-backed stores.

The contract names under the explicit `d055_compile_red` cfg are provisional.
The invariants and stored facts in the tests are authoritative. A production
implementation may rename an API only by updating this matrix and its compile
oracle in the same reviewed change.

## Evidence separation

| Oracle | Current expectation | Credit boundary |
| --- | --- | --- |
| D034 authority guard | GREEN, exactly one selected test | Proves D055 did not reintroduce a crate/feature-forgeable test authority surface. |
| Normal buffered + streaming recovery | GREEN, exactly one selected test | One real local HTTP dispatch, one provider receipt, one final; both delivery modes recover without redispatch. Local HTTP is not external-provider credit. |
| Unproven post-final drift | GREEN, exactly one selected test | Both recovery modes fail closed; no fake done event and no redispatch. |
| Real sensitive-Memory accept at SEALING | RED, exactly one selected test | Real Main Chat kernel + ReviewWorkflow + ProposalStore path. Current code accepts during SEALING; target returns the exact typed defer before claim. |
| Forged free-text origin | RED, exactly one selected test | Diagnostic counterexample only. `source_detail` and `after` must never gain TaskSession or successor authority. |
| Writer deletion guard | RED, exactly one selected test | Deletion/absence evidence only; it does not claim source strings prove behavior. |
| Explicit target compile contract | RED at production API compilation | Four concrete tests use real file-backed Conversation, EventStore, TaskSessionStore, ProposalStore, and Memory lifecycle owners. RED must not be a missing test file or zero-test filter. |

## Mechanical commands

Run serially in the same target directory. Concurrent Cargo commands create a
build-lock wait and are not independent evidence.

```sh
cargo check -p openlife-tauri --tests

cargo test -p openlife-tauri \
  d034_automatic_retry_has_no_bare_or_crate_forgeable_claim_authority \
  -- --nocapture

cargo test -p openlife-tauri \
  normal_buffered_and_streaming_recovery_controls_remain_idempotent \
  -- --nocapture

cargo test -p openlife-tauri \
  unproven_post_final_owner_drift_stays_fail_closed_without_redispatch \
  -- --nocapture

cargo test -p openlife-tauri \
  real_sensitive_memory_accept_defers_at_sealing_then_commits_one_successor \
  -- --nocapture

cargo test -p openlife-tauri \
  forged_source_detail_and_after_cannot_gain_terminal_owner_authority \
  -- --nocapture

cargo test -p openlife-tauri \
  turn_bound_writer_matrix_has_no_direct_product_bypass_after_gateway_cutover \
  -- --nocapture

RUSTFLAGS='--cfg d055_compile_red --check-cfg=cfg(d055_compile_red)' \
  cargo test -p openlife-tauri --lib d055_target --no-run
```

The compile command must currently fail on missing production terminal-owner
types/methods while the test module file itself exists. It contains exactly
four `d055_target_*` tests:

1. file-backed canonical-message-bound origin acceptance and reopen-verifiable
   owner-local receipt;
2. cross-SQLite owner commit -> successor-confirm crash, restart reconciliation,
   and unknown-external-effect no-retry;
3. real EventStore final insert + SEALED-CAS transaction rollback failpoint;
4. real sensitive-Memory runtime defer, post-seal exact-once acceptance, and
   buffered/streaming successor recovery.

## Required facts before GREEN

- The real product staging path persists a typed immutable origin through
  ReviewWorkflow/ProposalStore. The epoch consumes a real opaque canonical
  user-message commit and persists its owner ref/digest before it can issue an
  origin proof. No EventStore API may mint origin authority later from caller
  ids or a free-text reference. `source_detail`, `after`, and `run_id` do not
  authorize an origin relationship.
- SEALING admission happens before mutable owner heads are read.
- The exact EventStore API called by OpenLifeTurnRuntime atomically inserts the
  final and commits the epoch SEALED. The failpoint between those statements
  rolls both back.
- A SEALING accept returns `success=false`, `status=deferred`,
  `reasonCode=origin_turn_sealing`, `dispatchState=unclaimed`, and
  `durableWriteExecuted=false`; Proposal, Memory, and TaskSession remain
  unchanged.
- A post-SEALED accept consumes a real ProposalStore claim and a non-Serde
  `ReviewWorkflow::claimed_acceptance_snapshot`. Callers cannot select a raw
  canonical effect enum.
- The owner-local transaction removes the exact `proposal:<id>` blocker,
  performs the real TaskSession state transition, increments a real owner
  revision, and persists a receipt whose before/after digests match that
  non-no-op change after reopen.
- The owner-local SQLite commit and EventStore successor confirmation are not
  presented as cross-database atomic. A crash between them leaves durable
  `confirmed_projection_pending` truth, then restart reconciliation consumes
  the Proposal claim plus verified owner-local receipt to add one successor.
  It executes neither Memory nor Task effect again. An unqueryable external
  `unknown` remains unknown and is never automatically retried.
- The one successor fact binds Proposal id, immutable final event id,
  TaskSession owner id, before/after revision and digest, and the verified local
  receipt ref/digest. Unknown or unbound history remains rejected.
- Normal test builds stay GREEN without a `src-tauri` `test-utils` feature or
  opaque integration harness.
- Before GREEN, move the four named target tests from the custom cfg into normal
  `#[cfg(test)]`, then prove the filter is non-zero and all four execute:

```sh
cargo test -p openlife-tauri --lib d055_target -- --list
cargo test -p openlife-tauri --lib d055_target -- --nocapture
```

The first command must list the four named tests; the second must report four
passed.
No fixture, static string, or another LLM review can replace the stored facts
above.
