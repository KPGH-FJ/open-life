# OpenLife Phase 3A-1 Goal
## TodayViewModel Limited Slice: Contract Adapter + Fixtures + Tests
### Codex Goal Mode Specification v1.0

## Goal

Implement the first narrow Phase 3 vertical slice:

```text
TodayViewModel limited slice
```

This is the first implementation step after Phase 2.

The objective is NOT to build Frontend V2.

The objective is to prove the contract-first development pattern:

```text
existing backend read model
        ↓
pure frontend adapter
        ↓
ViewModelEnvelope<TodayViewModel>
        ↓
fixtures + tests
```

## Why This Slice

Phase 2 classified:

- `TodayViewModel`: `READY_WITH_LIMITS`
- `WorkspaceViewModel`: `NOT_READY`
- `ReviewCenterViewModel`: `NOT_READY`
- `TasksViewModel`: `NOT_READY`
- `MemoryViewModel`: `NOT_READY`

Therefore, do not implement Workspace, Review Center, Tasks, or Memory in Phase 3A-1.

The first implementation should use existing `LifeStateProjection` plus the existing daily-goal read path, while explicitly preserving unknowns and partial fields.

---

# Role

You are acting as a Staff Frontend Engineer implementing a small contract-first vertical slice.

You must be conservative.

You must not make the product appear more complete than the contracts allow.

---

# Required Inputs

Read before implementation:

```text
docs/phase2_viewmodel_contract/14_phase2_summary_and_phase3_readiness.md
docs/phase2_viewmodel_contract/10_today_viewmodel_contract.md
docs/phase2_viewmodel_contract/03_shared_viewmodel_envelope_and_types.md
docs/phase2_viewmodel_contract/13_contract_test_plan.md
docs/phase2_viewmodel_contract/12_backend_contract_gap_register.md
docs/phase2_viewmodel_contract/04_lifestate_projection_extension_plan.md
docs/phase1_ux_ia/09_view_model_contract_proposal.md
docs/phase0_5/06_view_model_gap_inventory.md
```

Inspect source code before writing:

```text
frontend/src/tauri.ts
frontend/src/utils/lifeStateProjection.ts
frontend/src/pages/TodayPage.tsx
frontend/package.json
```

If paths differ, search for equivalent files.

---

# Hard Non-Goals

Do not:

- implement full Frontend V2
- create Workspace UI
- create Review Center UI
- create Tasks UI
- create Memory UI
- create LifeModel V2 UI
- modify ProductShell
- modify ChatPage
- modify MailboxPage
- modify RunsPage
- modify MemorySearch
- modify SettingsPage
- modify backend Rust code
- add new Tauri commands
- create fake backend endpoints
- create fake backend ViewModel owners
- replace TodayPage
- create a full V2 navigation shell
- restore old routes
- import from `frontend/src/tauriDev.ts` in product code

This phase is adapter + fixtures + tests only.

Optional UI preview is explicitly out of scope for Phase 3A-1.

---

# Allowed Implementation Scope

You may create or modify only files needed for the TodayViewModel limited slice.

Preferred new files:

```text
frontend/src/viewmodels/shared/
frontend/src/viewmodels/today/
```

Expected implementation candidates:

```text
frontend/src/viewmodels/shared/viewModelEnvelope.ts
frontend/src/viewmodels/today/todayViewModel.ts
frontend/src/viewmodels/today/todayViewModelAdapter.ts
frontend/src/viewmodels/today/todayViewModel.fixtures.ts
frontend/src/viewmodels/today/todayViewModel.test.ts
```

If the repository already has a better convention, follow it and document the reason.

Do not move existing page code in this phase.

---

# Contract Rules

## 1. Use ViewModelEnvelope

Use the Phase 2 envelope:

```ts
type ViewModelEnvelope<T> = {
  data: T | null
  status: 'loading' | 'ready' | 'empty' | 'error' | 'stale'
  lastUpdatedAt: string | null
  source: 'backend-readmodel'
  evidenceRefs?: EvidenceRef[]
  warnings?: ViewModelWarning[]
  actions: {
    primary: ProductAction[]
    review?: ReviewAction[]
    debugOnly?: DebugAction[]
  }
}
```

If an equivalent shared type already exists, reuse it.

If not, create the minimal shared type needed for this slice.

## 2. Do Not Claim Backend Ownership

The TodayViewModel adapter is a frontend adapter over existing backend-owned values.

It is not a backend owner.

Do not name it as a backend read model owner.

If a field is not backend-owned today, represent it as:

```text
unknown
limited
partial
PHASE_2_REQUIRED
```

through warnings, empty values, or explicit limited-state copy.

## 3. Source of Truth

For the limited slice, allowed verified sources are:

- `LifeStateProjection.safeMode`
- `LifeStateProjection.pending`
- `LifeStateProjection.taskState`
- `LifeStateProjection.readiness` if present
- `LifeStateProjection.sourceRefs` if present
- existing daily goal read path / daily goals data

The UI/adapter must not infer:

- global pending review count from proposal lists
- readiness from diagnostics
- safe mode from diagnostics
- task pressure from raw task fragments
- next recommended action as product truth if backend does not provide it
- blocker truth from local card classification

## 4. Daily Goals

Daily goals may be displayed, but local classification must not be promoted to product truth.

If the backend does not provide a goal classification, mark:

```text
backendClassification: 'unknown'
```

or equivalent.

Add a warning such as:

```text
today.goal_classification_limited
```

## 5. Actions

Separate action lanes:

- `actions.primary`: ordinary product actions such as refresh, open current workspace route, open current review route.
- `actions.review`: none directly for Today limited slice unless backend review owner explicitly provides it.
- `actions.debugOnly`: inspect projection/source refs if applicable.

