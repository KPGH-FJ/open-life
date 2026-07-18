# OpenLife Roadshow H1 Runtime Truth Evidence

Date: 2026-07-14

## Decision

- H1 implementation and local mechanical gates: **GREEN**.
- H1 independent read-only review: **PENDING**.
- Roadshow release: **NO-GO**. H2 and V1-V4 remain incomplete, and external
  live Web remains red from H0.
- This evidence closes only the roadshow runtime subset. It does not globally
  close BR4-D010, BR4-D020, or BR4-D050.

## Verified runtime facts

| Contract | Mechanical evidence | Result |
| --- | --- | --- |
| one buffered/stream runtime owner, replay, restart, one dispatch winner, durable FinalDelivery | `cargo test -p openlife-tauri 'main_chat_turn_runtime::' -- --nocapture` | 32 passed |
| cancellation, commit barrier, local abort, remote unknown, no late canonical commit | `cargo test -p openlife-tauri main_chat_cancellation -- --nocapture` | 29 passed |
| minimal TaskSession/transcript persistence and restart-stable owner digest | `cargo test -p openlife-core session_content_minimization -- --nocapture` | 7 passed |
| authenticated body receipt, replay, mutation rejection, no body copy | `cargo test -p openlife-core bound_content_receipt -- --nocapture` | 17 passed |
| compile all Tauri test targets | `cargo check -p openlife-tauri --tests` | passed |
| external direct answer provider lifecycle | credential-injected ignored live test; secrets were not printed | passed |
| true external provider streaming | credential-injected ignored live test; 37 provider-bound chunks, durable start before complete, final last | passed |
| provider privacy capture | local HTTP capture test; raw LifeModel sentinel absent and email masked | passed |

## Verified D020 roadshow foundation

The acceptance path now validates the complete reviewed Memory contract before
native confirmation or dispatch claim. Canonical task-session ownership comes
from reviewed task-session fields and an explicit source marker; disagreement
fails closed. `memory.propose_write` and maturation now generate typed reviewed
payloads instead of creating Proposals that cannot later be accepted.

| Contract | Mechanical evidence | Result |
| --- | --- | --- |
| Proposal claim/effect/projection CAS and retry | `cargo test -p openlife-core proposal_store -- --nocapture` | 16 passed |
| Proposal effect, projection-pending, AgentRun reconciliation, Memory accept/rollback | `cargo test -p openlife-tauri commands::proposal::tests -- --nocapture` | 63 passed |
| canonical Memory lifecycle, outbox, task-session binding, rollback/restart | `cargo test -p openlife-core memory_lifecycle -- --nocapture` | 36 passed |
| typed CoreOS Memory Proposal | focused contract and integration tests | passed |
| typed maturation Proposal non-regression | `cargo test -p openlife-core maturation -- --nocapture` | 42 passed |
| product pending/degraded truth | ChatPage and MailboxPage Vitest suites | 80 passed |
| frontend contract compatibility | `corepack pnpm --dir frontend typecheck` | passed |

The frontend worktree had no local `node_modules`. The tests used the same
repository's already-installed dependency tree through a temporary symlink;
the symlink was removed after the gates and was never staged.

## Root-fix commits

- `e4fcdcd` FinalDelivery test distinguishes append owner from recovery guard.
- `3457368` preserves the first durable tool start across retry.
- `e6d6e56` classifies provider failure only at an observed boundary.
- `8eef3c9` requires durable provider lifecycle evidence.
- `d0c30b2` supports fixed official HTTPS endpoints through bounded macOS
  loopback fake-IP proxy conditions without weakening generic SSRF policy.
- `b39ab65`, `7b3b8fe`, `b4a89d3` bind and version durable task/run receipt
  ownership across restart.
- `2d3270d` permits only authenticated raw receipt replay and rejects changed
  bodies without mutating the canonical owner.
- `f4e4a6c` binds reviewed Memory Proposal, effect, projection, AgentRun, and
  task-session truth.
- `ef6653d` generates typed conservative Memory Proposals from CoreOS and
  maturation paths.

## Remaining non-claims

- The required independent H1 review has not happened.
- External live Web search/synthesis/citation truth is still red.
- H2 governance is not yet accepted as a phase.
- Resource ingestion, StateStore, artifact materialization, cumulative trial,
  keychain restart proof, and final Phase7 GREEN remain incomplete.
- Existing dead-code warnings were observed; they were not introduced by this
  slice and are not being hidden as a successful cleanup claim.
