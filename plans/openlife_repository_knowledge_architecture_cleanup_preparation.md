# OpenLife Repository Knowledge Architecture Cleanup Preparation

> Date: 2026-07-07
> Status: preparation artifact only; not implementation completion
> Authority: proposed cleanup contract subordinate to `AGENTS.md`,
> `plans/README.md`, `plans/openlife_single_system_deletion_manifest.md`, and
> `plans/openlife_single_system_development_preparation.md`.

This document adapts the external repository knowledge cleanup plan to the
current OpenLife checkout. It is intentionally a preparation artifact: it does
not authorize runtime changes, Rust/Tauri architecture changes, source
directory moves, LifeModel schema changes, or deletion of historical knowledge.

Do not add this file to the active authority list until the current Phase7
single-system worktree is either committed or explicitly paused. The current
worktree contains a large Phase7 deletion/cleanup diff, so repository knowledge
cleanup must avoid competing with that product-route cleanup.

## 1. Preparation Objective

Prepare a repository knowledge cleanup that makes OpenLife easier for humans
and AI agents to maintain without changing product behavior.

The target is not a new software architecture. The target is a cleaner
knowledge architecture:

- stable AI coding entry point;
- explicit source-of-truth ownership;
- current architecture references that match code;
- preserved historical plans without letting them steer new work;
- bounded public product/development documentation;
- repeatable validation before any document reorganization is accepted.

## 2. Current Repository Facts

These facts were verified from the current checkout before writing this
preparation artifact.

| Area | Current fact | Preparation implication |
| --- | --- | --- |
| Worktree | Branch is `codex/openlife-product-core-baseline` with a large dirty diff, including many Phase7 deletions and new files. | Do not move broad doc trees or rewrite authority docs in the same unverified batch. |
| Markdown/HTML surface | `git ls-files '*.md' '*.html'` reports 190 tracked Markdown/HTML files. | The cleanup problem is real; the repo has far more historical text than current authority. |
| Plans surface | `plans/` has 163 tracked plan/doc/json files and only one subdirectory, `plans/adr/`. | A plan lifecycle system is needed, but mass-moving plans before authority guards are green is risky. |
| Docs surface | `docs/` has 11 files and only one subdirectory, `docs/decisions/`. | The proposed `docs/architecture`, `docs/development`, and `docs/product` ownership boundaries are not present. |
| AI entry point | Root `AGENTS.md` already exists and is 882 lines. | Do not create it; compress and stabilize it after the active Phase7 authority is preserved. |
| Public README | Root `README.md` is already compact at 66 lines and points to the current Phase7 authority set. | README cleanup is not the main blocker; it may only need a documentation map after cleanup. |
| Existing doc governance | `docs/repository_document_governance.md` already defines public/local/private document rules. | Reuse it instead of inventing a competing governance model. |
| Current plan authority | `plans/README.md` is the current authority map for Phase7 and names the active single-system docs. | Keep it as the authority map; do not replace it with generic `plans/active` prematurely. |
| Current deletion manifest | `plans/openlife_single_system_deletion_manifest.md` records Phase7 dispositions and says the trial is still `red-until-trial-green`. | Cleanup must not claim Phase7 complete or weaken old-route guards. |
| ADRs | `docs/decisions/0001-0003` exist, while accepted ADR 0013 lives in `plans/adr/`. | ADR cleanup is consolidation/indexing, not creating duplicate ADR-001 style records. |
| GitHub governance | `.github/PULL_REQUEST_TEMPLATE.md`, `.github/CODEOWNERS`, and `.github/ISSUE_TEMPLATE/04_adr_proposal.yml` already exist. CI does not currently run a Markdown link checker. | Reuse existing review/publication gates and add doc-link validation instead of inventing a separate process. |
| Governance snapshot | `docs/repository_document_governance.md` still records the older 2026-06-16 document surface count. | Refresh this before relying on it as the cleanup source of truth. |

## 3. Verified Mismatch With The External Plan

