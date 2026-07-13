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
| Terminal-origin minter deletion guard | RED, exactly one selected test | Full Rust production-source absence scan. It is deletion evidence only; dynamic negative cases prove authority behavior. |
| Explicit target compile contract | RED at production API compilation | Eleven concrete tests use real file-backed Conversation, EventStore, TaskSessionStore, ProposalStore, and Memory lifecycle owners plus an observable external-dispatch adapter. RED must not be a missing test file or zero-test filter. |

### Cross-store crash matrix

| Injected boundary | Durable truth before reopen | Reconciliation obligation |
| --- | --- | --- |
| Claim persisted, before effect | Original claim only; zero Memory, Task transition, local receipt, successor, or Proposal projection | Resume that claim; execute each remaining stage once. |
| Memory committed, before Task transaction | One Memory owner keyed by Proposal; Task still `WaitingPermission`; no local receipt or successor | Observe the Memory owner; do not invoke Memory again; commit Task/receipt once. |
| Task + local receipt committed, before Proposal checkpoint | One Memory owner and one Task revision; exact blocker removed; original claim still `claimed`; no successor | Verify the owner-local receipt; do not invoke Memory or Task again. |
| Proposal checkpoint committed, before successor | Dispatch is `confirmed_projection_pending`; Proposal read model remains pending; no successor | Append one successor from claim + verified receipt, then project Proposal. |
| Successor committed, before Proposal projection | Exactly one successor exists; Proposal read model remains pending | Reuse the successor; project Proposal once without appending or dispatching again. |

Every row reopens all independent SQLite stores, preserves the original claim
id, converges to one Memory owner, one Task transition/receipt, one successor,
and one Proposal projection, then proves a second reconciliation is a no-op.

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

cargo test -p openlife-tauri \
  terminal_origin_authority_surface_has_no_naked_id_or_string_minter_after_cutover \
  -- --nocapture

rg -n '^async fn d055_target_' \
  src-tauri/src/d055_terminal_owner_graph_compile_red.rs

RUSTFLAGS='--cfg d055_compile_red --check-cfg=cfg(d055_compile_red)' \
  cargo test -p openlife-tauri --lib d055_target --no-run
```

The compile command must currently fail on missing production terminal-owner
types/methods while the test module file itself exists. It contains exactly
eleven `d055_target_*` tests:

1. file-backed canonical-message-bound origin acceptance and reopen-verifiable
   owner-local receipt;
2. claim durable -> before effect crash and reopen;
3. Memory committed -> before Task owner transaction crash and reopen;
4. Task transition/owner-local receipt committed -> before Proposal checkpoint
   crash and reopen;
5. Proposal durable checkpoint committed -> before EventStore successor crash
   and reopen;
6. EventStore successor committed -> before Proposal status projection crash
   and reopen;
7. detailed owner-local receipt -> successor-confirm restart reconciliation;
8. operation/session/task mismatch, foreign canonical-store identity,
   tombstoned-message and owner rebind rejection; exact cloned/idempotent replay
   recovers the same admission/epoch and cannot mint another generation;
9. observable external-dispatch `remote_unknown` with two restart reconciliation
   passes and an exact adapter call count of one;
10. real EventStore final insert + SEALED-CAS transaction rollback failpoint;
11. real sensitive-Memory runtime defer, post-seal exact-once acceptance, and
   buffered/streaming successor recovery.

## Required facts before GREEN

- The real product staging path persists a typed immutable origin through
  ReviewWorkflow/ProposalStore. TaskSessionStore consumes and revalidates a real
  opaque canonical user-message commit against Task id, run operation, chat
  session, active Conversation row, and canonical-store identity. EventStore
  accepts only the resulting non-Serde admission and persists its owner
  ref/digest before it can issue an origin proof. No production API may mint
  origin authority later from caller ids or a free-text reference.
  `source_detail`, `after`, and `run_id` do not authorize an origin relationship.
- Dynamic negative cases reject operation/session/task mismatch, owner
  rebinding, a proof from a different canonical-store identity, and a
  stale/tombstoned canonical message. An exact cloned/idempotent commit replay
  is crash-safe: it recovers the same admission id and the same OPEN or SEALED
  epoch generation, never a second epoch. The production-source minter scan
  remains only deletion evidence.
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
- Conversation, Proposal, Memory lifecycle, TaskSession, and EventStore are not
  presented as one cross-database transaction. Independent failpoints cover:
  durable claim before effect; Memory commit before Task transaction; Task plus
  owner-local receipt before Proposal checkpoint; Proposal checkpoint before
  successor; and successor before Proposal status projection. Every reopen
  resumes the original claim, reaches exactly one Memory owner, one Task owner
  transition, one successor, and one Proposal projection, then a second
  reconciliation changes nothing.
- An observable external-dispatch adapter returns `remote_unknown` after one
  actual call. Reopening all stores and running reconciliation twice leaves its
  call count at one, its Proposal dispatch `unknown`, its Task blocked, and its
  successor absent. Summary counters are supporting evidence, not the oracle.
- The one successor fact binds Proposal id, immutable final event id,
  TaskSession owner id, before/after revision and digest, and the verified local
  receipt ref/digest. Unknown or unbound history remains rejected.
- Normal test builds stay GREEN without a `src-tauri` `test-utils` feature or
  opaque integration harness.
- Before GREEN, move the eleven named target tests from the custom cfg into
  normal `#[cfg(test)]`, then prove the filter is non-zero and all eleven
  execute:

```sh
cargo test -p openlife-tauri --lib d055_target -- --list
cargo test -p openlife-tauri --lib d055_target -- --nocapture
```

The first command must list the eleven named tests; the second must report
eleven passed.
No fixture, static string, or another LLM review can replace the stored facts
above.
