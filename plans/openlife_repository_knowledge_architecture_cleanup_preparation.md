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

## Stage5B Current Status Override

Stage2/Stage3/Stage4 validation rows in this document are preserved as original
time-point records. Rows that say
`cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` failed as
an inherited blocker are superseded for current status by the Stage5A run.

Stage5A repaired the runtime-module guard to check the current Phase7 owner
shape: reusable final-gate aggregation and live-provider report builders live in
`src-tauri/src/main_chat_final_gate.rs`, live-provider harness contract tests
live in `src-tauri/src/main_chat_live_provider_tests.rs`, and the retired final
acceptance command/test owner remain absent. The current run of
`cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` passes.

This current-status override only removes the inherited runtime-module blocker.
It does not claim Phase7 completion, Main Chat Agent Execution v1 completion,
external live-provider evidence completion, link-baseline recomputation, ADR
movement, plan archival readiness, or authority promotion.

Do not add this file to the active authority list until the current Phase7
single-system worktree is either committed or explicitly paused. The current
worktree contains a large Phase7 deletion/cleanup diff, so repository knowledge
cleanup must avoid competing with that product-route cleanup.

## 0. Development Readiness Decision

As of 2026-07-07, this preparation is not a broad implementation green light.
It only supports a narrow Phase A documentation-baseline slice if the user
explicitly approves that exact slice.

| Work type | Current decision | Reason |
| --- | --- | --- |
| Phase A inventory and link baseline | May start as docs-only work | It creates missing evidence without changing product behavior or active runtime authority. |
| `AGENTS.md` compression | Not yet ready for direct edit | It needs a reviewed inventory and stale-claim allowlist first, because it is the highest-risk active AI entry point. |
| Architecture doc split or replacement | Not yet ready | Current architecture claims must be source-mapped before new `docs/architecture/*` files are written as current truth. |
| ADR moves | Not yet ready | Moving ADR 0013 must update GitHub templates, CODEOWNERS, active docs, and plan references in one reviewed slice. |
| Plan archive or broad file moves | Not yet ready | The Phase7 worktree is dirty and link impact has not been baselined. |
| Runtime/source code changes | Out of scope | This document is a repository knowledge cleanup preparation, not a Phase7 runtime repair plan. |
| Active authority promotion | Blocked | Stage5A has cleared the inherited `main_chat_runtime_module` blocker, but this preparation artifact still does not promote authority; Phase7 completion, Main Chat completion, external live-provider evidence, and reviewed authority-promotion criteria remain incomplete. |

The first executable cleanup slice should touch only this preparation area and
new baseline artifacts such as `plans/openlife_repository_document_inventory.*`
and `plans/openlife_repository_document_link_baseline.*`. It must not edit
Rust/Tauri/React product behavior.

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
| Markdown/HTML surface | `git ls-files '*.md' '*.html'` reports 191 tracked Markdown/HTML files, including this preparation artifact. | The cleanup problem is real; the repo has far more historical text than current authority. |
| Plans surface | `plans/` has 164 tracked plan/doc/json files, including this preparation artifact, and only one subdirectory, `plans/adr/`. | A plan lifecycle system is needed, but mass-moving plans before authority guards are green is risky. |
| Docs surface | `docs/` has 11 files and only one subdirectory, `docs/decisions/`. | The proposed architecture, development, and product-doc ownership boundaries are not all present. |
| AI entry point | Root `AGENTS.md` already exists and is 882 lines. | Do not create it; compress and stabilize it after the active Phase7 authority is preserved. |
| Public README | Root `README.md` is already compact at 66 lines and points to the current Phase7 authority set. | README cleanup is not the main blocker; it may only need a documentation map after cleanup. |
| Existing doc governance | `docs/repository_document_governance.md` already defines public/local/private document rules. | Reuse it instead of inventing a competing governance model. |
| Current plan authority | `plans/README.md` is the current authority map for Phase7 and names the active single-system docs. | Keep it as the authority map; do not replace it with a generic active-plan namespace prematurely. |
| Current deletion manifest | `plans/openlife_single_system_deletion_manifest.md` records Phase7 dispositions and says the trial is still `red-until-trial-green`. | Cleanup must not claim Phase7 completion or weaken old-route guards. |
| ADRs | The ADR 0001, 0002, and 0003 decision files exist under `docs/decisions/`, while accepted ADR 0013 lives in `plans/adr/`. | ADR cleanup is consolidation/indexing, not creating duplicate ADR-001 style records. |
| GitHub governance | `.github/PULL_REQUEST_TEMPLATE.md`, `.github/CODEOWNERS`, and `.github/ISSUE_TEMPLATE/04_adr_proposal.yml` already exist. CI does not currently run a Markdown link checker. | Reuse existing review/publication gates and add doc-link validation instead of inventing a separate process. |
| Governance snapshot | `docs/repository_document_governance.md` still records the older 2026-06-16 document surface count. | Refresh this before relying on it as the cleanup source of truth. |

## 3. Verified Mismatch With The External Plan

| External plan item | Current repo reality | Correct preparation decision |
| --- | --- | --- |
| Create root `AGENTS.md`. | `AGENTS.md` already exists and is overgrown with roadmap, progress, and historical logs. | Compress and refocus it; do not create a second entry point. |
| Create architecture, development, and product-doc ownership boundaries. | Those ownership boundaries were not all present at preparation time. | Create only when populated by real content; no empty folders. |
| Create `docs/architecture/agent-runtime.md`, `life-model.md`, `governance.md`, `memory.md`. | Current `docs/ARCHITECTURE.md` is stale in places and still references old paths. | First replace or split stale architecture content from current source scans. |
| Create ADR system under `docs/decisions`. | ADR system exists but is split; ADR 0013 is under `plans/adr/`. | Add ADR index/consolidation plan before moving files. Avoid duplicate decisions. |
| Cleanup README as public entry point. | README is already a compact current-authority entry point. | Keep it small; optionally add links after new doc structure exists. |
| Restructure plans into active/archive namespace families. | Current authority is `plans/README.md` plus Phase7 manifest/inventory; no active/archive tree exists. | Use classification and status first. Defer mass moves until current guards pass. |
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

### P0: Runtime-module authority guard was red in the preparation snapshot

The preparation snapshot records a failing
`cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` guard.
That failure was not caused by documentation cleanup. Stage5A later repaired
the guard to match the current Phase7 owner shape, and the current run passes.

Required handling:

- do not treat Stage2/Stage3/Stage4 validation rows as current runtime-module
  truth after Stage5A;
- an implementation slice may proceed only if it is explicitly docs-only and
  records that Stage5A only removed the inherited runtime-module blocker;
- before any cleanup claim says Phase7, Main Chat Agent Execution v1, or
  external live-provider evidence is ready or complete, the separate authority
  and live-provider gates must pass.

### P0: Active authority has unverified current-runtime claims

The current `AGENTS.md` is the active AI entry point, but it contains progress
claims that must be re-verified before being reused as current truth. One
example is the Main Chat stabilization paragraph that says the final acceptance
runner calls the reusable final-gate aggregation module and that final
acceptance tests live in the deleted old final-acceptance test-owner file.
The current `main_chat_runtime_module` guard still expects that older shape,
while the Phase7 deletion manifest classifies
`run_main_chat_agent_execution_v1_final_acceptance_gate` as retired. That
conflict must be resolved by a reviewed Phase7 decision or guard update, not by
blindly restoring a retired shipped command.

