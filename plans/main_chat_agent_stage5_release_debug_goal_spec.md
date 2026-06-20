# Main Chat Agent Stage 5 Release Debug Goal Spec

> Date: 2026-06-20
> Status: prepared for CLI goal mode
> Depends on: Stage 4 implementation commit `d072283`

## 1. Objective

Implement **Main Chat Agent Stage 5: Internal Trial Release and Debug
Operations**.

The output should make OpenLife ready to run serious internal testing by making
Agent runs diagnosable, exportable, redacted, and tied to build/scenario
evidence.

Stage 5 must produce:

- release/build provenance;
- environment/provider/workspace/MCP/database preflight;
- metadata-safe Agent debug bundles;
- failure taxonomy and recovery recommendations;
- issue-report/export workflow for internal testers;
- DBG5-01 through DBG5-24 Stage 5 coverage report;
- implementation report.

Stage 5 is not limited-internal-trial readiness and must not fill or fabricate
S2-D manual dogfood rows.

## 2. Required Reading

Read before editing code:

- `AGENTS.md`
- `plans/main_chat_stage5_preparation_index.md`
- `plans/main_chat_stage5_release_debug_best_practices.md`
- `plans/main_chat_stage5_objective_and_scope.md`
- `plans/main_chat_stage5_current_gap_inventory.md`
- `plans/main_chat_stage5_release_debug_product_contract.md`
- `plans/main_chat_stage5_debug_privacy_redaction_contract.md`
- `plans/main_chat_stage5_internal_tester_workflow.md`
- `plans/main_chat_stage5_failure_taxonomy.md`
- `plans/main_chat_stage5_release_debug_eval_matrix.md`
- `plans/main_chat_stage4_implementation_report.md`
- `plans/main_chat_stage2_readiness_gate_contract.md`
- `plans/main_chat_stage2_manual_dogfood_artifact_template.json`

## 3. Non-goals

- Do not create a second Agent runtime.
- Do not create a second task/session store.
- Do not create a second proposal or memory system.
- Do not implement a hosted telemetry backend.
- Do not implement public release/distribution.
- Do not add full OpenTelemetry exporter unless all local Stage 5 goals are
  already complete and tests prove no scope expansion.
- Do not run or fill S2-D01 through S2-D24 manual dogfood rows.
- Do not claim `ready_for_limited_internal_trial`.
- Do not lower Stage 1, Stage 2, Stage 3, Stage 4, final acceptance, or
  live-provider gates.

## 4. Required Implementation Areas

### Phase 0: report skeleton

Add `main_chat_stage5_release_debug` report with DBG5-01 through DBG5-24 rows.

The report must preserve:

- `notAReadinessGate=true`;
- `readinessClaim=false`;
- Stage 2 readiness fail-closed semantics;
- no manual/live evidence fabrication.

### Phase 1: release and environment preflight

Add a read-only preflight model for:

- build commit/branch/version/timestamp/dirty-state where available;
- provider identity, key presence boolean, network opt-in, scheduler type;
- workspace root digest, safe path summary, database/store availability;
- MCP registry/candidate availability;
- Stage 2 readiness status and final/live-provider blockers.

Do not invoke an external provider during default preflight.

Build provenance must come from deterministic build/app metadata where possible:
build-time commit/branch metadata, Tauri/package app version, build timestamp,
and dev-only dirty-state if available. Do not run ad hoc runtime git commands
from the packaged app. Missing build fields must become named blockers rather
than fabricated values.

### Phase 2: metadata-safe debug bundle

Add a bundle assembler for a task/session/run. It must include:

- task/run/transcript/action/proposal/final delivery ids and statuses;
- route/provider/model/policy metadata;
- tool/action/observation summaries;
- memory/context/knowledge inventory summaries;
- failure classification and recovery recommendation;
- redaction report.

Default export must be metadata-safe and fail closed on secret leakage.

Bundle and issue artifacts must be stored under the app data directory by
default, not inside the git workspace. They must use schema-versioned JSON,
atomic temp-file-then-rename writes, artifact id/storage alias/digest/byte-size
metadata, list/get after refresh, and explicit delete or retention pruning.

### Phase 3: failure taxonomy

