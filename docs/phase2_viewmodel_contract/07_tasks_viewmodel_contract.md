# TasksViewModel Contract

Status: proposed contract. Do not implement in Phase 2.

## Purpose

`DESIGN_DECISION`: `TasksViewModel` is the backend-owned read model for `任务`: active tasks, historical tasks, task/run relationships, lifecycle, latest result preview, blocker/review counts, next action, detail, evidence, deletion danger preflight, retry/resume/cancel controls.

Backend owner: Proposed `TasksViewModel`
Owner status: `PHASE_2_REQUIRED`
Required validation: Phase 3 must resolve the canonical relationship between `AgentRun` and Main Chat task session before V2 Tasks implementation.

## Existing Support

`EXISTING_CODE`: Existing bridge contracts include `AgentRun`, `listAgentRuns`, `listAgentRunsForSession`, `MainChatTaskSummary`, `MainChatTaskDetail`, `listMainChatAgentTasks`, `getMainChatAgentTaskDetail`, `RunEvidenceView`, task controls, and danger preflight for deletion.

`VERIFIED_FACT`: Current Runs page locally merges AgentRun and Main Chat task summaries.

## Canonical AgentRun / Main Chat Task Relationship

`PHASE_2_REQUIRED`: The canonical relationship is unresolved for product contract purposes.

Current evidence:

- `AgentRun` has `id`, `taskId`, optional `sessionId`, status, kind, model route, actions, observations, generated proposals, and timestamps.
- `MainChatTaskSummary` has `taskSessionId`, `conversationId`, `runId`, lifecycle/status, blocker/proposal counts, next recommended control, stale state, and optional `RunEvidenceView`.
- `MainChatTaskDetail` has task session, actions, transcript, proposals, blockers, final delivery, continuity diagnostics, allowed controls, and evidence view.

Contract requirement:

`PHASE_2_REQUIRED`: TasksViewModel must make one backend-owned merged task identity and treat raw AgentRun-only or task-only rows as partial/debug-only unless backend marks them product-safe.

## Required Field Contract

| Field | Type | Required | Source of truth | Owner status | Evidence | Frontend may infer? | Empty behavior | Error behavior | Stale behavior | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `activeTasks` | `TaskListItem[]` | Yes | Tasks read model | `PHASE_2_REQUIRED` | Task summaries exist | No | Empty active list | Error envelope | Disable controls | Task refs |
| `historicalTasks` | `TaskListItem[]` | Yes | Tasks read model | `PHASE_2_REQUIRED` | AgentRun/history exists | No | Empty history | Error envelope | Mark stale | Run/task refs |
| `canonicalTaskId` | string | Yes per item | Tasks read model | `PHASE_2_REQUIRED` | AgentRun/task ids partial | No | Item omitted | Error item | Disable controls | Canonical id audit |
| `agentRunRef` | `TaskEntityRef \| null` | Conditional | AgentRun store | `PARTIAL` | AgentRun exists | No | `null` if no run | Error warning | Mark stale | Run ref |
| `mainChatTaskRef` | `TaskEntityRef \| null` | Conditional | Main Chat task store | `PARTIAL` | Task sessions exist | No | `null` if no task | Error warning | Mark stale | Task ref |
| `lifecycleStatus` | `TaskLifecycleStatus` | Yes | Tasks read model | `PHASE_2_REQUIRED` | Statuses exist separately | No | Not shown | Error item | Disable controls | Lifecycle refs |
| `latestResultPreview` | `TaskResultPreview \| null` | Yes | FinalDelivery/evidence owner | `PHASE_2_REQUIRED` | Final delivery partial | No | `null` | Error warning | Mark stale | Result refs |
| `blockerCount` | number | Yes | Tasks read model | `PHASE_2_REQUIRED` | Summaries/evidence partial | No | 0 if backend says | Error item | Stale count warning | Blocker refs |
| `reviewCount` | number | Yes | Review/Tasks read model | `PHASE_2_REQUIRED` | Proposal counts partial | No | 0 if backend says | Error item | Stale count warning | Review refs |
| `nextRecommendedAction` | `ProductAction \| null` | Yes | Tasks read model | `PHASE_2_REQUIRED` | `nextRecommendedControl` exists | No | `null` | Retry refresh only | Disable action | Action refs |
| `taskDetail` | `TaskDetailSummary` | Detail view | Tasks read model | `PHASE_2_REQUIRED` | `MainChatTaskDetail` exists | No | Empty detail unavailable | Error detail | Disable controls | Detail refs |
| `evidenceRefs` | `EvidenceRef[]` | Yes | Evidence/audit stores | `PHASE_2_REQUIRED` | RunEvidenceView exists | No | Empty with warning | Error warning | Mark stale | Evidence refs |
| `deletionPreflight` | `TaskDeletionPreflight \| null` | Conditional | Danger preflight owner | `PARTIAL` | Danger preflight exists | No | No delete action | Error warning | Disable delete | Preflight audit |
| `resumeRetryCancelActions` | `ProductAction[]` | Yes | Tasks read model | `PHASE_2_REQUIRED` | Task controls exist | No | Empty | Retry refresh only | Disable all | Control refs |