| External plan item | Current repo reality | Correct preparation decision |
| --- | --- | --- |
| Create root `AGENTS.md`. | `AGENTS.md` already exists and is overgrown with roadmap, progress, and historical logs. | Compress and refocus it; do not create a second entry point. |
| Create `docs/architecture`, `docs/development`, `docs/product`. | Those directories do not exist. | Create only when populated by real content; no empty folders. |
| Create `docs/architecture/agent-runtime.md`, `life-model.md`, `governance.md`, `memory.md`. | Current `docs/ARCHITECTURE.md` is stale in places and still references old paths. | First replace or split stale architecture content from current source scans. |
| Create ADR system under `docs/decisions`. | ADR system exists but is split; ADR 0013 is under `plans/adr/`. | Add ADR index/consolidation plan before moving files. Avoid duplicate decisions. |
| Cleanup README as public entry point. | README is already a compact current-authority entry point. | Keep it small; optionally add links after new doc structure exists. |
| Restructure `plans/active/...` and `plans/archive`. | Current authority is `plans/README.md` plus Phase7 manifest/inventory; no active/archive tree exists. | Use classification and status first. Defer mass moves until current guards pass. |
| Create product docs `vision.md` and `scenarios.md`. | Product definition exists across root PRDs, historical Beta docs, and current product pages. | Create public product docs only after privacy/publication review; do not copy confidential PRD text. |
| Create `docs/development/testing.md`. | Testing commands are scattered across README, AGENTS, plans, and historical docs. | This is a good first concrete doc once commands are checked against current package scripts. |

## 4. Current Problems That Are Real

### P0: Authority worktree is not clean

The repository is already in a large Phase7 cleanup diff. A knowledge cleanup
that edits many active docs or moves many files could obscure whether Phase7
old-route deletion is correct.

Required handling:

- keep this preparation artifact separate from runtime/source cleanup;
- do not claim the current code state is final until Phase7 gates run;
- if this cleanup proceeds before Phase7 is committed, keep edits limited to new
  docs or clearly scoped doc-only changes.

### P0: Runtime-module authority guard is red

The preparation snapshot records a failing
`cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` guard.
That failure is not caused by documentation cleanup. It is still a hard signal
that the current Phase7 authority boundary is not green.

Required handling:

- do not promote repository-knowledge cleanup into active authority while this
  guard is red;
- an implementation slice may proceed only if it is explicitly docs-only and
  records that this guard is an inherited blocker, not resolved evidence;
- before any cleanup claim says "ready" or "complete", the guard must either
  pass or be formally scoped out by a reviewed Phase7 decision.

### P1: `AGENTS.md` is not a stable instruction layer

`AGENTS.md` should be the AI coding rule layer. It currently also contains:

- long Main Chat live-provider/eval progress details;
- W-series history and roadmap status;
- stale module listings, including deleted `router.rs`, `layer_router.rs`,
  `multi_strategy_runtime.rs`, and `runtime_migration_gate.rs`;
- detailed testing/progress history that belongs in plans or archives.

Preparation decision:

- reduce `AGENTS.md` to identity, current authority, hard boundaries,
  verification rules, and concise active caveats;
- move historical timeline/status references to a plan archive/index instead of
  leaving them in the AI entry point;
- retain the current hard constraints around no silent writes, proposal-first
  mutation, metadata-safe evidence, and Phase7 single-system authority.

### P1: Architecture docs contain stale paths

Observed stale references include:

- `docs/ARCHITECTURE.md` still describes `IntentRouter` and `LayerRouter` as the
  Chat flow;
- `docs/ARCHITECTURE.md` and `docs/DEV_HANDOVER.md` mention `hermes.rs`;
- `docs/ARCHITECTURE.md` links to missing `docs/ARCHITECTUREDETAILED.md` and
  missing `docs/api/`;
- `architecture_diagram.md` is explicitly a 2026-05-01 snapshot and still shows
  old router state.

Preparation decision:

- keep historical snapshots only if labeled historical;
- make the current architecture explainer source-backed and centered on
  `OpenLifeTurnRuntime`, `MainChatKernel`, current product read model,
  proposal/governance, memory, model routing, and tool gateways;
- remove or replace broken links before marking docs current.

### P1: Plans are lifecycle-mixed

The plan surface mixes active authority, completed audit trails, preparation
contracts, historical PRDs, design-only plans, implementation reports, and
debug/diagnosis packets.

Preparation decision:

