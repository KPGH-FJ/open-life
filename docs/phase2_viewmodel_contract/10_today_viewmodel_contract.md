# TodayViewModel Contract

Status: partial contract. No Today UI implementation.

## Purpose

`DESIGN_DECISION`: `TodayViewModel` is the daily landing read model for daily state summary, safe mode, pending review count, current task pressure, blockers, suggestions, primary daily goal, next recommended action, and links to Workspace / Review Center.

Backend owner: `LifeStateProjection` plus proposed Today-specific read model
Owner status: `PARTIAL`
Required validation: Phase 3 must decide whether daily goal classification, suggestions, blockers, and next daily action belong in expanded `LifeStateProjection` or a dedicated Today read model.

## Existing Support

`EXISTING_CODE`: `TodayPage` reads `getLifeStateProjection` and `getDailyGoals`.

`EXISTING_CODE`: `LifeStateProjection` owns safe mode, pending review counts, readiness, and task pressure counts.

`VERIFIED_FACT`: Phase 0.5 identified Today as a medium-risk gap because daily goal cards are classified locally.

## Required Field Contract

| Field | Type | Required | Source of truth | Owner status | Evidence | Frontend may infer? | Empty behavior | Error behavior | Stale behavior | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `dailyStateSummary` | `TodayDailyStateSummary` | Yes | Today read model / projection | `PHASE_2_REQUIRED` | Phase 1 IA | No | Neutral "no current summary" | Error envelope | Mark stale | Source refs |
| `safeMode` | `TodaySafeModeSummary` | Yes | `LifeStateProjection.safeMode` | `EXISTING` | Projection source inspected | No | inactive if backend says | Error warning | Mark stale; disable risky actions | Safe-mode refs |
| `pendingReviewCount` | number | Yes | `LifeStateProjection.pending` | `EXISTING` | Projection source inspected | No | 0 if backend says | Error warning | Mark stale | Proposal store refs |
| `currentTaskPressure` | `TodayTaskPressureSummary` | Yes | `LifeStateProjection.taskState` | `EXISTING/PARTIAL` | Projection task counts | No | idle counts | Error warning | Mark stale | Task refs |
| `blockers` | `TodayBlockerSummary[]` | Yes | Today/projection/task owner | `PHASE_2_REQUIRED` | Blocker state exists elsewhere | No | Empty if backend says | Error warning | Mark stale | Blocker refs |
| `suggestions` | `TodaySuggestion[]` | Yes | Today read model | `PHASE_2_REQUIRED` | Current local card classification | No | Empty suggestions | Error warning | Mark stale | Suggestion refs |
| `primaryDailyGoal` | `TodayDailyGoalSummary \| null` | Conditional | Daily-goal read path | `PARTIAL` | `getDailyGoals` exists | No for classification | `null` | Error warning | Mark stale | Goal refs |
| `nextRecommendedAction` | `ProductAction \| null` | Yes | Today read model | `PHASE_2_REQUIRED` | Phase 1 requirement | No | Start in Workspace | Retry refresh | Disable risky action | Action refs |
| `workspaceLink` | `ProductAction` | Yes | Product route/read model | `PROPOSED` | IA decision | No | Enabled if app ready enough | Error warning | Mark stale | Target refs |
| `reviewCenterLink` | `ProductAction` | Yes | Projection/review owner | `PARTIAL` | Pending counts exist | No | Enabled with 0 count | Error warning | Mark stale | Review refs |
| `sourceRefs` | `EvidenceRef[]` | Yes | Projection/daily read model | `PHASE_2_REQUIRED` | Evidence rule | No | Empty with warning | Error warning | Stale refs | Source refs |

## Today Nested Contract Types

`PHASE_2_REQUIRED`: These target types preserve the limited Today slice without letting the page classify raw daily goals or projection fragments on its own.

```ts
type TodayDailyStateSummary = {
  headline: string
  summary: string
  readiness: 'ready' | 'limited' | 'blocked' | 'safe_mode' | 'empty' | 'unknown'
  providerPrivacyBoundary: ProviderPrivacyBoundarySummary
  evidenceRefs: EvidenceRef[]
}

type TodaySafeModeSummary = {
  active: boolean
  reason: string | null
  blocksExternalActions: boolean
  blocksDurableWrites: boolean
  evidenceRefs: EvidenceRef[]
}

type TodayTaskPressureSummary = {
  activeCount: number
  waitingPermissionCount: number
  blockedCount: number
  staleCount: number
  highestRisk: RiskLevel | 'none' | 'unknown'
  evidenceRefs: EvidenceRef[]
}

type TodayBlockerSummary = {
  id: string
  category:
    | 'safe_mode'
    | 'waiting_review'
    | 'waiting_permission'
    | 'blocked_task'
    | 'provider_privacy'
    | 'missing_context'
    | 'unknown'
  title: string
  nextAction: ProductAction | ReviewAction | null
  evidenceRefs: EvidenceRef[]
}

type TodaySuggestion = {
  id: string
  title: string
  reason: string
  targetSurface: 'workspace' | 'review_center' | 'tasks' | 'lifemodel' | 'memory' | 'settings'
  action: ProductAction
  evidenceRefs: EvidenceRef[]
}

type TodayDailyGoalSummary = {
  goalRef: BackendEntityRef
  title: string
  status: 'not_started' | 'in_progress' | 'blocked' | 'done' | 'stale' | 'unknown'
  priority: 'low' | 'medium' | 'high' | 'unknown'
  backendClassification: string
  evidenceRefs: EvidenceRef[]
}
```

## Fields From `LifeStateProjection`

`EXISTING_CODE`: Today should consume these from `LifeStateProjection`:

- safe mode active/reason/source refs;
- pending review count;
- high-risk review count;
- task pressure counts;
- readiness status where needed;
- source refs.

## Fields Needing Today-specific Read Model

`PHASE_2_REQUIRED`:

- primary daily goal classification;
- suggestions versus blockers;
- next recommended daily action;
- provenance for goal cards;
- current task pressure summary in user-facing wording.

## UI Cannot Infer

`PHASE_2_REQUIRED`: Today cannot reconstruct global pending/readiness/task truth locally. Daily-goal display can format backend fields, but cannot promote local card classification into product truth.

## Empty / Error / Stale Behavior

`DESIGN_DECISION`: Empty means no daily goal/current task; show start-work action and Review Center count.

`DESIGN_DECISION`: Error means no projection fallback from diagnostics/proposals.

`DESIGN_DECISION`: Stale means show last updated and disable risky actions.

## Tests Needed

- Projection-backed pending/safe-mode/task count tests.
- Today next-action fixture tests.
- Empty/error/stale UI contract tests.
- Static frontend guard preventing Today from rebuilding pending counts from proposal list.

## Readiness

`READY_WITH_LIMITS`: A limited Today implementation can use existing projection and daily-goal read path if local card classification is not treated as product truth.
