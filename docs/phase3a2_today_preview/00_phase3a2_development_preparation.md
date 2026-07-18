# Phase 3A-2 Today V2 Preview Development Preparation

Status: preparation artifact, not implementation completion.
Scope: prepare the next Agent to implement `Phase 3A-2: Today V2 Preview Surface`
without widening into full Frontend V2.

## 1. Authority Stack

Read in this order before implementation:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`
5. `OpenLife_Codex_Handoff_Context_v1.0.md`
6. `docs/phase2_viewmodel_contract/10_today_viewmodel_contract.md`
7. `docs/phase2_viewmodel_contract/14_phase2_summary_and_phase3_readiness.md`
8. `docs/phase3a_today_slice/03_test_report.md`
9. `docs/phase3a_today_slice/04_self_review_and_hallucination_check.md`
10. `docs/phase3a_today_slice/05_phase3a_summary.md`
11. This file.

If these documents conflict, the Phase7 authority stack wins. The handoff file
is task context, not a replacement for Phase7.

## 2. Verified Current Baseline

Current repo state verified during preparation:

- `Phase 3A-1: TodayViewModel limited slice` exists under
  `frontend/src/viewmodels/shared/`, `frontend/src/viewmodels/today/`, and
  `docs/phase3a_today_slice/`.
- `buildTodayViewModelEnvelope(...)` is a pure frontend adapter over
  caller-provided `LifeStateProjection` and daily-goal input.
- `TodayViewModel` is still `READY_WITH_LIMITS`, not a backend-owned read model.
- The old `/today` route still renders `TodayPage`.
- `TodayPage` currently reads `getLifeStateProjection` and `getDailyGoals`, then
  locally classifies daily-goal cards through `dailyGoalDisplayGuard`.
- Product routes are defined in `frontend/src/productShellContract.ts`.
- `ProductShell` primary navigation must not change in this slice.
- `frontend/src/tauri.ts` is the product bridge. `frontend/src/tauriDev.ts` is
  dev/test compatibility only and must not be imported by product pages.
- Phase7 trial status remains `red-until-trial-green`; this slice must not claim
  product readiness.

Worktree note: the Phase 0 / 0.5 / 1 / 2 / 3A handoff package and ViewModel
files are currently untracked in git. Do not delete, rewrite, or normalize them
as part of Phase 3A-2 unless the user explicitly asks.

## 3. Objective

Implement a minimal Today V2 preview surface that proves:

```text
TodayViewModel -> product-like UI
```

The preview must render from `TodayViewModelEnvelope` and preserve the
projection/read-model boundary. It is not a route replacement and not a broader
IA migration.

## 4. Allowed Scope

Allowed implementation work:

- Create a small Today V2 preview page or preview surface.
- Use `buildTodayViewModelEnvelope(...)` as the only mapping layer from current
  projection/daily-goal inputs to UI data.
- Render ready, empty, error, stale, safe-mode, blocked/task-pressure, and
  pending-review states from `TodayViewModelEnvelope`.
- Render `actions.primary` as normal user actions.
- Keep `actions.debugOnly` behind an explicit advanced/details lane.
- Render warnings for limited fields such as provider/privacy boundary,
  suggestions, next action, and daily-goal classification.
- Add focused UI/contract tests.
- Generate the remaining Phase 3A-2 docs under `docs/phase3a2_today_preview/`.

Recommended route strategy:

```text
Create TodayV2PreviewPage and wire it to a non-default, unlisted route such as
/today-v2-preview.
```

Runtime note: `frontend/src/main.tsx` uses `HashRouter`, so manual browser
access is `/#/today-v2-preview` even though the React route path remains
`/today-v2-preview`.

Do not add it to `PRIMARY_PRODUCT_ROUTES`. Do not add it to `ProductShell`
primary tabs. Do not redirect `/today` to it. If the implementation Agent wants
zero route changes, use a component-level test harness instead and document that
the preview is not manually reachable.

## 5. Hard Non-goals

Do not:

- replace `/today`;
- modify ProductShell primary navigation;
- implement full Frontend V2;
- implement Workspace, Review Center, Tasks, Memory, LifeModel V2, or Settings
  V2;
- refactor `ChatPage`, `MailboxPage`, `RunsPage`, `MemorySearch`, or
  `SettingsPage`;
- modify backend Rust;
- add Tauri commands;
- invent backend endpoints, stores, projections, or ViewModel owners;
- import `tauriDev.ts` in product code;
- restore retired Phase7 route, command, wrapper, module, or old acceptance
  surfaces;
- use assistant prose, local card heuristics, or raw proposal/diagnostic reads
  as product truth;
