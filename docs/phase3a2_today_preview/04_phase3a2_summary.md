# Phase 3A-2 Summary

Status: Today V2 preview surface complete within the requested slice.

## Implemented

- Added `frontend/src/pages/TodayV2PreviewPage.tsx`.
- Added a pure `TodayV2PreviewSurface` that accepts `TodayViewModelEnvelope`.
- Added a container that reads `getLifeStateProjection()` and `getDailyGoals()`, then immediately builds the envelope with `buildTodayViewModelEnvelope(...)`.
- Added unlisted `/today-v2-preview` route in `App.tsx`; manual browser access
  uses `/#/today-v2-preview` because the app is mounted under `HashRouter`.
- Added focused tests for ready, empty, error, stale, Safe Mode, pending review count, debug/evidence lane behavior, route preservation, container input calls, and forbidden source imports.
- Added Phase 3A-2 implementation docs under `docs/phase3a2_today_preview/`.

## Not Implemented

- Full Frontend V2.
- `/today` replacement.
- ProductShell primary navigation changes.
- Workspace, Review Center, Tasks, Memory, LifeModel V2, or Settings V2.
- ChatPage, MailboxPage, RunsPage, MemorySearch, or SettingsPage refactors.
- Backend Rust changes.
- New Tauri commands.
- New backend endpoint, store, projection, or ViewModel owner.
- `tauriDev.ts` product import.

## Validation Summary

Focused and broad frontend validation passed:

- `git diff --check`
- `corepack pnpm --dir frontend typecheck`
- `corepack pnpm --dir frontend format:check`
- `corepack pnpm --dir frontend test -- TodayV2Preview`
- `corepack pnpm --dir frontend test -- todayViewModel`
- `corepack pnpm --dir frontend test -- App`
- `corepack pnpm --dir frontend test`

## Limitations

1. `TodayViewModel` is still a frontend limited-slice adapter, not a backend-owned Today read model.
2. Daily-goal classification, priority, suggestions, rich blockers, and next recommended action remain limited or unknown.
3. Provider/privacy boundary remains `PHASE_2_REQUIRED`.
4. The preview route is unlisted and does not represent approved V2 IA.
   Manual browser access uses the `HashRouter` form `/#/today-v2-preview`.
5. Browser E2E, desktop/Tauri trial, external live-provider evidence, Web AgentLoop evidence, and MCP AgentLoop evidence were not run and remain separate.

## Recommended Next Step

Stop for human review. The next slice should either add a backend-owned Today
read model for the missing fields or choose another approved limited slice such
as LifeModel or Settings. Do not start those from this Phase 3A-2 slice.
