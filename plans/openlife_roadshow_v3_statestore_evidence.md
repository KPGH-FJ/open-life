# OpenLife Roadshow V3 StateStore Evidence

Status: bounded daily tasks, the typed short-lived `/state` observation slice,
legacy YAML daily-task shadow staging, and legacy MemoryStore state-history
shadow staging/parity/data-restore rehearsal are mechanically verified through
the existing StateStore and the one Main Chat TurnRuntime. Daily-task authority
cutover, legacy state-history canonical import and product read cutover,
independent read-only review, and release evidence remain pending. Native
packaging is outside the current backend-freeze slice and receives no credit
here. This file does not claim ADR 0015, Phase7, the roadshow release, or global
backend remediation is complete.

## Scope and commits

The daily-task base is commit
`65a0017a7c2bf1ecba5ecdeb7484ad77f5e29793`. The typed observation extension
and old-route deletion are commit
`99d53a81d7cf0ebc85bd104eda3c1f594094bd7a` on
`codex/roadshow-core-recovery`. Legacy daily-task migration shadow evidence is
commit `d6ceb2f3232da5d5e1028d8264ed4cdd78910d59`. Legacy MemoryStore
state-history shadow evidence and StateStore schema v6 are commit
`e967c9a5913f8275591356afbc1e23380eb86893`.

The current scoped implementation establishes these facts:

- `StateStore` schema v6 is the one SQLite owner for bounded daily tasks and
  typed short-lived observations, including versions, operation receipts,
  lifecycle state, and canonical outbox facts;
- the exact command grammar is `/state <dimension> <numeric-value> <unit>`,
  `/state`, and `/state undo <dimension>`; malformed, sensitive, long-term,
  quoted/untrusted, or non-current-user inputs do not obtain direct commit
  authority;
- a non-serializable PolicyRouter grant binds the current user message,
  operation UUIDv4, exact typed intent, policy contract, and request digest;
- observation create/undo/expiry commits the asset, version, operation receipt,
  and outbox event in one transaction; injected cancellation before commit
  leaves all four absent;
- task, resource-task-batch, and observation operations share one UUID
  namespace. An operation id cannot name effects in two owner tables;
- exact replay returns the original receipt without a second write-admission
  window. Request or payload drift fails closed;
- active observations have a 24-hour TTL in the current command lane.
  StateGateway reconciles owner-controlled expiry before every canonical
  product read/write, and restart tests preserve the expiry tombstone;
- observation receipts and outbox events retain ids, digests, type, lifecycle,
  timestamps, and state only. They do not copy dimension/value/unit or the user
  message reference;
- observations currently require no derived compatibility projection, so their
  outbox has zero projection targets and the receipt reports `applied` as “no
  projection work required”. This is not a claim that YAML or HS was updated;
- daily tasks keep the existing outbox-driven YAML compatibility view. YAML is
  not a second product-write owner;
- startup maps only unmarked legacy YAML daily goals into a bounded migration
  shadow inside the existing StateStore database. The source digest covers
  only that asset category, so unrelated identity/preference changes do not
  invalidate daily-task evidence;
- StateStore validates and normalizes each shadow candidate, persists it,
  reads it back, recomputes the candidate digest, deletes the staged rows, and
  restores them before committing a metadata-only receipt. Any mismatch or
  injected failure restores the previously verified shadow snapshot;
- the migration shadow keeps only one body-bearing current snapshot. Historical
  evidence is body-free and bounded to 32 digest/count/status records;
- current typed observations also create one ordered StateStore history fact in
  the same transaction as the observation/version/operation/outbox effect.
  Exact replay creates no second history row, and schema v5 databases backfill
  from their existing create versions before advancing to v6;
- MemoryStore exposes one migration-only, ordered source snapshot bound to its
  stable canonical store identity. The read is capped at 50,000 rows plus one
  overflow sentinel; overflow, invalid timestamps, non-finite values,
  unordered ids, and incomplete or invalid legacy operation bindings fail
  closed;
- startup stages the complete legacy MemoryStore history snapshot in
  StateStore, rereads and hashes it, destructively removes and restores it in
  the same transaction, and commits only after parity and rollback rehearsal
  succeed;
- the state-history shadow also keeps one body-bearing current snapshot and at
  most 32 metadata-only evidence rows. Its receipt stores ids, counts, digests,
  timestamps, and states, not dimensions, values, units, notes, or operation
  references;
- the source digest includes the canonical MemoryStore identity, so identical
  rows transplanted across profiles cannot reuse another profile's parity
  receipt;
- shadow rows are excluded from `list_daily_tasks`, Main Chat, the shipped
  command surface, and the YAML projector. Existing unmarked YAML remains the
  read-only migration owner; no HS authority flag or product read owner was
  switched in this slice;
- Main Chat buffered send and stream both use `OpenLifeTurnRuntime` and
  `StateGateway`; the typed state journey invokes no Provider, Tool,
  ActionQueue effect, or Proposal;
- the old `record_state` shipped Tauri command, frontend `recordState` bridge,
  MemoryGateway write adapter, mocks, and tests are deleted. The now-dead
  `persist_life_model` compatibility wrappers exposed by that deletion are also
  deleted;
- read-only legacy state history remains the product read owner and migration
  input. Its rows have not been imported into canonical StateStore history,
  declared migrated, or deleted.

## Mechanical evidence

Verified on 2026-07-16 in `/Users/tw/Desktop/open-life-roadshow`:

