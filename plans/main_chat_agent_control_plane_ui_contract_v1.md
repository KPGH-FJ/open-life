# Main Chat Agent Control Plane UI Contract v1

> Date: 2026-06-16
> Status: required preparation artifact before Main Chat Agent Productization v1
> Parent: `plans/openlife_agent_product_capability_matrix_v1.md`

## 1. Purpose

Main Chat remains the user entry, but it must become an Agent control plane.
That means the UI renders real task/runtime state, not decorative "thinking"
content.

This document defines the product objects, UI states, controls, and rendering
rules required for the first implementation.

## 2. Core Principle

Every visible Agent claim must be backed by runtime evidence:

- A displayed task must map to a task/session/run id.
- A displayed action must map to ActionQueue or AgentLoop evidence.
- A displayed observation must map to an observation record.
- A displayed proposal must map to ProposalStore.
- A displayed memory change must map to accepted proposal provenance.
- A displayed final delivery must map to task finalization.

If evidence is missing, the UI must show unknown, pending, blocked, or failed.
It must not infer execution from assistant prose.

## 3. Product Objects

### 3.1 TaskSessionView

```ts
type TaskSessionView = {
  taskId: string;
  runId: string;
  conversationId: string;
  userMessageId: string;
  title: string;
  strategy: AgentStrategy;
  status: AgentTaskStatus;
  createdAt: string;
  updatedAt: string;
  traceAvailable: boolean;
  privacyRoute: PrivacyRouteSummary;
  contextSources: ContextSourceView[];
  plan?: PlanView;
  actions: ActionView[];
  observations: ObservationView[];
  blockers: BlockerView[];
  proposals: ProposalView[];
  finalDelivery?: FinalDeliveryView;
  controls: TaskControl[];
};
```

Required behavior:

- `taskId` and `runId` are mandatory for every non-legacy Agent path.
- `strategy` must be one of the router outputs, not inferred from message text.
- `status` must be derived from runtime state.
- `controls` must be computed from valid state transitions.

### 3.2 AgentStrategy

```ts
type AgentStrategy =
  | "direct_answer"
  | "read_action"
  | "react_tool_execution"
  | "plan_execute"
  | "memory_proposal"
  | "permission_request"
  | "task_control"
  | "blocked"
  | "legacy_fallback"
  | "unknown";
```

Rules:

- `legacy_fallback` must be visible in trace and should not be styled as normal
  completion.
- Explicit tool requests must not route to `direct_answer` unless the router
  explains that no tool is required.
- `task_control` is for user actions against an existing task, action,
  permission, or proposal, such as resume, retry, cancel, accept, reject, defer,
  or rollback. It must reference the prior object it controls.
- `unknown` is diagnostic-only and cannot be used to render a completed task.

### 3.2.1 Route / Capability / Delivery Vocabulary

These terms must not be collapsed:

| Concept | Purpose | Example values |
| --- | --- | --- |
| Strategy route | Runtime path selected for this task or control turn. | `direct_answer`, `read_action`, `react_tool_execution`, `plan_execute`, `memory_proposal`, `permission_request`, `task_control`, `blocked` |
| Capability group | Product/eval grouping. | Ordinary answer, file read, MCP read, ReAct, memory proposal, recovery, final delivery |
| Task status | Current lifecycle state. | `planning`, `executing`, `blocked`, `completed` |
| Delivery status | Final delivery outcome. | `completed`, `completed_with_pending_items`, `blocked`, `failed`, `cancelled` |

Scenario documents may use human-readable shorthand such as `DirectAnswer` or
`ReAct`; the machine-readable eval fixture must map each shorthand to one
canonical strategy route.

Recommended mapping:

| Scenario shorthand | Canonical strategy route |
| --- | --- |
| `DirectAnswer` | `direct_answer` |
| `ReadAction` | `read_action` |
| `ReAct` | `react_tool_execution` |
| `PlanExecute` | `plan_execute` |
| `Proposal` / `MemoryProposal` | `memory_proposal` |
| `Permission` / `PermissionRequest` | `permission_request` |
| `Cancel` / `Resume` / `Retry` | `task_control` |
| `AcceptProposal` / `RejectProposal` / `Rollback` | `task_control` |
| `Blocker` | `blocked` |
| `Recovery` | `task_control` for the recovery turn; resumed execution keeps its own strategy route |
| `Skill` | `react_tool_execution` or `read_action`, with selected skill metadata |
| `FinalDelivery` | `task_control` when the user asks to inspect or continue from terminal delivery; final delivery itself is a terminal object |

### 3.3 AgentTaskStatus

```ts
type AgentTaskStatus =
  | "classifying"
  | "answering"
  | "planning"
  | "waiting_for_user"
  | "queued"
  | "executing"
  | "observing"
  | "synthesizing"
  | "proposal_pending"
  | "blocked"
  | "failed"
  | "completed"
  | "cancelled";
```

Minimum state machine:

```text
classifying
  -> answering -> completed
  -> planning -> queued -> executing -> observing -> synthesizing -> completed
  -> planning -> waiting_for_user -> queued
  -> proposal_pending
  -> blocked
  -> failed
  -> cancelled
```

Invalid transitions:

- `proposal_pending -> completed` unless the final delivery clearly says the
  durable change is still pending review.
- `blocked -> completed` without blocker resolution evidence.
- `failed -> completed` without retry or valid fallback evidence.
- `cancelled -> executing`.

## 4. Object Views

### 4.1 PlanView

