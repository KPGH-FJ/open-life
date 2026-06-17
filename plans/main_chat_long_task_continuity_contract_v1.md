# Main Chat Long Task Continuity Contract v1

> Date: 2026-06-17
> Status: preparation artifact for Product Maturity v2
> Parent: `plans/main_chat_agent_product_maturity_v2_goal_spec.md`

## 1. Purpose

This document defines the product contract for finding, inspecting, and safely
continuing existing Main Chat Agent tasks.

Productization v1 added resume/retry/cancel for existing task states. Product
Maturity v2 must make long task continuity a visible product surface.

## 2. Baseline

OpenLife already has:

- AgentTaskSessionStore,
- ActionQueueStore,
- ExecutionTranscript,
- resume/retry/cancel commands,
- permission-preserving replay,
- terminal state guards,
- Productization v1 task-control scenarios.

Missing:

- task list,
- task detail surface,
- stale context detection surfaced to user,
- cross-session task discovery,
- "last done / next action" summary,
- continuity UX after app restart/navigation.

## 3. Benchmark Lessons

### Codex-style lesson

Agent work should be resumable through stable task/thread context. Users should
not need to reconstruct what happened.

### Hermes-style lesson

Blocked and waiting tasks should remain visible until resolved, not disappear
into chat history.

### OpenLife constraint

OpenLife must not resume tasks merely because a user said "continue". It must
validate task status, context freshness, and permission scope.

## 4. Task Continuity Objects

### 4.1 TaskSummary

Required fields:

- `taskSessionId`
- `conversationId`
- `runId`
- `title`
- `strategy`
- `status`
- `lastUpdatedAt`
- `lastObservationPreview`
- `pendingBlockerCount`
- `pendingProposalCount`
- `nextRecommendedControl`
- `staleState`
- `resumeSafetyDigest`

### 4.2 TaskDetail

Required fields:

- `taskSession`
- `actions`
- `transcript`
- `proposals`
- `blockers`
- `finalDelivery`
- `continuityDiagnostics`
- `allowedControls`
- `lastSafeResumePoint`
- `contextDigest`
- `selectedSkillDigest`
- `toolManifestDigest`

### 4.3 ContinuityDiagnostics

Required fields:

- `staleContext`
- `missingActionEvidence`
- `permissionScopeMismatch`
- `terminalNoResume`
- `providerUnavailable`
- `toolUnavailable`
- `requiresUserDecision`

## 5. Commands

Minimum commands:

- `list_main_chat_agent_tasks(filter, limit, offset)`
- `get_main_chat_agent_task_detail(taskSessionId)`
- `resume_main_chat_agent_task(taskSessionId)`
- `retry_main_chat_agent_action(taskSessionId, actionId)`
- `cancel_main_chat_agent_task(taskSessionId)`
- `refresh_main_chat_agent_task_context(taskSessionId)`

Existing commands may be reused, but list/detail/refresh need product-level
contracts.

## 6. Resume Rules

Resume is allowed only when:

- task exists,
- task is non-terminal or explicitly resumable,
- pending permission scope is still exact,
- context is fresh or user confirms refresh,
- selected tool/manifest still exists,
- action is replayable or next step can be safely planned.

Stale context detection must check at least:

- selected context source ids and digests,
- selected skill id and instruction digest,
- MCP/tool manifest digest and risk/action type,
- pending ToolPermission scope and action input hash,
- provider route availability for model-backed continuation,
- task age or last update threshold,
- plan/step revision if the task belongs to a plan,
- materialized memory view version if the task depends on memory context.

Resume is blocked when:

- task is completed without a new user goal,
- action target changed,
- permission proposal was denied,
- tool manifest disappeared,
- context source changed in a way that affects action safety,
- stale task needs user review first.

`refresh_main_chat_agent_task_context` may update stale diagnostics and
recommended next action, but it must not automatically replay tool actions.

## 7. UI Contract

Task continuity surface must show:

- active tasks,
- waiting tasks,
- blocked tasks,
- failed retryable tasks,
- stale tasks,
- terminal tasks,
- last observation,
- next recommended action,
- allowed controls.

Main Chat should show inline continuity for the current task and a separate
task list/detail entry for older tasks.

## 8. Eval Scenarios

Minimum scenarios:

- list active task,
- list waiting permission task,
- resume after exact permission acceptance,
- block resume after target changed,
- retry failed safe read,
- cancel queued action,
- block resume of completed task,
- stale task asks user before continuing,
- missing tool produces blocker,
- reopened app can load task detail.

## 9. Acceptance

This contract is satisfied when:

- user can find previous tasks,
- task detail is evidence-backed,
- stale/terminal/permission-sensitive states are visible,
- resume/retry/cancel controls are real backend commands,
- continuation never silently replays unsafe work.

## 10. Stop Conditions

Stop if:

- tasks cannot be listed without scanning raw chat text,
- stale context cannot be detected,
- permission scope cannot be revalidated,
- terminal task can accidentally resume,
- UI would need to fabricate last observation or next action.
