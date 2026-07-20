# Phase 4D Read-Only Spine Execution Baseline

Status: `IMPLEMENTED_PENDING_HUMAN_REVIEW`
Date: 2026-07-20

## Verified Mainline

- Phase 4C PR `#58` merged as
  `45828f13580036b80b8efabfc3d12f30103081dc` after human approval.
- protected-main CI run `29738119770` completed `success` for that exact SHA;
- the run passed Frontend, Rust Linux/macOS/Windows, coverage, audit, and Smoke
  Test jobs;
- local `main` and `origin/main` were identical and clean before branching;
- local frontend typecheck, formatting, production build, and production
  absence guard passed on the same mainline;
- Phase 4D branch: `codex/phase4d-read-only-spine`.

This branch was not created from the former convergence branch or from an
unverified local continuation.

## Slice Goal

Connect the approved desktop workbench Shell candidate to the first real
backend-owned, read-only product journey:

1. Today reads the strict `openlife.today-adapter.v1` inputs;
2. Tasks reads the backend `TasksViewModel` envelope;
3. both surfaces refresh from Tauri rather than changing local product truth;
4. evidence metadata opens in the structured Inspector;
5. unported journeys return a visible unavailable state;
6. missing, stale, incoherent, or unknown data remains fail-closed.

## Allowed Work

- production-candidate Today and Tasks renderers with no production caller;
- a typed Tauri read-only data source;
- an isolated dev-only Vite/Tauri harness and browser QA fixtures outside the
  product Shell;
- desktop layout, interaction, accessibility, field-source, and release
  isolation tests;
- migration/deletion ledger updates for the exact new owners and remaining old
  callers;
- test-only or absence-guard changes in Rust, with no business behavior change.

## Explicit Non-Goals

- no edit to `frontend/src/App.tsx`;
- no edit to `frontend/src/components/ProductShell.tsx`;
- no edit to `frontend/src/productShellContract.ts`;
- no production route, navigation authority, or Shell switch;
- no Workspace permission dispatch, Review decision, task control, settings
  save, LifeModel write, or other durable/external action;
- no raw `AgentRun` join in Tasks V2;
- no page-local inference of provider route, external transmission, proposal
  status, task lifecycle, or completion;
- no mobile app bar, bottom navigation, drawer, sheet, route, viewport, or
  acceptance gate.

The Phase 4B Foundation still contains historical narrow-viewport token names
and rules. They are not a mobile product contract, are not exercised by this
slice, and do not authorize mobile implementation. The Phase 4D Tauri window
keeps the desktop minimum width at `1024px`.

## Production Authority Boundary

The old `ProductShell`, `TodayPage`, and `RunsPage` remain production owners
through this slice. The new journey is reachable only through the isolated
`/dev/phase4d/` entry and must be absent from the normal release bundle.

Phase 4E remains the only authority-switch point. There is no `/v2` product
route and no production fallback between old and new shells.

## Exit Evidence Required

1. exact field-source and action tables match implementation;
2. component and data-source tests cover loading, ready, empty, stale, error,
   unknown boundary, completed-without-evidence, and unavailable navigation;
3. enabled controls have a verifiable result and disabled controls name a
   reason;
4. keyboard focus, `aria-current`, live feedback, Inspector open/close, search,
   and filter paths pass;
5. normal production build proves Phase 4D code and markers absent;
6. the dev-only Vite entry rejects non-canonical HTML routes;
7. real Tauri starts with an isolated `OPENLIFE_DATA_DIR` and reads the actual
   commands without using browser fixtures;
8. desktop QA passes at `1440x900` and `1280x800`;
9. the migration/deletion ledger names all remaining old callers;
10. the PR remains unmerged pending human review.

Implementation and automated evidence are complete. The real isolated Tauri
run proved command connectivity and fail-closed rendering, but did not produce
a ready/empty Tasks result because required stores were unavailable. That
bounded distinction is recorded in `03_qa_report.md` and must not be restated
as backend-ready proof.
