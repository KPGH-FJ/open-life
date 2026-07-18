# WorkspaceViewModel Contract

Status: proposed contract. Do not implement in Phase 2.

## Purpose

`DESIGN_DECISION`: `WorkspaceViewModel` is the backend-owned read model for `工作区`: intent composer, understanding, execution timeline, control/review drawer, result, evidence drawer, and advanced inspector refs.

Backend owner: Proposed `WorkspaceViewModel`
Owner status: `PHASE_2_REQUIRED`
Required validation: Phase 3 must verify or implement a backend owner that consolidates current task/session, route/privacy boundary, timeline, blockers, review refs, final result, and allowed controls.

## Existing Backend Support

`EXISTING_CODE`: Current primitives include `MainChatAgentStateSnapshot`, `MainChatAgentTaskState`, `MainChatTaskDetail`, `MainChatTaskSummary`, `RunEvidenceView`, kernel events, durable agent events, `StreamMessageDonePayload`, `ReasoningTrace`, `ToolCallResult`, and `LifeStateProjection`.

`VERIFIED_FACT`: Phase 0.5 identifies ChatPage as the largest ViewModel gap because it assembles many raw sources locally.

## Missing Backend Fields

`PHASE_2_REQUIRED`: Missing fields include consolidated workspace summary, editable/confirmable understanding object, default timeline stage model, unified blocker taxonomy, review item refs, final result summary, provider/privacy boundary summary, and backend-owned allowed controls.

## Required Field Contract

| Field | Type | Required | Source of truth | Owner status | Evidence | Frontend may infer? | Empty behavior | Error behavior | Stale behavior | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `workspaceId` | string | Yes | Workspace read model | `PROPOSED` | Phase 1 workspace model | No | New empty workspace id | Error envelope | Preserve stale id, disable actions | Links task/session refs |
| `currentTaskRef` | `{ taskSessionId?: string; runId?: string; conversationId?: string }` | Conditional | Main Chat task/run owner | `PARTIAL` | Existing task/run primitives | No | `null` when idle | Error envelope | Disable resume/retry/cancel | Task/run audit refs |
| `intentComposer` | `WorkspaceIntentComposer` | Yes | Workspace read model | `PHASE_2_REQUIRED` | Phase 1 intent composer requirement | No | Empty composer | Error envelope | Draft visible, submit disabled | Input provenance refs |
| `understanding` | `WorkspaceUnderstanding` | Yes after start | IntentFrame / route owner | `PHASE_2_REQUIRED` | Existing ingress/route evidence partial | No | Ask user to start | Show unknown/blocked | Mark stale, require refresh | Route/privacy evidence refs |
| `lifecycleStatus` | `WorkspaceLifecycleStatus` | Yes | Task/final-delivery owner | `PHASE_2_REQUIRED` | Existing task status/final delivery | No | `idle` | `error` | Disable risky controls | Status evidence refs |
| `executionTimeline` | `TimelineStage[]` | Yes | Durable/kernel/task evidence | `PHASE_2_REQUIRED` | Events exist; stage model missing | No | Empty timeline | Error state | Mark stale stages | Stage evidence refs |
| `controlDrawer` | `WorkspaceControlDrawer` | Yes | Workspace read model | `PHASE_2_REQUIRED` | Current controls exist in several primitives | No | Start-only controls | Retry refresh only | Disable risky controls | Control source refs |
| `reviewItemRefs` | `ReviewItemRef[]` | Yes | Review Center read model | `PHASE_2_REQUIRED` | Proposal refs exist; unified ReviewItem missing | No | Empty list | Error warning | Open disabled/stale | Review audit refs |
| `result` | `WorkspaceResult \| null` | Conditional | FinalDelivery owner | `PARTIAL` | `MainChatAgentStateSnapshot.finalDelivery` exists | No | `null` until result | Error state | Mark stale result | Final delivery refs |
| `blockers` | `BlockerSummary[]` | Yes | Task/final-delivery owner | `PHASE_2_REQUIRED` | Blockers exist in task/snapshot/evidence | No | Empty list | Error state | Preserve stale blockers | Blocker evidence refs |
| `toolSummary` | `WorkspaceToolSummary` | Yes | ToolGateway/task owner | `PHASE_2_REQUIRED` | Tool evidence exists | No | Zero tools | Error warning | Mark stale; disable execute | Tool evidence refs |
| `providerPrivacyBoundary` | `ProviderPrivacyBoundarySummary` | Yes | Runtime/provider evidence | `PHASE_2_REQUIRED` | Runtime disclosure helper is frontend partial | No | Unknown boundary warning | Error warning | Mark stale; block external actions | Provider/privacy refs |
| `evidenceDrawerRefs` | `EvidenceRef[]` | Yes | Evidence/audit stores | `PHASE_2_REQUIRED` | Existing evidence surfaces | No | Empty with warning | Error warning | Stale refs marked | Evidence ids |
| `advancedInspectorRefs` | `DebugAction[]` | No | Support/debug policy | `PHASE_2_REQUIRED` | Diagnostics visibility policy | No | Hidden | Hidden unless advanced | Hidden/stale | Debug action refs |

