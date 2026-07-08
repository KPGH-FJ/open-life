# OpenLife Repository Active Claim Audit - Stage 2A

> Generated: 2026-07-07
> Scope: docs-only audit, boundary setting, and source-map. This file does not
> rewrite active authority docs and does not grant readiness.

## Inputs

- `plans/openlife_repository_document_inventory.json`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- Current local source/file scans in the repository root.

## Action Vocabulary

| Action | Meaning |
| --- | --- |
| `fix_now` | Stage 2B should rewrite the named active document claim from current source evidence. |
| `mark_historical` | Stage 2B may keep the text only with explicit historical/snapshot labeling, or remove it from active guidance. |
| `retarget_link` | Stage 2B should update the broken link to an existing local target or remove the link if no canonical target exists. |
| `source_map_required` | Do not rewrite as current truth until the owning source/test/guard decision is verified. |
| `defer_with_reason` | Do not solve in Stage 2B; keep a named blocker or false-positive rationale. |

## Source Map Snapshot

| Check | Result | Stage 2A meaning |
| --- | --- | --- |
| `rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts` | No matches. | The retired final acceptance command is not present in shipped command/product bridge surfaces. Do not restore it as a docs cleanup fix. |
| Deleted old final-acceptance test-owner module | Missing. | It cannot be named as the current final-acceptance test owner. |
| `src-tauri/src/main_chat_final_gate.rs` | Present; owns reusable final-gate helpers. | The helper module exists, but this does not prove a shipped final acceptance command exists. |
| `src-tauri/src/main_chat_runtime_module_tests.rs` | Present; still contains guard expectations for final-gate aggregation and the missing old test owner. | `main_chat_runtime_module` remains an inherited red guard until Phase7 updates or scopes it out. |
| `src-tauri/src/main_chat_kernel.rs`, `src-tauri/src/main_chat_send.rs`, `src-tauri/src/main_chat_streaming.rs` | Present. | Current Main Chat source-map must start here, not from old `IntentRouter` / `LayerRouter` docs. |
| Old core router, layer-router, and Hermes module names | Missing as current dedicated modules. | Active docs cannot present these as current core modules. |
| `openlife-core/src/agent/multi_strategy_runtime.rs`, `openlife-core/src/agent/runtime_migration_gate.rs` | Missing. | Old preview/migration authority must be historical unless re-source-mapped. |
| `src-tauri/src/commands/router.rs` | Present. | This is a current Tauri command module name, not proof that old core `IntentRouter` or `LayerRouter` exists. |
| Obsolete detailed-architecture and API-doc targets | Missing. | Links to these targets are active broken links; do not create the stale targets as a cleanup fix. |
| `.github/PULL_REQUEST_TEMPLATE.md` | Present, no local markdown links. | Include it in PR/publication cleanup scope as a governance checklist surface; no link retarget is required. |
| `.github/workflows/*` markdown-link checker | No dedicated Markdown link checker found. | PR template checklist is manual governance, not CI link validation. |

## Stage5B Current Status Addendum

The Stage 2A source-map rows above are preserved as the original audit
snapshot. They are not the current runtime-module truth after Stage5A.

Stage5A repaired `src-tauri/src/main_chat_runtime_module_tests.rs` to guard the
current Phase7 ownership model instead of requiring the retired final
acceptance command/test owner. The current run of
`cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` passes
with 26 tests. This supersedes the Stage2/3/4 "runtime-module guard is red"
status only. It does not restore
`run_main_chat_agent_execution_v1_final_acceptance_gate`, does not restore
`src-tauri/src/main_chat_final_acceptance_tests.rs`, and does not claim Phase7,
Main Chat Agent Execution v1, or external live-provider evidence completion.

## Active Authority Claim Register