Required handling:

- treat long progress paragraphs in `AGENTS.md` as candidate historical/current
  claims until source and guard checks prove each one;
- do not copy `AGENTS.md` runtime-progress text into new docs without a source
  map and a passing or explicitly-scoped guard;
- Phase A inventory must mark these active-entry stale claims explicitly, rather
  than letting them become hidden assumptions in the cleanup plan.

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
- `docs/ARCHITECTURE.md` links to obsolete detailed-architecture and API-doc
  targets that should be retargeted or removed instead of created;
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

The ADR 0001, 0002, and 0003 decision files are present and partly historical.
ADR 0013 is accepted and remains canonical at
`plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` by Stage4A no-move
decision.

Preparation decision:

- add an ADR index before any future move;
- preserve ADR numbers and links;
- do not create duplicate ADR-001/002/003/004 documents;
- keep ADR 0013 at the existing `plans/adr/` canonical path for Stage4A and
  use the ADR index as the canonical pointer.

### P2: Product/development docs are not bounded enough

Root PRDs, Beta checklists, user guides, dogfood notes, and product contracts
exist, but current/public versus historical/local boundaries are uneven.

Preparation decision:

- use `docs/repository_document_governance.md` as the publication rule;
- create public product docs only for concise public product vision/scenarios;
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
| Architecture decisions | `docs/decisions/*.md` plus ADR 0013 no-move pointer | Indexed in Stage4A. | ADR 0013 remains at `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`; future moves require a separate reviewed slice. |
| Product principles | future public product-doc files | Missing. | Create only public-safe summaries. |
| Development/testing guide | `docs/development/testing.md` | Missing. | Good first doc after command verification. |
| Historical plans | future plan archive namespace or status-classified `plans/` | Not physically separated. | Defer mass move until references/gates are green. |
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
- encode the inventory in the reviewed
  `plans/openlife_repository_document_inventory.json` artifact;
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

Phase A is the only cleanup phase this preparation currently authorizes. Its
first slice should create the inventory and link/stale-reference baseline, not
rewrite authority documents. Phases B-E remain blocked until Phase A artifacts
exist and the red runtime-module guard is either green or formally scoped out
for docs-only cleanup.

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
- historical progress is preserved in a plan archive index form;
- no current entry point directs agents to deleted `router.rs`, `layer_router.rs`,
  `multi_strategy_runtime.rs`, or `runtime_migration_gate.rs` as live code.

### Phase C: Build real docs ownership

Tasks:

- create `docs/architecture/agent-runtime.md`;
- create `docs/architecture/life-model.md`;
- create `docs/architecture/governance.md`;
- create `docs/architecture/memory.md`;
- create `docs/development/testing.md`;
- optionally create public product vision and scenario docs after public-safety
  review.

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

### Phase D: ADR index and consolidation

Tasks:

- maintain the ADR index under `docs/decisions/README.md`;
- classify the ADR 0001, 0002, and 0003 decision files as accepted/historical/superseded where
  appropriate;
- keep ADR 0013 at
  `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` for Stage4A and
  record that path as the canonical index pointer;
- if a future slice moves ADR 0013, update `.github/CODEOWNERS`,
  `.github/ISSUE_TEMPLATE/04_adr_proposal.yml`,
  `.github/ISSUE_TEMPLATE/config.yml`, active docs, and any plan references in
  the same slice;
- while ADR 0013 stays in `plans/adr/`, keep `docs/decisions/README.md` as the
  canonical decision-log index and point to the existing file without
  duplicating it;
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
- only after link impact is understood, move historical plans into a plan
  archive namespace or keep them in place with explicit historical status;
- do not move active Phase7 files until Phase7 is complete or explicitly
  superseded.

Acceptance:

- active current work is obvious within one minute;
- old Goal/Stage/Beta/Migration docs cannot override active authority;
- no current command/test/doc references break.

## 7. Anti-Hallucination Checkpoints

Use these checks before converting any preparation claim into active
documentation. A cleanup slice is not accepted if it relies on any of these
claims without current local evidence.

| Claim to avoid unless proven | Required proof |
| --- | --- |
| "Phase7 is complete." | `plans/openlife_single_system_deletion_manifest.md` has no blocking `red-until-trial-green` or `not-done` item, and the named Phase7 gates pass. |
| "The final acceptance runner uses reusable final-gate aggregation." | First reconcile this with Phase7 authority: if `run_main_chat_agent_execution_v1_final_acceptance_gate` remains retired, shipped command/product bridge scans must stay empty and `main_chat_runtime_module` must be updated to the Phase7-current guard shape. If a reviewed Phase7 decision reintroduces or preserves a non-shipped runner, the approved runner owner must call `crate::main_chat_final_gate::build_main_chat_agent_execution_v1_final_gate_report(` and `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` must pass. |
| "The deleted old final-acceptance test-owner file is the final-acceptance test owner." | The file exists in the working tree, or a newer source-mapped test owner replaces it and the runtime-module guard is updated. |
| "`AGENTS.md` is current enough to use as source truth." | Each reused claim is source-mapped to current code/tests/plans, and stale old-route references are either removed or labeled historical. |
| "No old product command or route remains." | `single_system` guards pass, and old-symbol scans are interpreted by surface: product code, tests, guards, docs, dev bridge, and archive. |
| "`frontend/src/tauriDev.ts` is product behavior." | Product pages/components or `frontend/src/tauri.ts` import it. Otherwise it remains dev/test-only and must be classified, not treated as shipped product surface. |
| "Link validation exists." | A deterministic local command or CI job is present and its output is recorded for the slice. |
| "The document inventory/link baseline exists." | `plans/openlife_repository_document_inventory.*` and `plans/openlife_repository_document_link_baseline.*` or reviewed equivalents exist and name their generation commands. |
| "Live-provider evidence is complete." | The live-provider harness reports credited direct, web AgentLoop, MCP AgentLoop, and proposal-permission scenarios with the required metadata-safe traces. |

Raw `rg` output is evidence input, not a conclusion. Every hit must be
classified by meaning and surface before it is used to claim presence, absence,
or readiness.

## 8. Required Validation

Run the smallest validation set for preparation-only changes:

```sh
git diff --check
python3 -m json.tool plans/openlife_single_system_phase1_inventory.json >/tmp/openlife_phase1_inventory_pretty.json
rg -n "ARCHITECTUREDETAILED|docs[/]api|router\\.rs|layer_router|hermes\\.rs|multi_strategy_runtime|runtime_migration_gate" README.md AGENTS.md docs plans/README.md
rg -n "run_multi_strategy_agent_preview|check_runtime_migration_gate|get_react_beta_execution_status|run_main_chat_agent_stage|run_main_chat_agent_beta|run_main_chat_agent_step6|run_main_chat_external_live_productization_gate|run_main_chat_agent_product_maturity|controlled_chat_migration|controlled_chat_cutover" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts
rg -n "from ['\"].*tauriDev|tauriDev" frontend/src/pages frontend/src/components frontend/src/App.tsx frontend/src/tauri.ts
```