- do not mass move plans in the dirty Phase7 branch;
- add a classification pass first;
- keep `plans/README.md` as the current authority map;
- later move or archive only after references and guards are updated.

### P2: ADR ownership is split

`docs/decisions/0001-0003` are present and partly historical. ADR 0013 is
accepted but lives under `plans/adr/`.

Preparation decision:

- add an ADR index before moving;
- preserve ADR numbers and links;
- do not create duplicate ADR-001/002/003/004 documents;
- decide whether `plans/adr/0013...` is moved to `docs/decisions/0013...` or
  kept with an explicit pointer.

### P2: Product/development docs are not bounded enough

Root PRDs, Beta checklists, user guides, dogfood notes, and product contracts
exist, but current/public versus historical/local boundaries are uneven.

Preparation decision:

- use `docs/repository_document_governance.md` as the publication rule;
- create `docs/product/` only for concise public product vision/scenarios;
- create `docs/development/testing.md` for current command semantics and
  acceptance/eval distinctions;
- keep private/product-draft material local-only unless explicitly promoted.

### P2: Link and publication validation are not automated enough

The repository already has broken local documentation references and a CI comment
that points at a missing decision doc. A cleanup that moves files without a
repeatable local-link check can easily make the repository less reliable.

Preparation decision:

- make Markdown local-link validation a hard cleanup acceptance item;
- treat broken active-doc links as blockers unless they are in an explicit
  historical allowlist;
- connect cleanup PRs to the existing PR template risk checks instead of relying
  on ad hoc reviewer memory.

## 5. Proposed Source-Of-Truth Ownership

| Knowledge type | Owner after cleanup | Current state | Notes |
| --- | --- | --- | --- |
| Public entry | `README.md` | Mostly current and compact. | Add doc map only after target docs exist. |
| AI coding rules | `AGENTS.md` | Exists but overloaded. | Compress; no long history. |
| Doc/publication policy | `docs/repository_document_governance.md` | Exists and useful. | Refresh current repository notes after cleanup. |
| Current plan authority | `plans/README.md` | Exists and Phase7-specific. | Preserve as active map. |
| Single-system cleanup contract | `plans/openlife_single_system_*` | Exists and active. | Do not weaken or replace. |
| Architecture explanation | `docs/architecture/*.md` | Missing; current `docs/ARCHITECTURE.md` stale. | Split or replace from source-backed content. |
| Architecture decisions | `docs/decisions/*.md` plus ADR 0013 handling | Split. | Add index/consolidate. |
| Product principles | `docs/product/*.md` | Missing. | Create only public-safe summaries. |
| Development/testing guide | `docs/development/testing.md` | Missing. | Good first doc after command verification. |
| Historical plans | `plans/archive/` or status-classified `plans/` | Not physically separated. | Defer mass move until references/gates are green. |
| PR/publication checks | `.github/PULL_REQUEST_TEMPLATE.md` | Exists and already asks for Markdown public-safety and authority sync checks. | Reuse for cleanup acceptance; update only if cleanup changes authority rules. |
| Ownership checks | `.github/CODEOWNERS` | Exists but is intentionally simple and still points ADR 0013 at `plans/adr/`. | Update with any ADR canonical-path change. |
| Link validation | CI/local script | Missing as a dedicated docs gate. | Add or document a deterministic local-link check before broad moves. |

### 5.1 External Practice Anchors

Use these as practice patterns, not imported authority:

- Rust RFCs keep substantial changes on a consistent, statused design path while
  ordinary documentation improvements can use normal PR review:
  <https://github.com/rust-lang/rfcs>.
- Kubernetes uses OWNERS files to make directory responsibility and review
  authority explicit: <https://www.kubernetes.dev/docs/guide/owners/>.
- ADR practice treats each decision as a record of context, decision, and
  consequences, and the collection as a decision log:
  <https://github.com/architecture-decision-record/architecture-decision-record>.
- Markdown link checking should be automated or at least repeatable before file
  moves: <https://github.com/tcort/markdown-link-check>.

## 6. Execution Sequence For The Cleanup

### Phase A: Freeze and classify

Scope:

- no Rust/Tauri/React behavior edits;
- no broad file moves;
- no new active authority unless the user approves it.

Tasks:

