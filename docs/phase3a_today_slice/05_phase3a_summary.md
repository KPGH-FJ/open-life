# Phase 3A-1 Summary

Status: TodayViewModel limited slice complete.

## Implemented

- Shared `ViewModelEnvelope` and action/evidence types under
  `frontend/src/viewmodels/shared/`.
- Limited TodayViewModel types under `frontend/src/viewmodels/today/`.
- Pure `buildTodayViewModelEnvelope(...)` adapter over existing
  `LifeStateProjection` and daily-goal input.
- Fixtures for ready, empty, Safe Mode, stale, and error states.
- 10 focused Vitest contract tests for the adapter.
- Required documentation under `docs/phase3a_today_slice/`.

## Not Implemented

- Full Frontend V2.
- Today V2 preview page.
- Workspace UI.
- Review Center UI.
- Tasks UI.
- Memory UI.
- LifeModel V2 UI.
- V2 navigation shell.
- Backend Rust changes.
- New Tauri commands or bridge wrappers.
- Fake backend ViewModel owner, endpoint, projection, or store.

## Test Results

See `docs/phase3a_today_slice/03_test_report.md`.

Focused adapter validation passed:

- `corepack pnpm --dir frontend test -- todayViewModel`: 10 tests passed.
- `corepack pnpm --dir frontend typecheck`: passed.
- `corepack pnpm --dir frontend format:check`: passed after formatting the two new Today files.

Final validation passed:

- `git diff --check`: passed.
- `corepack pnpm --dir frontend typecheck`: passed.
- `corepack pnpm --dir frontend format:check`: passed.
- `corepack pnpm --dir frontend test`: passed, 40 test files and 440 tests.

## Limitations

1. `TodayViewModel` is still a frontend limited-slice adapter, not a backend-owned read model.
2. Daily-goal classification and priority remain `unknown`.
3. Suggestions, rich blockers, and next recommended action remain `PHASE_2_REQUIRED`.
4. Provider/privacy boundary summary remains unknown because no backend owner is available in this slice.
5. The adapter does not update `TodayPage`; existing page behavior is intentionally left untouched.

## Remaining Phase 3 Blockers

- Backend-owned Today-specific daily goal classification.
- Backend-owned Today suggestions/blockers/next action.
- Shared provider/privacy boundary summary owner.
- Full static guard strategy for pages that still reconstruct product truth from raw domain reads.
- Browser smoke and desktop/Tauri product trial remain separate readiness gates.

## Recommended Next Slice

Stop here for human review.

If approved, the next slice should remain contract-first: either add a backend
owner for the missing Today-specific fields or add static/frontend guards that
prevent existing pages from reconstructing projection-covered product truth.
