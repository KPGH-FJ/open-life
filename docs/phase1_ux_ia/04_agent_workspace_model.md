# Agent Workspace Model

Status: Phase 1 workspace model proposal.
Scope: Product model and ViewModel requirements only. No ChatPage refactor.

## Classification Legend

- `VERIFIED_FACT`
- `DESIGN_DECISION`
- `DESIGN_ASSUMPTION`
- `CANDIDATE`
- `UNKNOWN`
- `PHASE_2_REQUIRED`

## Purpose

`DESIGN_DECISION`: `工作区` is where the user and OpenLife complete current work together. It is not a renamed ChatPage and not only a conversation transcript.

`VERIFIED_FACT`: Current Chat/Companion already contain user input, task state, streaming, final delivery, tool calls, proposals, diagnostics, LifeModel context, daily goals, runs, and controls. Source: `docs/phase0_5/03_chat_companion_workspace_mapping.md`.

## Required Zones

```text
Workspace
├── Intent Composer
├── Understanding Panel
├── Execution Timeline
└── Control / Review Drawer
```

## Zone Specs

### Intent Composer

Purpose: `DESIGN_DECISION` - Let the user state a goal, optionally attach skill/context, and correct scope before consequential work.
Default state: calm input area with current context and privacy boundary summary.
User actions: start, clarify, attach capability/context, stop draft.
Must not: present skill/tool selection as the primary mental model or imply write authorization from prose.

### Understanding Panel

Purpose: `DESIGN_DECISION` - Show "OpenLife 理解为..." before or during execution.
Displays: user goal, inferred intent, route/policy summary, privacy boundary, missing context, confidence/uncertainty, selected capability.
Missing context: ask user, block, or proceed with visible assumptions.
User correction: `PHASE_2_REQUIRED` - editable/confirmable intent frame requires backend/read-model validation.

### Execution Timeline

Purpose: `DESIGN_DECISION` - Replace raw trace-first UX with a staged user-readable execution timeline.
Displays: understanding, plan, tool/skill action summaries, blocker/waiting states, proposal creation, final result, evidence refs.
States: idle, understanding, planning, running, waiting_permission, blocked, failed, cancelled, completed, completed_with_pending_items.
Evidence refs: link to advanced inspector without making raw events default.

### Control / Review Drawer

Purpose: `DESIGN_DECISION` - Keep current work controls and linked review items close to the task without making Workspace the review authority.
Displays: continue/resume, cancel, retry, inspect evidence, open review item, clarify, view related task.
Links to Review Center: proposals, permissions, external writes, memory updates, LifeModel changes, policy changes, dangerous actions.
Actions: product actions by default; review actions only when acting on a Review Center item; debug actions only in advanced inspection.

## Core Objects

| Object | Classification | Definition | Backend status |
| --- | --- | --- | --- |
| User goal object | `DESIGN_DECISION` | Current user goal, source prompt, context, constraints, and selected capability hints. | `PHASE_2_REQUIRED` for canonical WorkspaceViewModel field. |
| Agent understanding object | `CANDIDATE` | Editable/confirmable interpretation of goal, intent, policy route, privacy boundary, and uncertainty. | Existing `IntentFrame` / route evidence is partial; editable UI contract is `PHASE_2_REQUIRED`. |
| Plan/lifecycle object | `CANDIDATE` | Current task plan, lifecycle state, next action, and whether user review is required. | Plan/task primitives exist; unified workspace summary is `PHASE_2_REQUIRED`. |
| Execution timeline | `CANDIDATE` | User-readable stages derived from kernel events, durable events, final delivery, tool calls, and blockers. | Events exist; default timeline stage model is `PHASE_2_REQUIRED`. |
| Review links | `DESIGN_DECISION` | Pointers to `审核中心` items; Workspace does not own final approval state. | Related task/proposal evidence exists; unified ReviewItem refs are `PHASE_2_REQUIRED`. |
| Result object | `DESIGN_DECISION` | Final answer plus completed actions, blockers, pending items, durable changes, and next control. | Final delivery primitives exist; UI summary contract needs validation. |
| Evidence drawer | `DESIGN_DECISION` | Collapsed user-accessible evidence, source refs, tool summary, and privacy/provider boundary. | Existing components support evidence, but V2 drawer contract is `PHASE_2_REQUIRED`. |
| Advanced inspector | `DESIGN_DECISION` | Raw reasoning trace, kernel events, durable stream, transcript, tool metadata, provider/router diagnostics. | Existing evidence exists; visibility/gating policy is `PHASE_2_REQUIRED`. |

## State Model