## Tasks Nested Contract Types

`PHASE_2_REQUIRED`: These types define the target read-model contract. Raw `AgentRun` and raw Main Chat task rows are not substitutes for `TaskListItem`.

```ts
type TaskEntityRef = {
  id: string
  kind: 'agent_run' | 'main_chat_task' | 'conversation' | 'final_delivery' | 'evidence'
  label: string
  href?: string
  evidenceRefs: EvidenceRef[]
}

type TaskLifecycleStatus =
  | 'running'
  | 'waiting_permission'
  | 'blocked'
  | 'completed'
  | 'completed_with_pending_items'
  | 'failed'
  | 'cancelled'
  | 'stale'
  | 'deleted'
  | 'archived'
  | 'unknown'

type TaskListItem = {
  canonicalTaskId: string
  title: string
  agentRunRef: TaskEntityRef | null
  mainChatTaskRef: TaskEntityRef | null
  lifecycleStatus: TaskLifecycleStatus
  latestResultPreview: TaskResultPreview | null
  blockerCount: number
  reviewCount: number
  nextRecommendedAction: ProductAction | null
  resumeRetryCancelActions: ProductAction[]
  evidenceRefs: EvidenceRef[]
}

type TaskResultPreview = {
  finalDeliveryRef: TaskEntityRef | null
  status: 'completed' | 'completed_with_pending_items' | 'blocked' | 'failed' | 'cancelled' | 'unknown'
  headline: string
  preview: string
  pendingReviewItemRefs: ReviewItemRef[]
  evidenceRefs: EvidenceRef[]
}

type TaskDetailSummary = {
  canonicalTaskId: string
  transcriptAvailable: boolean
  timelineRefs: BackendEntityRef[]
  blockerRefs: BackendEntityRef[]
  reviewItemRefs: ReviewItemRef[]
  finalDelivery: TaskResultPreview | null
  continuityStatus: 'current' | 'stale' | 'requires_refresh' | 'unknown'
  allowedControls: ProductAction[]
  evidenceRefs: EvidenceRef[]
}

type TaskDeletionPreflight = {
  eligible: boolean
  disabledReason?: string
  affectedItemCount: number
  scopeDigest: string
  privacySensitivity: 'low' | 'medium' | 'high' | 'unknown'
  backupStatus: 'not_needed' | 'available' | 'missing' | 'unknown'
  confirmationPhrase: string | null
  blockedBySafeMode: boolean
  evidenceRefs: EvidenceRef[]
}
```

## Lifecycle Status

`DESIGN_DECISION`: Tasks must preserve at least:

- running;
- waiting_permission;
- blocked;
- completed;
- completed_with_pending_items;
- failed;
- cancelled;
- stale;
- deleted/archived if backend supports it.

## Delete / Danger Preflight Contract

`PHASE_2_REQUIRED`: Deletion eligibility, affected item count, scope digest, privacy sensitivity, backup status, confirmation phrase, and safe-mode blocking must come from danger preflight/read model. The page cannot compute them locally.

## UI Cannot Infer

`PHASE_2_REQUIRED`: Tasks UI cannot infer merged lifecycle, stale classification, deletion safety, next recommended action, blocker category, or review count from raw AgentRun plus task fragments.

## Empty / Error / Stale Behavior

`DESIGN_DECISION`: Empty means no active or historical tasks. Route user to `工作区`.

`DESIGN_DECISION`: Error means no local reconstruction from partial run lists.

`DESIGN_DECISION`: Stale means resume/retry/cancel/delete actions disabled until refresh.

## Tests Needed

- Backend merged AgentRun/task identity tests.
- Lifecycle mapping tests.
- Stale task control-disable tests.
- Danger preflight fixture tests.
- Review/blocker count source tests.
- Frontend contract tests banning local lifecycle merge for product truth.

## Implementation Stop Rules

`PHASE_2_REQUIRED`: Do not implement V2 `任务` while AgentRun/Main Chat task relationship remains page-local or ambiguous.
