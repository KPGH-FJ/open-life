# Files Changed

Status: Phase 3A-1 file inventory.

## Frontend Contract Implementation

| File | Change | Reason |
| --- | --- | --- |
| `frontend/src/viewmodels/shared/viewModelEnvelope.ts` | Added shared `ViewModelEnvelope`, evidence, warning, product action, review action, debug action, provider/privacy, risk, and entity-ref types. | Provides the minimal shared Phase 2 envelope/action/evidence contract needed by the Today limited slice. |
| `frontend/src/viewmodels/today/todayViewModel.ts` | Added limited TodayViewModel and nested Today summary types. | Defines the Phase 3A-1 frontend contract shape without claiming backend ownership. |
| `frontend/src/viewmodels/today/todayViewModelAdapter.ts` | Added pure `buildTodayViewModelEnvelope(...)` adapter over caller-provided projection and daily-goal input. | Maps existing backend-owned projection fields into a limited envelope and preserves missing fields as unknown/limited. |
| `frontend/src/viewmodels/today/todayViewModel.fixtures.ts` | Added ready, empty, safe-mode, stale, and error fixtures. | Provides stable contract-test inputs for the limited slice. |
| `frontend/src/viewmodels/today/todayViewModel.test.ts` | Added 10 Vitest contract tests. | Verifies ready/empty/safe-mode/stale/error behavior, pending-count source, goal classification unknowns, action-lane separation, no invented review actions, and evidence preservation. |

## Documentation

| File | Change | Reason |
| --- | --- | --- |
| `docs/phase3a_today_slice/00_phase3a_methodology.md` | Added methodology report. | Records sources read, files inspected, scope, non-goals, and production-code boundaries. |
| `docs/phase3a_today_slice/01_today_viewmodel_mapping.md` | Added field mapping. | Maps each TodayViewModel field to source, owner status, adapter behavior, and limitation. |
| `docs/phase3a_today_slice/02_files_changed.md` | Added file inventory. | Lists every file added in this slice and why. |
| `docs/phase3a_today_slice/03_test_report.md` | Added test report. | Records validation commands and results. |
| `docs/phase3a_today_slice/04_self_review_and_hallucination_check.md` | Added self-review. | Answers the required non-hallucination and scope questions. |
| `docs/phase3a_today_slice/05_phase3a_summary.md` | Added summary. | Summarizes implementation, limitations, and next slice recommendation. |

## Files Deliberately Not Modified

- `frontend/src/tauri.ts`
- `frontend/src/utils/lifeStateProjection.ts`
- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/ProductShell.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/pages/MemorySearch.tsx`
- `frontend/src/pages/SettingsPage.tsx`
- `src-tauri/**`
- `openlife-core/**`