- claim Phase7, Main Chat Agent Execution v1, live provider, desktop/Tauri
  trial, Web AgentLoop, or MCP AgentLoop readiness.

## 6. Source Map For The Implementation Agent

### Existing Product Path

```text
frontend/src/App.tsx
  -> productRoutePath("Today")
  -> frontend/src/pages/TodayPage.tsx
      -> getLifeStateProjection()
      -> getDailyGoals()
      -> dailyGoalDisplayGuard local classification
      -> reviewRequiredCountFromProjection(...)
```

This path must remain intact. Phase 3A-2 may compare against it, but must not
replace it.

### Phase 3A-1 ViewModel Path

```text
frontend/src/viewmodels/shared/viewModelEnvelope.ts
frontend/src/viewmodels/today/todayViewModel.ts
frontend/src/viewmodels/today/todayViewModelAdapter.ts
frontend/src/viewmodels/today/todayViewModel.fixtures.ts
frontend/src/viewmodels/today/todayViewModel.test.ts
```

The preview surface should render the envelope produced by
`buildTodayViewModelEnvelope(...)`.

### Route And Navigation Contracts

```text
frontend/src/productShellContract.ts
frontend/src/App.tsx
frontend/src/components/ProductShell.tsx
```

Use these only for a non-default preview route if needed. Do not change primary
navigation labels or route aliases.

### Test Patterns To Reuse

```text
frontend/src/pages/TodayPage.test.tsx
frontend/src/viewmodels/today/todayViewModel.test.ts
frontend/src/test/mocks/tauri.ts
```

Use page tests for Tauri command mocking and route rendering patterns. Use
ViewModel tests for canonical ready/empty/error/stale/safe-mode fixtures.

## 7. Recommended Component Structure

Preferred small structure:

```text
frontend/src/pages/TodayV2PreviewPage.tsx
frontend/src/pages/TodayV2PreviewPage.test.tsx
```

Inside that page, keep two responsibilities separate:

1. `TodayV2PreviewPage` loads only the minimal current inputs:
   `getLifeStateProjection()` and `getDailyGoals()`.
2. A pure rendering component, for example `TodayV2PreviewSurface`, accepts a
   `TodayViewModelEnvelope` and renders the UI.

The pure rendering component should not import:

- `getLifeStateProjection`
- `getDailyGoals`
- `dailyGoalDisplayGuard`
- `reviewRequiredCountFromProjection`
- proposal list/read commands
- diagnostics fallback commands
- `tauriDev.ts`

This separation makes it possible to prove that the UI consumes TodayViewModel,
while the container remains a temporary bridge until a backend-owned Today read
model exists.

## 8. Data And State Rules

The preview must render from these envelope fields:

- `status`
- `lastUpdatedAt`
- `warnings`
- `evidenceRefs`
- `actions.primary`
- `actions.review`
- `actions.debugOnly`
- `data.dailyStateSummary`
- `data.safeMode`
- `data.pendingReviewCount`
- `data.currentTaskPressure`
- `data.blockers`
- `data.primaryDailyGoal`
- `data.nextRecommendedAction`
- `data.workspaceLink`
- `data.reviewCenterLink`

The preview must not:

- recompute pending review count from proposals or diagnostics;
- use `LifeStateProjection.surfaces[*].pendingReviewCount` as a local override;
- classify daily goals into suggestions/blockers outside the ViewModel adapter;
- fabricate `nextRecommendedAction` when the adapter returns `null`;
- display `已记住`, `已更新 LifeModel`, `已应用`, or `已写入长期状态` unless the
  ViewModel/backend evidence explicitly proves durable materialization;
- promote `debugOnly` actions into the primary action row.

## 9. UX Direction

Use the restrained existing product style:

- calm, dense, high-trust layout;
- no hero page;
- no marketing copy;
- no dashboard stat overload;
- no raw trace or raw JSON in the default view;
- Chinese-first labels in normal UI;
- `LifeModel` remains English-branded if mentioned;
- Safe Mode wording may remain `Safe Mode` or `Safe Mode（安全模式）` until the
  human terminology decision is final.

Recommended preview layout:

```text
TodayV2Preview
├── Header: 今日 + status/last-updated chips
├── Summary band: dailyStateSummary headline/summary/readiness
├── Safe Mode / blocker strip when present
├── Primary daily goal panel
├── Task pressure and pending review row
├── Blockers list
├── Warnings / limited-field notes
├── Primary actions row
└── Collapsed advanced evidence/actions area
```

Keep cards as individual panels only; do not create nested cards. Use compact
headers and stable spacing. Use existing Tailwind conventions from `TodayPage`
and `ProductShell`.

