# Phase 3A-1 Methodology

Status: implementation report for the TodayViewModel limited slice.

## Documents Read

- `AGENTS.md`
- `plans/README.md`
- `plans/openlife_single_system_deletion_manifest.md`
- `plans/openlife_single_system_development_preparation.md`
- `OpenLife_Phase3A_TodayViewModel_Limited_Slice_Codex_Goal_v1.0.md`
- `docs/phase2_viewmodel_contract/14_phase2_summary_and_phase3_readiness.md`
- `docs/phase2_viewmodel_contract/10_today_viewmodel_contract.md`
- `docs/phase2_viewmodel_contract/03_shared_viewmodel_envelope_and_types.md`
- `docs/phase2_viewmodel_contract/13_contract_test_plan.md`
- `docs/phase2_viewmodel_contract/12_backend_contract_gap_register.md`
- `docs/phase2_viewmodel_contract/04_lifestate_projection_extension_plan.md`
- `docs/phase1_ux_ia/09_view_model_contract_proposal.md`
- `docs/phase0_5/06_view_model_gap_inventory.md`

## Source Files Inspected

- `frontend/src/tauri.ts`
- `frontend/src/utils/lifeStateProjection.ts`
- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/TodayPage.test.tsx`
- `frontend/src/types.ts`
- `frontend/package.json`
- `frontend/tsconfig.json`
- `frontend/vite.config.ts`

## Implementation Scope

This slice added a frontend-only contract adapter layer for `TodayViewModel`.
The adapter accepts existing `LifeStateProjection` and daily-goal input and
returns `ViewModelEnvelope<TodayViewModel>`.

Implemented:

- shared ViewModel envelope, evidence, warning, product action, review action,
  debug action, provider/privacy, risk, and entity-ref types;
- limited TodayViewModel nested types;
- pure `buildTodayViewModelEnvelope(...)` adapter;
- Today fixtures for ready, empty, safe-mode, stale, and error cases;
- unit/contract tests for required Phase 3A-1 behavior;
- this documentation package.

## Non-goals Honored

- No Frontend V2 shell was implemented.
- No Workspace UI, Review Center UI, Tasks UI, Memory UI, LifeModel V2 UI, or
  V2 navigation shell was created.
- `ProductShell`, `ChatPage`, `MailboxPage`, `RunsPage`, `MemorySearch`, and
  `SettingsPage` were not modified.
- `TodayPage` was not replaced or refactored.
- Backend Rust code was not modified.
- No Tauri command, endpoint, backend ViewModel owner, store, or fake projection
  was added.
- Product code did not import `frontend/src/tauriDev.ts`.

## Production-code Modification Summary

New frontend contract files were added under `frontend/src/viewmodels/`.
Existing production page, shell, bridge-command, and backend files were not
modified.

The adapter is pure: it performs no Tauri invocation and has no side effects.
It only maps caller-provided projection and daily-goal values into a limited
contract envelope.

## Commands Run

See `docs/phase3a_today_slice/03_test_report.md` for command results.
