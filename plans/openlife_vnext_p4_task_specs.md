# OpenLife vNext P4 Task Specifications

Date: 2026-05-06

This document starts after P0/P1, P2/P3, and P3 Hardening have landed.

## Scope

P4 target:

```text
Confirmed Plan Execution and Trace UI Integration
```

P4 should turn PlanMode from a planning skeleton into a minimal governed execution loop, and make AgentRunEvent trace visible through the chat surface without rewriting ChatPage.

## Current Baseline

Completed before P4:

- Durable `AgentRunEventStore` is wired into product paths.
- `AgentRunEventType::Unknown(String)` preserves future event types.
- `PromptStack` includes PlanMode planning prompt and AgentPlan output schema.
- `PlanStore`, `AgentPlan`, PlanMode read-only exploration, and confirmation protocol exist.
- `SubAgentRuntime` has call-as-tool/review boundaries and structured outcomes.
- `ExecutionSandbox` exists and is hardened, but no shell executor exists.
- `RunTracePanel`, frontend AgentRunEvent types, render tests, and list events API contract exist.

## P4-0: Documentation and ADR Status Sync

Goal:

Bring docs in line with the code baseline before new implementation.

Allowed edit areas:

- `AGENTS.md`
- `plans/adr/README.md`
- `plans/adr/0007-planmode-confirmation-policy.md`
- `plans/adr/0008-subagent-permissions.md`
- `plans/adr/0009-execution-sandbox-bash.md`
- `plans/adr/0010-chatpage-state-model.md`
- `plans/openlife_vnext_migration_plan.md`
- `plans/openlife_vnext_agent_coding_prompts.md`

Verification:

- docs reference P4 task specs
- ADR 0007-0010 no longer appear as blockers for already implemented P2/P3/P3-hardening work

Non-goals:

- no code changes

## P4-1: Plan Execution Contract and Events

Prerequisite:

- P4-0 complete.
- ADR 0007 accepted.

Affected primitive:

- `AgentPlan`
- ExecuteMode
- `AgentRunEvent`

Goal:

Define the minimal backend contract for executing a confirmed plan. Add execution-oriented types and event kinds without wiring broad UI behavior.

Expected additions:

- `PlanExecutionMode` or equivalent
- `PlanStepExecutionResult` or equivalent
- `PlanExecutionOutcome` or equivalent
- event types for:
  - `plan.execution_started`
  - `plan.step_started`
  - `plan.step_completed`
  - `plan.step_failed`
  - `plan.deviation_recorded`
  - `plan.execution_completed`
  - `plan.execution_failed`

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/event_store.rs`
- tests under `openlife-core/src/agent/`

Verification:

- new event types round-trip through `AgentRunEventStore`
- unknown event type still round-trips as `Unknown(String)`
- execution outcome types serialize/deserialize

Non-goals:

- no tool execution yet
- no frontend UI
- no plan executor loop yet

## P4-2: Minimal Confirmed Plan Executor

Prerequisite:

- P4-1 complete.

Affected primitive:

- `PlanExecutor`
- `PlanStore`
- `ActionExecutor`
- `AgentRunEventStore`

Goal:

Implement a minimal executor for confirmed plans. It should execute plan steps in order, route declared tool intents through `ActionExecutor`, and record every step transition in `AgentRunEvent`.

Rules:

- A plan with `requires_confirmation=true` must have status `Confirmed` before execution.
- A low-risk read-only plan may execute if policy says confirmation is not required.
- Write/external side-effect tools still go through ToolRuntime/Permission/Proposal; do not bypass existing policy.
- Deviations from declared plan steps/tool intents must be recorded as events.
- Failed steps stop execution unless a rollback/fallback policy is explicitly defined.

Allowed edit areas:

- new `openlife-core/src/agent/plan_executor.rs`
- `openlife-core/src/agent/mod.rs`
- `openlife-core/src/agent/plan_store.rs`
- tests under `openlife-core/src/agent/`

Verification:

- confirmed plan can execute a read-only step
- unconfirmed high-risk plan is rejected
- write tool intent creates blocked/needs-confirmation outcome through existing ActionExecutor policy
- step events are recorded in order
- deviation event is recorded when executed action differs from plan intent

Non-goals:

- no Bash/Shell
- no parallel plan execution
- no sub-agent handoff execution
- no polished frontend UI

## P4-3: Tauri Plan Commands

Prerequisite:

- P4-2 complete.

Affected surface:

- Tauri command layer
- frontend API contract

Goal:

Expose minimal commands for Plan lifecycle and execution.

Expected commands:

- `get_agent_plan(plan_id)`
- `list_agent_plans_for_run(run_id)`
- `list_agent_plans_for_session(session_id, limit)`
- `confirm_agent_plan(plan_id)`
- `reject_agent_plan(plan_id)`
- `execute_agent_plan(plan_id)`

Allowed edit areas:

- `src-tauri/src/commands/agent.rs` or new focused command module
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/bootstrap.rs` / `state.rs` if `PlanStore` is not yet in AppState
- `frontend/src/tauri.ts`
- `frontend/src/types.ts`
- tests/mocks as needed

Verification:

- Tauri tests for confirm/reject/execute lifecycle
- frontend typecheck passes
- command mocks updated only if frontend tests need them

Non-goals:

- no full plan editor
- no ChatPage rewrite

## P4-4: Chat Trace UI Integration

Prerequisite:

- P4-3 complete.
- `list_agent_run_events` API exists.

Affected surface:

- ChatPage / trace panel integration

Goal:

Wire `RunTracePanel` into the chat experience in the smallest safe way.

Rules:

- Do not rewrite ChatPage.
- Preserve streaming behavior.
- Preserve proposal banner behavior.
- Fetch events for the active/latest run after run completion, and refresh when plan execution emits events.
- Empty trace should not clutter the UI.

Allowed edit areas:

- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/tauri.ts`
- `frontend/src/test/mocks/tauri.ts`
- focused frontend tests

Verification:

- ChatPage test or focused component test proves trace panel appears for a run with events
- existing chat tests still pass
- `pnpm --dir frontend typecheck`

Non-goals:

- no visual redesign
- no component extraction beyond what is necessary

## P4-5: Plan Execution Review Gate

Prerequisite:

- P4-2 complete.
- ReviewAgent exists.

Affected primitive:

- `ReviewAgent`
- `PlanExecutor`
- `AgentRunEvent`

Goal:

Add an optional review gate for medium/high-risk plan execution results. The reviewer should inspect the plan execution outcome and produce structured review output without mutating state.

Rules:

- ReviewAgent remains read-only.
- Review output is stored as observation/structured output and linked to the parent run.
- Critical review issues prevent plan status from becoming completed unless explicitly overridden by future policy.

Allowed edit areas:

- `openlife-core/src/agent/plan_executor.rs`
- `openlife-core/src/agent/sub_agent.rs`
- tests under `openlife-core/src/agent/`

Verification:

- review gate records parent trace event
- critical issue leaves plan non-completed / failed-review status
- approved review allows completed status

Non-goals:

- no parallel reviewers
- no handoff
- no UI for review editing

## P4 Completion Gate

Run before declaring P4 complete:

```bash
cargo check -q
cargo test -p openlife-core agent
cargo test -p openlife-tauri
pnpm --dir frontend typecheck
pnpm --dir frontend test
```

P4 is complete when:

- confirmed plans can execute through governed runtime paths
- every plan execution step is traceable
- unconfirmed high-risk plans cannot execute
- Chat can show run trace without a rewrite
- no Bash/Shell executor has been introduced
