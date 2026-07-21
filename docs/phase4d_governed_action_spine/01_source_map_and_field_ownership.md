# Phase 4D Governed-Action Source Map And Field Ownership

Status: `IMPLEMENTED`
Date: 2026-07-20

## Runtime Path

```text
dev-only Phase 4D Tauri/browser entry
  -> ReadOnlySpineJourney (one Shell composition owner)
  -> useGovernedActionJourney
      -> GovernedActionDataSource.load
          -> get_workspace_view_model
          -> get_review_center_view_model
          -> get_tasks_view_model
      -> dispatch ReviewAction or exact TaskControl
      -> reload all three read models
      -> verify same review/task target
  -> WorkspaceGovernedView or ReviewGovernedView
  -> OpenLifeWorkbenchShell + structured Inspector
```

`tauriGovernedActionDataSource` invokes existing commands. Browser fixtures
implement the same interface but are selected only in the QA toolbar outside
the Shell.

## Workspace Field Sources

| Visible fact | ViewModel field | Rendering rule |
| --- | --- | --- |
| current task title | `WorkspaceViewModel.activeTask.title` | no recent-task fallback |
| lifecycle label | `activeTask.lifecycleStatus`, terminal delivery fields | running and waiting are not completion |
| current blocker | linked `pendingReviewItems[].decisionContext` or `activeTask.pendingBlockers` | show one primary blocker, no count dashboard |
| permission entry | linked `pendingReviewItems` with `type=tool_permission` | navigation only; viewing records no decision |
| resume button | exact `activeTask.allowedControls[]` | requires `resume + task_resume_request + exact taskSessionId` |
| next step | pending item or exact enabled resume control | no page-local action synthesis |
| activity list | `WorkspaceViewModel.activity` | metadata-only labels/summaries; no transcript reconstruction |
| local/private boundary | `providerPrivacyBoundarySummary` plus Workspace envelope status | stale/error/missing cannot render verified green |
| evidence | Workspace/task/review/activity/boundary `EvidenceRef` values | preserve id, label, source, sensitivity |

The Workspace surface contains no metric cards and no fixture-derived product
counts. Its fixed priority is current task, blocker, next action, evidence, and
recent metadata activity.

## Review Field Sources

| Visible fact | ReviewItem field | Rendering rule |
| --- | --- | --- |
| title and one-line change | `decisionContext.title/summary` | product language, no DTO labels |
| current -> proposed | permission policy or `decisionContext.before/after` | missing before remains explicitly unavailable |
| tool/capability/target | `decisionContext.permission` | no inference from tool name or proposal body |
| purpose and impact | `reasonSummary`, `impactSummary` | backend-projected explanation only |
| transmission boundary | `permission.transmissionBoundary` | possible/unknown stays protective, never green |
| validity/revocation | `policy`, `expiresAt`, `revocationSummary` | incomplete fields disable approval |
| source and affected object | `sourceSummary`, `affectedObjectLabels`, `source` | raw IDs remain in Inspector |
| decision status | `ReviewItem.status` | approved permission is labelled not resumed |
| materialization status | `materializationStatus` | approved is never collapsed into applied |
| decisions | `allowedActions` | only typed approve/reject/later are dispatched |
| evidence entry | `view_evidence` plus EvidenceRefs | opens Inspector; no command dispatch |

The current backend has no typed edit payload contract and no Review apply or
revoke command for this journey. Those controls are intentionally absent rather
than represented by fake buttons.

## Refresh And Resume Ownership

| Stage | Authority | Required check |
| --- | --- | --- |
| review request | `ReviewAction` | id, kind/effect, enabled, disabledReason, confirmation, target ReviewItem |
| review command | existing accept/reject/postpone command | callback only advances to refresh |
| decision proof | refreshed `ReviewCenterViewModel` | same ReviewItem id and expected decision status |
| resume availability | refreshed `WorkspaceViewModel.activeTask.allowedControls` | exact task/session id and resume effect |
| resume command | existing `resume_main_chat_agent_task` | callback only advances to refresh |
| running proof | refreshed `TasksViewModel.items` | exact canonicalTaskId and taskSessionId; lifecycle no longer waiting/blocked/unknown |
| completion proof | refreshed task terminal fields | `completed + delivered + finalDeliveryEvidencePresent` only |

All three governed read models are loaded together after a decision or resume.
A partial error becomes an error envelope and does not borrow truth from the
other two models.

## Local-Only UI State

The following values are presentation state, not product truth:

- active Shell destination and Settings utility context;
- selected ReviewItem id;
- Inspector open/closed state and selected EvidenceRef id;
- confirmation dialog state;
- loading/dispatch/refresh feedback;
- QA fixture selection outside the product Shell.

## Fixture Field Source Table

| Fixture content | Source classification |
| --- | --- |
| interview task, permission request, activity, evidence, and actions | typed layout/interaction fixture mirroring current DTO fields |
| pending -> approved -> running transitions | mutable QA state used only to verify frontend sequencing |
| local provider/privacy summary | static fixture, never backend proof |
| incomplete target/scope/expiry | static contract-negative fixture |
| Today goals and Tasks list outside the governed flow | previous Phase 4D layout fixtures |

The QA toolbar labels every fixture as non-backend state. No fixture value is
described as a real task, permission, provider route, or durable result.
