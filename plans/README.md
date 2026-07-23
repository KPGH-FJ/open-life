# OpenLife Plans Document Governance

> Last updated: 2026-07-23
> Status: active authority map for the Phase7 restart-baseline cleanup

This file is the active plan index for Agents. Its purpose is to keep old
planning documents from steering new work and to keep current claims at the
evidence level actually proved by the current baseline.

## Current Precedence

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`
5. `plans/openlife_restart_baseline_cleanup.json` for restart-baseline refs,
   facts, finding states, retention boundaries, and deletion evidence only.
6. A task-specific decision/preparation file explicitly named by the user,
   subordinate to items 1-5.

V4, roadshow, Goal, Stage, Step6, Beta, dogfood, eval, adapter, migration,
cutover, productization, maturity, W-series, and older roadmap documents are
historical reference and evidence only. Their point-in-time `active` or
execution wording does not grant current authority and must not restart their
task order.

## Restart Baseline Cleanup Boundary

- `/Users/tw/Desktop/open-life` is the only writable OpenLife checkout.
- `main` is the only long-term local and remote product branch. A short-lived
  `codex/...` branch may exist only in this checkout for a reviewed PR and must
  be removed after merge.
- PR #64 is merged in the recorded baseline
  `74059dbc819851f0ef4597f055d0d6c956e0cd77`. That fact does not close an
  unrelated product finding.
- The exact branch/ref inventory, V4 13-commit classification, 72-finding
  registry, recovery assets, and evidence statuses are owned by
  `plans/openlife_restart_baseline_cleanup.json`.
- Finding closure requires current-baseline implementation, independent
  verification, and closure evidence. Passing tests or historical evidence
  alone cannot change an `UNKNOWN` finding to closed.
- Phase4F grants only the exact native-artifact credit recorded in
  `docs/phase4f_desktop_product_acceptance/03_native_trial_report.md`. Its
  broad six-route observations belong to a historical artifact; the reviewed
  exact artifact grants bounded Today/Settings and Settings fail-closed
  evidence only.
- Phase7 remains `red-until-trial-green`. Browser-shell, native-Tauri, and
  external-live evidence stay separate.

This cleanup does not repair, delete, or refactor production-code findings such
as `ReflexEngine`, invalid V4 config, `save_chat_message`, old route references,
or Projection error folding. Those remain inputs to the next formal full-repo
review.

## Phase7 Contract

Phase7 is a deletion and product-trial pass. It is not another compatibility
adapter and not a documentation-only supersession.

The shipped product must have:

- no old runtime module in the product crate graph;
- no old command in the shipped Tauri handler;
- no old wrapper in `frontend/src/tauri.ts`;
- no product page/component importing dev-only bridges;
- no product page/component consuming old fallback/status fields;
- no active README or active plan index authorizing old routes;
- a native desktop trial report that is green, or red with explicit fail-closed
  blockers and no Phase7 completion claim.

## Active Documents

- `plans/openlife_single_system_deletion_manifest.md`
- `plans/openlife_single_system_development_preparation.md`
- `plans/openlife_restart_baseline_cleanup.json`
- `plans/openlife_single_system_phase1_inventory.json` (supporting source-map
  and guard inventory, not current task order)
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `plans/adr/0014-explicit-user-memory-write-lane.md`
- `plans/adr/0015-transient-state-command-lane.md`

The deletion manifest owns expected-absent and current-authority disposition.
The restart cleanup JSON owns this cleanup's machine-readable baseline facts.
The Phase1 inventory supports guards and source tracing. None of these artifacts
may infer implementation or finding closure that lacks current-SHA evidence.

## Historical Evidence

Historical files remain in Git where useful. They explain prior decisions,
findings, and bounded evidence; they do not set current task order and do not
authorize product-visible old routes.

This historical set includes:

- `plans/main_chat_agent_kernel_rescue_goal_8_cleanup_final_gate.md`,
  `plans/main_chat_stage2_preparation_index.md`,
  `plans/main_chat_agent_stage2_internal_trial_goal_spec.md`, and
  `plans/main_chat_agent_migration_v1_goal_spec.md` as guard-pinned historical
  reference examples, not active execution entries;
- `plans/openlife_backend_remediation_v4.md` and its inventory, discovered
  findings, traceability, scenario, waiver, phase evidence, and D0xx support
  files;
- `plans/openlife_roadshow_core_capability_execution.md` and its scenario,
  state, and waiver files;
- Stage, Step6, Goal, Beta, migration, cutover, productization, maturity, and
  older Main Chat planning documents.

If historical wording conflicts with this active stack, the current
single-system contract wins. Expected-absent paths in historical evidence must
not be recreated to make an old document or scan pass.

## Guard Rules

- no new system without deletion;
- no product-visible legacy/beta/stage/migration/cutover route;
- no direct durable write outside gateway authority;
- no frontend independent product readiness source;
- no readiness/completion claim without the required gates and exact trial
  evidence;
- no browser-shell result described as native Tauri or external-live credit.
