# OpenLife Roadshow Core Capability Recovery

> Date: 2026-07-14
> Status: active time-bounded execution overlay
> Authority: subordinate to `AGENTS.md`, the Phase7 authority stack, Backend
> Remediation v4, ADR 0013, ADR 0014, and ADR 0015
> Baseline: `6704d6d1d56c26600b47f1bff68fe4ac743dbd96`

## 1. Purpose And Truth Boundary

This package restores the core user-facing Agent capabilities needed for an
OpenLife roadshow without creating a second runtime, policy system, persistence
authority, or readiness source. It selects the shortest transitive dependency
closure through the active Phase7 and Backend Remediation v4 work; it does not
replace either plan.

The following claims remain separate:

1. `slice_provisionally_green`: a bounded implementation slice has focused
   deterministic evidence but has not passed cumulative or live trial gates.
2. `roadshow_path_verified`: the exact roadshow path has deterministic,
   failure, lifecycle, live, and product-trial evidence.
3. `global_finding_closed`: a Backend Remediation v4 finding has independently
   satisfied all of its global closure evidence.

A roadshow path may become verified while a mapped global finding remains open.
This package must never translate the former into the latter.

## 2. Verified Starting Facts

At this baseline:

- ordinary buffered and streaming Main Chat share `OpenLifeTurnRuntime` after
  their transport wrappers, but Main Chat Agent Execution v1 remains in
  remediation;
- `PreparedProviderRequest`, `BoundedContextBlock`, and `ContextManifest`
  already exist and are the required document-context seam;
- `file.write_proposal` already enters proposal-first governance, but proposal
  creation is not file materialization or effect confirmation;
- ADR 0015 defines `StateStore` as a target canonical owner; product code does
  not yet implement the `StateStore` / `StateAsset` contract;
- BR4-D050-C and the frontend quick-command cutover are not started;
- BR4-D053 is not started;
- BR4-D008, D010, D011, D020, and D050 remain globally open;
- external live-provider, live-Web AgentLoop, and product-trial credit remain
  incomplete;
- the current roadshow branch is isolated from the mixed frontend-refactor
  worktree.

These are source-backed starting boundaries, not completion credit.

## 3. Roadshow Capability Contract

### 3.1 Hard Gates

The roadshow release candidate must support these real user journeys:

1. ordinary question answering, writing, rewriting, and plan decomposition;
2. true incremental response streaming with truthful terminal state;
3. upload/import and analysis of TXT, Markdown, JSON, common source text, PDF,
   DOCX, CSV, and XLSX, including bounded multi-file comparison and citations;
4. live Web search/fetch with source attribution and truthful live/fixture
   distinction;
5. creation, inspection, completion, undo, expiry, and restart recovery of
   transient daily tasks through the ADR 0015 StateStore lane;
6. explicit reversible Memory writes and undo under ADR 0014;
7. inferred Memory staged as a deduplicated, non-blocking deferred ReviewBatch;
8. Markdown and CSV output through the existing proposal/materialization path;
9. permission wait, resume, cancellation, retry, restart, and replay without
   duplicate dispatch or false completion;
10. consistency between network/tool/canonical facts, durable receipt, backend
    projection, and the product UI.

### 3.2 Conditional Gates

The following enter the roadshow candidate only if H0 proves the complete live
dependency chain and budget:

- image understanding through typed multimodal content parts;
- OCR/scanned-PDF extraction;
- PDF output generation.

Failure to admit a conditional gate is reported explicitly. It cannot be shown
or documented as supported.

### 3.3 Quarantined From The Roadshow Release

- MCP and A2A product capability;
- generic Computer Use and broad local process execution;
- generic connector/plugin installation;
- audio, video, and PPTX understanding;
- Deep Research and unbounded project knowledge bases;
- Scheduler-backed automation;
- general long-term TaskStore writes;
- VectorStore retrieval for roadshow attachments.

Quarantine means absent or disabled from the release product path, not silently
available behind an undocumented fallback.

## 4. Shared Invariants Before Feature Slices

All hard-gate journeys share these horizontal invariants:

- one ordinary Main Chat `OpenLifeTurnRuntime` owner;
- Policy decides authority and permission but does not execute effects;
- Provider routing occurs only after an allowed data-route decision;
- provider adapters receive prepared bounded requests, never raw LifeModel;
- all product tool execution uses ToolGateway;
- all canonical writes use the domain gateway;
- local cancellation forbids subsequent local tool and durable commits;
- remote state is `remote_unknown` unless cancellation is actually observed;
- terminal/durable facts are persisted before product emission;
- canonical content is not duplicated into events, AgentRun, receipts, or
  product read models;