## 10. Route Strategy Decision

Preparation recommendation: use an unlisted preview route.

Why:

- It allows manual preview without replacing `/today`.
- It avoids changing primary navigation.
- It avoids adding another advanced menu item before human approval.
- It keeps the implementation narrow and testable.

Suggested implementation shape:

```text
const TodayV2PreviewPage = React.lazy(() => import("./pages/TodayV2PreviewPage"));
<Route path="/today-v2-preview" element={<TodayV2PreviewPage />} />
```

Because the app is mounted under `HashRouter`, the manual browser URL for this
route is `/#/today-v2-preview`, not a direct history path.

Do not add `/today-v2-preview` to:

- `PRIMARY_PRODUCT_ROUTES`
- `LEGACY_PRODUCT_REDIRECTS`
- `ADVANCED_PRODUCT_ROUTES`
- `ADVANCED_PRODUCT_ROUTE_GROUPS`
- `PRODUCT_ROUTE_ALIASES`

If future human review wants a visible menu entry, handle that in a separate IA
slice.

## 11. Required Tests

At minimum, add focused tests that cover:

1. Ready state renders the daily summary, primary goal, pending review count,
   and primary actions from `TodayViewModelEnvelope`.
2. Empty state renders no fake goal and no fake next action.
3. Error state renders `data: null` behavior and does not fall back to daily
   goals.
4. Stale state shows stale wording and disables stale-sensitive primary actions.
5. Safe Mode shows safe-mode state and no direct durable-write actions.
6. Pending review count comes from `envelope.data.pendingReviewCount`, not from
   diagnostics, proposal lists, or surface-row overrides.
7. `actions.debugOnly` is not shown with primary actions.
8. Debug/evidence details are available only in a collapsed/advanced lane.
9. The old `/today` route still renders `TodayPage` if a route is added.
10. Static source scan confirms the preview surface does not import forbidden
    raw-domain helpers or write/proposal-apply wrappers.

Recommended test files:

```text
frontend/src/pages/TodayV2PreviewPage.test.tsx
```

Reuse `todayViewModel.fixtures.ts` where possible. If a container test mocks
Tauri commands, assert only `get_life_state_projection` and `get_daily_goals`
are called.

## 12. Suggested Validation Gates

Run:

```sh
git diff --check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend test -- TodayV2Preview
corepack pnpm --dir frontend test -- todayViewModel
```

If implementation touches `App.tsx` route wiring, also run:

```sh
corepack pnpm --dir frontend test -- App
corepack pnpm --dir frontend test
```

Do not run broad Rust gates unless the implementation unexpectedly touches Rust;
it should not.

## 13. Phase 3A-2 Documentation To Produce After Implementation

The implementation Agent should add the remaining docs under this directory:

```text
docs/phase3a2_today_preview/01_files_changed.md
docs/phase3a2_today_preview/02_test_report.md
docs/phase3a2_today_preview/03_self_review_and_hallucination_check.md
docs/phase3a2_today_preview/04_phase3a2_summary.md
```

Each doc should state that Phase 3A-2 is a preview slice only and does not
replace `/today` or prove full Frontend V2 readiness.

## 14. Self-review Checklist For The Implementation Agent

Before final response, answer:

1. Did I preserve `/today` and existing `TodayPage`?
2. Did I leave ProductShell primary navigation unchanged?
3. Did I avoid backend Rust and new Tauri commands?
4. Did the preview UI render from `TodayViewModelEnvelope`?
5. Did I avoid reconstructing pending/safeMode/task truth from raw reads?
6. Did I avoid local daily-goal suggestion/blocker classification in the
   preview?
7. Did I keep `debugOnly` actions out of primary actions?
8. Did I avoid importing `tauriDev.ts`?
9. Did I avoid write/proposal-apply actions?
10. Did I document all remaining unknowns as `UNKNOWN` or `PHASE_2_REQUIRED`
    rather than pretending they are solved?
11. Did tests prove ready, empty, error, stale, safe-mode, and pending-review
    behavior?
12. Did I avoid claiming Phase7, live-provider, desktop/Tauri trial, or full
    Frontend V2 completion?

## 15. Stop Condition

Stop after:

- the preview surface exists;
- focused tests and required frontend gates pass or failures are documented;
- Phase 3A-2 docs are generated;
- the final response lists files changed, tests run, implemented scope,
  non-implemented scope, limitations, and next recommended slice.

The next slice after Phase 3A-2 should be chosen by human review. Current
source-backed candidates remain LifeModel limited slice or Settings limited
slice. Do not start either during Phase 3A-2.
