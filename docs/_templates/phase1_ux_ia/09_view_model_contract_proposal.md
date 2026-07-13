# ViewModel Contract Proposal

## Hard Rules

1. Pages cannot reconstruct product truth from raw domain reads.
2. Pages can only render backend-owned ViewModels / ReadModels, or raw data explicitly marked as debug-only.
3. Do not invent backend ViewModels, endpoints, projections, stores, or workflows.
4. Future required backend fields must be marked `PHASE_2_REQUIRED`.

## ViewModel Envelope

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

## Action Types

### ProductAction

Default user-facing actions needed to complete the task.

### ReviewAction

Approval/rejection/edit/later/evidence actions for consequential changes.

### DebugAction

Advanced/developer-only actions such as raw trace, export JSON, provider health.

## Required Per-ViewModel Fields

For each ViewModel define:

- Backend owner
- Owner status: EXISTING / PARTIAL / PROPOSED / UNKNOWN / PHASE_2_REQUIRED
- UI cannot infer
- Empty state
- Error state
- Stale state
- Evidence model
- Product actions
- Review actions
- Debug-only actions
- Auditability
- Required fields
- Existing backend support
- Missing backend projection fields
- Frontend-only formatter candidates
- Risks
- Human decisions needed
- Phase 2 implication

---

## TodayViewModel

Purpose:

Backend owner:

Owner status:

UI cannot infer:

Empty state:

Error state:

Stale state:

Evidence model:

Product actions:

Review actions:

Debug-only actions:

Auditability:

Required fields:

Existing backend support:

Missing backend projection fields:

Risks:

Phase 2 implication:

---

## WorkspaceViewModel

Purpose:

Backend owner:

Owner status:

UI cannot infer:

Empty state:

Error state:

Stale state:

Evidence model:

Product actions:

Review actions:

Debug-only actions:

Auditability:

Required fields:

Existing backend support:

Missing backend projection fields:

Risks:

Phase 2 implication:

---

## TasksViewModel

## ReviewCenterViewModel

## LifeModelViewModel

## MemoryViewModel

## SettingsViewModel

## Expand LifeStateProjection vs Dedicated Read Models

## Fields That Must Not Be Page-local

## Backend Contract Non-Hallucination Check

List every proposed backend owner/read model and classify:

| Proposed backend owner/read model | Status | Evidence | Phase 2 required validation |
|---|---|---|---|

## Phase 2 Engineering Questions
