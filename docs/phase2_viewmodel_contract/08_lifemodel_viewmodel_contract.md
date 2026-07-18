# LifeModelViewModel Contract

Status: proposed contract. No LifeModel UI implementation.

## Purpose

`DESIGN_DECISION`: `LifeModelViewModel` is the backend-owned read model for the user's structured long-term understanding: canonical/current distinction, dimension summaries, trust/quality state, pending updates, provenance, candidate changes, materialized changes, manual override state, Memory linkage, ReviewItem refs, and debug-only raw internals.

Backend owner: Proposed `LifeModelViewModel`
Owner status: `PHASE_2_REQUIRED` for full contract, `PARTIAL` for limited existing primitives.
Required validation: Phase 3 must define whether the default user view is canonical LifeModel truth, current compatibility view, or a labeled hybrid.

## Existing Support

`EXISTING_CODE`: Existing support includes `getLifeModel`, `getLifeModelCurrentView`, `getModel4DCompletion`, `getSystemDiagnostics`, `getLifeStateProjection`, pending proposal lists, memory tier stats, LifeModel patch/provenance primitives, and `lifeModelTrust.ts` display helper.

`VERIFIED_FACT`: Phase 0 audit says LifeModel is a real structured domain model and has compatibility/provenance views.

## Canonical vs Current / Compatibility Rule

`DESIGN_DECISION`: The UI must not claim canonical truth when only current/compatibility view is available.

`PHASE_2_REQUIRED`: Backend must expose a `truthMode` or equivalent field that labels:

- canonical durable LifeModel truth;
- current materialized/compatibility view;
- candidate change;
- pending review;
- manual override;
- unknown/unavailable.

## Required Field Contract

| Field | Type | Required | Source of truth | Owner status | Evidence | Frontend may infer? | Empty behavior | Error behavior | Stale behavior | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `truthMode` | `LifeModelTruthMode` | Yes | LifeModel read model | `PHASE_2_REQUIRED` | Compatibility/provenance exists; user-facing owner missing | No | `unknown` if backend says | Error envelope | Mark stale | Truth/provenance refs |
| `canonicalSummary` | `LifeModelCanonicalSummary \| null` | Conditional | LifeModel owner | `PHASE_2_REQUIRED` | LifeModel exists | No | `null` when not built | Error warning | Mark stale | Canonical refs |
| `currentViewSummary` | `LifeModelCurrentViewSummary \| null` | Conditional | LifeModel current view | `PARTIAL` | `getLifeModelCurrentView` exists | No | `null` | Error warning | Mark stale | Current view refs |
| `dimensionSummaries` | `LifeModelDimensionSummary[]` | Yes | LifeModel read model | `PHASE_2_REQUIRED` | Display helper currently computes | No | Empty dimensions | Error envelope | Mark stale | Dimension refs |
| `trustQualityState` | `LifeModelTrustQualityState` | Yes | LifeModel read model | `PHASE_2_REQUIRED` | Diagnostics/completion/proposals partial | No | Unknown quality | Error warning | Mark stale | Quality refs |
| `pendingUpdateCounts` | `LifeModelPendingUpdateCounts` | Yes | Review/LifeModel owner | `PHASE_2_REQUIRED` | Pending proposal list partial | No | 0 if backend says | Error warning | Stale counts | Review refs |
| `provenanceRefs` | `EvidenceRef[]` | Yes | LifeModel provenance/evidence owner | `PHASE_2_REQUIRED` | Provenance primitives exist | No | Missing provenance warning | Error warning | Stale refs | Provenance refs |
| `candidateChanges` | `LifeModelCandidateChange[]` | Yes | Review/LifeModel owner | `PHASE_2_REQUIRED` | Proposals exist | No | Empty | Error warning | Disable actions | Review refs |
| `materializedChanges` | `LifeModelMaterializedChange[]` | Yes | LifeModel materialization owner | `PHASE_2_REQUIRED` | Materialization primitives partial | No | Empty if backend says | Error warning | Mark stale | Apply refs |
| `manualOverrideState` | `LifeModelManualOverrideState \| null` | Conditional | LifeModel write gateway | `PHASE_2_REQUIRED` | Manual/state paths require classification | No | `null` | Error warning | Disable manual save | Override audit |
| `relatedReviewItemRefs` | `ReviewItemRef[]` | Yes | Review Center owner | `PHASE_2_REQUIRED` | Proposal refs partial | No | Empty | Error warning | Stale/disabled | Review audit |
| `memoryLinkage` | `LifeModelMemoryLinkageSummary` | Yes | Memory/LifeModel read model | `PHASE_2_REQUIRED` | Memory lifecycle partial | No | Unknown linkage | Error warning | Mark stale | Memory refs |
| `debugRawControls` | `DebugAction[]` | No | Support/debug policy | `PHASE_2_REQUIRED` | Diagnostics policy | No | Hidden | Hidden | Hidden/stale | Debug refs |