| State | Default user meaning | Must not imply / 不得暗示 |
| --- | --- | --- |
| idle | 可以开始新的工作 | 产品 readiness / trial 已经是 green。 |
| loading | 正在读取当前状态 | 后端读取失败时仍然有当前数据。 |
| understanding | OpenLife 正在理解任务 | 已经开始 durable action。 |
| planning | 正在形成执行计划 | 计划已经获批。 |
| running | 正在执行 | 外部写入或 durable write 已经获授权。 |
| waiting_permission | 等待你确认 | 任务已经失败。 |
| blocked | 已阻断，需要处理原因 | 工作已经完成。 |
| failed | 失败，有可解释原因或重试路径 | 用户已经批准不安全动作。 |
| cancelled | 已取消 | 任务失败。 |
| completed | 已完成 | 待确认项已经应用。 |
| completed_with_pending_items | 已完成，但有待确认项 | durable change 已经完成。 |

## Composer Behavior

- `DESIGN_DECISION`: Composer should keep the user's goal visible during execution.
- `DESIGN_DECISION`: Consequential writes must become Review Center items or blockers, not hidden side effects.
- `DESIGN_ASSUMPTION`: Skill selection can remain secondary because normal users think in goals, not tool names.
- `PHASE_2_REQUIRED`: Define backend-supported draft/session persistence before implementing composer continuity.

## ChatPage Responsibility Migration

| Existing ChatPage responsibility | V2 destination | Reason |
| --- | --- | --- |
| user input | 工作区 | Composer is the primary current-work input. |
| natural language intent | 工作区 | Intent should be visible and correctable. |
| skill selection | 工作区 / 高级检查 | Goal-first default; capability details can be secondary. |
| task session | 工作区 / 任务 | Current task in Workspace, history/detail in Tasks. |
| task resume | 工作区 / 任务 / 审核中心 | Resume may follow review approval or task detail. |
| task cancel | 工作区 / 任务 | User control belongs near active and historical tasks. |
| retry | 工作区 / 任务 | Retry belongs near failed action/task state. |
| reasoning trace | 高级检查 | Raw trace is not default product comprehension. |
| kernel events | 高级检查 / collapsed timeline details | Preserve evidence without default raw event UI. |
| durable agent events | 工作区 timeline / 高级检查 | Use summary by default, full stream on inspection. |
| tool calls | 工作区 summary / 高级检查 detail | Summary supports trust; internals are advanced. |
| blockers | 工作区 / 任务 | Blocked state must be default-visible and fail-closed. |
| generated proposals | 审核中心 | Review Center owns decisions. |
| pending review | 审核中心 / global summary | Counts and decisions must not be page-local truth. |
| final delivery | 工作区 result / 任务 detail | Current result in Workspace; durable evidence in Tasks. |
| run history | 任务 | Historical work should not clutter current Workspace. |
| execution transcript | 高级检查 / 任务 detail | Full transcript is evidence, not default output. |
| memory impact | 审核中心 / 记忆 / LifeModel | Decisions in Review Center; resulting state in Memory/LifeModel. |
| LifeModel impact | 审核中心 / LifeModel | Pending change review separate from canonical understanding. |
| diagnostics/provider/router status | 高级检查 / 设置 | Default Workspace shows trust summary only. |
| old reply-only chat wrapper | 删除/隐藏 | V2 should consume structured send/stream results. |

## Scenario Coverage

## Scenario S1: Plan today's priorities

User goal:
Plan today's priorities without turning the product into a todo app.

Entry surface:
`今日` or `工作区`.

Surfaces involved:
`今日`, `工作区`, `任务`, possibly `审核中心`.

Default UI:
Composer plus concise understanding and proposed plan timeline.

System understanding:
OpenLife should identify a planning task, current context, constraints, and whether memory/LifeModel context is being used.

Execution timeline:
Understand goal, gather local context, draft priorities, surface blockers, present result.

Review Center trigger:
Only if the plan proposes durable Memory/LifeModel changes or external writes.

Task state:
running, blocked, failed, completed, or completed_with_pending_items.

LifeModel / Memory impact:
Context use is allowed; durable updates require lane policy and review threshold.

Diagnostics visibility:
Default timeline; raw route/tool trace in advanced inspector.

Required ViewModel fields:
goal, understanding, timeline stages, blockers, pending review refs, result, evidence refs.

Failure / empty state:
If context/read model is unavailable, show stale/error state and ask user to proceed without claiming complete context.

Success criteria:
User understands the plan, blockers, and whether anything was remembered or proposed.

Evidence classification:
`CANDIDATE`; planning primitives exist but full desktop planning journey is not verified.

Open questions:
What daily context belongs in the default plan?

## Scenario S2: Execute a task requiring external write

User goal:
Ask OpenLife to complete a task that would write to an external or sensitive target.

Entry surface:
`工作区`.

Surfaces involved:
`工作区`, `审核中心`, `任务`, `设置`.

Default UI:
Workspace shows intent and a blocked/waiting confirmation state.

System understanding:
OpenLife should identify the action, target, risk, privacy boundary, and required confirmation.

Execution timeline:
Understand goal, prepare action, stop before write, create review item, resume only after approval.

Review Center trigger:
External write, dangerous action, or permission request.