- replay uses the original operation identity and has one dispatch winner;
- product UI formats backend projection truth and never authors completion;
- every new product path deletes the product authority it replaces.

The roadshow dependency subset includes:

- BR4-D008 canonical mutation/outbox for paths used here;
- BR4-D010 minimal content and product DTO boundaries for paths used here;
- BR4-D011 tombstone, late-write, and restart behavior for resources used here;
- BR4-D020 proposal/effect/AgentRun/Receipt/product consistency for artifact and
  governed-write paths used here;
- BR4-D050 stable operation, StateStore, and quick-command cutover;
- BR4-D051 current-message-only Memory authority;
- BR4-D053 non-blocking deduplicated inferred Memory review.

Passing the roadshow subset does not globally close these finding IDs.

## 5. Resource And Citation Architecture

### 5.1 Imported Attachments

Small imported attachment bytes use SQLite BLOB canonical ownership for the
roadshow lane. The same owner records resource metadata, digest, request/message
references, parser status, chunks, provenance, outbox, and tombstone state.

The ingest contract is:

```text
picker/import request
  -> ResourceGateway
  -> byte count + MIME magic + digest + expansion-limit validation
  -> bounded extractor with provenance
  -> canonical ResourceStore transaction
  -> deterministic bounded selection
  -> BoundedContextBlock + backend citation_id
  -> ContextManifest
  -> PreparedProviderRequest
  -> TurnRuntime
  -> validated ResourceCitation projection
```

No provider-specific PDF, DOCX, CSV, or XLSX upload route is added. Only images
may introduce a typed multimodal content part after the conditional H0 gate.

### 5.2 Generated Artifacts

Generated Markdown/CSV artifacts continue through `file.write_proposal` and
ReviewWorkflow. The filesystem owns materialized artifact bytes. Materialization
uses staged temporary bytes, `fsync`, and atomic rename; SQLite records the
proposal/effect status, digest, and reference.

Filesystem rename and SQLite commit are not described as one atomic
transaction. Crash recovery inspects staged/final bytes and digest, then
reconciles to `confirmed`, `failed`, or `unknown` without blind redispatch.

### 5.3 Deterministic Retrieval

Roadshow attachment retrieval does not call VectorStore. It uses a bounded,
deterministic selector with:

- Unicode-normalized exact phrase matching;
- Unicode-aware character n-grams suitable for Chinese and mixed text;
- BM25-style scoring over bounded chunks;
- field boosts for titles, PDF pages, DOCX sections, spreadsheet headers,
  sheets, and explicit ranges;
- stable tie-breaking by resource id and chunk ordinal;
- explicit per-file, per-turn, and request-context caps.

The selector must prove through a call/trace guard that no embedding or vector
route was invoked.

### 5.4 Citation Truth

An extractor emits structured provenance. Selection binds that provenance to a
backend-issued `citation_id`. The model may cite only supplied ids. The backend
validates every returned id against the current request, resource, chunk, and
provenance before producing `ResourceCitation`.

Minimum provenance:

- text/source: resource id and line/section span;
- PDF: resource id and page number;
- DOCX: resource id and paragraph or section ordinal;
- CSV: resource id and row/column range;
- XLSX: resource id, sheet, and cell/range.

Unsupported or unselected provenance cannot appear as a verified citation.

## 6. StateStore Architecture Boundary

ADR 0015 is implementation work, not an existing reusable service. The minimum
complete roadshow lane must provide:

- typed transient `StateAsset` and daily-task state;
- stable asset/version identity;
- UUIDv4 operation identity bound to canonical payload digest;
- current authenticated user message reference;
- risk, sensitivity, source, confidence, and privacy class;
- expiry between 24 hours and 7 days;
- canonical state mutation, minimal receipt, and outbox in one SQLite
  transaction;
- exact-replay receipt reuse and payload-drift rejection;
- concurrent single-winner CAS;
- undo and expiry tombstones;
- projection-degraded truth distinct from canonical failure;
- idempotent YAML compatibility materialization only after the canonical
  commit;
- deletion of React mutating quick-command authority and frontend-generated
  mutation success prose.

This lane does not use Scheduler and does not become a general long-term task
system.

## 7. Default Resource Budgets