## LifeModel Nested Contract Types

`PHASE_2_REQUIRED`: These target types make the canonical/current distinction explicit. A frontend helper such as `lifeModelTrust.ts` may format these values, but it must not become the owner.

```ts
type LifeModelTruthMode =
  | 'canonical'
  | 'current_compatibility'
  | 'candidate'
  | 'pending_review'
  | 'manual_override'
  | 'unknown'
  | 'unavailable'

type LifeModelCanonicalSummary = {
  lifeModelRef: BackendEntityRef
  title: string
  summary: string
  versionLabel: string
  lastMaterializedAt: string | null
  evidenceRefs: EvidenceRef[]
}

type LifeModelCurrentViewSummary = {
  currentViewRef: BackendEntityRef
  compatibilityMode: boolean
  label: string
  summary: string
  divergenceFromCanonical: 'none' | 'minor' | 'material' | 'unknown'
  evidenceRefs: EvidenceRef[]
}

type LifeModelDimensionSummary = {
  id: string
  label: string
  summary: string
  confidence: 'low' | 'medium' | 'high' | 'unknown'
  stale: boolean
  pendingReviewItemRefs: ReviewItemRef[]
  evidenceRefs: EvidenceRef[]
}

type LifeModelTrustQualityState = {
  readiness: 'not_built' | 'limited' | 'usable_with_limits' | 'ready' | 'stale' | 'unknown'
  completionScore: number | null
  missingDimensionCount: number
  staleDimensionCount: number
  warningRefs: EvidenceRef[]
}

type LifeModelPendingUpdateCounts = {
  candidate: number
  pendingReview: number
  approvedNotApplied: number
  failedMaterialization: number
}

type LifeModelCandidateChange = {
  changeRef: BackendEntityRef
  title: string
  changeKind: 'add' | 'update' | 'remove' | 'merge' | 'manual_override' | 'unknown'
  affectedDimensionIds: string[]
  reviewItemRefs: ReviewItemRef[]
  evidenceRefs: EvidenceRef[]
}

type LifeModelMaterializedChange = {
  changeRef: BackendEntityRef
  title: string
  materializationStatus: ReviewItemMaterializationStatus
  materializedAt: string | null
  rollbackAvailable: boolean
  evidenceRefs: EvidenceRef[]
}

type LifeModelManualOverrideState = {
  active: boolean
  blockedReason?: string
  draftRef: BackendEntityRef | null
  saveAction: ProductAction | null
  reviewItemRefs: ReviewItemRef[]
  evidenceRefs: EvidenceRef[]
}

type LifeModelMemoryLinkageSummary = {
  linkedMemoryCount: number
  candidateMemoryCount: number
  materializedMemoryCount: number
  conflictCount: number
  memoryRefs: BackendEntityRef[]
  evidenceRefs: EvidenceRef[]
}
```

## Product Actions

`ProductAction`: build/update LifeModel, open Review Center, inspect evidence, refresh.

`ReviewAction`: approve/reject/edit/later linked LifeModel changes only through Review Center.

`DebugAction`: raw patch/provenance, compatibility internals, raw model JSON/export, diagnostics.

## Empty / Error / Stale Behavior

`DESIGN_DECISION`: Empty means LifeModel not built or no confirmed content. Show build/update route without claiming readiness.

`DESIGN_DECISION`: Error means do not use stale raw LifeModel as current truth.

`DESIGN_DECISION`: Stale means show last updated and disable manual/durable actions until refreshed.

## Tests Needed

- Backend tests for `truthMode` and canonical/current labels.
- Dimension provenance fixture tests.
- Pending versus materialized change tests.
- Manual override visibility tests.
- Memory linkage tests.
- Frontend contract tests preventing page-local canonical/current inference.

## Readiness Note

`READY_WITH_LIMITS`: A limited LifeModel surface can use existing LifeModel/current/completion/proposal primitives if it labels compatibility/current limits clearly and avoids canonical overclaim.

`PHASE_2_REQUIRED`: Full LifeModelViewModel remains required before a full V2 redesign.