| Gate | Result | Credit boundary |
| --- | --- | --- |
| `cargo test -p openlife-core state_store::tests -- --nocapture` | 41/41 passed | schema v1/v2/v3/v4/v5-to-v6 migration, current observation history atomicity/backfill, transaction/outbox atomicity, typed observation validation, replay/drift, global operation namespace, concurrency, CAS, cancellation, undo, expiry, restart, both shadow parity/restore/fault injection paths, operation-binding validation, bounded migration evidence, minimal receipts |
| focused MemoryStore state-history source tests | 2/2 passed | exact payload-bound replay/source snapshot and 50,000-row overflow fail-closed boundary |
| `cargo test -p openlife-tauri legacy_yaml -- --nocapture` | 5/5 passed | lossless semantic mapping, invalid due-time fail-closed, per-category digest scope, legacy YAML read ownership, real bootstrap shadow reconciliation |
| `cargo test -p openlife-tauri legacy_memory_state_history_shadow -- --nocapture` | 3/3 passed | every source field preserved, source identity included in digest, invalid timestamp and missing identity fail closed |
| focused Tauri bootstrap state-history shadow test | 1/1 passed | real on-disk MemoryStore source is staged while canonical StateStore history remains empty and the legacy product owner remains readable |
| `cargo test -p openlife-core main_chat_agent_v1 -- --nocapture` | 144/144 passed | deterministic PolicyRouter and broader Main Chat authority/runtime regression |
| exact typed state Tauri test | passed | buffered create, streamed list and undo, canonical receipt/event facts, zero Provider/Tool/Proposal/ActionQueue, tombstone truth |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | 30/30 passed | one runtime and deletion/authority absence guards |
| `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` | 96/96 passed | send/stream product command surface and all current roadshow command journeys |
| `cargo test -p openlife-tauri single_system -- --nocapture` | 32/32 passed | inventory and single-authority guards |
| `cargo check -p openlife-tauri --tests` | passed | all current Rust/Tauri test targets compile |
| frontend typecheck and focused Tauri bridge tests | typecheck passed; 44/44 tests passed | deleted bridge leaves no type/mock/test drift |
| frontend format, Rust format, diff, and JSON parse checks | passed | formatting, patch, and inventory syntax hygiene |
| `cargo clippy -p openlife-core --lib --no-deps` | completed with 35 existing cross-module warnings; no StateStore warning remains | no warning-free repository claim |

The exact Tauri test additionally verifies that the durable observation effect
uses `projectionStatus=applied`, while the body-bearing dimension/value/unit
exists only in canonical StateStore and the user-facing reply. It verifies
both positive behavior and the absence of a proposal, tool action, or provider
invocation.

## Failure and counterfactual evidence

- a nonnumeric value, missing unit, sensitive dimension, or quoted remote
  instruction cannot receive `TransientStateCommit`;
- a non-finite or out-of-range value, TTL outside 24 hours to seven days, or
  non-current-user create source fails before a transaction opens;
- an injected commit guard failure leaves zero observation, version, operation,
  receipt, and outbox rows;
- concurrent same-operation observation writes have one canonical winner and
  one replayed receipt;
- a task/batch operation UUID transplanted into the observation owner, or the
  inverse, fails with a namespace conflict;
- changing a payload or policy-sealed request under the same operation UUID
  fails rather than creating a second effect;
- undo and owner-controlled expiry create durable tombstones and remain visible
  after StateStore reopen;
- an already committed replay performs no second write admission;
- old `record_state` product strings are protected by shipped-handler,
  frontend-bridge, and implementation absence guards.
- the same legacy daily-task snapshot reuses one receipt; a source digest bound
  to different candidates fails as a collision rather than manufacturing new
  parity evidence;
- unrelated LifeModel categories do not change the legacy daily-task source
  digest, while a daily-task status change does;
- an injected pre-commit failure leaves the previous verified shadow snapshot
  intact; the receipt contains no title, time block, due time, or legacy
  operation reference;
- an exact current observation replay creates one StateStore history fact, and
  a v5 database with an existing typed observation backfills that fact before
  schema version v6 is committed;
- an identical legacy state-history source from a different MemoryStore
  identity produces a different source digest;
- invalid timestamps, non-finite values, more than 50,000 rows, unordered or
  duplicate legacy ids, incomplete operation bindings, and non-UUID/non-digest
  bindings cannot create a verified shadow;
- an injected state-history shadow commit failure preserves the previous
  verified body snapshot; repeated runs keep one body-bearing snapshot and 32
  metadata-only evidence rows;
- static callsite scans show no product reader of the shadow tables and no
  authority promotion/cutover call in this slice.

## Remaining V3 evidence

The following remain explicitly red or uncredited:

- unmarked legacy YAML daily tasks have completed bounded shadow staging,
  read-back digest parity, and shadow-data restore rehearsal, but have not been
  promoted into canonical product task rows and have not switched product read
  authority away from YAML;
- legacy MemoryStore state history has completed bounded shadow staging,
  read-back digest parity, destructive data-restore rehearsal, failure
  rollback, profile binding, and metadata minimization, but has not completed
  canonical import or source-of-truth cutover;
- the read-only legacy history/alert surfaces have not been replaced by a typed
  StateStore projection and must not be treated as proof that new observations
  were migrated;
- no independent read-only reviewer has re-traced the typed observation source
  and evidence;
- existing Core Clippy warnings outside this slice remain;
- signed/notarized release identity, healthy production Keychain, cumulative
  live product rounds, and final roadshow acceptance remain pending.

V3 is therefore
`daily_task_observation_legacy_daily_and_state_history_shadow_mechanical_verified_canonical_import_authority_cutover_and_review_pending`,
not fully complete, and the roadshow release remains NO-GO.