For the old-command scan, a no-match exit status is the desired result for the
shipped handler/product bridge surface above. Expected matches in dev/test-only
surfaces such as `frontend/src/tauriDev.ts`, frontend tests, or single-system
guards must remain explicitly classified as test/dev/archive-only.

For the `tauriDev` import scan, a no-match exit status is the desired result for
product pages/components and `frontend/src/tauri.ts`. If it matches, classify the
import before claiming product behavior.

For the stale-doc scan, the preparation-time result may contain expected hits.
After cleanup, active entry points must either have zero hits or list each
remaining hit in a historical allowlist. Do not treat raw `rg` output as a pass
without interpreting the target surface.

Run these readiness checks before promoting cleanup to Phase B-E or active
authority:

```sh
rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts
rg -n "crate::main_chat_final_gate::build_main_chat_agent_execution_v1_final_gate_report\\(" src-tauri/src
test -f src-tauri/src/main_chat_final_acceptance_tests.rs || rg -n "main_chat_final_acceptance|final_acceptance" src-tauri/src/main_chat_*tests.rs src-tauri/src/*acceptance* 2>/dev/null
```

While Phase7 keeps the final acceptance command retired, the first command must
have no product/shipped-surface matches. The second command is diagnostic unless
a reviewed Phase7 decision names a current final-runner owner; in that case, the
approved owner must call the reusable final-gate aggregation module. The third
command is diagnostic: either the old test-owner file exists, or a newer
source-mapped owner must be documented and guarded.

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

## 9. Stage 1 Validation Snapshot

Validation run during this preparation pass:

| Check | Result | Meaning |
| --- | --- | --- |
| Whitespace and Rust formatting checks | Passed | `git diff --check` has no tracked-diff whitespace errors, this preparation file has no trailing whitespace or CRLF, and `cargo fmt --check` passed. |
| `python3 -m json.tool plans/openlife_single_system_phase1_inventory.json` | Passed | The current Phase7 inventory remains parseable JSON. |
| Expanded static scan for old commands in `src-tauri/src/lib.rs`, `src-tauri/src/commands`, and `frontend/src/tauri.ts` | Passed with no matches | Retired migration/beta/stage command wrappers are not in the shipped handler or product bridge. |
| Product import scan for `frontend/src/tauriDev.ts` | Passed with no matches | The dev bridge is not imported by product pages/components or `frontend/src/tauri.ts` in the checked surface. |
| Static scan for stale doc references in `README.md`, `AGENTS.md`, `docs`, and `plans/README.md` | Found expected hits | Stale references remain in `AGENTS.md`, `docs/ARCHITECTURE.md`, and `docs/DEV_HANDOVER.md`; these are cleanup targets, not preparation blockers. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests | Current single-system authority guards still pass. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed, 24 passed and 2 failed | At Stage 1 preparation time, the Phase7 runtime-module guard was not green. It still expected `src/main_chat_final_acceptance_tests.rs`, which was deleted in the working tree, and still expected a final acceptance runner call from `src/commands/agent_runtime/mod.rs`; Stage5A later superseded this current-status finding by repairing the guard to the current owner shape. |
| Stage 1 inventory/link baseline artifacts | Created | `plans/openlife_repository_document_inventory.json` and `plans/openlife_repository_document_link_baseline.json` now record the docs-only baseline; they do not authorize Stage 2 authority edits. |
| Stage 1 artifact JSON parse | Passed | Both new baseline JSON artifacts parse with `python3 -m json.tool`. |
| Stage 1 link/stale-claim summary | Completed with blockers | The baseline covers 190 Markdown/HTML files, records 14 active-doc broken Markdown/HTML links, 118 active-doc missing path mentions, 208 historical/private allowlist candidates, and stale active claims that must be source-mapped before Stage 2. |

The failed runtime-module guard is an implementation/test-boundary blocker for
promoting this cleanup into active work. It should be fixed in the Phase7
runtime-module cleanup context, not hidden inside repository documentation
reorganization.

Stage5B current-status note: the paragraph above describes the Stage 1
preparation-time validation result. Stage5A subsequently fixed the
runtime-module guard in the Phase7 runtime-module cleanup context. The original
row remains as a historical validation record, but it is not current truth.

### 9.1 Stage 2A Active-Claim Scope Record

Stage 2A has a docs-only source-map/scope boundary:

- `plans/openlife_repository_active_claim_audit.md` records each Stage 1
  active-authority stale claim and each active broken Markdown link with an
  explicit action: `fix_now`, `mark_historical`, `retarget_link`,
  `source_map_required`, or `defer_with_reason`.
- `plans/openlife_repository_stage2a_scope_decision.md` defines the Stage 2B
  editable and non-editable file lists, including the `.github` PR/publication
  cleanup decision.
- `.github/PULL_REQUEST_TEMPLATE.md` has no local Markdown links to retarget,
  but remains in publication cleanup scope as the manual public-doc and
  authority-sync checklist surface.
- At Stage 2A time, the red `main_chat_runtime_module` guard remained an
  inherited blocker. Stage5A supersedes that current-status portion by making
  the guard pass without restoring retired final acceptance commands.

## 10. Explicit Non-Goals

This cleanup preparation does not include:

- Rust module refactoring;
- Tauri command migration;
- frontend product behavior changes;
- LifeModel schema or data migration;
- provider/runtime/live eval changes;
- deleting historical knowledge without classification;
- treating documentation cleanup as Phase7 completion evidence.

## 11. Ready-To-Start Gates

### 11.1 May start now: Phase A baseline only

The only executable cleanup currently supported by this preparation is a
docs-only Phase A baseline slice. Start it only when all are true:

- the user explicitly approves entering the cleanup implementation;
- the current Phase7 branch status is understood and either committed or
  intentionally kept dirty;
- the `main_chat_runtime_module` guard is green, or the slice explicitly
  records any red result as an inherited blocker it does not resolve; Stage5A
  now satisfies the green side of this condition for the runtime-module guard
  only;
- the slice touches only this preparation surface and new inventory/link-baseline
  artifacts, unless the user explicitly approves another named file;
- the slice names the exact files it will touch before editing;
- Phase A inventory schema, owner surface, link baseline, and stale-reference
  allowlist format are defined before generation;
- validation commands for that slice are known before work starts.

### 11.2 Blocked until further proof: Phase B-E and authority promotion

Do not start AGENTS compression, architecture doc replacement, ADR relocation,
plan archive moves, or active-authority promotion until all are true:

- Phase A inventory and link/stale-reference baseline artifacts exist and are
  reviewed;
- active authority docs are protected from stale or duplicate source-of-truth
  claims;
- any ADR canonical-path change has an explicit same-slice update list for
  GitHub templates, CODEOWNERS, active docs, and plan references;
- broad file moves have a link-impact report and rollback plan;
- `main_chat_runtime_module` is green, or a reviewed Phase7 decision formally
  scopes any red guard out for the specific docs-only slice. Stage5A now makes
  this specific guard green, but authority promotion remains separately blocked.

## 12. Stage 2B Validation Record

Stage 2B was executed as a docs-only repair for active-doc stale claims and
broken links identified by `plans/openlife_repository_active_claim_audit.md` and
bounded by `plans/openlife_repository_stage2a_scope_decision.md`.

Files touched in this Stage 2B pass:

- `AGENTS.md`
- `docs/ARCHITECTURE.md`
- `docs/DEV_HANDOVER.md`
- `CONTRIBUTING.md`
- `OpenLife_Final_PRD.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`