Task state:
waiting_permission or blocked until reviewed.

LifeModel / Memory impact:
No durable Memory/LifeModel update unless separately proposed.

Diagnostics visibility:
Default shows risk and target summary; raw payload/trace is advanced.

Required ViewModel fields:
risk, target, reviewItemRef, allowed actions, resume relation, evidence refs.

Failure / empty state:
If safe-path or permission validation fails, show blocker and no write completed.

Success criteria:
User can approve/reject/modify before any consequential write.

Evidence classification:
`VERIFIED_FACT` for safe-path/danger primitives; unified ReviewItem is `PHASE_2_REQUIRED`.

Open questions:
Which external writes are supported in first V2?

## Scenario S3: Candidate memory requires confirmation

User goal:
Let OpenLife remember something only if it is appropriate.

Entry surface:
`工作区` or `记忆`.

Surfaces involved:
`工作区`, `审核中心`, `记忆`, `LifeModel`.

Default UI:
Workspace shows a memory candidate preview and opens Review Center for decision.

System understanding:
OpenLife should distinguish context-only information, candidate memory, and canonical LifeModel change.

Execution timeline:
Detect candidate, classify lane/risk, create review item, wait for decision, reflect resulting state.

Review Center trigger:
memory_update.

Task state:
completed_with_pending_items or waiting_permission.

LifeModel / Memory impact:
Candidate memory does not become confirmed memory until approved or allowed by a documented low-risk lane.

Diagnostics visibility:
Lane/status/provenance visible; raw memory row/vector details advanced.

Required ViewModel fields:
candidate text, lane, confidence, risk, source, review actions, resulting status.

Failure / empty state:
If lane cannot be classified, ask user or mark unknown; do not silently remember.

Success criteria:
User sees what might be remembered and controls the outcome.

Evidence classification:
`VERIFIED_FACT` for MemoryGateway/lifecycle primitives; lane-level UI read model is `PHASE_2_REQUIRED`.

Open questions:
Which low-risk memory lanes can bypass Review Center?

## Scenario S4: Update long-term LifeModel preference

User goal:
Update how OpenLife understands a long-term preference.

Entry surface:
`工作区`, `LifeModel`, or `审核中心`.

Surfaces involved:
`工作区`, `审核中心`, `LifeModel`, `记忆`.

Default UI:
Show proposed change, source, affected LifeModel area, and review actions.

System understanding:
OpenLife should identify that this is a canonical long-term understanding change, not just conversation context.

Execution timeline:
Capture source, draft change, create Review Center item, apply only after approval/materialization.

Review Center trigger:
lifemodel_change.

Task state:
completed_with_pending_items until materialized; completed only after durable apply.

LifeModel / Memory impact:
Preference may create memory evidence and LifeModel change; both need provenance.

Diagnostics visibility:
Default shows source and impact; patch/provenance internals advanced.

Required ViewModel fields:
before/after summary, evidence, source refs, risk, status, materialization state.

Failure / empty state:
If provenance is missing, mark open item and avoid claiming canonical update.

Success criteria:
User can distinguish proposed, approved, and applied LifeModel states.

Evidence classification:
`VERIFIED_FACT` for LifeModel/provenance primitives; V2 LifeModelViewModel is `PHASE_2_REQUIRED`.

Open questions:
Should manual LifeModel editing remain, and how is it labeled?

## Scenario S5: Tool call fails without raw trace

User goal:
Understand why a tool action failed without reading raw trace.

Entry surface:
`工作区` or `任务`.

Surfaces involved:
`工作区`, `任务`, `高级检查`, possibly `设置`.

Default UI:
Concise failed/blocker state, reason, next action, and optional inspection link.

System understanding:
OpenLife should explain whether the issue was permission, manifest, policy, provider, path, or runtime failure.

Execution timeline:
Show attempted action, failure reason, blocked/failed status, retry or review path.

Review Center trigger:
Only if failure can be resolved by permission/review; otherwise show retry/fix path.

Task state:
failed or blocked, not completed.

LifeModel / Memory impact:
No durable update unless a separate review item exists.

Diagnostics visibility:
Default reason and next action; raw trace/tool metadata advanced.

Required ViewModel fields:
failure category, user-readable reason, retry eligibility, debug evidence ref, related permission state.

Failure / empty state:
If reason unknown, say unknown and expose advanced evidence; do not invent cause.

Success criteria:
User understands what happened and what they can do next.

Evidence classification:
`VERIFIED_FACT` for ToolGateway/fail-closed primitives; concise failure taxonomy is `PHASE_2_REQUIRED`.

Open questions:
What failure taxonomy should be canonical?

## Human Decisions Needed

1. Whether companion/ambient mode remains as a sub-mode inside `工作区`.
2. Which task controls appear in Workspace versus `任务`.
3. Whether skill selection is visible by default.
4. What evidence must always be visible before raw traces are hidden.
5. Which workspace fields must be backend-owned before implementation.
