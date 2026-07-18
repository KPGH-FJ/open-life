# MemoryViewModel Contract

Status: proposed contract. No Memory UI implementation.

## Purpose

`DESIGN_DECISION`: `MemoryViewModel` explains what OpenLife remembers, what is context-only, candidate, confirmed, used in LifeModel, withdrawn, expired, archived, restored, or rolled back.

Backend owner: Proposed `MemoryViewModel`
Owner status: `PHASE_2_REQUIRED`
Required validation: Phase 3 must define lane counts, lifecycle status, provenance, review refs, archive/restore controls, and Memory/LifeModel linkage before top-level `记忆` implementation.

## Top-level Memory Readiness

`NOT_READY`: Top-level `记忆` is not implementation-ready as a full V2 surface because backend lane/status/provenance summaries are not yet verified as a product read model.

`CANDIDATE`: Memory remains a first-class OpenLife capability. If top-level Memory is not approved, it should move to a LifeModel sub-surface, Settings/Data Management sub-surface, or Workspace evidence preview without deleting the capability.

## Existing Support

`EXISTING_CODE`: Current support includes memory search, tier stats, archive/restore/rollback controls, proposal list, `MemoryLifecycleRecord`, MemoryGateway/lifecycle primitives, and diagnostics.

`VERIFIED_FACT`: Phase 0 audit says Memory is split across chat messages, memory rows, vector chunks, lifecycle, evidence, and gateway decisions. It must not be represented as one raw search table.

## Memory Lane Model

`CANDIDATE`: First user-facing lanes:

- context;
- event;
- preference;
- rule;
- evidence;
- LifeModel truth.

`PHASE_2_REQUIRED`: Backend must own lane assignment, counts, review requirement, provenance, and lifecycle status.

## Required Field Contract

| Field | Type | Required | Source of truth | Owner status | Evidence | Frontend may infer? | Empty behavior | Error behavior | Stale behavior | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `laneSummaries` | `MemoryLaneSummary[]` | Yes | Memory read model | `PHASE_2_REQUIRED` | MemoryGateway lanes exist | No | Empty lanes | Error envelope | Mark stale | Lane source refs |
| `statusCounts` | `MemoryStatusCounts` | Yes | Memory lifecycle owner | `PHASE_2_REQUIRED` | MemoryLifecycleRecord partial | No | 0 if backend says | Error warning | Stale counts | Lifecycle refs |
| `candidateMemories` | `MemoryItemSummary[]` | Yes | Memory/review owner | `PHASE_2_REQUIRED` | Proposals partial | No | Empty | Error warning | Disable review actions | Review refs |
| `confirmedMemories` | `MemoryItemSummary[]` | Yes | Memory read model | `PHASE_2_REQUIRED` | Memory store/lifecycle exists | No | Empty | Error warning | Mark stale | Memory refs |
| `usedInLifeModel` | `MemoryLifeModelLink[]` | Yes | Memory/LifeModel owner | `PHASE_2_REQUIRED` | Memory lifecycle/provenance partial | No | Empty | Error warning | Mark stale | LifeModel refs |
| `withdrawnExpired` | `MemoryItemSummary[]` | Yes | Memory lifecycle owner | `PHASE_2_REQUIRED` | Rollback/superseded fields partial | No | Empty | Error warning | Mark stale | Lifecycle audit |
| `sourceProvenance` | `MemoryProvenanceSummary` | Yes | Evidence/audit owner | `PHASE_2_REQUIRED` | Evidence stores exist | No | Missing evidence warning | Error warning | Stale refs | Evidence refs |
| `reviewItemRefs` | `ReviewItemRef[]` | Yes | Review Center owner | `PHASE_2_REQUIRED` | Proposal refs partial | No | Empty | Error warning | Stale/disabled | Review audit |
| `archiveRestoreControls` | `ProductAction[]` | Conditional | Memory lifecycle/danger owner | `PHASE_2_REQUIRED` | Archive/restore exists | No | No controls | Error warning | Disable controls | Control audit |
| `search` | `MemorySearchSummary` | Yes | Memory read model | `PARTIAL` | `searchMemory` exists | No for product status; yes for query string | Empty results | Error search state | Mark stale | Search refs |
| `lifecycle` | `MemoryLifecycleSummary` | Yes | Memory lifecycle owner | `PHASE_2_REQUIRED` | Lifecycle primitives exist | No | Unknown lifecycle | Error warning | Mark stale | Lifecycle refs |
| `rawDebugRefs` | `DebugAction[]` | No | Support/debug policy | `PHASE_2_REQUIRED` | Diagnostics policy | No | Hidden | Hidden | Hidden/stale | Debug refs |

