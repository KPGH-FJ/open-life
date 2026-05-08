# OpenLife vNext P11 Task Specifications

Date: 2026-05-08

Status: current

Package:

```text
Beta Trial Readiness
```

P11 starts after P10 Frontend Agent Workspace has passed acceptance. The
runtime spine and workspace surfaces are now visible enough for internal trial
use. P11 should not reopen the vNext architecture or expand runtime authority.
Its job is to make OpenLife testable by a real user in a repeatable, diagnosable,
and recoverable way.

Companion checklist:

- `plans/openlife_vnext_p11_trial_path_matrix.md`

The product goal is simple: a tester should be able to install or launch the
app, configure a model, build or inspect a LifeModel, run an agent conversation,
review proposals, inspect runs/traces, recover from data/config issues, and
send useful feedback without developer intervention.

## Baseline Review

Before P11:

- P0-P9 runtime governance exists: AgentRunEvent, PromptStack, ToolRuntime,
  Proposal, MemoryEvidence, AgentSpec, PlanMode, Compaction, and P9
  ExecutionSandbox/Shell default-off policy.
- P10 workspace surfaces exist: Workspace, Runs, AgentRunDetail, run timeline,
  tool observation panel, proposal evidence context, and plan operations.
- `make ci` is the hard release gate and passed at P10 acceptance.
- Remaining work is trial readiness, not new framework authority.

## Global Rules

- Execute exactly one P11 task spec at a time.
- Do not rewrite `ChatPage`.
- Do not enable normal chat shell, scheduled shell, proactive shell, or
  sub-agent shell.
- Do not add new runtime privileges unless a separate ADR and task spec exist.
- Keep mutations proposal-first or behind existing governed operations.
- Prefer diagnostics, checklists, and clear recovery paths over new features.
- Add tests for any changed UI or command contract.
- `make ci` remains the final gate.

## P11-0: Documentation And Phase Sync

Goal:

Make P11 discoverable and mark P10 as completed.

Expected behavior:

- `README.md` and `AGENTS.md` state that P10 has passed acceptance and P11 Beta
  Trial Readiness is current.
- P11 task specs are linked from the standard vNext entry points.
- P11 non-goals are explicit: no ChatPage rewrite, no default shell enablement,
  no new runtime authority, no SubAgent expansion.

Allowed edit areas:

- `AGENTS.md`
- `README.md`
- `plans/openlife_vnext_p11_task_specs.md`
- `plans/openlife_vnext_migration_plan.md`
- `plans/openlife_vnext_test_and_acceptance_matrix.md`
- `plans/openlife_vnext_agent_coding_prompts.md`
- `plans/openlife_vnext_p11_trial_path_matrix.md`

Verification:

- `rg -n "openlife_vnext_p11_task_specs|P11|Beta Trial Readiness|P10 .*complete|P10 .*完成" AGENTS.md README.md plans`
- `git diff --name-only` contains documentation files only for this task.

## P11-1: Trial Path Matrix

Goal:

Define repeatable manual trial scripts that cover the product's core loop.

Expected behavior:

- Add a trial matrix with setup, steps, expected outcome, failure signals, and
  recovery path for:
  - first launch and diagnostics
  - model provider configuration
  - quick LifeModel build
  - chat to proposal generation
  - proposal review and apply
  - run trace inspection
  - plan confirmation / legal operation inspection
  - data export / backup / safe mode recovery
- The matrix distinguishes must-pass smoke paths from optional exploratory paths.

Allowed edit areas:

- `plans/openlife_vnext_p11_task_specs.md`
- `README.md`
- `plans/openlife_vnext_p11_trial_path_matrix.md`

Verification:

- Trial scripts can be followed without reading source code.
- Each script has a concrete expected result and recovery instruction.

## P11-2: Trial Readiness Console

Goal:

Make readiness visible in the app without requiring log inspection.

Expected behavior:

- Settings or Workspace shows a compact Beta readiness checklist.
- The checklist covers model readiness, data health, safe paths, proposal
  backlog, recent failed runs, backup availability, and `make ci` / build
  provenance where available.
- Each blocked item links to the surface that can fix it.

Allowed edit areas:

- frontend Settings / Workspace components
- `frontend/src/tauri.ts` and mocks if an existing diagnostics command needs a
  typed wrapper
- minimal read-only Tauri diagnostics additions if existing data is unavailable

Constraints:

- No new mutation behavior.
- No new model calls.
- No shell enablement.

Verification:

- frontend tests for ready, partial, blocked, and safe-mode states
- `pnpm --dir frontend typecheck`

## P11-3: End-To-End Smoke Checklist

Goal:

Create a lightweight release smoke suite for trial builds.

Expected behavior:

- A smoke checklist exists for a clean profile and an existing profile.
- It includes manual steps and, where practical, automated tests.
- It verifies that Chat streaming, Proposal Review, Runs/Trace, Workspace, and
  Settings diagnostics still work together.

Allowed edit areas:

- `plans/`
- frontend tests only if adding automated smoke coverage
- Tauri tests only if adding command-level smoke coverage

Verification:

- `make ci`
- documented manual smoke run with pass/fail notes before a trial build

## P11-4: Recovery And Data Safety Drill

Goal:

Prove that a trial user can recover from common data and config problems.

Expected behavior:

- Backup/export/import and snapshot behavior are documented and tested where
  automated coverage exists.
- Safe Mode guidance is actionable.
- Proposal apply failures leave proposals pending and explain the problem.
- External writes remain safe-path bounded.

Allowed edit areas:

- Settings / Data / Recovery UI
- proposal error copy
- docs and tests

Constraints:

- Do not implement automatic rollback unless ADR 0011 is accepted and a separate
  implementation task exists.
- Do not weaken safe-path checks.

Verification:

- relevant proposal/storage/settings tests
- manual recovery checklist
- `make ci`

## P11-5: Feedback And Trial Telemetry Loop

Goal:

Make trial feedback useful without collecting unnecessary private data.

Expected behavior:

- Users can export a diagnostic bundle or summary that excludes raw sensitive
  content by default.
- Feedback can reference run IDs, proposal IDs, diagnostics state, and app
  version/build context.
- Feedback guidance tells testers what to include when reporting failures.

Allowed edit areas:

- Feedback / Settings / diagnostics surfaces
- read-only diagnostic DTOs
- docs and tests

Constraints:

- No automatic upload of private data.
- No raw prompt, memory, LifeModel, or tool output export by default.

Verification:

- diagnostic export redaction tests
- frontend tests for feedback states
- `make ci`

## P11 Exit Criteria

P11 is complete when:

- P10 is documented as accepted and P11 is the current phase.
- A tester can follow the trial path matrix without source-code knowledge.
- The app exposes a clear readiness / diagnostics surface.
- Core manual smoke paths pass on a clean profile and an existing profile.
- Recovery guidance covers safe mode, backups, proposal failures, and safe-path
  writes.
- Feedback/diagnostic export is useful and privacy-governed.
- P9 shell guarantees remain unchanged.
- `make ci` passes.

Recommended final verification:

- `rg -n "P11|Beta Trial Readiness|trial path|smoke|recovery" README.md AGENTS.md plans`
- `pnpm --dir frontend test`
- `pnpm --dir frontend typecheck`
- `cargo test -p openlife-core`
- `cargo test -p openlife-tauri`
- `pnpm --dir frontend build`
- `make ci`
