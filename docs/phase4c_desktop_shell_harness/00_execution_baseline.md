# Phase 4C Execution Baseline

Status: `PASS_FOR_HUMAN_REVIEW`
Date: 2026-07-20

## Verified Mainline

- Phase 4B PR `#57` merged as `5ae8cc89cbe281c47791d53ab61c754f4ff3f484`.
- local `main` and `origin/main` were identical at that commit before branching;
- protected-main CI run `29727526369` completed `success`, including Frontend,
  Rust Linux/macOS/Windows, coverage, audit, and Smoke Test;
- Phase 4C branch: `codex/phase4c-desktop-shell-harness`.

This satisfies the rule that Phase 4C starts from a merged, CI-green, locally
reverified `main`, not from the old convergence branch.

## Product Scope Correction

OpenLife's current product target is the Tauri desktop application. Phase 4C
is therefore desktop-only:

- review viewports: `1440x900` and `1280x800`;
- Tauri review window: `1280x800`, minimum `1024x720`;
- no mobile app bar, bottom navigation, drawer, bottom sheet, responsive mobile
  route, or mobile acceptance gate;
- prior `390x844` artifacts remain historical visual/reflow references only.

The earlier Phase 3F mobile section does not authorize a mobile product or
backend. It is not an implementation requirement for Phase 4C or Phase 4D.

## Allowed Work

- reusable semantic desktop Shell component;
- isolated Vite/Tauri development harness;
- static fixtures and QA toolbar outside the product shell;
- layout tokens, interaction tests, accessibility checks, screenshots, and
  release absence guards;
- migration/deletion ledger update.

## Explicit Non-Goals

- no import into `frontend/src/App.tsx`;
- no change to `frontend/src/components/ProductShell.tsx`;
- no change to `frontend/src/productShellContract.ts`;
- no production route, navigation entry, page replacement, or authority switch;
- no Rust/Tauri business command, read model, permission, review, or durable
  write change;
- no claim that fixture values are live backend state;
- no Phase 4D page migration.

## Authority Boundary

`OpenLifeWorkbenchShell` is a candidate source owner but has no production
caller. The existing ProductShell remains the only production shell until the
Phase 4E atomic switch. Production build and Rust guards prove this boundary.

## Infrastructure Drift Found During Execution

Tauri CLI `2.11.0` chooses a different default hook directory depending on
whether its repo-local binary is launched from the repository root or from
`src-tauri/`. A string hook plus `pnpm --dir ...` therefore worked in only one
invocation shape. Independent review reproduced the failure before merge.

Both dev-only overlays now use Tauri's structured `beforeDevCommand` with an
explicit `cwd: "../frontend"`; the script itself has no path inference. Rust
guards freeze that configuration, and Phase 4C startup is rechecked from the
repository root. The normal product startup scripts already provide their own
absolute frontend directory and were not changed.
