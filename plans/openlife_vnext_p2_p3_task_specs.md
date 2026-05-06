# OpenLife vNext P2/P3 Task Specifications

Date: 2026-05-06

This document continues after P0/P1. It assumes the traceable and governed runtime foundation is in place.

## Scope

P2 target:

```text
Planning and Context Resilience
```

P3 target:

```text
SubAgent and Sandbox Foundation
```

Do not start P3 until P2 is reviewed and accepted.

## P1 Carry-Over: Wire AgentRunEventStore Into Product Paths

Prerequisite:

- P0-1 and P0-2 complete.
- P1-7 facade/path convergence reviewed.

Affected primitive:

- `AgentRunEventStore`
- `AgentLoop`
- `ActionExecutionContext`
- Tauri `AppState`

Goal:

Create and hold a durable `AgentRunEventStore` in Tauri state, pass it into chat, streaming, replay, direct tool execution, scheduled/proactive execution, and facade paths that already support event recording.

Verification:

- product chat path records `run.created`, model events, and terminal event
- blocked replay/direct tool path records `tool.call_blocked`
- fallback path records fallback start/completion or documented failure event
- existing core and Tauri tests pass

Non-goals:

- no event timeline UI
- no migration for old runs
- no new event schema

## P2-1: PlanMode Schema and Store Skeleton

Prerequisite:

- ADR 0007 accepted.

Affected primitive:

- `AgentPlan`
- PlanMode

Goal:

Add structured `AgentPlan` types and persistence/trace skeleton without changing normal chat behavior.

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- new `openlife-core/src/agent/plan_store.rs`
- `openlife-core/src/agent/mod.rs`
- tests under `openlife-core/src/agent/`

Verification:

- unit tests for creating and listing plans
- event test for `plan.created`

Non-goals:

- no UI
- no execution of plans

## P2-2: PlanMode Planner PromptStack

Prerequisite:

- P2-1 complete.
- PromptStack exists.

Affected primitive:

- `PromptStack`
- PlanMode

Goal:

Add PlanningPrompt and AgentPlan output schema through PromptStack.

Allowed edit areas:

- `openlife-core/src/agent/prompt_stack.rs`
- `openlife-core/src/agent/agent_loop.rs`
- tests under `openlife-core/src/agent/`

Verification:

- test planning prompt block inclusion
- test output schema inclusion
- test cloud filtering still works

Non-goals:

- no sub-agent
- no frontend plan UI

## P2-3: PlanMode Read-Only Exploration

Prerequisite:

- P2-1 and P2-2 complete.

Affected primitive:

- PlanMode
- ToolRuntime

Goal:

Allow Planner to use only read-only tools and emit an AgentPlan.

Allowed edit areas:

- `openlife-core/src/agent/`
- tests under `openlife-core/src/agent/`

Verification:

- read-only tool allowed
- write tool blocked
- `plan.created` event recorded
- no LifeModel/Memory mutation

Non-goals:

- no plan execution
- no UI

## P2-4: Plan Confirmation Protocol

Prerequisite:

- P2-3 complete.

Affected primitive:

- AgentPlan
- Proposal/Permission protocol

Goal:

Determine when plans require confirmation and record confirmation request events.

Allowed edit areas:

- `openlife-core/src/agent/`
- `src-tauri/src/commands/` if adding minimal commands
- tests under relevant modules

Verification:

- high-risk plan requires confirmation
- low-risk read-only plan can proceed
- confirmation request records event

Non-goals:

- no polished frontend plan editor

## P2-5: CompactionSummary Skeleton

Affected primitive:

- context compaction

Goal:

Define compacted context summary schema and tests. Do not wire automatic compaction yet.

Allowed edit areas:

- `openlife-core/src/agent/`
- tests under `openlife-core/src/agent/`

Verification:

- summary preserves active proposals
- summary preserves unresolved tool observations
- summary redacts sensitive fields

Non-goals:

- no automatic trigger
- no UI

## P3-1: SubAgentSpec Skeleton

Prerequisite:

- ADR 0008 accepted.
- PlanMode foundation reviewed.

Affected primitive:

- `AgentSpec`
- `SubAgentSpec`

Goal:

Add canonical AgentSpec and SubAgentSpec wrapper, without execution.

Allowed edit areas:

- `openlife-core/src/agent/`
- tests under `openlife-core/src/agent/`

Verification:

- AgentSpec can describe main agent
- SubAgentSpec wraps AgentSpec
- delegation modes serialize/deserialize

Non-goals:

- no sub-agent execution

## P3-2: SubAgent call_as_tool Runtime

Prerequisite:

- P3-1 complete.

Affected primitive:

- `SubAgentRuntime`

Goal:

Implement first sub-agent mode: `call_as_tool`.

Verification:

- child AgentRun links to parent
- isolated context policy applied
- tool policy enforced
- result returns to main agent

Non-goals:

- no handoff
- no parallel workers
- no bash

## P3-3: ReviewAgent Mode

Prerequisite:

- P3-2 complete.

Affected primitive:

- `ReviewAgent`
- `SubAgentRuntime`
- parent/child AgentRun trace

Goal:

Add a reviewer sub-agent that reviews a plan/output/patch without mutating state.

Verification:

- reviewer cannot call write tools
- reviewer output schema validated
- review result appears in parent run trace

Non-goals:

- no writes
- no handoff
- no parallel workers

## P3-4: ExecutionSandbox Skeleton

Prerequisite:

- ADR 0009 accepted.

Affected primitive:

- `ExecutionSandbox`

Goal:

Add sandbox policy types and path/command validation helpers. No shell execution yet.

Verification:

- deny-read patterns block secret paths
- safe paths allow expected reads
- dangerous command denylist works
- env allowlist test

Non-goals:

- no BashExecutor
- no command execution

## P3-5: ChatPage AgentRunEvent Timeline Contract

Prerequisite:

- ADR 0010 accepted.

Affected primitive:

- frontend agent workspace
- AgentRunEvent UI contract

Goal:

Define frontend event type and Tauri mock contract for future event timeline UI.

Verification:

- frontend mock includes new event shape
- timeline component can render static events

Non-goals:

- no ChatPage rewrite