| Baseline id | Source | Stage 1 classification | Stage 2A action | Source-map decision |
| --- | --- | --- | --- | --- |
| `agents_k8_authority_vs_plans_readme` | `AGENTS.md` lines 13, 36, 44 | `needs_source_map` | `source_map_required` | `AGENTS.md` still names Goal 8/K8 as current Main Chat authority, while `plans/README.md` now says the active precedence is Phase7 single-system cleanup and older Goal/Stage/Beta/Migration docs are historical unless explicitly named. Stage 2B must reconcile this before AGENTS compression. |
| `agents_final_runner_and_test_owner` | `AGENTS.md` lines 15, 16, 17, 19, 27 | `contradicted` | `source_map_required` | Stage 2A snapshot recorded `main_chat_runtime_module` as red at that time; after Stage5A, the current run has passed. Do not restore the retired `run_main_chat_agent_execution_v1_final_acceptance_gate` command or `src-tauri/src/main_chat_final_acceptance_tests.rs` test owner, and do not claim Phase7, Main Chat Agent Execution v1, or external live-provider evidence completion. |
| `agents_old_router_module_listing` | `AGENTS.md` lines 162, 163, 225, 227 | `contradicted` | `mark_historical` | Old module listings for `router.rs`, `layer_router.rs`, `multi_strategy_runtime.rs`, and `runtime_migration_gate.rs` cannot steer current runtime work. Stage 2B should remove them from active guidance or label them historical. |
| `agents_live_provider_completion_boundary` | `AGENTS.md` lines 14, 15, 16, 19, 27, 28, 33, 71, 234 | `current` | `defer_with_reason` | The incomplete-live-provider boundary may remain as a blocker statement. It must not be rewritten as external live-provider completion evidence until the credited live-provider harness runs. |
| `architecture_chat_flow_intent_layer_router` | `docs/ARCHITECTURE.md` lines 91, 94 | `contradicted` | `fix_now` | The old Chat flow names `IntentRouter` and `LayerRouter`; current Main Chat source-map must use `MainChatKernel` / send / stream / turn-runtime surfaces. |
| `architecture_missing_architecture_detailed_and_api` | `docs/ARCHITECTURE.md` lines 192, 195, 256, 259 | `contradicted` | `retarget_link` | The obsolete detailed-architecture and API-doc targets do not exist. Stage 2B must retarget or remove these links before `docs/ARCHITECTURE.md` can be treated as current. |
| `architecture_hermes_module_listing` | `docs/ARCHITECTURE.md` line 169 | `contradicted` | `fix_now` | The old Hermes module listing is missing as a current dedicated source module. Replace the module list from source or mark the entire structure table historical. |
| `dev_handover_hermes_snapshot` | `docs/DEV_HANDOVER.md` line 32 | `historical` | `mark_historical` | The file already warns that module names may lag. Keep `hermes.rs` only as historical handover snapshot content. |
| `dev_handover_old_command_registration_shape` | `docs/DEV_HANDOVER.md` lines 41, 51, 92 | `needs_source_map` | `mark_historical` | The command-count guidance is not current authority. Stage 2B should label it historical or remove the counts; do not refresh counts inside Stage 2A. |

## Active Broken Markdown Links

Stage 1 recorded 14 active-doc broken Markdown links. Each receives an action
below; no source files are changed in Stage 2A.