## Workspace Nested Contract Types

`PHASE_2_REQUIRED`: These types are contract targets, not implemented backend structs. `EvidenceRef`, `ProductAction`, `ReviewAction`, `ReviewItemRef`, `BackendEntityRef`, `RiskLevel`, `ReviewItemMaterializationStatus`, and `ProviderPrivacyBoundarySummary` come from the shared contract.

```ts
type WorkspaceLifecycleStatus =
  | 'idle'
  | 'loading'
  | 'understanding'
  | 'planning'
  | 'running'
  | 'waiting_permission'
  | 'blocked'
  | 'failed'
  | 'cancelled'
  | 'completed'
  | 'completed_with_pending_items'
  | 'stale'

type WorkspaceIntentComposer = {
  draftId: string | null
  mode: 'new_task' | 'continue_task' | 'clarify' | 'review_followup'
  inputPreview: string | null
  selectedCapabilityRefs: BackendEntityRef[]
  contextRefs: BackendEntityRef[]
  privacyBoundary: ProviderPrivacyBoundarySummary
  canSubmit: boolean
  disabledReason?: string
  evidenceRefs: EvidenceRef[]
}

type WorkspaceUnderstanding = {
  userGoalSummary: string
  interpretedIntent: string
  intentConfidence: 'low' | 'medium' | 'high' | 'unknown'
  uncertaintyReasons: string[]
  routeSummary: string
  policySummary: string
  providerPrivacyBoundary: ProviderPrivacyBoundarySummary
  missingContextQuestions: string[]
  assumptions: string[]
  editable: boolean
  confirmationRequired: boolean
  evidenceRefs: EvidenceRef[]
}

type TimelineStage = {
  id: string
  kind:
    | 'understanding'
    | 'planning'
    | 'tool'
    | 'review'
    | 'memory'
    | 'lifemodel'
    | 'result'
    | 'blocker'
  status: 'not_started' | 'running' | 'waiting' | 'blocked' | 'failed' | 'completed' | 'skipped'
  title: string
  summary: string
  startedAt?: string | null
  finishedAt?: string | null
  evidenceRefs: EvidenceRef[]
  reviewItemRefs: ReviewItemRef[]
  debugRefs: DebugAction[]
}

type WorkspaceControlDrawer = {
  primaryActions: ProductAction[]
  reviewActions: ReviewAction[]
  disabledDangerousActions: ProductAction[]
  relatedTaskRef: BackendEntityRef | null
  refreshRequired: boolean
  refreshReason?: string
}

type WorkspaceResult = {
  finalDeliveryId: string
  status: 'completed' | 'completed_with_pending_items' | 'blocked' | 'failed' | 'cancelled'
  headline: string
  answerPreview: string
  completedActions: string[]
  pendingUserActions: string[]
  durableChanges: Array<{
    label: string
    materializationStatus: ReviewItemMaterializationStatus
    evidenceRefs: EvidenceRef[]
  }>
  blockers: BlockerSummary[]
  nextSteps: string[]
  traceAvailable: boolean
  evidenceRefs: EvidenceRef[]
}

type BlockerSummary = {
  id: string
  category:
    | 'missing_context'
    | 'waiting_review'
    | 'waiting_permission'
    | 'safe_mode'
    | 'provider_unavailable'
    | 'tool_unavailable'
    | 'policy_denied'
    | 'external_write_blocked'
    | 'stale_task_context'
    | 'materialization_failed'
    | 'unknown_failure'
  title: string
  detail: string
  recoverable: boolean
  recommendedAction: ProductAction | ReviewAction | null
  evidenceRefs: EvidenceRef[]
}

type WorkspaceToolSummary = {
  attemptedCount: number
  succeededCount: number
  waitingPermissionCount: number
  blockedCount: number
  failedCount: number
  highestRisk: RiskLevel | 'none' | 'unknown'
  permissionReviewItemRefs: ReviewItemRef[]
  evidenceRefs: EvidenceRef[]
}
```

