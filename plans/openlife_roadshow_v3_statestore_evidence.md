# OpenLife Roadshow V3 StateStore Evidence

Status: bounded daily tasks, the typed short-lived `/state` observation slice,
legacy YAML daily-task shadow staging/parity/data-restore, atomic canonical
import, and shipped daily-task read-owner cutover are mechanically verified.
The parallel legacy MemoryStore state-history shadow/import/history-alert read
cutover is also mechanically verified through the existing StateStore and the
one Main Chat TurnRuntime. Independent read-only review and final backend
freeze evidence remain pending. Native packaging is outside the current
backend-freeze slice and receives no credit here. This file does not claim ADR
0015, Phase7, the roadshow release, or global backend remediation is complete.

## Scope and commits

The daily-task base is commit
`65a0017a7c2bf1ecba5ecdeb7484ad77f5e29793`. The typed observation extension
and old-route deletion are commit
`99d53a81d7cf0ebc85bd104eda3c1f594094bd7a` on
`codex/roadshow-core-recovery`. Legacy daily-task migration shadow evidence is
commit `d6ceb2f3232da5d5e1028d8264ed4cdd78910d59`. Legacy MemoryStore
state-history shadow evidence and StateStore schema v6 are commit
`e967c9a5913f8275591356afbc1e23380eb86893`. Atomic canonical import and
StateStore schema v7 are commit
`2d3c9a9ae8ed98a9049c59be23ed56cbf0b4266c`. The shipped history/alert read
cutover and old MemoryStore product-route deletion are commit
`ec616b8dc5d298ac710d8f8d2dee5debced3130e`. Lossless canonical time-block
support and StateStore schema v8 are commit
`0bace9c95befb5bd0b8e8cd28efd5a0bef60b134`; atomic daily-task import,
migration provenance, StateStore schema v9, shipped read cutover, and
permanent-merge deletion are commit
`411fe044a2dabd36aed86b2776235066fce21dec`.

The current scoped implementation establishes these facts:

- `StateStore` schema v9 is the one SQLite owner for bounded daily tasks and
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
- schema v8 adds canonical `time_block_start`/`time_block_end` fields to the
  task and immutable version rows. Existing databases migrate by actual column
  presence rather than trusting only the version marker, so an interrupted
  partial migration cannot duplicate columns;
- schema v9 rebuilds the task owner under foreign-key verification so imported
  rows carry `legacy_lifemodel_migration` provenance rather than fabricated
  current-user authorization. A real v8 fixture preserves its existing task
  through the rebuild;
- StateStore imports only the verified daily-task shadow. Canonical task rows,
  immutable create versions, a references/digests-only import mapping, one
  metadata-only import receipt, and one compatibility-projection outbox event
  commit in the same transaction;
- imported tasks use a documented seven-day migration retention window.
  Legacy `due_at` outside that bounded transient-state window blocks cutover
  instead of being truncated or silently reclassified as a long-term goal;
- exact import replay reuses the receipt and immutable version-1 snapshot.
  Injected failure leaves zero task, mapping, receipt, or outbox rows. A
  changed legacy source after cutover fails closed;
- shipped `get_daily_goals` validates the remaining YAML only as migration
  integrity evidence, then requires `StateStore::get_product_daily_tasks`.
  It no longer merges unmarked YAML with canonical tasks;
- the compatibility projector consumes the same receipt-gated StateStore read,
  removes the imported unmarked YAML source, and re-materializes canonical
  task state including the exact time block. New unmarked YAML after cutover
  degrades/fails instead of becoming a second product owner;
- current typed observations also create one ordered StateStore history fact in
  the same transaction as the observation/version/operation/outbox effect.
  Exact replay creates no second history row, and schema v5 databases backfill
  from their existing create versions before advancing through v6 to v7;
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
- StateStore imports only a verified shadow. Canonical legacy rows, one
  metadata-only import receipt, and one metadata-only outbox event commit in
  the same transaction. Exact replay reuses the receipt; source drift after
  cutover fails rather than replacing canonical history;
- injected import failure leaves zero canonical legacy rows, import receipt,
  or import outbox event. Retry after the proven pre-commit failure performs
  one import;
- shipped `get_state_history` and state alerts require the canonical import
  receipt and read `StateStore::get_product_state_history`. Missing or
  inconsistent ownership fails closed as a structured database/degraded
  error; there is no MemoryStore fallback;
- the product DTO `StateHistoryEntry` is owned by the StateStore module.
  MemoryStore consumes it only for bounded legacy migration reads, and its old
  state-history write helpers compile only under tests;
