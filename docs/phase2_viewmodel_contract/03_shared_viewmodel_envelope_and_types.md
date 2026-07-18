# Shared ViewModel Envelope And Types

Status: shared contract proposal. No backend or frontend implementation.

## Source Ownership Rules

`DESIGN_DECISION`: Every product ViewModel must use `ViewModelEnvelope<T>`.

`DESIGN_DECISION`: `source` must be `'backend-readmodel'` for product truth. Raw domain reads are allowed only inside `debugOnly` actions or advanced inspector payloads.

`DESIGN_DECISION`: Product, review, and debug actions are separate action lanes:

- `ProductAction`: default user-facing action required to complete the task.
- `ReviewAction`: approval, rejection, edit, later, revoke, apply, resume, or evidence action for consequential changes.
- `DebugAction`: advanced or developer-only action such as raw trace, export JSON, provider health, route evidence, or raw transcript.

## Type Status Table

| Type | Owner status | Evidence | Notes |
| --- | --- | --- | --- |
| `ViewModelEnvelope<T>` | `PROPOSED` / `PHASE_2_REQUIRED` | Phase 1 ViewModel proposal. | Required shared envelope for Phase 3 implementation. |
| `EvidenceRef` | `PROPOSED` / `PHASE_2_REQUIRED` | Existing evidence stores/events exist; unified frontend ref shape proposed. | Must point to backend/audit/task/review/memory/LifeModel source. |
| `ViewModelWarning` | `PROPOSED` / `PHASE_2_REQUIRED` | Needed by stale/error/limits semantics. | Must not replace blocker/failure state. |
| `ProductAction` | `PROPOSED` / `PHASE_2_REQUIRED` | Phase 1 action split. | Default product command, not review approval. |
| `ReviewAction` | `PROPOSED` / `PHASE_2_REQUIRED` | Phase 1 Review Center model. | Backend must own availability. |
| `DebugAction` | `PROPOSED` / `PHASE_2_REQUIRED` | Diagnostics visibility policy. | Hidden by default. |
| `ReviewItem` | `PROPOSED` / `PHASE_2_REQUIRED` | Proposal primitives exist; unified ReviewItem does not. | Must separate decision and materialization. |
| `ReviewItemType` | `PROPOSED` / `PHASE_2_REQUIRED` | Phase 1 Review Center model. | Preserves proposals, permissions, external writes, Memory, LifeModel, policy, dangerous actions. |
| `ReviewItemStatus` | `PROPOSED` / `PHASE_2_REQUIRED` | Phase 1 Review Center model. | User decision lifecycle only. |
| `ReviewItemMaterializationStatus` | `PROPOSED` / `PHASE_2_REQUIRED` | Goal requirement and MemoryLifecycle evidence. | Durable apply/materialization lifecycle only. |
| `RiskLevel` | `PARTIAL` | `AgentProposal.riskLevel` exists; review-wide risk not unified. | ReviewItem risk must be backend-owned. |
| `ImpactScope` | `PROPOSED` / `PHASE_2_REQUIRED` | Existing display helpers infer impact; backend owner missing. | Must be backend-owned for Review Center. |
| `ProviderPrivacyBoundarySummary` | `PROPOSED` / `PHASE_2_REQUIRED` | Runtime disclosure/provider evidence exists only partially. | Shared by Workspace, Today, Settings, and Review impact summaries. |

## Shared Type Definitions