## State Model

| State | Meaning | Required behavior |
| --- | --- | --- |
| `idle` | No active task. | Show composer, no readiness overclaim. |
| `loading` | Loading workspace state. | No local reconstruction from ChatPage state. |
| `understanding` | OpenLife is interpreting the goal. | Do not imply durable action started. |
| `planning` | OpenLife is forming a plan. | Do not imply plan approved. |
| `running` | Work is executing. | External writes and durable writes still require review/permission. |
| `waiting_permission` | User confirmation is required. | Link ReviewItem; disable hidden completion. |
| `blocked` | Fail-closed blocker exists. | Show blocker and recovery action. |
| `failed` | Work failed. | Show reason, retry eligibility, evidence refs. |
| `cancelled` | User/system cancelled. | No retry unless backend offers it. |
| `completed` | Work completed. | Must not include pending durable changes. |
| `completed_with_pending_items` | Output exists and review remains. | Show review refs; do not claim durable apply. |

## Blocker / Failure Taxonomy

`PHASE_2_REQUIRED`: Backend should classify blockers at least as:

- missing context;
- waiting review;
- waiting permission;
- safe mode;
- provider unavailable;
- tool unavailable;
- policy denied;
- external write blocked;
- stale task context;
- materialization failed;
- unknown failure.

## Action Split

`ProductAction`: start, continue, retry, cancel, refresh, inspect evidence, open task.

`ReviewAction`: open/act on related ReviewItem; actual approve/reject/edit/later remains Review Center authority.

`DebugAction`: raw trace, kernel events, durable events, transcript, provider health, route evidence.

## UI Cannot Infer

`PHASE_2_REQUIRED`: Workspace UI cannot infer canonical intent, route/privacy boundary, lifecycle state, blocker category, proposal/materialization state, allowed controls, or final result status from raw fragments.

## Empty / Error / Stale Behavior

`DESIGN_DECISION`: Empty means no active task; show composer and optional recent context only if backend provides it.

`DESIGN_DECISION`: Error means no fabricated timeline. Show retry and preserve debug-only raw data only in advanced inspection if available.

`DESIGN_DECISION`: Stale means refresh before resume/retry/cancel/approve.

## Tests Needed

- Backend contract tests for lifecycle/final-delivery mapping.
- Event-to-timeline fixture tests.
- Review ref linkage tests.
- Provider/privacy boundary tests.
- Stale-state disables risky actions tests.
- Static frontend guard banning Workspace product truth reconstruction from raw ChatPage state.

## Implementation Stop Rules

`PHASE_2_REQUIRED`: Do not build or rename a Workspace route until this owner is approved or implemented.

`PHASE_2_REQUIRED`: Do not refactor ChatPage into V2 by preserving the same local-state responsibilities.
