# Phase 4D Governed-Action Spine Execution Baseline

Status: `IMPLEMENTED_PENDING_HUMAN_REVIEW`
Date: 2026-07-20

## Verified Starting Point

- previous Phase 4D read-only spine: merged through PR `#59`;
- verified main commit before this branch:
  `5b08f53d7a98746d54528d31e83156485f12f91e`;
- implementation branch: `codex/phase4d-governed-action-spine`;
- backend authority baseline: the merged Roadshow/Phase7 backend plus the Phase
  4A read-model contracts already on main;
- no V4 backend is assumed or described by this slice.

The current backend remains the authority for task lifecycle, permission
context, review decisions, provider/privacy boundaries, and durable truth. This
slice adds no alternative business authority.

## Scope

This slice implements one continuous desktop candidate journey in the existing
dev-only Phase 4D workbench:

```text
Workspace
  -> inspect exact permission request
  -> Review decision
  -> refresh Workspace + Review + Tasks read models
  -> return to Workspace
  -> request exact task resume
  -> refresh and verify the same task identity/state
```

Included:

- `WorkspaceGovernedView` and `ReviewGovernedView`;
- typed Tauri data source over Workspace, Review Center, and Tasks read models;
- review and task-resume state machines;
- deterministic dev-only fixtures for pending, approved, resumed, stale,
  error, empty, and incomplete-permission states;
- desktop browser and keyboard QA at `1440x900`, `1280x800`, and `1024x720`;
- release absence guards and migration/deletion ledger updates.

## Non-Goals

- no change to `App.tsx`, `ProductShell.tsx`, `productShellContract.ts`, or any
  production route;
- no production navigation switch, `/v2` route, or fallback system;
- no backend business-rule, durable-write, permission, or materialization
  change;
- no JSON proposal editor, review apply command, revoke command, or page-local
  action inference;
- no LifeModel/Memory durable-truth journey or Settings write journey;
- no mobile implementation or mobile acceptance.

## Required Invariants

1. Opening a permission request records no decision.
2. Approval command return is not approval proof; the same ReviewItem must be
   confirmed by a refreshed ReviewCenterViewModel.
3. Approved permission does not automatically resume the task.
4. Resume command return is not running/completion proof; a refreshed Tasks
   read model must contain the exact task/session identity.
5. Running is not completed; completed requires delivered final evidence.
6. Stale, error, missing, incoherent, or incomplete scope remains fail-closed.
7. ReviewAction, TaskControl, ProductAction, and DebugAction stay distinct.
8. Fixture truth remains visibly outside the product Shell and never counts as
   backend readiness evidence.

## Exit Gate

The slice is reviewable only when all of the following are true:

- targeted and full frontend tests pass;
- Rust authority guard and `single_system` suite pass;
- production build proves the harness and candidate journey absent;
- dev build and desktop browser QA pass;
- screenshots show no overflow and keep the primary review decision visible;
- real Tauri is attempted with isolated state and any unavailable journey is
  reported as bounded evidence, not fixture-backed success;
- no production owner is changed and no ledger row is marked `delete_ready`.
