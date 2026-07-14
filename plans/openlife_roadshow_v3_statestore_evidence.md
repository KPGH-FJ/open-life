# OpenLife Roadshow V3 StateStore Evidence

Status: the bounded daily-task StateStore slice is mechanically verified. A
native product trial, independent read-only review, legacy-data parity and
rollback rehearsal, and the separate typed `/state` observation slice remain
pending. This file does not claim that ADR 0015, Phase7, the roadshow release,
or global backend remediation is complete.

## Scope and commit

The V3 daily-task implementation is commit
`65a0017a7c2bf1ecba5ecdeb7484ad77f5e29793` on
`codex/roadshow-core-recovery`:

- `StateStore` is the canonical SQLite owner for bounded daily-task assets,
  versions, operations, receipts, and shared persistence outbox facts;
- one transaction commits the asset mutation, version, operation receipt, and
  outbox delivery; transaction fault injection leaves none of them half-written;
- UUIDv4 operation identity is bound separately to the exact policy-sealed
  request digest and resolved canonical effect digest;
- exact create, complete, and undo retries return the original receipt even
  after wall-clock drift, version changes, or a tombstone; a different request
  under the same operation id fails closed;
- PolicyRouter alone issues the non-serializable transient-state grant, and
  only for an explicit, low-risk, current-user daily-task request;
- sensitive, long-term, ambiguous, quoted/untrusted, and forged routes cannot
  acquire direct StateStore authority;
- Main Chat `send` and `stream` retain the existing single
  `OpenLifeTurnRuntime`; deterministic state commands do not require or invoke
  a Provider or Tool;
- local cancellation admission spans the canonical commit, while recovery of
  an already committed receipt is a read-only operation and opens no second
  write window;
- the event store persists a minimal `effect_committed` fact before final
  delivery and does not copy the task title;
- YAML is an outbox-driven compatibility projection only. Concurrent
  projectors use LifeModel hash CAS plus bounded retry, so an older snapshot
  cannot overwrite a newer view and acknowledge its outbox delivery;
- existing unmarked YAML goals remain read-only migration input; product
  daily-task writes have one owner and do not dual-write independently;
- missing or corrupt release StateStore state is explicit degraded truth; no
  temp or in-memory product fallback is installed;
- ChatPage no longer parses or executes mutating `/goal` or `/state` shortcuts,
  generates mutation success prose, or exposes the assistant-to-goal direct
  write action. Slash commands enter the same TurnRuntime as ordinary chat;
- retired add/update/delete/toggle goal commands are absent from the frontend
  bridge and shipped Tauri handler surface.

## Mechanical evidence

Verified on 2026-07-15 in `/Users/tw/Desktop/open-life-roadshow`:

| Gate | Result | Credit boundary |
| --- | --- | --- |
| `cargo test -p openlife-core state_store -- --nocapture` | 15/15 passed | transaction/outbox atomicity, exact replay, request drift, schema migration, CAS, expiry, undo, restart, degraded projection, cancellation admission |
| `cargo test -p openlife-core transient_state -- --nocapture` | 2/2 passed | deterministic policy route, serialized-authority loss, sensitive/long-term/quoted/forged fail-closed cases |
| current compiled Tauri test binary, `transient_state` filter | 4/4 passed | one TurnRuntime, response-loss replay, no Provider/Tool dispatch, missing-store failure, concurrent projection, old-route absence |
| exact Tauri end-to-end state test through `send_message_with_operation_state` | passed | create/list/complete/undo, same-operation retry, minimal durable events, final after effect, empty derived view after tombstone |
| event payload registry coverage | passed | `effect_committed/state_effect` is registered with a strict minimal schema |
| state command compatibility tests | 2/2 passed | read-only legacy YAML migration input and existing state-history replay behavior |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` plus current compiled rerun | 30/30 passed | one runtime owner and deletion/authority guards |
| `cargo check -p openlife-tauri --tests` | passed with two existing warning groups | all Rust/Tauri test targets compile; no warning-free claim |
| frontend typecheck and full format check | passed | bridge/type and formatting integrity |
| focused ChatPage, TodayPage, and Tauri bridge tests | 116/116 passed | TurnRuntime command routing, retry identity, no direct goal write UI, due-time projection |
| current single-system set | 25/32 passed | V3 inventory/write-surface guard is green; seven unrelated pre-existing authority failures remain red |
| `cargo fmt --all --check`, `git diff --check`, JSON parse | passed | formatting, patch, and inventory syntax hygiene |

The compiled-binary reruns use the same current test executable produced by the
successful Cargo build. They avoid repeatedly filling the nearly full disk with
regenerated Rust archives; they are deterministic local evidence, not native
desktop product-trial credit.

## Failure and counterfactual evidence

- StateStore absence returns `state_store_unavailable_degraded`, emits a failed
  turn, and emits no Provider, Tool, or effect-committed fact;
- transaction failure before commit leaves zero asset, version, operation, and
  outbox rows;
- cancellation admission failure before commit leaves no canonical effect;
- reusing an operation id with a different semantic request or canonical
  payload fails instead of applying a second effect;
- exact completion retry works after the asset version advanced, and exact undo
  retry works after the asset left the active-task set as a tombstone;
- concurrent duplicate operations have one canonical winner; concurrent
  distinct version transitions have one CAS winner;
- concurrent state commits preserve both tasks in canonical StateStore and the
  YAML compatibility projection;
- a projection failure remains `projection_degraded` while the canonical
  receipt stays committed;
- EventStore facts and receipts contain ids, digests, versions, mutation kind,
  projection status, and timestamps, but not the task body;
- old ChatPage and shipped-command mutation paths are protected by an absence
  guard rather than merely left unused.

## Bounded red and remaining V3 evidence

The following results are intentionally not converted into green credit:

- `/state` observations currently enter TurnRuntime but fail closed as
  `state_observation_requires_typed_statestore_slice`; general metric/state
  assets are not yet implemented in StateStore;
- legacy unmarked YAML goals are shadow-read only. Per-asset import, digest
  parity, rollback rehearsal, and source-of-truth cutover have not run;
- no native desktop trial has yet proved add/list/complete/undo/restart behavior
  and UI/backend wording against an on-disk user profile;
- no independent read-only reviewer has re-traced the V3 source and evidence;
- the full single-system suite remains red in seven pre-existing areas:
  D011 marker drift, empty retired proposal category handling, ReviewWorkflow
  marker drift, MemoryGateway marker drift, Chat pending-state reconstruction,
  ActionExecutor test-surface classification, and ProviderPrivacy marker drift;
- the existing Rust warning groups remain; no Clippy-clean claim is made;
- cumulative stress, mixed-capability smoke, and roadshow trial gates remain
  pending.

V3 is therefore
`daily_task_slice_implementation_verified_migration_and_product_trial_pending`,
not fully complete, and the roadshow release remains NO-GO.