## Memory Nested Contract Types

`PHASE_2_REQUIRED`: These types are required before top-level `记忆` can move out of `NOT_READY`.

```ts
type MemoryLane =
  | 'context'
  | 'event'
  | 'preference'
  | 'rule'
  | 'evidence'
  | 'lifemodel_truth'

type MemoryItemStatus =
  | 'candidate'
  | 'confirmed'
  | 'used_in_lifemodel'
  | 'withdrawn'
  | 'expired'
  | 'archived'
  | 'rolled_back'
  | 'unknown'

type MemoryLaneSummary = {
  lane: MemoryLane
  label: string
  description: string
  candidateCount: number
  confirmedCount: number
  usedInLifeModelCount: number
  withdrawnExpiredCount: number
  reviewRequiredCount: number
  directWriteAllowed: boolean
  directWritePolicyReason: string
  evidenceRefs: EvidenceRef[]
}

type MemoryStatusCounts = {
  candidate: number
  confirmed: number
  usedInLifeModel: number
  withdrawn: number
  expired: number
  archived: number
  rolledBack: number
  unknown: number
}

type MemoryItemSummary = {
  memoryId: string
  lane: MemoryLane
  status: MemoryItemStatus
  title: string
  preview: string
  sourceSummary: string
  confidence: number | null
  sensitivity: 'low' | 'medium' | 'high' | 'unknown'
  createdAt: string | null
  updatedAt: string | null
  reviewItemRefs: ReviewItemRef[]
  lifecycleRefs: BackendEntityRef[]
  evidenceRefs: EvidenceRef[]
  allowedActions: ProductAction[]
}

type MemoryLifeModelLink = {
  memoryRef: BackendEntityRef
  lifeModelRef: BackendEntityRef
  relation:
    | 'candidate_input'
    | 'confirmed_input'
    | 'materialized_into_lifemodel'
    | 'superseded'
    | 'rolled_back'
  materializationStatus: ReviewItemMaterializationStatus
  evidenceRefs: EvidenceRef[]
}

type MemoryProvenanceSummary = {
  sourceTaskRefs: BackendEntityRef[]
  sourceReviewRefs: ReviewItemRef[]
  sourceEvidenceRefs: EvidenceRef[]
  privacySummary: string
  auditRefs: EvidenceRef[]
}

type MemorySearchSummary = {
  query: string
  resultCount: number
  lowConfidenceHiddenCount: number
  results: MemoryItemSummary[]
  searchEvidenceRefs: EvidenceRef[]
}

type MemoryLifecycleSummary = {
  latestEventAt: string | null
  latestEventKind:
    | 'created'
    | 'confirmed'
    | 'materialized'
    | 'archived'
    | 'restored'
    | 'rolled_back'
    | 'expired'
    | 'unknown'
  hasConflicts: boolean
  conflictRefs: BackendEntityRef[]
  rollbackAvailable: boolean
  restoreAvailable: boolean
  evidenceRefs: EvidenceRef[]
}
```

## Product Actions

`ProductAction`: search, inspect memory, open Review Center, archive/restore when backend allows, refresh.

`ReviewAction`: approve/reject/edit/later memory candidates through Review Center.

`DebugAction`: raw memory row, vector/index details, tier internals, diagnostics, export.

## Fallback If Top-level Memory Is Not Approved

`DESIGN_DECISION`: Fallback preserves Memory capability:

- LifeModel sub-surface for memory impact on long-term understanding.
- Settings/Data Management sub-surface for export/archive/restore.
- Workspace evidence preview for task-produced memory candidates.
- Review Center remains decision authority.

## Empty / Error / Stale Behavior

`DESIGN_DECISION`: Empty means no confirmed memories or no search results; explain candidate versus durable memory.

`DESIGN_DECISION`: Error means do not show raw rows as product truth.

`DESIGN_DECISION`: Stale means archive/restore and review actions disabled until refresh.

## Tests Needed

- Lane count/status backend tests.
- Candidate/confirmed/used/withdrawn lifecycle tests.
- ReviewItem linkage tests.
- Archive/restore/danger preflight tests.
- Search fixture tests separating search results from lane truth.
- Frontend contract tests banning raw memory rows as product ViewModel.
