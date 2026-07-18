# ReviewCenterViewModel Contract

Status: proposed contract. Do not implement in Phase 2.

## Purpose

`DESIGN_DECISION`: `ReviewCenterViewModel` is the backend-owned read model for `审核中心`: grouped consequential review items, risk, impact, evidence, allowed actions, expiration, audit refs, task resume relations, and durable materialization/apply state.

Backend owner: Proposed `ReviewCenterViewModel`
Owner status: `PHASE_2_REQUIRED`
Required validation: Phase 3 must verify or implement a backend ReviewItem owner. Current proposal pages and helpers are not enough.

## Required Principle

`DESIGN_DECISION`: Do not mix user decision status with backend durable materialization/apply state.

`DESIGN_DECISION`: `ReviewItemStatus` is decision state. `ReviewItemMaterializationStatus` is durable application state.

## Existing Support

`EXISTING_CODE`: Current support includes `AgentProposal`, `listProposals`, accept/reject/edit/postpone actions, `LifeStateProjection.pending`, safe paths, safe-mode data, `MemoryLifecycleRecord`, tool permissions, and danger preflight primitives.

`VERIFIED_FACT`: Phase 1 says current `/mailbox` is proposal-oriented and a unified ReviewItem model is still `PHASE_2_REQUIRED`.

## Required Field Contract

| Field | Type | Required | Source of truth | Owner status | Evidence | Frontend may infer? | Empty behavior | Error behavior | Stale behavior | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `groups` | `ReviewGroup[]` | Yes | Review Center read model | `PHASE_2_REQUIRED` | Phase 1 grouping requirement | No | Empty groups with no-review copy | Error envelope | Disable item actions | Group source refs |
| `items` | `ReviewItem[]` | Yes | Review Center read model | `PHASE_2_REQUIRED` | Proposal list partial; unified missing | No | Empty list | Error envelope | Disable actions | Item audit refs |
| `item.type` | `ReviewItemType` | Yes | Review Center read model | `PHASE_2_REQUIRED` | Required taxonomy | No | Item omitted | Error item | Disable actions | Type recorded |
| `item.status` | `ReviewItemStatus` | Yes | Review workflow | `PHASE_2_REQUIRED` | Proposal status partial | No | No default pending | Error item | Refresh required | Decision audit |
| `item.materializationStatus` | `ReviewItemMaterializationStatus` | Yes | Apply/materializer owner | `PHASE_2_REQUIRED` | Goal requirement; Memory lifecycle partial | No | `not_applicable` only if backend says so | Error for approved item | Disable apply/resume | Durable apply audit |
| `allowedActions` | `ReviewAction[]` | Yes | Review Center read model | `PHASE_2_REQUIRED` | Phase 1 stop rule | No | Empty means no action | Retry refresh only | Disable all review actions | Action audit |
| `risk` | `RiskLevel` | Yes | Backend risk authority | `PHASE_2_REQUIRED` | Proposal risk partial | No | Item cannot be approved | Error item | Disable approval | Risk evidence refs |
| `impact` | `ImpactScope` | Yes | Backend review owner | `PHASE_2_REQUIRED` | Phase 1 impact requirement | No | Item cannot be approved | Error item | Disable approval | Impact audit |
| `source` | `ReviewItemSourceRef` | Yes | Source workflow | `PHASE_2_REQUIRED` | Proposal source partial | No | Unknown source warning | Error item | Mark stale | Source refs |
| `evidenceRefs` | `EvidenceRef[]` | Yes | Evidence/audit stores | `PHASE_2_REQUIRED` | Evidence stores exist | No | Insufficient evidence blocker | Error item | Stale evidence warning | Evidence refs |
| `expiresAt` | `string \| null` | Conditional | Review owner | `PHASE_2_REQUIRED` | Phase 1 expiration requirement | No | `null` means no expiry if backend says | Error if required | Refresh required | Expiry audit |
| `auditRefs` | `EvidenceRef[]` | Yes | Audit stores | `PHASE_2_REQUIRED` | Audit primitives exist | No | Missing audit warning | Error item | Stale warning | Decision/apply audit |
| `taskResumeRelation` | `ReviewTaskResumeRelation \| null` | Conditional | Task/review owner | `PHASE_2_REQUIRED` | Mailbox resume path exists locally | No | `null` | Error warning | Disable resume | Task refs |
| `toolPermissionRelation` | `ReviewToolPermissionRelation \| null` | Conditional | Tool permission owner | `PHASE_2_REQUIRED` | ToolPermissionStore exists | No | `null` | Error warning | Disable permission actions | Permission refs |
| `memoryRelation` | `ReviewMemoryRelation \| null` | Conditional | MemoryGateway/lifecycle owner | `PHASE_2_REQUIRED` | Memory lifecycle partial | No | `null` | Error warning | Disable memory apply | Memory refs |
| `lifeModelRelation` | `ReviewLifeModelRelation \| null` | Conditional | LifeModel write owner | `PHASE_2_REQUIRED` | LifeModel gateway/provenance partial | No | `null` | Error warning | Disable apply | LifeModel refs |
| `externalWriteRelation` | `ReviewExternalWriteRelation \| null` | Conditional | Safe-path/danger owner | `PHASE_2_REQUIRED` | Safe write/danger primitives exist | No | `null` | Error warning | Disable approve | Safe-path/audit refs |

