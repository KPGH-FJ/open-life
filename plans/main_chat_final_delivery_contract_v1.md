# Main Chat Final Delivery Contract v1

> Date: 2026-06-16
> Status: required preparation artifact before Main Chat Agent Productization v1
> Parent: `plans/openlife_agent_product_capability_matrix_v1.md`

## 1. Purpose

An excellent Agent does not merely reply. It delivers a clear task result.

This document defines the final delivery object and completion rules for Main
Chat Agent Productization v1.

## 2. Final Delivery Definition

Final delivery is the terminal product object that tells the user:

- what was actually done
- which tools/actions ran
- what observations/sources were used
- what proposals were created
- what was blocked or skipped
- what remains pending
- what the user can do next

A plain assistant message is not enough for tool, plan, proposal, permission, or
long-running tasks.

## 3. Delivery Status

| Status | Meaning | Allowed wording |
| --- | --- | --- |
| `completed` | All required supported work executed and no pending user action remains. | "Done", "Completed", "Here is the result". |
| `completed_with_pending_items` | Executed work completed, but proposals/permissions/follow-ups remain. | "I completed X; Y is pending review". |
| `blocked` | Task cannot continue until user/environment changes. | "Blocked because..." |
| `failed` | Runtime attempted and failed without safe recovery. | "Failed because..." |
| `cancelled` | User cancelled and pending work stopped. | "Cancelled; no further actions will run". |

The UI must not use `completed` when work was only proposed, blocked, skipped, or
not attempted.

## 4. Canonical FinalDeliveryView

This schema is the single source of truth for final delivery. Other documents
may reference or derive compact views from it, but they must not redefine a
different final delivery object.

```ts
type CanonicalFinalDeliveryView = {
  deliveryId: string;
  taskId: string;
  runId: string;
  status:
    | "completed"
    | "completed_with_pending_items"
    | "blocked"
    | "failed"
    | "cancelled";
  headline: string;
  answer: string;
  completedActions: CompletedActionSummary[];
  observationsUsed: ObservationSummary[];
  proposalsCreated: ProposalSummary[];
  blockers: BlockerSummary[];
  pendingUserActions: PendingUserActionSummary[];
  durableChanges: DurableChangeSummary[];
  nextSteps: string[];
  traceAvailable: boolean;
};
```

Required sections may be hidden when empty, but the object must preserve them for
audit and tests.

## 5. Section Contracts

### 5.1 Completed Actions

Must include:

- action id
- action type
- tool/source label
- target
- status
- observation ids

Only succeeded actions appear here. Failed, blocked, skipped, or cancelled
actions go to their own sections.

### 5.2 Observations Used

Must include:

- observation id
- source kind
- source label
- bounded preview
- citation/reference if available

Final answer cannot claim a source was used unless it references an observation.

### 5.3 Proposals Created

Must include:

- proposal id
- proposal type
- status
- summary
- link to review

Proposal created is not durable change completed.

### 5.4 Blockers

Must include:

- blocker id
- reason code
- affected action or step
- whether user can resolve it
- valid next controls

If blockers remain, status cannot be plain `completed`.

### 5.5 Pending User Actions

Must include:

- permission requests
- proposal review
- missing input
- plan confirmation
- follow-up decisions

If pending user actions remain, status should be `completed_with_pending_items`
or `blocked`.

### 5.6 Durable Changes

Must include:

- change type
- target
- proposal/permission provenance
- timestamp
- rollback availability

No durable memory/LifeModel/file/external change may appear without provenance.

## 6. Completion Rules By Strategy

The table below uses scenario shorthand. Machine-readable eval fixtures must map
these names to canonical strategy routes from
`main_chat_agent_control_plane_ui_contract_v1.md`.

| Strategy | Completion requirement |
| --- | --- |
| DirectAnswer | Final answer plus task/run trace. No action sections required. |
| ReadAction | At least one succeeded action and observation before final answer. |
| ReAct | All required actions accounted for as completed, blocked, failed, or skipped. |
| PlanExecute | Plan steps have terminal state and review summary exists. |
| MemoryProposal | Proposal listed as pending/accepted/rejected; memory not claimed complete unless accepted. |
| PermissionRequest | Pending permission or exact resumed action outcome is shown. |
| Blocker | Blocker reason and valid next controls are shown. |
| Cancelled | Pending work stopped and cancellation is visible. |

## 7. Negative Assertions

Final delivery must not:

- say "done" when only a plan was drafted
- say "done" when only a proposal was created
- hide failed actions
- hide blockers
- cite sources not present in observations
- claim memory was updated before acceptance
- claim external write happened without confirmation evidence
- omit pending user action when task waits on user
- merge executed and proposed work into one vague summary

## 8. UI Rendering

Recommended order:

1. Headline status.
2. Final answer/result.
3. Completed actions.
4. Sources/observations.
5. Proposals created.
6. Blockers or failed items.
7. Pending user actions.
8. Next steps.
9. Trace expansion.

For simple DirectAnswer, sections 3 to 8 may be collapsed or absent.

For tool/plan/proposal tasks, at least one structured section must appear below
the answer.

## 9. Acceptance

The final delivery contract is satisfied when tests prove:

- every completed read/tool task includes completed action and observation
- every proposal task lists proposal status
- every blocked task lists blocker and next control
- every permission task distinguishes pending permission from executed action
- every final answer with sources references observation ids
- no scenario uses completed status for proposed, blocked, skipped, failed, or
  unexecuted work
