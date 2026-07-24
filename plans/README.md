# OpenLife Plans Document Governance

> Last updated: 2026-07-24
> Status: active authority map for Phase7 remediation and the Current
> Development Program

This file is the active plan index for Agents. Its purpose is to keep old
planning documents from steering new work and to keep current claims at the
evidence level actually proved by the current baseline.

## Current Precedence

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`
5. `plans/openlife_current_development_program.md` for current goals, Wave
   order, go/no-go gates, feature-reopen rules, and Agent task contracts.
6. `plans/openlife_current_development_program.json` for the machine-readable
   live Program status, dependencies, gate state, and approval boundary. Item 5
   is the stable human contract; any substantive drift from its approved
   version stops activation.
7. `plans/openlife_problem_ledger.json` for the current 101-card owner,
   evidence, assigned-Wave, next-proof, and closure state.
8. `plans/openlife_restart_baseline_cleanup.json` for the frozen 2026-07-22
   ref, retention, V4, historical 72-card, and cleanup evidence snapshot only.
9. A task-specific decision/preparation file explicitly named by the user,
   subordinate to items 1-8 and bound to a Current Development Program slice.

The Program's immutable machine gate is
`scripts/validate-current-development-program.mjs`; its disposable Git-fixture
mutation suite is `scripts/test-current-development-program-validator.mjs`.
Both are part of the approved Program surface, not optional helper scripts.

V4, roadshow, Goal, Stage, Step6, Beta, dogfood, eval, adapter, migration,
cutover, productization, maturity, W-series that predate the Current
Development Program, and older roadmap documents are historical reference and
evidence only. Their point-in-time `active` or execution wording does not grant
current authority and must not restart their task order.

## Current Baseline And Development Boundary

- `/Users/tw/Desktop/open-life` is the only writable OpenLife checkout.
- `main` is the only long-term local and remote product branch. A short-lived
  `codex/...` branch may exist only in this checkout for a reviewed PR and must
  be removed after merge.
- PR #65 completed the restart-baseline cleanup. The formal review evidence
  baseline is `de158ce53018c9c649f7dc0dcb3bdd8271ed4977`; it is not a claim
  that later Program commits or execution slices have the same HEAD.
- The formal review run completed its fact-collection pass and produced 101
  distinct baseline problem cards, but closed zero findings. This is not an
  exhaustive claim that every possible repository defect is known. The current
  owner, evidence, Wave, next-proof, and closure record is
  `plans/openlife_problem_ledger.json`.
- The exact cleanup-time branch/ref inventory, V4 13-commit classification,
  historical 72-finding registry, recovery assets, and retention evidence are
  frozen in
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

The Current Development Program reorganizes remediation on current `main`.
Creating or reviewing that Program does not authorize execution while its JSON
sets `execution_authorized` to `false`. Production-code candidates such as
`ReflexEngine`, invalid V4 config, `save_chat_message`, old route references,
or Projection error folding may change only through a finding-bound,
current-SHA slice after Program authorization.

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
- `plans/openlife_current_development_program.md`
- `plans/openlife_current_development_program.json`
- `plans/openlife_problem_ledger.json`
- `plans/openlife_restart_baseline_cleanup.json`
- `plans/openlife_single_system_phase1_inventory.json` (supporting source-map
  and guard inventory, not current task order)
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `plans/adr/0014-explicit-user-memory-write-lane.md`
- `plans/adr/0015-transient-state-command-lane.md`

The deletion manifest owns expected-absent and current-authority disposition.
The development preparation owns single-system architecture boundaries. The
Current Development Program Markdown owns task order and execution policy; its
JSON owns machine state and dependencies; the problem ledger owns per-card
facts and closure credit. The restart cleanup JSON is a frozen recovery/fact
snapshot, and the Phase1 inventory supports guards and source tracing. None of
these artifacts may infer implementation or finding closure that lacks
current-SHA evidence.

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

V4 may contribute its subordinate root-cause method: source map, real RED,
root invariant, minimal fix, same-slice old-path deletion, and
positive/counterfactual/absence/non-regression evidence. Its branch, 13
commits, old phase order, and historical state do not regain execution
authority. Roadshow evidence grants no current product or feature credit.

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
- no new Goal, Stage, Roadshow, or legacy W-series document may bypass the
  Current Development Program or assign work without a Program slice ID.