```ts
type ViewModelStatus = 'loading' | 'ready' | 'empty' | 'error' | 'stale'

type ViewModelEnvelope<T> = {
  data: T | null
  status: ViewModelStatus
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

type EvidenceRef = {
  id: string
  label: string
  source:
    | 'backend-readmodel'
    | 'audit'
    | 'task'
    | 'review'
    | 'memory'
    | 'lifemodel'
    | 'settings'
    | 'provider'
  sensitivity?: 'public' | 'local_private' | 'sensitive' | 'redacted'
}

type ViewModelWarning = {
  code: string
  message: string
  severity: 'info' | 'warning' | 'error'
  evidenceRefs?: EvidenceRef[]
}

type ProductAction = {
  id: string
  label: string
  kind: 'open' | 'start' | 'continue' | 'retry' | 'cancel' | 'refresh' | 'inspect' | 'configure'
  enabled: boolean
  disabledReason?: string
  targetRef?: string
}

type ReviewActionBase = {
  id: string
  label: string
  enabled: boolean
  disabledReason?: string
  requiresConfirmation?: boolean
  targetReviewItemId: string
  expectedMaterializationStatusAfterDispatch?: ReviewItemMaterializationStatus
}

type ReviewActionKindEffectInvariant =
  | { kind: 'approve' | 'reject' | 'edit' | 'later' | 'revoke'; effect: 'decision_only' }
  | { kind: 'apply'; effect: 'materialization_request' }
  | { kind: 'resume'; effect: 'task_resume_request' }
  | { kind: 'view_evidence'; effect: 'evidence_only' }

type ReviewAction = ReviewActionBase & ReviewActionKindEffectInvariant

type DebugAction = {
  id: string
  label: string
  kind: 'raw_trace' | 'raw_json' | 'export' | 'provider_health' | 'route_evidence' | 'transcript'
  enabled: boolean
  developerOnly?: boolean
  targetRef?: string
}

type ReviewItemType =
  | 'proposal'
  | 'permission_request'
  | 'external_write'
  | 'memory_update'
  | 'lifemodel_change'
  | 'policy_change'
  | 'dangerous_action'

type ReviewItemStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'expired'
  | 'blocked'
  | 'revoked'
  | 'failed'

type ReviewItemMaterializationStatus =
  | 'not_applicable'
  | 'not_started'
  | 'applying'
  | 'applied'
  | 'failed'
  | 'rolled_back'
  | 'unknown'

type RiskLevel = 'low' | 'medium' | 'high' | 'critical'

type ImpactScope = {
  summary: string
  affectedDomains: Array<'task' | 'memory' | 'lifemodel' | 'tool' | 'file' | 'provider' | 'policy' | 'settings'>
  externalTransmission: 'none' | 'possible' | 'sent' | 'unknown'
  durableWrite: boolean
  reversible: boolean
}

type ProviderPrivacyBoundarySummary = {
  routeType: 'local' | 'cloud' | 'hybrid' | 'auto' | 'unknown'
  externalTransmission: 'not_sent' | 'sent' | 'possible' | 'unknown'
  providerLabel: string
  modelLabel: string
  privacyLabel: string
  risk: RiskLevel | 'none' | 'unknown'
  localOnlyRequired: boolean
  blockedReason?: string
  evidenceRefs: EvidenceRef[]
}

type ReviewItem = {
  id: string
  type: ReviewItemType
  title: string
  status: ReviewItemStatus
  materializationStatus: ReviewItemMaterializationStatus
  risk: RiskLevel
  impact: ImpactScope
  source: string
  evidenceRefs: EvidenceRef[]
  auditRefs: EvidenceRef[]
  expiresAt?: string | null
  relatedTaskRef?: string | null
  relatedMemoryRef?: string | null
  relatedLifeModelRef?: string | null
  allowedActions: ReviewAction[]
}

type ReviewItemRef = {
  reviewItemId: string
  type: ReviewItemType
  status: ReviewItemStatus
  materializationStatus: ReviewItemMaterializationStatus
  title: string
  risk: RiskLevel
  href?: string
  evidenceRefs: EvidenceRef[]
}

type BackendEntityRef = {
  id: string
  kind:
    | 'task'
    | 'run'
    | 'conversation'
    | 'review_item'
    | 'memory'
    | 'lifemodel'
    | 'proposal'
    | 'tool_permission'
    | 'evidence'
  label: string
  href?: string
}
```

`DESIGN_DECISION`: `ReviewAction.kind = 'apply'` never means the durable change is already applied. It means the user may request backend materialization for an already approved item; after dispatch, UI must refresh and render `ReviewItem.materializationStatus`.

`DESIGN_DECISION`: `ReviewAction.kind = 'resume'` never means the blocked task has resumed. It means the user may request backend task resume after the review/materialization preconditions are met; after dispatch, Workspace/Tasks must refresh task lifecycle from backend read models.

`DESIGN_DECISION`: `ReviewAction.kind` and `ReviewAction.effect` must match `ReviewActionKindEffectInvariant`. A backend read model must not emit `approve` with `materialization_request`, `apply` with `decision_only`, or `resume` with `decision_only`.

`DESIGN_DECISION`: `ProviderPrivacyBoundarySummary` is shared contract surface, not Workspace-local wording. Product pages may format this summary, but they must not reconstruct provider trust, external transmission, or local-only blocking from diagnostics/config fragments.

## Shared Field Contract