| Source | Target | Stage 2A action | Stage 2B scope |
| --- | --- | --- | --- |
| `AGENTS.md:225` | `openlife-core/src/agent/multi_strategy_runtime.rs` | `mark_historical` | Remove from active current-module table or keep only in historical W106-W113 context. |
| `AGENTS.md:226` | deleted preview-audit utility path | `mark_historical` | Remove from active current-module table or source-map to current preview/audit surface before keeping. |
| `AGENTS.md:227` | `openlife-core/src/agent/runtime_migration_gate.rs` | `mark_historical` | Remove from active current-module table or keep only as historical W19 pilot/gate context. |
| `CONTRIBUTING.md:206` | `docs/ARCHITECTURE.md` | `retarget_link` | Retarget to `docs/ARCHITECTURE.md`, with a status label if that doc is still being repaired. |
| `CONTRIBUTING.md:207` | obsolete detailed-architecture target | `source_map_required` | No direct target exists. Choose a current detailed architecture target or remove the line in Stage 2B. |
| `CONTRIBUTING.md:208` | `plans/openlife_development_plan.md` | `retarget_link` | Retarget from repo root to `plans/openlife_development_plan.md`. |
| `CONTRIBUTING.md:209` | obsolete API-doc target | `source_map_required` | No root API docs exist. Choose a current API/command contract target or remove the line. |
| `CONTRIBUTING.md:210` | `docs/decisions/` | `retarget_link` | Retarget to `docs/decisions/`. |
| `OpenLife_Final_PRD.md:85` | `plans/openlife_development_plan.md` | `retarget_link` | Retarget the original absolute desktop link to this repo-relative target, or mark this PRD section historical-only. |
| `OpenLife_Final_PRD.md:86` | `plans/openlife_codex_execution_playbook.md` | `retarget_link` | Retarget the original absolute desktop link to this repo-relative target, or mark this PRD section historical-only. |
| `OpenLife_Final_PRD.md:1852` | `plans/openlife_development_plan.md` | `retarget_link` | Same retarget as line 85. |
| `OpenLife_Final_PRD.md:1853` | `plans/openlife_codex_execution_playbook.md` | `retarget_link` | Same retarget as line 86. |
| `docs/ARCHITECTURE.md:256` | obsolete detailed-architecture target | `retarget_link` | Retarget to a reviewed current architecture target or remove the "deep architecture" link. |
| `docs/ARCHITECTURE.md:259` | obsolete API-doc target | `retarget_link` | Retarget to a current API/command contract doc or remove the link. |

## Active Path-Mention Groups

The link baseline also recorded active-doc path-like mentions that are not all
Markdown links. Stage 2A groups the relevant blockers instead of editing text.

| Group | Examples | Stage 2A action | Decision |
| --- | --- | --- | --- |
| AGENTS final-gate/test-owner paths | deleted old final-acceptance test-owner module and retired final-acceptance command | `source_map_required` | Do not restore the retired command. Resolve through Phase7 runtime-module guard/update or a reviewed scope-out. |
| AGENTS old helper/module paths | deleted legacy agent-loop, legacy fallback, and strategy helper module names | `source_map_required` | These examples are missing in the current file check. Do not present them as current modules without a reviewed replacement source-map. |
| AGENTS deleted old runtime paths | `openlife-core/src/agent/multi_strategy_runtime.rs`, `openlife-core/src/agent/runtime_migration_gate.rs` | `mark_historical` | Keep only as historical progress context or remove from active tables. |
| AGENTS old progress/doc-index paths | old progress and doc-index namespaces | `source_map_required` | These cannot become active authority without a current doc target. |
| AGENTS legacy-write progress path | `src-tauri/src/legacy_write_convergence.rs` | `defer_with_reason` | Treat as historical W79-W87 progress text until AGENTS compression; not a Stage 2A source edit. |
| CONTRIBUTING branch-name example | `docs-architecture-update` | `defer_with_reason` | This is a branch naming example, not a local documentation link. Exclude or refine the future link checker. |

## GitHub PR / Publication Surface

`.github/PULL_REQUEST_TEMPLATE.md` has no local Markdown links to retarget. It
does include public Markdown/HTML and authority-sync checkboxes. Stage 2A
therefore includes it in the PR/publication cleanup scope as a governance
surface, but excludes it from required link-retarget edits.

Stage 2B may edit the template only if the cleanup owner wants an explicit
"local link baseline checked" checkbox. That would be a publication-governance
tightening, not a fix for a broken template link.

## Claims That Still Cannot Be Current Truth