- daily-task shadow rows remain migration evidence only. Product task reads,
  Main Chat lifecycle operations, and YAML compatibility projection consume
  canonical StateStore assets after the verified import receipt exists;
- Main Chat buffered send and stream both use `OpenLifeTurnRuntime` and
  `StateGateway`; the typed state journey invokes no Provider, Tool,
  ActionQueue effect, or Proposal;
- the old `record_state` shipped Tauri command, frontend `recordState` bridge,
  MemoryGateway write adapter, mocks, and tests are deleted. The now-dead
  `persist_life_model` compatibility wrappers exposed by that deletion are also
  deleted;
- legacy MemoryStore history remains physically present as read-only
  migration/backout evidence, but it is no longer a shipped product read or
  write owner.

## Mechanical evidence

Verified on 2026-07-16 in `/Users/tw/Desktop/open-life-roadshow`:

| Gate | Result | Credit boundary |
| --- | --- | --- |
| `cargo test -p openlife-core state_store::tests -- --nocapture` | 49/49 passed | schema v1-v9 migration including real v8 task preservation/source-kind rebuild, lossless time blocks, both legacy shadow paths, both canonical imports, receipt/outbox atomicity, replay/source-drift/fault rollback, bounded retention, typed observations, global operation namespace, concurrency, CAS, cancellation, undo, expiry, restart, and minimal receipts |
| focused MemoryStore state-history source tests | 2/2 passed | exact payload-bound replay/source snapshot and 50,000-row overflow fail-closed boundary |
| `cargo test -p openlife-tauri legacy_yaml -- --nocapture` plus exact shipped daily-goal owner test | 4/4 plus 1/1 passed | lossless semantic mapping, invalid due-time fail-closed, per-category digest scope, real bootstrap shadow/import, missing-receipt failure, canonical read, exact time-block projection, and post-cutover YAML drift rejection |
| `cargo test -p openlife-tauri state_history_ -- --nocapture` | 7/7 passed | mapping validation, real bootstrap shadow/import, missing-receipt fail-closed behavior, history/alert StateStore reads, and product authority guard |
| `cargo test -p openlife-core main_chat_agent_v1 -- --nocapture` | 144/144 passed | deterministic PolicyRouter and broader Main Chat authority/runtime regression |
| exact typed state Tauri test | passed | buffered create, streamed list and undo, canonical receipt/event facts, zero Provider/Tool/Proposal/ActionQueue, tombstone truth |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | 30/30 passed | one runtime and deletion/authority absence guards |
| `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` | 97/97 passed | send/stream product command surface, atomic resource-task projection, and cross-process RC-05 task lifecycle after the new import contract |
| `cargo test -p openlife-tauri single_system -- --nocapture` | 34/34 passed | inventory, old-route absence, and StateStore-only shipped daily-task/history read authorities |
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
- daily-task canonical import preserves title, completion, due time, time
  block, and legacy operation references in the canonical owner while keeping
  its receipt/outbox body-free;
- an injected daily-task import failure leaves zero canonical tasks, mapping
  rows, receipt, or outbox event; retry creates one import;
- imported daily tasks carry migration provenance and a bounded seven-day
  expiry. A due time outside that window blocks the import;
- missing daily-task import receipt makes shipped reads fail closed. After
  cutover, a new unmarked YAML goal is rejected rather than merged;
- the exact canonical projector replaces the imported YAML source with one
  StateStore-derived compatibility view and preserves the time block;
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
- canonical import commits the legacy rows, metadata-only receipt, and one
  outbox event atomically; an injected failure leaves all three absent;
- exact import replay creates no second row or outbox event, while a changed
  source snapshot after cutover fails closed;
- missing import receipt makes shipped history and alert reads fail closed
  instead of returning the legacy store or a partial StateStore view;
- source and absence scans show no production MemoryStore state-history writer
  and no shipped history/alert MemoryStore read fallback.

## Remaining V3 evidence

The following remain explicitly red or uncredited:

- legacy MemoryStore state history has completed bounded shadow staging,
  profile-bound parity, atomic canonical import, outbox/receipt binding, and
  shipped history/alert read cutover. The physical legacy rows remain
  read-only backout evidence until the migration retention window is closed;
- no independent read-only reviewer has re-traced the typed observation source
  and evidence;
- existing Core Clippy warnings outside this slice remain;
- cumulative live product rounds and final roadshow acceptance remain pending
  outside this backend-only slice.

V3 is therefore
`daily_task_and_state_history_import_read_cutover_mechanical_verified_independent_review_pending`,
not fully complete, and the roadshow release remains NO-GO.