Files intentionally not touched:

- Rust/Tauri/React source files
- `README.md`
- `plans/README.md`
- ADR paths/content
- `plans/**` historical docs, except this preparation record
- `.github/workflows/**`
- `.github/PULL_REQUEST_TEMPLATE.md`

Validation results:

| Check | Result |
| --- | --- |
| `git diff --check` | Passed after Stage 2B doc edits. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_pretty.json` | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts` | Passed with no matches; `rg` exited 1 as expected for absence. |
| `rg -n "Hermes\|hermes\\.rs\|LayeredReasoner\|IntentRouter\|LayerRouter\|run_main_chat_agent_execution_v1_final_acceptance_gate" AGENTS.md CONTRIBUTING.md docs/ARCHITECTURE.md docs/DEV_HANDOVER.md OpenLife_Final_PRD.md plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | Re-run in the Stage 2B follow-up. It still reports historical/retired/incomplete contexts, including `OpenLife_Final_PRD.md` historical Hermes/HERM content, explicit historical `LayeredReasoner` notes in `AGENTS.md`, historical `hermes.rs` handover notes, and preparation-file blocker text. These are not zero-match scans and must be interpreted by surface. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

Stage 2B-Rework2 record:

- Finding: `AGENTS.md` still claimed `try_auto_checkin_daily_goals` ran after
  every assistant reply and linked it to an old root lib source-line placeholder
  location. Source scan found only the helper definition in
  `src-tauri/src/main_chat_conversation_updates.rs` plus runtime-module guard
  text; current ordinary Main Chat source files did not contain a product
  callsite.
- Fix: `AGENTS.md` now marks `try_auto_checkin_daily_goals` as a historical /
  not-wired helper, maps the helper to
  `src-tauri/src/main_chat_conversation_updates.rs`, and states that the current
  `main_chat_send.rs` / `main_chat_streaming.rs` -> `main_chat_turn_runtime.rs`
  -> `main_chat_kernel.rs` path has no product callpoint.
- Finding/fix: old `AGENTS.md` source links such as
  `openlife-core/src/vectors.rs:285`, `openlife-core/src/config.rs:96`,
  `openlife-core/src/vectors.rs:369`, `openlife-core/src/memory.rs:702`,
  `openlife-core/src/scheduler.rs:191`, and `openlife-core/src/mcp.rs:536`
  were not safe as current exact source maps. They were downgraded to file-level
  source areas or explicit re-scan instructions.
- Scan explanation: the Rework2 active-doc scan for
  `try_auto_checkin_daily_goals|src-tauri/src/lib.rs:[0-9]+|LayeredReasoner|Hermes|IntentRouter|LayerRouter`
  is not expected to be zero because historical/retired PRD and handover
  surfaces intentionally retain classified terms. After Rework2,
  `try_auto_checkin_daily_goals` appears only as the not-wired helper note and
  this validation record, exact old root lib source-line placeholders are
  removed from `AGENTS.md`, and remaining `LayeredReasoner` /
  `Hermes` / `IntentRouter` / `LayerRouter` hits are historical, retired, PRD,
  or preparation-record contexts rather than current implementation authority.

Rework2 validation rerun:

| Check | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json` | Passed; pretty output written to `/tmp/openlife_repository_document_inventory_pretty_rework2.json`. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json` | Passed; pretty output written to `/tmp/openlife_repository_document_link_baseline_pretty_rework2.json`. |
| `rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts` | No matches; `rg` exited 1 as expected for an absence guard. |
| `rg -n "\.rs:[0-9]+" AGENTS.md` | No matches after Rework2; exact line-number source links were removed from `AGENTS.md`. |
| `rg -n "try_auto_checkin_daily_goals" src-tauri/src/main_chat_send.rs src-tauri/src/main_chat_streaming.rs src-tauri/src/main_chat_turn_runtime.rs src-tauri/src/main_chat_kernel.rs src-tauri/src/commands frontend/src/tauri.ts` | No matches; current ordinary Main Chat product surfaces do not call the helper. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. The same run also reports `try_auto_checkin_daily_goals` as unused, reinforcing the Rework2 not-wired classification. |
| Active-doc scan for `try_auto_checkin_daily_goals\|src-tauri/src/lib.rs:[0-9]+\|LayeredReasoner\|Hermes\|IntentRouter\|LayerRouter` | Non-zero by design: remaining hits are historical/retired notes in `AGENTS.md`, `docs/ARCHITECTURE.md`, `CONTRIBUTING.md`, historical PRD content in `OpenLife_Final_PRD.md`, and this preparation record. No hit re-establishes those terms as current Main Chat implementation authority. |

Stage 2B outcome boundary:

- Active docs no longer present old Goal 8/K8 docs as current authority over
  the Phase7 single-system contract.
- Stage 2B follow-up corrected the remaining active-current stale claims in
  `CONTRIBUTING.md` and the middle `AGENTS.md` Main Chat data flow: `Hermes`,
  `hermes.rs`, `LayeredReasoner`, old `src-tauri/src/lib.rs` Main Chat ownership,
  `IntentRouter`, `LayerRouter`, deleted runtime files, the retired final
  acceptance command, and the missing final-acceptance test owner may still
  appear in historical/retired/blocker contexts, but are not presented as current
  Main Chat implementation authority.
- External live-provider-backed evidence remains incomplete.
- `main_chat_runtime_module` remained an inherited blocker at Stage 2B time;
  Stage5A later superseded that current-status portion by making the guard pass.
- This pass did not claim Phase7 completion, Main Chat Agent Execution v1
  completion, runtime guard success, ADR move readiness, plans archive
  readiness, or CI/link-checker coverage. Stage5A later supplied the runtime
  guard success only.

## 13. Stage 2C Phase C Readiness Record

Stage 2C creates the formal entry decision for Phase C / Stage3 document build
in `plans/openlife_repository_stage2c_phase_c_readiness_decision.md`.

Decision summary:

- At Stage2C time, Phase C could continue while `main_chat_runtime_module` was
  red only as a docs-only, source-backed explanatory slice.
- At Stage2C time, the red runtime-module guard was formally scoped out only
  for explanatory docs. Stage5A later made this guard pass, but runtime
  promotion, active-authority promotion, shipped command promotion, final-gate
  promotion, Phase7 completion, and Main Chat Agent Execution v1 completion
  claims remain separately blocked.
- New Phase C docs had to explicitly record the inherited blocker at that time;
  Stage5B docs instead record the Stage5A supersession.
- Stage3 may not create empty directories. Architecture and development
  namespaces were checked before Stage3 creation, while the product-doc
  namespace remains absent by decision; any future directory creation must
  happen in the same patch as a real approved file.
- Stage3 readiness verdict:
  `ready_for_stage3_doc_build=true`, with `ready_for_authority_promotion=false`.

Stage2C does not create `docs/architecture/*`, does not create
`docs/development/testing.md`, does not move or delete documents, does not edit
Rust/Tauri/React source, and does not restore
`run_main_chat_agent_execution_v1_final_acceptance_gate`.

Stage2C-rework fixes a source-map hallucination in the Stage2C decision: the
nonexistent root package manifest was removed from the Phase C testing source map
and replaced with existing package evidence such as `frontend/package.json`.
The Stage2C validation command set now includes a source-map existence check
parsed from the decision document, and the required result is `missing_count=0`.

Stage2C-rework validation results:

| Check | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage2c_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage2c_pretty.json` | Passed. |
| Source-map existence check | Passed with `source_map_path_count=62`, `missing_count=0`. |
| `rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts` | No matches; `rg` exited 1 as expected for absence. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

## Stage4A ADR No-Move Index Consolidation Record

Date: 2026-07-07

Stage4A creates the ADR index and resolves the active ADR blocker without moving
ADR 0013. ADR 0013 remains canonical at
`plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`.

Stage4A created:

- `docs/decisions/README.md`
- `plans/openlife_repository_stage4a_adr_no_move_index_decision.md`

Stage4A updated:

- `docs/repository_document_governance.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_stage3f_active_missing_actionability_decision.md`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`

Regenerated JSON records now show:

| Category | Before Stage4A | After Stage4A |
| --- | ---: | ---: |
| `active_doc_missing_records` | 92 | 76 |
| `active_actionable_repair_records` | 0 | 0 |
| `active_expected_absent_records` | 37 | 37 |
| `active_future_blocked_records` | 39 | 39 |
| `active_adr_blocked_records` | 16 | 0 |

The Stage4A pass made no Rust/Tauri/React/frontend source code changes, did not
move ADR 0013, did not create a duplicate ADR 0013 file under `docs/decisions/`,
created no product-doc, plan-archive, or active-plan namespace,
performed no authority promotion, and made no Phase7, Main Chat Agent Execution
v1, live-provider evidence, or runtime-module completion claim.

Stage4A validation results:

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage4a_verify.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage4a_verify.json` | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker with the same failure set: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

## Stage4B Future Namespace Reference Rewording Record

Date: 2026-07-07

Stage4B clears the Stage4A `active_future_blocked_records=39` set by rewording
future namespace references in active docs. It does not create future
directories, placeholder files, broad plan moves, or source-code changes.

Stage4B created:

- `plans/openlife_repository_stage4b_future_namespace_decision.md`

Stage4B updated:

- `docs/repository_document_governance.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_stage2c_phase_c_readiness_decision.md`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`

Regenerated JSON records now show:

| Category | Before Stage4B | After Stage4B |
| --- | ---: | ---: |
| `active_doc_missing_records` | 76 | 37 |
| `active_actionable_repair_records` | 0 | 0 |
| `active_expected_absent_records` | 37 | 37 |
| `active_future_blocked_records` | 39 | 0 |
| `active_adr_blocked_records` | 0 | 0 |

The Stage4B pass made no Rust/Tauri/React/frontend source code changes, created
no future namespace or placeholder file, moved no plans, performed no authority
promotion, and made no Phase7, Main Chat Agent Execution v1, live-provider
evidence, or runtime-module completion claim.

Stage4B validation results:

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage4b_verify.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage4b_verify.json` | Passed. |
| Future namespace absence shell check | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker with the same failure set: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

## Stage3G First Actionable Repair Record

Date: 2026-07-07

Stage3G executed a bounded docs-only repair pass over the Stage3F
`active_actionable_repair_records=51` set. It repaired only deterministic,
non-ADR, non-future, non-expected-absent, non-runtime/source items in the
authorized file list.

Stage3G repaired 47 actionable records and left 4 actionable records skipped
because their source files are forbidden in this slice:

| Source | Records | Skip reason |
| --- | ---: | --- |
| `AGENTS.md` | 2 | Forbidden by Stage3G scope; root AI authority file was not edited. |
| `docs/decisions/0002-proposal-unified.md` | 1 | Forbidden by Stage3G scope; `docs/decisions/*` was not edited. |
| `docs/decisions/0003-agent-run-tracking.md` | 1 | Forbidden by Stage3G scope; `docs/decisions/*` was not edited. |

Regenerated JSON records now show:

| Category | Before Stage3G | After Stage3G |
| --- | ---: | ---: |
| `active_doc_missing_records` | 143 | 96 |
| `active_actionable_repair_records` | 51 | 4 |
| `active_expected_absent_records` | 37 | 37 |
| `active_future_blocked_records` | 39 | 39 |
| `active_adr_blocked_records` | 16 | 16 |

The Stage3G pass did not edit Rust/Tauri/React/frontend source, did not edit
`AGENTS.md`, `README.md`, `plans/README.md`, `docs/decisions/*`, `plans/adr/*`,
product-doc or plan-archive namespace, and did not create or restore missing
files.

Stage3G validation results:

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3g_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3g_pretty.json` | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |
| Active-doc prohibited-completion scan | Non-zero by design: two `AGENTS.md` matches, both in retired-command / incomplete-evidence / inherited-blocker wording. |

## Stage3H Residual Forbidden-File Actionable Repair Record

Date: 2026-07-07

Stage3H clears only the four Stage3G residual
`active_actionable_repair_records`. It does not attempt general active missing
cleanup, ADR consolidation, authority promotion, future namespace creation,
runtime/source cleanup, Main Chat readiness closure, or live-provider completion.

Stage3H repairs:

- root current-authority caveat reworded so the deleted final-acceptance
  test-owner remains historical residue, not a current file target;
- historical preview-audit utility residue reworded while preserving
  `src-tauri/src/commands/agent_runtime/` as the current source-map surface;
- ADR 0002 related frontend surface retargeted to
  `frontend/src/pages/ChatPage.tsx`;
- ADR 0003 related trace surface retargeted to
  `frontend/src/components/RunTracePanel.tsx`.

Regenerated JSON records now show:

| Category | Before Stage3H | After Stage3H |
| --- | ---: | ---: |
| `active_doc_missing_records` | 96 | 92 |
| `active_actionable_repair_records` | 4 | 0 |
| `active_expected_absent_records` | 37 | 37 |
| `active_future_blocked_records` | 39 | 39 |
| `active_adr_blocked_records` | 16 | 16 |

The Stage3H pass made no Rust/Tauri/React/frontend source code changes, created
no future namespace, performed no ADR consolidation, created no ADR index file,
changed no ADR status, performed no authority promotion, and made no Main Chat
complete or live-provider complete claim.

Stage3H validation results:

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3h_verify.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3h_verify.json` | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

## 14. Stage3-A Repository Knowledge Docs Record

Stage3-A created the first source-backed repository knowledge docs:

- `docs/architecture/agent-runtime.md`
- `docs/architecture/life-model.md`
- `docs/architecture/governance.md`
- `docs/architecture/memory.md`
- `docs/development/testing.md`

Before creation, `docs/architecture/` and `docs/development/` were checked and
did not exist. The directories were created only through the patch that added
real approved files. No product-doc directory or file was created.

`docs/ARCHITECTURE.md` was converted into an index and historical pointer page
only. It now points to the focused Stage3-A explainers and does not carry a
second unverified architecture narrative.

Stage3-A stayed docs-only:

- no Rust, Tauri, React, or frontend source file was edited;
- no active authority promotion was made;
- no retired final acceptance command was restored or promoted;
- no Phase7, Main Chat Agent Execution v1, live-provider, or runtime-module
  completion claim was made.

Stage3-A validation results:

| Check | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3a_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3a_pretty.json` | Passed. |
| Source-map existence check | Passed with `source_map_path_count=62`, `missing_count=0`. |
| `rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts` | No matches; `rg` exited 1 as expected for absence. |
| Active-doc prohibited-completion scan | Non-zero by design: two `AGENTS.md` matches, both in retired-command / incomplete-evidence / inherited-blocker wording. No new match was introduced in `docs/ARCHITECTURE.md`. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

The Stage3-A docs are explanatory repository knowledge only. They do not change
the Phase7 authority stack, do not close external live-provider evidence gaps,
and did not make `main_chat_runtime_module` green at the time. Stage5A later
made the runtime-module guard green by updating the current guard owner shape;
that later run supersedes only the runtime-module blocker status.

## 15. Stage3-B Governance / Inventory / Link Baseline Refresh Record

Stage3-B refreshed the repository document governance snapshot, document
inventory, and link/path baseline after Stage3-A added the focused
architecture/development docs. This was a docs-only baseline refresh.

Files touched in this Stage3-B pass:

- `plans/openlife_repository_document_inventory.json`
- `plans/openlife_repository_document_link_baseline.json`
- `docs/repository_document_governance.md`
- `docs/development/testing.md`
- `README.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`

Files and actions intentionally not touched:

- Rust/Tauri/React source files
- `AGENTS.md`
- `plans/README.md`
- ADR paths and the not-yet-created ADR index
- product-doc namespace
- plan-archive namespace or broad plan moves
- runtime authority, Phase7 completion, Main Chat Agent Execution v1
  completion, live-provider completion, or runtime-module green claims

Inventory and link baseline summary:

- `plans/openlife_repository_document_inventory.json` now uses the
  `openlife_repository_document_inventory.stage3b.v1` schema and records 198
  Markdown/HTML documents in the current
  `rg --files -g '*.md' -g '*.html'` scope.
- The inventory records `docs/ARCHITECTURE.md` as
  `stage3a_architecture_index_explanatory_not_authority`.
- The inventory records `docs/architecture/agent-runtime.md`,
  `docs/architecture/life-model.md`, `docs/architecture/governance.md`,
  `docs/architecture/memory.md`, and `docs/development/testing.md` as public
  stable explanatory docs beneath active authority.
- `plans/openlife_repository_document_link_baseline.json` now uses the
  `openlife_repository_document_link_baseline.stage3b.v1` schema.
- The Stage3-A new docs have zero broken Markdown/HTML links in the refreshed
  baseline.
- The refreshed baseline has zero uncategorized broken records. Remaining
  missing path records are classified by source scope and action.

Stage3-B validation results:

| Check | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3b_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3b_pretty.json` | Passed. |
| Source-map existence check | Passed with `source_map_path_count=62`, `missing_count=0`. |
| Local Markdown link/path baseline check | Passed with zero Stage3-A new-doc broken links and zero uncategorized broken records. |
| `rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts` | No matches; `rg` exited 1 as expected for the absence guard. |
| Active-claim scan for completion/restoration wording | Non-zero by design: two `AGENTS.md` matches, both in retired-command / incomplete-evidence / inherited-blocker wording. No Stage3-B README or governance edit introduced a completion claim. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

Stage3-B outcome boundary:

- This pass does not authorize ADR consolidation by itself.
- This pass does not move ADR 0013, create the ADR index, create the product-doc
  namespace, or move plans into a plan-archive namespace.
- This pass does not claim Phase7 completion, Main Chat Agent Execution v1
  completion, completed live-provider evidence, or a green runtime-module guard.

## Stage3-C ADR Readiness Decision

Date: 2026-07-07

Files touched in this Stage3-C pass:

- `plans/openlife_repository_stage3c_adr_readiness_decision.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`

Files and actions intentionally not touched:

- Rust/Tauri/React/frontend source files
- `AGENTS.md`
- `plans/README.md`
- ADR index file
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- product-doc namespace
- plan-archive namespace
- ADR path moves, product-doc creation, plan archive creation, or runtime
  authority promotion

Stage3-C verified that the required ADR/governance inputs exist:

- `docs/decisions/0001-lifemodel-patch.md`
- `docs/decisions/0002-proposal-unified.md`
- `docs/decisions/0003-agent-run-tracking.md`
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `.github/CODEOWNERS`
- `.github/ISSUE_TEMPLATE/04_adr_proposal.yml`
- `.github/ISSUE_TEMPLATE/config.yml`
- `plans/openlife_repository_document_inventory.json`
- `plans/openlife_repository_document_link_baseline.json`

Stage3-C decision:

- ADR readiness decision is complete.
- ADR consolidation implementation is not ready.
- ADR 0013 remains at
  `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`.
- The ADR index remains absent by scope.
- Active missing path records from Stage3-B are not resolved or scoped out by
  this pass.
- A future ADR 0013 move must update CODEOWNERS, GitHub issue template config,
  active architecture docs, active planning references, the ADR index, and the
  Stage3 inventory/link baseline in one reviewed slice. Without that full
  surface, the correct decision is to keep `plans/adr/` as ADR 0013's canonical
  location.

Stage3-C outcome boundary:

- This pass does not move ADR 0013.
- This pass does not create the ADR index.
- This pass does not create the product-doc namespace or plan-archive namespace.
- This pass does not claim Phase7 completion, Main Chat Agent Execution v1
  completion, completed live-provider evidence, or a green runtime-module guard.

Stage3-C validation results:

| Check | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3c_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3c_pretty.json` | Passed. |
| ADR reference impact scan | Completed with expected active, historical, template, and governance hits classified in `plans/openlife_repository_stage3c_adr_readiness_decision.md`. |
| `rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts` | No matches; `rg` exited 1 as expected for the absence guard. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

## Stage3D Active Path Triage Record

Date: 2026-07-07

Stage3D created:

- `plans/openlife_repository_stage3d_active_path_triage.md`

Files and actions intentionally not touched:

- Rust/Tauri/React/frontend source files
- `AGENTS.md`
- `plans/README.md`
- `README.md`
- ADR index file
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- product-doc namespace
- plan-archive namespace
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`

Stage3D extracted active missing path records from
`plans/openlife_repository_document_link_baseline.json` using
`broken_link_type == active_doc_broken_path_mention`.

Count check:

| Check | Value |
| --- | ---: |
| Baseline `summary.active_doc_missing_records` | 171 |
| Extracted active records | 171 |
| Unique source/raw/resolved groups | 114 |

Stage3D category summary:

| Category | Records | Groups |
| --- | ---: | ---: |
| `retarget_now_candidate` | 33 | 19 |
| `remove_or_reword_candidate` | 14 | 10 |
| `future_path_reference_keep_blocked` | 29 | 19 |
| `historical_should_not_be_active` | 83 | 59 |
| `adr_consolidation_blocker` | 8 | 5 |
| `needs_user_decision` | 4 | 2 |
| **Total** | **171** | **114** |

ADR-related missing targets remain separately blocked: the ADR index target,
duplicate ADR 0013 decision targets under `docs/decisions/`, and shorthand ADR
0013 path mentions that do not name the existing canonical file.

Stage3D decision:

- A bounded active path repair implementation can start next for deterministic
  non-ADR retarget/reword work.
- ADR consolidation cannot start directly from Stage3D.
- Stage3D did not repair links, move ADR files, create future namespaces,
  regenerate baselines, or change product/runtime authority.
- At Stage3D time, the runtime-module guard remained an inherited blocker
  unless a separate current test run proved otherwise. Stage5A later provided
  that separate current run and made the guard pass.

## Stage3E Bounded Active Path Repair Record

Date: 2026-07-07

Stage3E repaired only the deterministic non-ADR clear cases from the Stage3D
minimal first repair file list.

Edited files:

- `docs/DEV_HANDOVER.md`
- `CONTRIBUTING.md`
- `plans/openlife_repository_active_claim_audit.md`
- `plans/openlife_repository_stage2a_scope_decision.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`

Repair actions:

- `.github/*` governance paths are represented as `.github/...`.
- Repo-root links in the Stage3D clear set are represented as repo-relative
  paths.
- Stale detailed-architecture/API/example path labels were reworded so they do
  not imply files should be created.
- The inventory artifact reference was kept on the existing
  `plans/openlife_repository_document_inventory.json` baseline.

Count result:

| Check | Value |
| --- | ---: |
| Stage3D active missing records before repair | 171 |
| Stage3E active missing records after repair | 143 |
| Stage3E missing local path records, all scopes | 469 |
| Stage3E historical/private allowlist candidates | 326 |

Remaining blocker classes:

| Class | Status |
| --- | --- |
| ADR consolidation targets | Still blocked; ADR 0013 relocation and ADR index work were not performed. |
| Future namespaces | Still blocked; no product-doc, plan-archive, or active-plan namespace was created. |
| Historical retired source paths | Still present as historical/deletion evidence in non-Stage3E surfaces or allowlist sources. |
| Disallowed active-authority files | `AGENTS.md`, `README.md`, `plans/README.md`, ADR files, product docs, archive docs, Rust/Tauri/React/frontend source were not edited by Stage3E. |
| Runtime-module guard | At Stage3E time, still an inherited blocker; documentation cleanup did not reclassify it as passing. Stage5A later made the guard pass through guard repair, not docs cleanup. |

Validation results:

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3e_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3e_pretty.json` | Passed. |
| Active missing count check | Passed with `active_doc_missing_records=143`, below the Stage3E threshold of 171. |
| Forbidden completion/ADR wording scan over Stage3E prep and JSON artifacts | No matches; `rg` exited 1 as expected. |
| Retired command shipped-surface scan | No matches; `rg` exited 1 as expected. |
| Future directory absence checks | Passed for the ADR index target, product-doc namespace, and plan-archive namespace. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

Stage3E-rework validation note:

- The Stage3E count table now matches the regenerated JSON baseline:
  `missing_local_path_records=469`,
  `active_doc_missing_records=143`, and
  `historical_or_private_missing_records=326`.
- The active retarget candidate residue was removed from this preparation
  record. The only inventory artifact target named here is the existing
  `plans/openlife_repository_document_inventory.json`.
- The Stage3E edited-files list no longer includes
  `docs/github_repository_governance.md`, because that file has no actual diff
  in this worktree.

## Stage3F Active Missing Actionability Record

Date: 2026-07-07

Stage3F classifies the current `active_doc_missing_records=143` records from
`plans/openlife_repository_document_link_baseline.json`. It does not reuse the
older Stage3D count of 171.

Stage3F created:

- `plans/openlife_repository_stage3f_active_missing_actionability_decision.md`

Stage3F updated:

- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`

Files and paths intentionally not touched:

- Rust/Tauri/React/frontend source
- `AGENTS.md`
- `README.md`
- `plans/README.md`
- `docs/decisions/*`
- `plans/adr/*`
- product-doc namespace
- plan-archive namespace

Stage3F actionability summary:

| Category | Records | Decision |
| --- | ---: | --- |
| `active_actionable_repair_records` | 51 | Future authorized doc slice must retarget, reword, source-map, or refine checker handling. |
| `active_expected_absent_records` | 37 | Preserve absence as Phase7 deletion evidence; do not restore files. |
| `active_future_blocked_records` | 39 | Keep future namespaces blocked; do not create empty directories or placeholders. |
| `active_adr_blocked_records` | 16 | Keep ADR consolidation blocked in Stage3F; do not create the ADR index or move ADR 0013. |
| **Total classified active records** | **143** | Matches current Stage3E-after baseline. |

Stage3F decisions:

- Records matching `plans/openlife_single_system_deletion_manifest.md` deleted,
  test-archive, or product-valid-rename objects are expected-absent evidence,
  not restore targets.
- ADR index creation, duplicate ADR 0013 decision targets under
  `docs/decisions/`, and ADR 0013 path changes remain blocked; Stage3F does
  not create or move ADR files.
- Product-doc, plan-archive, active-plan, and local/private/draft namespace
  families remain blocked; Stage3F does not create empty directories.
- Stage3F is not closure and does not authorize ADR consolidation, active
  authority promotion, runtime changes, product behavior changes, or Phase7
  completion.

Validation results:

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3f_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3f_pretty.json` | Passed. |
| Future directory absence checks | Passed for the product-doc, plan-archive, active-plan, ADR index, and local/private/draft namespace targets. |
| Stage3F actionability count check | Passed with `143 = 51 + 37 + 39 + 16`. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

## Stage4C Expected-Absent Evidence Closure Record

Date: 2026-07-07

Stage4C audits and closes the remaining active missing-record set after
Stage4B. It does not restore missing targets, move ADR 0013, create future
namespaces, edit source code, promote active authority, or claim Phase7/Main
Chat/live-provider/runtime-module completion.

Stage4C created:

- `plans/openlife_repository_stage4c_expected_absent_evidence_decision.md`

Stage4C updated:

- `plans/openlife_single_system_deletion_manifest.md`
- `plans/openlife_repository_active_claim_audit.md`
- `plans/openlife_single_system_development_preparation.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`

Stage4C closure summary:

| Category | Records | Decision |
| --- | ---: | --- |
| `active_doc_missing_records` | 37 | Preserved as expected-absent Phase7 deletion evidence. |
| `active_expected_absent_records` | 37 | Verified against the deletion manifest. |
| `stage4c_verified_expected_absent_records` | 37 | New Stage4C closure count. |
| `active_actionable_repair_records` | 0 | No active repair blockers remain. |
| `active_future_blocked_records` | 0 | No future namespace blockers remain active. |
| `active_adr_blocked_records` | 0 | No ADR blockers remain active. |
| `active_unresolved_missing_records` | 0 | No unresolved active missing blockers remain. |

The 37 records cover 24 unique absent targets, all intentionally absent and
already covered by the Phase7 deletion manifest as deleted, test-only archive,
or product-valid rename evidence.

## Stage5B Status Sync Record

Date: 2026-07-07

Stage5B synchronizes current-state documentation and baseline metadata after
the Stage5A runtime-module guard repair.

Current status:

- `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` now
  passes in the current run.
- Stage2/Stage3/Stage4 validation rows that record this command as failed are
  retained as original time-point evidence and are superseded for current
  status by Stage5A.
- `run_main_chat_agent_execution_v1_final_acceptance_gate` remains retired and
  must not return to shipped command/product bridge surfaces.
- `src-tauri/src/main_chat_final_acceptance_tests.rs` remains expected-absent.
- No link-baseline or inventory recomputation is claimed by Stage5B; the JSON
  artifacts only receive a `stage5b_summary` metadata addition.

Stage5B does not claim Phase7 completion, Main Chat Agent Execution v1
completion, external live-provider evidence completion, authority promotion,
ADR movement, plan archival readiness, or historical validation rewrite.

## Stage7 Scope Reset And Baseline Refresh Record

Date: 2026-07-08

Stage7 resets the repository cleanup scope after the Stage6E native product
trial. The Stage6E trial remains a RED product result, but its RED findings are
product development TODOs rather than blockers for docs-only repository
cleanup.

Stage7 created:

- `plans/openlife_repository_stage7_scope_reset_baseline_decision.md`

Stage7 updated:

- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_active_claim_audit.md`
- `plans/openlife_repository_document_inventory.json`
- `plans/openlife_repository_document_link_baseline.json`

Stage7 explicitly does not enter product repair work:

- no Rust/Tauri/React/frontend behavior code edits;
- no ToolPermission accept/resume repair;
- no `web.search` / `web.fetch` / network-policy repair;
- no native trial or WebDriver repair;
- no ADR 0013 move;
- no `plans/` archive mass move;
- no `AGENTS.md` edit in this stage.

Baseline refresh boundary:

| Field | Stage7 value |
| --- | ---: |
| `refreshed_at` | `2026-07-08T13:38:59+0800` |
| `recomputed` | `false` |
| current `rg --files -g '*.md' -g '*.html'` count | 209 |
| inventory document records retained from existing baseline | 205 |
| active broken Markdown link count retained from existing baseline | 0 |
| active actionable repair records retained from existing baseline | 0 |
| active missing records retained as Stage4C expected-absent evidence | 37 |

Stage7 does not claim a full inventory or link-baseline recomputation. It only
adds `stage7_summary` metadata to the existing JSON artifacts and preserves
Stage4C / Stage5B as historical/current-status records.

Stage7 hallucination scan result:

- active completion-claim scan has hits only in prohibited-claim lists,
  validation command examples, historical plans, or explicit caveat language;
- retired final acceptance command/test-owner scan has no shipped handler or
  frontend bridge restoration; matches are preparation-file historical/guard
  text;
- `tauriDev` product import scan has no matches in checked product
  pages/components or `frontend/src/tauri.ts`.

Stage8 decision:

- `ready_for_stage8_agents_compression=true`
- Reason: active actionable repair records remain 0, active broken Markdown
  link count remains 0, Stage4C expected-absent records are classified, and the
  Stage6E RED state is a product TODO boundary rather than a repository cleanup
  blocker.

Stage8's only recommended next step is `AGENTS.md` compression. That future
slice must preserve the Phase7 authority stack, Main Chat Agent Execution v1
non-completion caveat, external live-provider non-completion caveat, retired
final acceptance command/test-owner absence, and no-silent-write /
proposal-first constraints. It must not repair product ToolPermission, web
read, network policy, or native trial behavior unless a later user instruction
explicitly changes scope.

## Stage8 AGENTS Compression Record

Date: 2026-07-08

Stage8 completed the allowed `AGENTS.md` compression slice.

Stage8 created:

- `plans/openlife_repository_stage8_agents_compression_decision.md`

Stage8 updated:

- `AGENTS.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_active_claim_audit.md`

Compression result:

| File | Before | After |
| --- | ---: | ---: |
| `AGENTS.md` | 883 lines | 179 lines |

The compressed entrypoint keeps the active authority order, current Main Chat
send/stream source-map, Phase7 single-system deletion contract, proposal-first
and no-silent-write constraints, external live-provider non-closure, and the
retired final acceptance command/test-owner absence contract.

Stage8 removed long W-series historical流水, repeated roadmap/progress narrative,
stale module tables, deleted-route source paths, and old product narrative from
`AGENTS.md`. The historical information remains delegated to `plans/README.md`
and existing historical plan/spec files; no project truth was deleted from the
repository.

Stage8 stayed docs-only:

- no Rust/Tauri/React/frontend behavior code edits;
- no `plans/README.md` edit;
- no ADR edit or ADR 0013 move;
- no `plans/` archive move;
- no Stage6E product RED repair;
- no retired command or deleted test-owner restoration.

## Stage8-Rework Source-Map Correction Record

Date: 2026-07-08

Stage8-rework corrected a precision issue in the compressed `AGENTS.md` Main
Chat source-map.

The rework checked `src-tauri/src/lib.rs`,
`src-tauri/src/main_chat_send.rs`, `src-tauri/src/main_chat_streaming.rs`,
`src-tauri/src/main_chat_turn_runtime.rs`, and
`src-tauri/src/main_chat_kernel.rs` before editing.

Correction made:

- `AGENTS.md` no longer shows `main_chat_send.rs` flowing into
  `main_chat_streaming.rs`.
- It now records two parallel command branches:
  `frontend/src/tauri.ts` -> `src-tauri/src/lib.rs send_message` ->
  `src-tauri/src/main_chat_send.rs` ->
  `OpenLifeTurnRuntime::run_buffered`, and
  `frontend/src/tauri.ts` -> `src-tauri/src/lib.rs start_stream_message` ->
  `src-tauri/src/main_chat_streaming.rs` ->
  `OpenLifeTurnRuntime::run_streaming`.
- Both branches converge in `main_chat_turn_runtime.rs`,
  `main_chat_kernel.rs`, and core agent areas.
- After rework, `AGENTS.md` is 190 lines and remains below the 250-line limit.

This rework did not enter product code repair, Stage9, broad architecture-doc
refresh, ADR movement, plan archive work, or Stage6E product RED repair.

## Stage9 Post-AGENTS Baseline Refresh Record

Date: 2026-07-08

Stage9 refreshes the repository document inventory and local link/path baseline
after Stage8-rework corrected the compressed `AGENTS.md` source map.

Stage9 created:

- `plans/openlife_repository_stage9_post_agents_baseline_refresh_decision.md`

Stage9 updated:

- `plans/openlife_repository_document_inventory.json`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_active_claim_audit.md`

Baseline refresh result:

| Field | Stage9 value |
| --- | ---: |
| `recomputed` | true |
| current Markdown/HTML documents | 212 |
| inventory document records | 212 |
| local missing path/link baseline records | 359 |
| active broken Markdown links | 0 |
| active expected-absent records | 40 |
| active actionable repair records | 0 |
| historical/private missing path or link records | 319 |

Stage9 records that `AGENTS.md` is now 190 lines and that Stage8-rework fixed
the Main Chat source map to parallel `send_message` and
`start_stream_message` branches before convergence in the turn runtime and
kernel.

The active expected-absent records remain deletion/retired-owner evidence.
They do not authorize restoring deleted files or old commands. Stage9 does not
rewrite Stage4C, Stage5B, Stage7, or Stage8 historical/current-status records
into new time-point facts.

Stage9 readiness decision:

- `ready_for_next_architecture_document_ownership_preparation=true`
- `ready_for_broad_architecture_doc_rewrite=false`

The next repository cleanup slice may prepare architecture/document ownership
cleanup only if it first names the source maps and exact file scope. It must not
start broad architecture-doc rewriting directly, must not move ADR 0013, must
not create plan archive namespaces, must not edit Rust/Tauri/React/frontend
behavior code, and must not treat Stage6E product RED findings as repository
cleanup blockers.