- generate a document inventory from `rg --files -g '*.md' -g '*.html'`;
- encode the inventory in a reviewed artifact, preferably
  `plans/openlife_repository_document_inventory.md` or `.json`;
- use this minimum inventory schema:
  `path`, `title`, `class`, `authority_rank`, `status`, `public_safety`,
  `owner_surface`, `source_refs`, `stale_refs`, `planned_action`,
  `link_impact`, and `validation`;
- classify every root/doc/plan document as `public_entry`, `current_authority`,
  `stable_reference`, `preparation`, `historical_reference`, `local_private`, or
  `remove_after_review`;
- identify broken links and missing local path references;
- identify old route/runtime references that must be historical-only;
- refresh `docs/repository_document_governance.md` current repository notes
  before using it as the cleanup baseline.

Acceptance:

- inventory is reviewed and uses the schema above;
- no active docs point to missing files as current references;
- broken-link baseline and historical-link allowlist are explicit;
- `plans/README.md` continues to name the current single-system authority.

### Phase B: Stabilize entry points

Tasks:

- compress `AGENTS.md` to stable project rules, current authority, hard
  boundaries, and verification commands;
- keep `README.md` compact and public;
- update `docs/repository_document_governance.md` current repository notes;
- reuse `.github/PULL_REQUEST_TEMPLATE.md` risk checks for docs cleanup PRs;
- update `.github/CODEOWNERS` only if canonical document ownership paths change.

Acceptance:

- `AGENTS.md` no longer contains long W-series history or stale deleted module
  lists;
- `AGENTS.md` is short enough to scan as an AI entry point, with a target of
  under 250 lines unless the user explicitly accepts a longer file;
- historical progress is preserved in plans/archive/index form;
- no current entry point directs agents to deleted `router.rs`, `layer_router.rs`,
  `multi_strategy_runtime.rs`, or `runtime_migration_gate.rs` as live code.

### Phase C: Build real docs ownership

Tasks:

- create `docs/architecture/agent-runtime.md`;
- create `docs/architecture/life-model.md`;
- create `docs/architecture/governance.md`;
- create `docs/architecture/memory.md`;
- create `docs/development/testing.md`;
- optionally create `docs/product/vision.md` and `docs/product/scenarios.md`
  after public-safety review.

Acceptance:

- each new doc starts with `Status`, `Authority`, `Last verified`, and
  `Source map`;
- each new doc contains source-backed current project knowledge, where the
  source map names the code modules, tests, ADRs, and active plans that justify
  the claims;
- claims derived only from historical documents are labeled historical and are
  not written as current runtime facts;
- no empty docs directories;
- old `docs/ARCHITECTURE.md` is either converted into an index or marked
  historical and no longer has broken links.

### Phase D: ADR consolidation

Tasks:

- create an ADR index under `docs/decisions/README.md`;
- classify `docs/decisions/0001-0003` as accepted/historical/superseded where
  appropriate;
- decide and execute one approach for ADR 0013:
  - move it into `docs/decisions/0013-lifemodel-hs-source-of-truth-governance.md`
    with updated links, or
  - leave it in `plans/adr/` and add a canonical index pointer.
- if ADR 0013 moves, update `.github/CODEOWNERS`,
  `.github/ISSUE_TEMPLATE/04_adr_proposal.yml`,
  `.github/ISSUE_TEMPLATE/config.yml`, active docs, and any plan references in
  the same slice;
- if ADR 0013 stays in `plans/adr/`, record `docs/decisions/README.md` as the
  canonical decision log and point to the existing file without duplicating it;
- ensure each ADR index row records status, date, canonical path, supersedes or
  superseded-by relationship, and current authority impact.

Acceptance:

- no duplicate ADR numbers;
- current LifeModel-HS governance remains anchored to ADR 0013;
- links from AGENTS/plans/docs/GitHub templates/CODEOWNERS are updated.

### Phase E: Plans lifecycle cleanup

Tasks:

- keep `plans/README.md` as the active authority map;
- add a machine-readable or tabular classification of plan files;
- only after link impact is understood, move historical plans into
  `plans/archive/` or keep them in place with explicit historical status;
- do not move active Phase7 files until Phase7 is complete or explicitly
  superseded.

Acceptance:

- active current work is obvious within one minute;
- old Goal/Stage/Beta/Migration docs cannot override active authority;
- no current command/test/doc references break.