```ts
type PlanView = {
  planId: string;
  status: "draft" | "confirmed" | "executing" | "reviewing" | "completed" | "cancelled";
  steps: PlanStepView[];
  editable: boolean;
  source: "plan_execute" | "agent_loop" | "user_edited";
};
```

Step states:

- `planned`
- `confirmed`
- `running`
- `completed`
- `blocked`
- `skipped`
- `cancelled`

The UI must not render a draft plan as completed work.

### 4.2 ActionView

```ts
type ActionView = {
  actionId: string;
  actionType: string;
  target: string;
  label: string;
  status: "queued" | "running" | "succeeded" | "failed" | "blocked" | "cancelled";
  riskLevel: "safe_read" | "local_low_risk" | "proposal_first" | "external_confirm" | "dangerous_blocked";
  policyDecisionId?: string;
  startedAt?: string;
  finishedAt?: string;
  observationIds: string[];
  retryable: boolean;
};
```

Rules:

- An action with no runtime id cannot be displayed as executed.
- Read actions may auto-run only when policy allows.
- Write-like actions require proposal or confirmation state.

### 4.3 ObservationView

```ts
type ObservationView = {
  observationId: string;
  actionId: string;
  sourceKind: "file" | "memory" | "session" | "web" | "mcp" | "skill" | "provider" | "system";
  sourceLabel: string;
  preview: string;
  citationAvailable: boolean;
  createdAt: string;
};
```

Rules:

- Observation preview must be bounded.
- Web/file/MCP observations must show a source label.
- A final answer citing a source must reference an observation id.

### 4.4 BlockerView

```ts
type BlockerView = {
  blockerId: string;
  reasonCode: string;
  title: string;
  detail: string;
  affectedActionId?: string;
  recoverable: boolean;
  controls: TaskControl[];
};
```

Valid blocker classes:

- missing input
- permission required
- policy blocked
- network unavailable
- provider unavailable
- tool unavailable
- invalid model action
- stale context
- unsafe manifest
- outside workspace

### 4.5 ProposalView

```ts
type ProposalView = {
  proposalId: string;
  proposalType: "memory" | "lifemodel" | "tool_permission" | "task_followup" | "write_request";
  status: ProposalStatus;
  title: string;
  summary: string;
  evidenceIds: string[];
  controls: TaskControl[];
};
```

Rules:

- Proposal is not completion.
- Accepted proposal must be visible as accepted, not silently applied.
- Rejected proposal must not appear as durable state.
- `ProposalStatus` is defined canonically in
  `main_chat_permission_proposal_memory_ux_contract_v1.md`.

### 4.6 FinalDeliveryView

```ts
type FinalDeliveryView = CanonicalFinalDeliveryView;
```

Canonical schema lives in `main_chat_final_delivery_contract_v1.md`. The UI may
derive compact lists from that object, but the backend/frontend contract has one
source of truth.

Rules:

- `completed` requires at least one answer or concrete deliverable.
- `completed_with_pending_items` must separate executed work from pending review.
- `blocked` and `failed` must not use "done" language.

## 5. Controls

```ts
type TaskControl =
  | "continue"
  | "retry"
  | "cancel"
  | "approve_once"
  | "deny"
  | "defer"
  | "edit_plan"
  | "skip_step"
  | "accept_proposal"
  | "reject_proposal"
  | "edit_proposal"
  | "rollback"
  | "open_trace"
  | "open_review_center";
```

Control rules:

| State | Allowed controls |
| --- | --- |
| `answering` | cancel, open_trace |
| `planning` | edit_plan, cancel, open_trace |
| `waiting_for_user` | approve_once, deny, defer, cancel, open_trace |
| `queued` | cancel, open_trace |
| `executing` | cancel if action supports cancellation, open_trace |
| `blocked` | retry, continue, edit_plan, cancel, open_trace depending on reason |
| `failed` | retry if safe, cancel, open_trace |
| `proposal_pending` | accept_proposal, reject_proposal, edit_proposal, defer, open_review_center |
| `completed` | continue, open_trace, open_review_center if proposal exists |
| `cancelled` | open_trace |

Invalid controls must not render disabled unless they help explain state. Prefer
not rendering invalid controls.

## 6. Chat Integration

Main Chat should render:

- user message
- assistant response
- compact task status row
- expandable task panel
- final delivery block when task ends

For simple DirectAnswer:

- keep the UI compact
- show trace only behind expansion
- do not show fake action cards

For tool/plan/proposal tasks:

- show task panel by default
- show active action state while running
- keep final answer connected to observations and proposals

## 7. Streaming Rules

During streaming:

- `classifying` appears before route is known.
- `answering` can stream text.
- `planning` can stream plan draft only if plan object exists or is being built.
- `executing` must not stream fake observations.
- `observing` begins only after action result exists.
- `synthesizing` can stream final answer after observation.

If stream fails:

- preserve partial answer separately from final delivery
- show failure state
- offer retry only if runtime says retry is safe

## 8. Anti-fake UI Rules

- Do not display "reading file" without an action id.
- Do not display "searched web" without a web observation.
- Do not display "memory updated" without accepted proposal provenance.
- Do not display "tool selected" without candidate/selection evidence.
- Do not display "completed" when the task only created a proposal.
- Do not hide policy blockers inside generic apology text.
- Do not infer sources from model output.
- Do not show stale selected `SKILL.md` after user clears it.

## 9. Implementation Gate

The first UI implementation is acceptable only when:

- all product objects are typed in frontend
- all visible task states map to runtime evidence
- tests cover direct answer, read action, ReAct, proposal, permission, blocked,
  failed, cancelled, and completed states
- no fixture renders an action, observation, proposal, or final delivery without
  a corresponding evidence id
