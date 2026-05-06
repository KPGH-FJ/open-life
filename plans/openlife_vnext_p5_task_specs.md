# OpenLife vNext P5 Task Specifications

Date: 2026-05-06

This document starts after P4 Confirmed Plan Execution and Trace UI Integration has landed and passed stabilization.

## Scope

P5 target:

```text
Governed Plan Operations and Recovery
```

P5 turns confirmed plan execution into an operational workflow: stable command contracts, cancellation, retry, permission-approved continuation, rollback policy, real read-only review, and minimal plan operation UI.

## Current Baseline

Completed before P5:

- `AgentPlan` can be created, confirmed, rejected, executed, failed, failed-review, and completed.
- `PlanExecutor` records plan execution events and step transitions.
- `execute_agent_plan` routes declared tool intents through `ActionExecutor` / ToolRuntime.
- Medium/high-risk execution passes through a deterministic read-only review gate.
- Critical review records a review observation plus terminal `plan.execution_failed`.
- Chat trace can refresh for the active run without rewriting ChatPage.

## P5-0: Closeout And Baseline

Goal:

Make the P4 closeout clean enough to serve as the P5 baseline.

Allowed edit areas:

- `openlife-core/src/agent/plan_executor.rs`
- `frontend/src/tauri.ts`
- focused tests
- documentation entrypoints if stale

Tasks:

- Remove lingering test-only unused imports.
- Add wrapper-level test proving `executeAgentPlan()` normalizes backend snake_case to frontend camelCase.
- Run the full P4 closeout verification.

Verification:

- `pnpm --dir frontend test -- --run tauri`
- `cargo test -p openlife-core agent::plan_executor`
- `cargo check -q`
- `make ci`

Non-goals:

- no new plan operations
- no UI changes
- no retry/cancel implementation

## P5-1: Stable Plan Operation Contract

Goal:

Define one stable frontend/backend contract for plan operations.

Expected additions:

- `PlanOperationResult` or equivalent
- `PlanExecutionCommandResult` or equivalent
- `PlanOperationErrorKind` or equivalent

Contract should include:

- `planId`
- `runId`
- `operation`
- `status`
- `success`
- `stepsCompleted`
- `stepsFailed`
- `deviations`
- `reviewStatus`
- `errorKind`
- `message`

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `src-tauri/src/commands/plan.rs`
- `frontend/src/tauri.ts`
- `frontend/src/types.ts`
- mocks and focused tests

Rules:

- Commands must not return ad hoc JSON once this contract lands.
- Frontend wrappers must expose camelCase.
- Backend may keep Rust snake_case internally, but the frontend contract must be stable.
- Do not add new operations in this task.

Verification:

- `cargo test -p openlife-tauri commands::plan`
- `pnpm --dir frontend test -- --run tauri`
- `pnpm --dir frontend typecheck`
- `cargo check -q`

## P5-2: Cancel Plan

Goal:

Add governed cancellation for plans that have not reached a terminal successful state.

Expected command:

- `cancel_agent_plan(plan_id)`

Expected events:

- `plan.cancel_requested`
- `plan.cancelled`

Rules:

- Allow cancel from `published`, `confirmed`, and `executing`.
- Reject cancel for `completed` and `rejected`.
- Do not use cancellation as retry or rollback.
- Cancellation must not mutate LifeModel, Memory, files, or external systems.

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/plan_store.rs`
- `openlife-core/src/agent/plan_executor.rs` only if lifecycle helper is needed
- `src-tauri/src/commands/plan.rs`
- frontend wrappers/types/mocks
- focused tests

Verification:

- `cargo test -p openlife-core agent::plan_store`
- `cargo test -p openlife-tauri commands::plan`
- `pnpm --dir frontend typecheck`

## P5-3: Retry Failed Plan

Goal:

Allow failed or failed-review plans to retry safely without erasing historical trace.

Expected command:

- `retry_agent_plan(plan_id)`

Rules:

- Only allow retry from `failed` or `failed_review`.
- First implementation retries the whole plan, not from an arbitrary step.
- Retry must create a new execution attempt marker.
- Existing failed events must remain append-only.
- Retry must still route through `PlanExecutor`, ToolRuntime, Permission, Proposal, and ReviewGate.

Expected events:

- `plan.retry_requested`
- `plan.retry_started`
- normal plan execution events for the new attempt

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/plan_executor.rs`
- `openlife-core/src/agent/plan_store.rs`
- `src-tauri/src/commands/plan.rs`
- frontend wrappers/types/mocks
- focused tests