## 7. Required Validation

Run the smallest validation set for preparation-only changes:

```sh
git diff --check
python3 -m json.tool plans/openlife_single_system_phase1_inventory.json >/tmp/openlife_phase1_inventory_pretty.json
rg -n "ARCHITECTUREDETAILED|docs/api|router\\.rs|layer_router|hermes\\.rs|multi_strategy_runtime|runtime_migration_gate" README.md AGENTS.md docs plans/README.md
rg -n "run_multi_strategy_agent_preview|check_runtime_migration_gate|get_react_beta_execution_status" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts
```

For the old-command scan, a no-match exit status is the desired result for the
shipped handler/product bridge surface above. Expected matches in dev/test-only
surfaces such as `frontend/src/tauriDev.ts`, frontend tests, or single-system
guards must remain explicitly classified as test/dev/archive-only.

For the stale-doc scan, the preparation-time result may contain expected hits.
After cleanup, active entry points must either have zero hits or list each
remaining hit in a historical allowlist. Do not treat raw `rg` output as a pass
without interpreting the target surface.

Run or add a deterministic local Markdown link check before merging any slice
that moves, deletes, or retargets documentation files. Until a checker is added
to CI, record the exact local command and output summary in the cleanup slice.

Run the authority guard set before promoting cleanup to active authority:

```sh
cargo fmt --check
cargo test -p openlife-tauri single_system -- --nocapture
cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture
```

Run frontend checks if `README.md`, `AGENTS.md`, `frontend/src/tauri.ts`, or
frontend-facing docs/types are touched:

```sh
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
```

## 8. Current Validation Snapshot

Validation run during this preparation pass:

| Check | Result | Meaning |
| --- | --- | --- |
| Whitespace checks | Passed | `git diff --check` has no tracked-diff whitespace errors, and this untracked preparation file has no trailing whitespace or CRLF. |
| `python3 -m json.tool plans/openlife_single_system_phase1_inventory.json` | Passed | The current Phase7 inventory remains parseable JSON. |
| Static scan for old commands in `src-tauri/src/lib.rs`, `src-tauri/src/commands`, and `frontend/src/tauri.ts` | Passed with no matches | Retired migration/beta/stage command wrappers are not in the shipped handler or product bridge. |
| Static scan for stale doc references in `README.md`, `AGENTS.md`, `docs`, and `plans/README.md` | Found expected hits | Stale references remain in `AGENTS.md`, `docs/ARCHITECTURE.md`, and `docs/DEV_HANDOVER.md`; these are cleanup targets, not preparation blockers. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests | Current single-system authority guards still pass. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed, 24 passed and 2 failed | Current Phase7 runtime-module guard is not green. It still expects `src/main_chat_final_acceptance_tests.rs`, which is deleted in the working tree, and expects the final acceptance runner to call the reusable final-gate aggregation module from `src/commands/agent_runtime/mod.rs`. |

The failed runtime-module guard is an implementation/test-boundary blocker for
promoting this cleanup into active work. It should be fixed in the Phase7
runtime-module cleanup context, not hidden inside repository documentation
reorganization.

## 9. Explicit Non-Goals

This cleanup preparation does not include:

- Rust module refactoring;
- Tauri command migration;
- frontend product behavior changes;
- LifeModel schema or data migration;
- provider/runtime/live eval changes;
- deleting historical knowledge without classification;
- treating documentation cleanup as Phase7 completion evidence.

## 10. Ready-To-Start Checklist

Start actual cleanup only when all are true:

- the user explicitly approves entering the cleanup implementation;
- the current Phase7 branch status is understood and either committed or
  intentionally kept dirty;
- the red `main_chat_runtime_module` guard is either green or explicitly
  recorded as an inherited blocker that this docs-only slice does not resolve;
- the first implementation slice is docs-only and has a rollback-friendly scope;
- the slice names the exact files it will touch before editing;
- Phase A inventory schema, owner surface, link baseline, and stale-reference
  allowlist are available for the slice;
- active authority docs are protected from stale or duplicate source-of-truth
  claims;
- any ADR canonical-path change also updates GitHub templates and CODEOWNERS;
- validation commands for that slice are known before work starts.