- `Phase7 is complete`.
- `Main Chat Agent Execution v1 is complete`.
- `run_main_chat_agent_execution_v1_final_acceptance_gate` is a shipped/current command.
- The deleted old final-acceptance test-owner file is the current final-acceptance test owner.
- Stage2/3/4-era rows that say `main_chat_runtime_module` was red are not
  current truth after Stage5A. The precise current claim allowed after Stage5A
  is only: `main_chat_runtime_module` passes with the updated Phase7
  owner-shape guard. It was not solved by documentation cleanup and it is not a
  Phase7/Main Chat completion signal.
- Old `IntentRouter` / `LayerRouter` / `hermes.rs` are current Chat/core runtime authority.
- `multi_strategy_runtime.rs` or `runtime_migration_gate.rs` are current implementation files.
- The obsolete detailed-architecture or API-doc targets exist.
- `docs/DEV_HANDOVER.md` command counts are current without source refresh.
- `OpenLife_Final_PRD.md` IntentRouter examples describe current implementation.
- The PR template or CI currently provides automated Markdown link validation.

## Stage4C Expected-Absent Closure

Stage4C reclassifies the remaining active missing-record set as closed
expected-absent evidence, not unresolved repair work.

| Category | Records |
| --- | ---: |
| `active_doc_missing_records` | 37 |
| `active_expected_absent_records` | 37 |
| `stage4c_verified_expected_absent_records` | 37 |
| `active_actionable_repair_records` | 0 |
| `active_future_blocked_records` | 0 |
| `active_adr_blocked_records` | 0 |
| `active_unresolved_missing_records` | 0 |

The remaining active missing records all map to Phase7 deleted, test-archive,
or product-valid-rename evidence in the deletion manifest. They do not justify
restoring old runtime, command, frontend, or legacy-write files.

## Runtime-Module Status

Stage 2A originally recorded
`cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` as an
inherited blocker. Stage5A supersedes that current status by updating the guard
to the current Phase7 owner shape; the command now passes in the current run.
The Stage5A repair does not restore the retired final acceptance command and
does not make Phase7, Main Chat Agent Execution v1, or external live-provider
evidence complete.

## Stage7 Scope Reset Addendum

Stage7 records the Stage6E native trial RED state as product development
follow-up, not as a repository cleanup blocker.

Stage7 does not update the active claim register by rewriting product behavior
claims. It only preserves the current forbidden-claim boundary and records that
the next repository cleanup slice may enter `AGENTS.md` compression.

Stage7 current counts retained from the existing baseline:

| Category | Records |
| --- | ---: |
| current `rg --files -g '*.md' -g '*.html'` count | 209 |
| inventory document records retained from existing baseline | 205 |
| active broken Markdown links | 0 |
| active actionable repair records | 0 |
| active missing records retained as expected-absent evidence | 37 |

Stage7 baseline recomputation status:

- `recomputed=false`
- The inventory and link baseline were not fully regenerated.
- Stage4C / Stage5B summary records remain preserved as their original
  time-point/current-status records.

Stage7 scan interpretation:

- completion-claim scan matches are prohibited-claim lists, validation command
  examples, historical plans, or explicit caveat wording;
- retired final acceptance command/test-owner matches are not in shipped
  handler or frontend bridge surfaces;
- `tauriDev` product import scan has no matches in the checked product
  pages/components and `frontend/src/tauri.ts` surface.

Stage7 decision:

- `ready_for_stage8_agents_compression=true`
- Stage8 is limited to `AGENTS.md` compression and must not convert Stage6E
  product RED blockers into repository cleanup blockers.

Claims that still cannot be current truth after Stage7:

- `Phase7 complete` or `Phase7 completed`;
- `Main Chat Agent Execution v1 complete` or `Main Chat Agent Execution v1
  completed`;
- external live-provider evidence complete;
- `finalCompletionReady=true`;
- restored `run_main_chat_agent_execution_v1_final_acceptance_gate`;
- restored `src-tauri/src/main_chat_final_acceptance_tests.rs`;
- Stage6E RED product findings as repository cleanup blockers.