Verification:

- `cargo test -p openlife-core agent::plan_executor`
- `cargo test -p openlife-tauri commands::plan`
- `pnpm --dir frontend typecheck`

## P5-4: Blocked Action Continuation

Goal:

Let a plan continue after a blocked or needs-confirmation action is approved through existing permission/proposal policy.

Expected behavior:

- Blocked plan action is linked to plan id and step index.
- User approval happens through existing Proposal / ToolPermission flow.
- Continuation replays through `ActionExecutor`, not direct execution.

Expected command options:

- `continue_agent_plan(plan_id)`
- or `replay_plan_action(plan_id, action_id)` if action-level replay is more precise

Expected events:

- `plan.action_replay_requested`
- existing `permission.replay_action`
- step completion or failure events

Rules:

- Do not bypass ToolRuntime.
- Do not auto-approve permissions.
- Do not implement from-step retry in this task unless required by replay linkage.
- Do not write LifeModel, Memory, files, or external systems directly.

Verification:

- blocked write intent creates proposal/permission state
- approved replay records events
- denied replay leaves plan non-completed
- `cargo test -p openlife-core agent`
- `cargo test -p openlife-tauri commands::plan`

## P5-5: Rollback Policy ADR

Goal:

Decide rollback boundaries before implementing rollback behavior.

Required document:

- `plans/adr/0011-plan-recovery-rollback-policy.md`

ADR must decide:

- Which plan operations are retryable.
- Which operations are cancellable.
- Which side effects are rollback-capable.
- Which side effects are irreversible and require explicit user warning.
- Whether rollback requires confirmation.
- Required event names and payload fields.

Non-goals:

- no automatic rollback executor
- no shell undo
- no external side-effect rollback

Verification:

- ADR 0011 is accepted before P5 rollback implementation.
- `plans/adr/README.md` references ADR 0011.

## P5-6: Real Read-Only ReviewAgent Integration

Goal:

Replace the production deterministic review gate with a governed read-only ReviewAgent path.

Rules:

- ReviewAgent remains read-only.
- ReviewAgent cannot mutate LifeModel, Memory, files, or external systems.
- Review output must be structured `ReviewAgentOutput`.
- Critical review still prevents `Completed`.
- Parent run must link to child review run or review observation.

Allowed edit areas:

- `openlife-core/src/agent/plan_executor.rs`
- `openlife-core/src/agent/sub_agent.rs`
- `src-tauri/src/commands/plan.rs`
- focused tests

Verification:

- approved real review allows completion
- critical real review yields `failed_review`
- reviewer denied write tools
- parent trace links review result
- `cargo test -p openlife-core agent`
- `cargo check -q`

## P5-7: Minimal Plan Operations UI

Goal:

Expose minimal plan operations without rewriting ChatPage.

Expected surface:

- plan status
- confirm/reject/execute/cancel/retry buttons where legal
- latest operation result
- link or compact display for trace events

Allowed edit areas:

- `frontend/src/components/`
- `frontend/src/pages/ChatPage.tsx` only for minimal integration
- `frontend/src/tauri.ts`
- `frontend/src/types.ts`
- tests/mocks

Rules:

- Do not redesign ChatPage.
- Do not add a full plan editor.
- Empty plan state must not clutter UI.
- Existing streaming, proposal banner, and trace behavior must stay covered.

Verification:

- `pnpm --dir frontend typecheck`
- `pnpm --dir frontend test -- --run ChatPage RunTracePanel tauri`

## P5 Completion Gate

Run before declaring P5 complete:

- `cargo test -p openlife-core agent`
- `cargo test -p openlife-tauri`
- `pnpm --dir frontend typecheck`
- `pnpm --dir frontend test -- --run ChatPage RunTracePanel tauri`
- `cargo check -q`
- `make ci`

P5 is complete when:

- plan operation command contracts are stable
- cancellation is governed and traceable
- retry preserves append-only history
- blocked action continuation uses existing permission/proposal replay
- rollback policy is accepted by ADR
- real ReviewAgent integration is read-only and traceable
- minimal UI operations do not destabilize ChatPage