Do not put debug actions in `actions.primary`.

Do not create review actions from local inference.

---

# Required TodayViewModel Shape

Define a limited TodayViewModel that is compatible with the Phase 2 contract.

It should include at minimum:

```ts
type TodayViewModel = {
  dailyStateSummary: TodayDailyStateSummary
  safeMode: TodaySafeModeSummary
  pendingReviewCount: number
  currentTaskPressure: TodayTaskPressureSummary
  blockers: TodayBlockerSummary[]
  suggestions: TodaySuggestion[]
  primaryDailyGoal: TodayDailyGoalSummary | null
  nextRecommendedAction: ProductAction | null
  workspaceLink: ProductAction
  reviewCenterLink: ProductAction
  sourceRefs: EvidenceRef[]
}
```

If the exact shape must differ due to existing code constraints, document why in the implementation report.

Use conservative unknown/limited states instead of inventing missing fields.

---

# Required Tests

Add tests for the adapter.

At minimum cover:

1. ready state with projection and daily goals
2. empty state with projection but no daily goal/current task
3. safe mode state
4. pending review count from projection
5. stale state disables risky actions
6. error state does not fall back to raw domain reads
7. daily goal classification remains unknown/limited when backend does not provide classification
8. debugOnly actions do not appear in primary actions
9. no review actions are invented locally
10. source/evidence refs are preserved where available

Tests should be unit/contract tests, not browser/E2E.

---

# Required Documentation Output

Create:

```text
docs/phase3a_today_slice/
```

Generate:

```text
00_phase3a_methodology.md
01_today_viewmodel_mapping.md
02_files_changed.md
03_test_report.md
04_self_review_and_hallucination_check.md
05_phase3a_summary.md
```

## 00_phase3a_methodology.md

Include:

- documents read
- source files inspected
- commands run
- implementation scope
- non-goals honored
- production-code modification summary

## 01_today_viewmodel_mapping.md

Map each TodayViewModel field:

| Field | Source | Owner status | Adapter behavior | Limitations |
|---|---|---|---|---|

## 02_files_changed.md

List every changed file and why.

## 03_test_report.md

Include commands run and results.

Required commands where feasible:

```sh
git diff --check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend test
```

If a command cannot run, explain why and do not claim success.

## 04_self_review_and_hallucination_check.md

Must answer:

1. Did this change create any fake backend endpoint, projection, store, or ViewModel owner?
2. Did this change modify backend Rust code?
3. Did this change modify ProductShell, ChatPage, MailboxPage, RunsPage, MemorySearch, or SettingsPage?
4. Did this change import from `tauriDev.ts` in product code?
5. Did this change infer pending/safeMode/task truth from raw domain reads?
6. Did this change promote local daily-goal classification into product truth?
7. Did this change mix debug actions into primary actions?
8. Did this change invent ReviewActions?
9. Are empty/error/stale states tested?
10. Are remaining unknowns documented?

## 05_phase3a_summary.md

Include:

- what was implemented
- what was not implemented
- test results
- limitations
- remaining Phase 3 blockers
- recommendation for next slice

---

# Required Commands

Before coding:

```sh
git status --short
```

After implementation:

```sh
git diff --check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend test
```

If the repository has a narrower frontend test command for a single file, you may run it first, but still run the full frontend test command if feasible.

Do not run broad E2E or Tauri desktop trial unless explicitly configured and feasible. Browser smoke/desktop trial remain separate readiness gates.

---

# Acceptance Criteria

Phase 3A-1 is complete only if:

1. TodayViewModel limited slice types exist.
2. A pure TodayViewModel adapter exists.
3. Fixtures exist.
4. Adapter tests exist and pass.
5. The adapter consumes existing `LifeStateProjection` / daily-goal inputs only.
6. No fake backend owner/endpoints/projections/stores/workflows are created.
7. No Workspace / Review Center / Tasks / Memory UI is implemented.
8. No ProductShell / ChatPage / MailboxPage / RunsPage refactor is done.
9. No backend Rust code is modified.
10. No product code imports `tauriDev.ts`.
11. `actions.primary`, `actions.review`, and `actions.debugOnly` are separated.
12. Empty/error/stale states are represented and tested.
13. Full frontend typecheck and tests are run, or failures are documented.
14. Phase 3A documentation is generated under `docs/phase3a_today_slice/`.

---

# Stop Rule

Stop after completing TodayViewModel adapter + fixtures + tests + documentation.

Do not proceed to a Today V2 preview page.

Do not implement UI shell.

Do not implement Workspace.

Do not implement Review Center.

Wait for human review.

---

# Final Response Format

When complete, respond:

```markdown
Phase 3A-1 TodayViewModel limited slice complete.

Implemented:
- <files>

Docs created:
- docs/phase3a_today_slice/00_phase3a_methodology.md
- docs/phase3a_today_slice/01_today_viewmodel_mapping.md
- docs/phase3a_today_slice/02_files_changed.md
- docs/phase3a_today_slice/03_test_report.md
- docs/phase3a_today_slice/04_self_review_and_hallucination_check.md
- docs/phase3a_today_slice/05_phase3a_summary.md

Tests:
- <command>: <result>
- <command>: <result>

Self-check:
- No fake backend contracts:
- No backend Rust changes:
- No ProductShell/Chat/Mailbox/Runs/Memory/Settings changes:
- No tauriDev import:
- No raw product-truth reconstruction:
- No Frontend V2 implementation started:

Known limitations:
1.
2.
3.

Recommended next step:
<Phase 3A-2 or stop>
```