H0 may tighten these values. Loosening one requires a versioned waiver and new
fault/performance evidence.

| Limit | Default |
| --- | ---: |
| files per message | 5 |
| bytes per file | 20 MiB |
| bytes per turn | 50 MiB |
| maximum expanded bytes | 100 MiB |
| compression expansion ratio | 20x |
| PDF pages | 300 |
| XLSX sheets | 20 |
| spreadsheet cells | 100,000 |
| parser wall time | 30 seconds |
| chunks per resource | 256 |
| selected context blocks | 32 |
| selected context characters | 262,144 |
| upload/import UI acknowledgement | under 300 ms |
| local cancellation acknowledgement | under 1 second |
| incremental peak memory above baseline | under 250 MiB |

Over-limit input is rejected with a typed reason or asks the user to reduce the
scope. Silent truncation is not allowed unless the UI and ContextManifest state
the exact truncation boundary.

## 8. Frozen Journey And Evidence Contract

The companion scenario file defines eight primary journeys and three combined
journeys. Every journey requires:

- positive behavior;
- negative/counterfactual behavior;
- failure/recovery behavior;
- lifecycle/restart behavior;
- deterministic contract evidence;
- backend canonical/receipt evidence;
- product projection/UI evidence;
- live external evidence when applicable;
- commit identity.

Evidence dimensions are independent:

```text
implementation
deterministic_contract
fault_injection
external_live
product_trial
independent_review
```

Each dimension is one of `not_applicable`, `pending`, `passed`, `failed`, or
`blocked`. A scenario is GREEN only when every applicable dimension is passed.

The exact prompts and expected outcomes are pre-frozen now. H0 must bind file
fixtures and their SHA-256 digests before product behavior changes. After that,
an expectation or fixture may change only through the waiver registry, which
retains the old version and human reason.

## 9. Execution Order And Gates

### H0 — Facts, Spikes, And Freeze (`T+0` to `T+3`)

No formal product behavior change is allowed before H0 exits.

Required outputs:

- current send/stream/provider/tool/resource/state/artifact source map;
- real provider, Web, streaming, and conditional vision probe results;
- PDF, DOCX, and XLSX parser library/version/license decision;
- parser sentinel plus page/section/sheet/cell provenance proof;
- malformed MIME, encrypted/corrupt file, ZIP expansion, XML external
  reference, timeout, and memory-bound proof;
- imported-resource canonical-byte decision confirmed against implementation;
- file proposal acceptance-to-materialization probe;
- StateStore implementation and migration dependency map;
- fixed `CARGO_TARGET_DIR`, dependency-fetch result, no-run warmup duration;
- scenario fixture digests and frozen rubric;
- MCP/A2A/Scheduler/vector roadshow route quarantine proof;
- current capability/performance/proposal-fatigue baseline;
- GO/NO-GO and updated P50/P80 estimate.

If any fact remains unknown at `T+3`, 34–36 hours cannot be claimed.

### H1 — Runtime Truth (`T+3` to `T+7`)

- provider terminal truth and captured HTTP parity;
- buffered/stream single runtime state machine parity;
- true token streaming and final-last ordering;
- cancellation, retry, replay, and restart identity;
- no local durable commit after cancellation;
- one dispatch winner across send/stream/retry;
- minimal AgentRun/event/receipt content;
- roadshow D020 effect/receipt consistency foundation.

### H2 — Governance (`T+7` to `T+10`)

- current-user-only explicit Memory authorization;
- file/Web/tool/quoted content cannot authorize Memory;
- inferred Memory does not block the answer;
- one deduplicated ReviewBatch per domain/session boundary;
- zero proposals for ordinary answer/read journeys;
- high-sensitivity and long-term truth remains proposal-first.

### V1–V4 — Vertical Capabilities (`T+10` to `T+20`)

- V1 ResourceGateway, extraction, deterministic retrieval, citations, cleanup;
- V2 live Web search/fetch and citation truth;
- V3 StateStore daily-task lane, projection, undo, expiry, restart;
- V4 Markdown/CSV proposal, materialization, digest, receipt, UI truth;
- frontend attachment, task, permission, artifact, and error states consume
  backend facts only.

Each slice enters `slice_provisionally_green` only with deterministic, failure,
lifecycle, replay/idempotency, backend-truth, UI-projection, and commit evidence.

### Cumulative Integration (`T+20` to `T+24`)

- all eight primary journeys;
- three combined journeys;
- negative injection and no-duplicate-content checks;
- single-system/absence guards;
- widened Rust and frontend regression;
- feature freeze at `T+24`.