| Field | Type | Required | Source of truth | Owner status | Evidence | Frontend may infer? | Empty behavior | Error behavior | Stale behavior | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `data` | `T \| null` | Yes | Backend read model | `PROPOSED` | Phase 1 ViewModel proposal | No | `null` with `empty` | `null` with `error` | Keep last known value only if status is `stale` | Envelope evidence refs |
| `status` | `ViewModelStatus` | Yes | Backend read model / fetch state | `PROPOSED` | Goal requirement | No | `empty` | `error` | `stale` | Status transition should be testable |
| `lastUpdatedAt` | `string \| null` | Yes | Backend read model | `PROPOSED` | Stale semantics requirement | No | `null` allowed | Preserve prior timestamp if stale | Required for stale copy | Timestamp audit ref |
| `source` | `'backend-readmodel'` | Yes | Backend read model | `PROPOSED` | Phase 1 hard rule | No | Always present | Always present | Always present | Prevents fake frontend owners |
| `evidenceRefs` | `EvidenceRef[]` | No, required for consequential states | Backend evidence/audit stores | `PHASE_2_REQUIRED` | Existing evidence stores/events | No | Empty list means no inspect link | Missing refs generate warning | Preserve stale refs with warning | Required for review/debug |
| `warnings` | `ViewModelWarning[]` | No | Backend read model | `PHASE_2_REQUIRED` | Need fail-closed wording | No | No warnings | Error warning visible | Stale warning visible | Warning code/source refs |
| `actions.primary` | `ProductAction[]` | Yes | Backend read model | `PHASE_2_REQUIRED` | Phase 1 action split | No | Empty when no action | Retry/refresh only | Disable risky actions | Action id and target refs |
| `actions.review` | `ReviewAction[]` | Conditional | ReviewItem owner | `PHASE_2_REQUIRED` | Review Center model | No | Empty when no review item | Disabled until refresh | Disabled if stale | Review action audit; `apply` and `resume` are request actions only |
| `actions.debugOnly` | `DebugAction[]` | No | Backend/support mode | `PHASE_2_REQUIRED` | Diagnostics visibility policy | No | Hidden | Hidden unless advanced mode | Hidden or marked stale | Debug action refs |

## ReviewItem Field Contract

| Field | Type | Required | Source of truth | Owner status | Evidence | Frontend may infer? | Empty behavior | Error behavior | Stale behavior | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `id` | string | Yes | Review read model | `PHASE_2_REQUIRED` | Proposal ids exist; unified item missing | No | Item omitted | Item omitted/error | Disable actions | Decision audit key |
| `type` | `ReviewItemType` | Yes | Review read model | `PHASE_2_REQUIRED` | Phase 1 type taxonomy | No | Item omitted | Item omitted/error | Disable actions | Type recorded |
| `title` | string | Yes | Review read model | `PHASE_2_REQUIRED` | Proposal display has partial copy | No | "untitled" not allowed | Error item | Stale label allowed with warning | User-visible audit |
| `status` | `ReviewItemStatus` | Yes | Review workflow | `PHASE_2_REQUIRED` | Goal requirement | No | `pending` cannot be assumed | Error item | Disable actions | Decision state audit |
| `materializationStatus` | `ReviewItemMaterializationStatus` | Yes | Materializer/apply owner | `PHASE_2_REQUIRED` | Memory lifecycle has materialization; review-wide missing | No | `unknown` only when backend says unknown | Error if missing for approved item | Disable apply/resume | Durable apply audit |
| `risk` | `RiskLevel` | Yes | Backend risk authority | `PHASE_2_REQUIRED` | Proposal risk exists; non-proposal missing | No | Item hidden or unknown-warning | Error item | Disable approve if stale | Risk source refs |
| `impact` | `ImpactScope` | Yes | Backend review owner | `PHASE_2_REQUIRED` | Phase 1 impact requirement | No | No approval action | Error item | Disable approve | Impact/audit refs |
| `evidenceRefs` | `EvidenceRef[]` | Yes for consequential changes | Evidence/audit stores | `PHASE_2_REQUIRED` | Existing evidence stores | No | Show insufficient evidence blocker | Error item | Stale evidence warning | Evidence refs |
| `allowedActions` | `ReviewAction[]` | Yes | Backend review owner | `PHASE_2_REQUIRED` | Phase 1 stop rule | No | Empty list | Retry/refresh only | Disable all review actions | Action audit |

## Empty / Error / Stale Semantics

`DESIGN_DECISION`: `empty` means the backend read model loaded successfully and found no product data for that surface.

`DESIGN_DECISION`: `error` means the backend read model could not load. Pages must not fill the gap from raw domain reads except in debug-only surfaces.

`DESIGN_DECISION`: `stale` means the backend read model returned or preserved old data whose actions are unsafe until refresh. Risky product and review actions must be disabled unless backend explicitly marks them safe.