## Item Types

```ts
type ReviewItemType =
  | 'proposal'
  | 'permission_request'
  | 'external_write'
  | 'memory_update'
  | 'lifemodel_change'
  | 'policy_change'
  | 'dangerous_action'
```

`CANDIDATE`: Not every type is currently backed by a unified ReviewItem owner. Preserve all types as required product capabilities; mark unsupported types as `PHASE_2_REQUIRED`, not deleted.

## Review Center Nested Contract Types

`PHASE_2_REQUIRED`: These types are contract targets for the backend ReviewItem owner. They do not assert that current proposal/mailbox code already emits this shape.

```ts
type ReviewGroup = {
  id: string
  label: string
  description: string
  itemCount: number
  highestRisk: RiskLevel | 'none' | 'unknown'
  defaultExpanded: boolean
  evidenceRefs: EvidenceRef[]
}

type ReviewItemSourceRef = {
  sourceType:
    | 'agent_proposal'
    | 'tool_permission'
    | 'memory_lifecycle'
    | 'lifemodel_change'
    | 'external_write'
    | 'policy'
    | 'danger_preflight'
    | 'unknown'
  sourceRef: BackendEntityRef | null
  createdBy: 'assistant' | 'user' | 'system' | 'tool' | 'unknown'
  createdAt: string | null
  evidenceRefs: EvidenceRef[]
}

type ReviewTaskResumeRelation = {
  taskRef: BackendEntityRef
  blockedOnReviewItemId: string
  resumeEligibility: 'not_eligible' | 'eligible_after_decision' | 'eligible_after_materialization' | 'eligible_now' | 'unknown'
  resumeAction: ReviewAction | null
  evidenceRefs: EvidenceRef[]
}

type ReviewToolPermissionRelation = {
  permissionRef: BackendEntityRef
  capabilityLabel: string
  risk: RiskLevel
  requestedScope: string
  expiresAt: string | null
  evidenceRefs: EvidenceRef[]
}

type ReviewMemoryRelation = {
  memoryRef: BackendEntityRef | null
  candidateLane: 'context' | 'event' | 'preference' | 'rule' | 'evidence' | 'lifemodel_truth' | 'unknown'
  lifecycleStatus: 'candidate' | 'confirmed' | 'materialized' | 'archived' | 'rolled_back' | 'expired' | 'unknown'
  materializationStatus: ReviewItemMaterializationStatus
  evidenceRefs: EvidenceRef[]
}

type ReviewLifeModelRelation = {
  lifeModelRef: BackendEntityRef | null
  changeKind: 'canonical_update' | 'current_view_update' | 'candidate_change' | 'manual_override' | 'rollback' | 'unknown'
  truthModeAfterApply: 'canonical' | 'current_compatibility' | 'candidate' | 'manual_override' | 'unknown'
  materializationStatus: ReviewItemMaterializationStatus
  evidenceRefs: EvidenceRef[]
}

type ReviewExternalWriteRelation = {
  targetLabel: string
  targetKind: 'file' | 'calendar' | 'email' | 'provider' | 'plugin' | 'shell' | 'external_api' | 'unknown'
  scopeDigest: string
  safePathState: 'inside_safe_path' | 'outside_safe_path' | 'not_applicable' | 'unknown'
  confirmationPhraseRequired: string | null
  providerPrivacyBoundary: ProviderPrivacyBoundarySummary
  evidenceRefs: EvidenceRef[]
}
```

## Allowed Actions

`PHASE_2_REQUIRED`: Frontend cannot infer allowed actions, risk, expiration, or materialization status from proposal type, tool name, safe path, or local page state.

Default ReviewActions:

- approve;
- reject;
- edit;
- later;
- revoke;
- view evidence;
- apply/resume only when backend marks safe.

`DESIGN_DECISION`: `apply` is a request to start or retry backend materialization. It must move through `materializationStatus = applying | applied | failed | rolled_back | unknown`; it must not be rendered as applied until the refreshed ReviewItem says `applied`.

`DESIGN_DECISION`: `resume` is a request to resume a related task after review/materialization preconditions are satisfied. It must not be rendered as resumed until Workspace/Tasks read models refresh the task lifecycle.

## Relations

`PHASE_2_REQUIRED`: Review Center must expose:

- task resume relation for waiting Main Chat task sessions;
- tool permission relation for permission requests;
- memory relation for candidate/confirmed/materialized/rollback memory records;
- LifeModel relation for canonical/current/candidate/materialized changes;
- external write relation for target, path/scope digest, safe-path state, confirmation phrase, and audit refs.

## Empty / Error / Stale Behavior

`DESIGN_DECISION`: Empty means no current review items.

`DESIGN_DECISION`: Error means review actions are unavailable. Pages must not fall back to local proposal action guesses.

`DESIGN_DECISION`: Stale means all review actions are disabled until refreshed.

## Tests Needed

- Backend ReviewItem grouping and allowed-action tests.
- Decision status versus materialization status tests.
- External-write safe-path/confirmation tests.
- Memory/LifeModel materialization relation tests.
- Expiration and revoked/failed item tests.
- Frontend contract tests verifying no local allowed-action inference.

## Implementation Stop Rules

`PHASE_2_REQUIRED`: Do not build Review Center V2 until backend owns ReviewItem grouping, action availability, risk, expiration, materialization, and relation fields.