### Reliability And Trial (`T+24` to `T+34`)

- 50 deterministic scenario loops;
- 20 concurrency/replay races;
- 20 mixed-capability smoke loops;
- parser/network/database/cancellation/restart fault injection;
- one to two hour desktop soak;
- two live roadshow rounds with real Provider, Web, files, tasks, permission,
  artifacts, cancellation, and restart;
- independent source/evidence rereview.

### Conditional Blocker Buffer (`T+34` to `T+36`)

This buffer repairs a verified blocker only. It cannot introduce a new feature
or weaken an evidence gate.

Current estimate before H0:

- 34–36 hours: conditional P50 after all H0 facts pass;
- 40–48 hours: current P80 risk range;
- 30–32 hours: aggressive only when H0 finds no infrastructure blocker;
- below 24 hours: not credible for the complete hard-gate contract.

## 10. Development Organization

- H0, H1, and H2 allow at most two modifying workers;
- after shared contracts freeze, at most three modifying workers;
- the Integration Owner owns hot shared files, merge decisions, status JSON,
  evidence classification, and all Cargo execution;
- feature workers may write tests and run non-Cargo static checks, but they do
  not run competing Rust builds;
- each worker uses an isolated branch/worktree and produces a small reversible
  commit;
- every 60–90 minutes the Integration Owner closes the merge window, locks the
  commit set, runs the queued gates, records evidence, then reopens integration;
- no `cargo clean`; one fixed target directory is warmed and reused;
- a tool with no output is polled after 10 minutes and diagnosed/stopped after
  20 minutes without activity;
- every 30 minutes an active worker reports hypothesis, files, test/evidence,
  and next action;
- 90 minutes without new mechanical evidence is `stalled`;
- three failures of the same root-cause hypothesis stop patching and trigger an
  architecture review.

Long-lived task context is not an authority. Handoffs use the current commit,
the state JSON, scenario evidence, and explicit blockers.

## 11. Anti-Hallucination Checkpoints

1. H0 claims use actual provider/network/parser/materialization observations.
2. Runtime truth is compared in this order: network capture, canonical store,
   durable receipt/event, backend projection, UI.
3. Resource proof checks actual bytes, digest, MIME, selected chunks,
   ContextManifest, provider payload, citation ids, and deletion/restart state.
4. Web proof distinguishes real HTTP from fixtures in the receipt and UI.
5. StateStore proof inspects canonical rows, operation digest, TTL, outbox,
   receipt, undo/expiry, YAML projection, and restart.
6. Artifact proof distinguishes proposal creation from accepted, dispatched,
   materialized, confirmed, unknown, and failed states.
7. Low-priority evidence cannot override a higher-priority failure.
8. An independent reviewer re-traces source and reruns gates; an LLM summary is
   not mechanical evidence.

## 12. Stop And Failure Rules

- A failed prerequisite stops downstream completion credit.
- A timeout/retry cannot hide deadlock, blocking I/O, or unknown external state.
- A new adapter cannot preserve the old authority as a hidden fallback.
- Scenario expectations cannot be edited to fit implementation without a
  versioned waiver.
- Conditional capability failure cannot be shown as supported.
- Hard-gate failure changes the forecast or blocks release; it does not lower
  the evidence standard.
- MCP, A2A, Scheduler, vector, and unrelated V4 work cannot consume the
  roadshow critical path.

## 13. Roadshow Definition Of Done

The roadshow candidate is GREEN only when:

- RC-01 through RC-08 pass twice in the live product;
- CC-01 through CC-03 pass once in the live product;
- every applicable evidence dimension is passed;
- ordinary answer/read journeys create zero proposals;
- high-risk effects interrupt before dispatch;
- local cancellation is observed within one second;
- there are zero late durable commits, duplicate dispatches, false completion
  states, or permanent permission waits;
- 50 deterministic loops, 20 concurrency/replay races, 20 mixed smokes, and the
  desktop soak pass;
- canonical store, receipt, projection, and UI facts agree;
- single-system and old-route absence guards pass;
- MCP/A2A/Scheduler/vector roadshow product routes are absent or quarantined;
- the integration branch is clean and every accepted slice is a reversible
  commit;
- remaining global V4 findings and Phase7/product-trial boundaries are reported
  honestly.

Roadshow GREEN is not Phase7 completion, Backend Remediation v4 completion, or
universal external capability readiness.