Implement stable classification using
`plans/main_chat_stage5_failure_taxonomy.md`.

At minimum support:

- routing;
- environment preflight;
- provider;
- tool selection;
- tool execution;
- policy blocker;
- memory context;
- knowledge asset;
- final delivery;
- UI state;
- recovery;
- redaction;
- release artifact;
- unknown.

### Phase 4: issue report/export workflow

Add commands and UI to create an internal issue report with:

- scenario id;
- reviewer/tester id;
- status;
- task session id for task-attached reports;
- run id for task-attached reports;
- bundle id;
- build commit;
- redaction mode;
- notes digest or bounded notes preview;
- local artifact id/storage alias/digest/byte size.

The issue report must not be treated as validated Stage 2 manual dogfood
evidence by default.

Task-attached issue reports must include both task session id and run id.
Preflight-only or environment-blocked reports may omit task/run ids only with a
named blocker and an explicit missing task/run reason, and must not be marked as
task behavior `pass`.

Issue reports must reuse the same app-data artifact store and must not be saved
as source-controlled files unless the user explicitly exports/copies them later.
They must support list/get after refresh and explicit delete or retention
pruning, matching the debug bundle artifact lifecycle.

### Phase 5: product UI

Add or extend UI surfaces so testers can:

- view preflight;
- export current/selected task debug bundle;
- see failure class and recovery recommendation;
- create issue report;
- inspect saved bundle/report metadata after refresh.

Reuse Main Chat / AgentControlPlane / Review Center patterns. Do not create a
second control plane.

UI evidence included in a bundle must be correlated with backend evidence. It
should include frontend route/surface, visible control labels, task session id,
backend snapshot id when available, timestamp, and optional screenshot/DOM
digest. UI evidence alone must not prove provider readiness, action execution,
memory usage, or rollback success.

### Phase 6: eval and implementation report

Add focused backend/frontend tests for DBG5-01 through DBG5-24.

Managed `USER.md` / `MEMORY.md` write and rollback DBG5 tests must use an
isolated eval AppState and temporary workspace root. They must not write the
real repository root, real user knowledge files, or source-controlled files.

Create `plans/main_chat_stage5_implementation_report.md` with:

- completed phases;
- changed files;
- DBG5 scenario results;
- tests run;
- remaining blockers;
- explicit statement that Stage 5 does not grant limited internal trial
  readiness.

## 5. Test Plan

Run at minimum:

```bash
git diff --check
cargo fmt --check
cargo test -p openlife-core main_chat_agent_v1 -- --nocapture
cargo test -p openlife-tauri main_chat_stage5_release_debug -- --nocapture
cargo test -p openlife-tauri main_chat_stage4_memory_knowledge -- --nocapture
cargo test -p openlife-tauri main_chat_stage3_execution_ux -- --nocapture
cargo test -p openlife-tauri main_chat_agent_stage2_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_agent_productization -- --nocapture
cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture
pnpm --dir frontend typecheck
pnpm --dir frontend format:check
pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/pages/ProposalReviewPage.test.tsx src/tauri.test.ts
```

Add focused frontend tests for new debug/export/preflight UI.

## 6. Acceptance

Stage 5 can be accepted when:

- DBG5-01 through DBG5-24 are covered as passed or blocked with named blockers;
- default preflight is read-only and no external provider is invoked;
- metadata-safe debug bundles exist for representative DirectAnswer, read
  action, policy blocker, memory proposal/context, managed knowledge write,
  rollback, and final delivery paths;
- export redaction proves no API key, auth header, full prompt, raw memory, or
  full knowledge file leaks by default;
- issue reports include required build/scenario/task/bundle ids and can be
  reloaded; task-attached reports also include run id, while preflight-only
  reports include named missing task/run blockers and cannot claim task pass;
- Stage 2 readiness remains fail-closed without real manual/live evidence;
- Stage 1/2/3/4/final/local gates are not weakened.

## 7. Required Final Response

After implementation, report:

- whether Stage 5 is complete;
- what changed;
- which DBG5 scenarios passed or remain blocked;
- tests run and any tests not run;
- whether Stage 2 readiness remains `not_ready_for_limited_internal_trial`;
- whether it is appropriate to proceed to real manual dogfood preparation or
  the next stage.