## Stage8 AGENTS Compression Addendum

Stage8 compressed root `AGENTS.md` from 883 lines to 179 lines and recorded the
decision in
`plans/openlife_repository_stage8_agents_compression_decision.md`.

The compressed AGENTS entrypoint keeps the allowed current claims:

- Phase7 is the active single-system deletion/product-trial contract;
- Main Chat current source-map has parallel `send_message` and
  `start_stream_message` entrypoints in `src-tauri/src/lib.rs`, which dispatch
  to `src-tauri/src/main_chat_send.rs` ->
  `OpenLifeTurnRuntime::run_buffered` and
  `src-tauri/src/main_chat_streaming.rs` ->
  `OpenLifeTurnRuntime::run_streaming` respectively, then converge through
  `src-tauri/src/main_chat_turn_runtime.rs` and
  `src-tauri/src/main_chat_kernel.rs`;
- Main Chat Agent Execution v1 remains in remediation;
- external live-provider-backed scenarios remain unclosed;
- proposal-first and no-silent-write constraints remain non-negotiable;
- `run_main_chat_agent_execution_v1_final_acceptance_gate` remains retired from
  shipped command/product bridge surfaces;
- `src-tauri/src/main_chat_final_acceptance_tests.rs` remains expected-absent.

Stage8 does not change the Stage7 interpretation of Stage6E: product RED
findings are product development TODOs, not repository cleanup blockers.

## Stage8-Rework Source-Map Addendum

Stage8-rework fixes the AGENTS source-map branch/parallel entrypoint issue
identified after compression. The correction was based on direct source checks
of `src-tauri/src/lib.rs`, `src-tauri/src/main_chat_send.rs`,
`src-tauri/src/main_chat_streaming.rs`,
`src-tauri/src/main_chat_turn_runtime.rs`, and
`src-tauri/src/main_chat_kernel.rs`.

The allowed current claim is now the parallel branch form:

- `frontend/src/tauri.ts` -> `src-tauri/src/lib.rs send_message` ->
  `src-tauri/src/main_chat_send.rs` ->
  `OpenLifeTurnRuntime::run_buffered`;
- `frontend/src/tauri.ts` -> `src-tauri/src/lib.rs start_stream_message` ->
  `src-tauri/src/main_chat_streaming.rs` ->
  `OpenLifeTurnRuntime::run_streaming`;
- both branches converge through `src-tauri/src/main_chat_turn_runtime.rs`,
  `src-tauri/src/main_chat_kernel.rs`, and core agent areas.

The old compressed wording that implied `main_chat_send.rs` flows into
`main_chat_streaming.rs` must not be reused.

After Stage8-rework, `AGENTS.md` is 190 lines and remains below the 250-line
limit.

## Stage9 Post-AGENTS Baseline Refresh Addendum

Stage9 recomputes the repository document inventory and local link/path
baseline after Stage8-rework.

Current Stage9 facts:

| Category | Records |
| --- | ---: |
| Markdown/HTML documents | 212 |
| active broken Markdown links | 0 |
| active expected-absent records | 40 |
| active actionable repair records | 0 |
| historical/private missing path or link records | 319 |

`AGENTS.md` is 190 lines, and the allowed Main Chat source-map claim remains
the parallel branch form introduced by Stage8-rework.

Stage9 blocker interpretation:

- Active broken Markdown links are zero.
- Active actionable repair records are zero.
- The active missing records are expected-absent evidence; they are not restore
  requests.
- Stage6E product RED findings remain product development TODOs, not repository
  cleanup blockers.

Stage9 therefore allows the next repository cleanup step to prepare a bounded
architecture/document ownership cleanup. It does not allow direct broad
architecture-doc rewriting, ADR 0013 movement, plan archive creation, product
behavior edits, or completion/readiness closure claims.
