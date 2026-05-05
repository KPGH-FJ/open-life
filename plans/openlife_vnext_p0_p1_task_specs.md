# OpenLife vNext P0/P1 Task Specifications

Date: 2026-05-06

This document converts the vNext migration plan into AI-coding-ready tasks. Each task is intentionally scoped. Do not combine tasks unless explicitly approved.

## Task Rules

Every task must:

- name the affected primitive
- list allowed edit areas
- include verification steps
- preserve existing behavior unless explicitly changing it
- update docs if behavior changes

Do not:

- implement sub-agents in P0/P1
- introduce bash/shell in P0/P1
- rewrite ChatPage in P0/P1
- remove fallback paths without tests
- add ad hoc prompt fragments after PromptStack work begins

## P0-1: AgentRunEvent Store Skeleton

Affected primitive:

- `AgentRunEvent`

Goal:

Add the minimal type and persistence skeleton for append-only run events.

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/store.rs` or new `openlife-core/src/agent/event_store.rs`
- `openlife-core/src/agent/mod.rs`
- tests under `openlife-core/src/agent/`

Expected behavior:

- Can create an event.
- Can append event to store.
- Can list events by `run_id`.
- Does not change chat behavior yet.

Verification:

- Rust unit tests for create/list.
- `cargo test -p openlife-core agent` or narrower available test target.

Non-goals:

- No UI.
- No migration of existing status updates.
- No prompt work.

## P0-2: Trace Current AgentLoop Milestones

Affected primitive:

- `AgentRunEvent`
- `AgentLoop`

Goal:

Append events for current AgentLoop milestones without changing execution semantics.

Allowed edit areas:

- `openlife-core/src/agent/agent_loop.rs`
- `openlife-core/src/agent/types.rs`
- event store files from P0-1
- tests under `openlife-core/src/agent/`

Expected events:

- `run.created` or equivalent bridge if run creation happens outside loop
- `model.call_started`
- `model.call_completed`
- `model.call_failed`
- `json_repair.started`
- `json_repair.completed`
- `tool.call_started`
- `tool.call_blocked`
- `tool.call_completed`
- `tool.call_failed`
- `run.completed`
- `run.failed`

Verification:

- Test that a no-tool response records model and completion events.
- Test that malformed JSON repair records repair events.
- Test that blocked tool call records blocked event.

Non-goals:

- No schema changes to PromptStack.
- No frontend display.

## P0-3: Execution Entry Point Map and Facade Spec

Affected primitive:

- runtime convergence

Goal:

Create a code-facing map of formal agent execution entrypoints and draft a facade signature before refactoring.

Allowed edit areas:

- `plans/current_agent_runtime_audit.md`
- `plans/openlife_vnext_migration_plan.md`
- optional new `plans/openlife_vnext_execution_entrypoints.md`

Expected output:

- table of entrypoints:
  - command/function
  - current behavior
  - creates AgentRun?
  - streaming?
  - fallback?
  - proposals?
  - future runtime mode
- proposed facade:
  - input
  - output
  - streaming adapter
  - error/fallback semantics

Verification:

- Documentation review only.

Non-goals:

- No code refactor.

## P0-4: Tool Metadata Audit and Enforcement Spec

Affected primitive:

- `ToolRuntime`
- `ToolPolicy`

Goal:

Inventory current tools and define which metadata is missing before enforcement changes.

Allowed edit areas:

- `plans/current_agent_runtime_audit.md`
- new `plans/openlife_vnext_tool_inventory.md`
- optionally tests marked ignored/pending if project style allows

Expected output:

- list of tools
- executable/declarative-only status
- risk level
- permission behavior
- executor source
- proposal behavior
- model-callable status

Verification:

- Documentation review.

Non-goals:

- No runtime enforcement change yet unless separately approved.

## P0-5: Existing Data Bridge Audit

Affected primitive:

- `AgentRunEvent`
- runtime trace migration

Goal:

Decide how existing `AgentRun` records, status updates, actions, observations, and proposals relate to the new append-only event model.

Allowed edit areas:

- new `plans/openlife_vnext_agentrun_event_data_bridge.md`
- `plans/adr/0001-agentrun-event-trace.md`
- `plans/openlife_vnext_migration_plan.md`

Expected output:

- inventory of existing persisted run fields
- mapping from current run fields to possible event types
- decision recommendation:
  - bridge only future runs
  - synthesize events for old runs on read
  - one-time migration
  - ignore old runs for event timeline
- risks for each option
- recommendation for P0 implementation

Verification:

- Documentation review.
- ADR 0001 open questions updated or closed.

Non-goals:

- No database migration.
- No code changes.

## P1-1: ToolRuntime Declarative-Only Enforcement

Prerequisite:

- ADR 0003 accepted.
- P0-4 completed.

Affected primitive:

- `ToolRuntime`

Goal:

Enforce declarative-only filtering both in ToolPrompt generation and ActionExecutor execution.

Allowed edit areas:

- `openlife-core/src/mcp.rs`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/agent/action_executor/`
- tests under `openlife-core/src/agent/`

Expected behavior:

- Declarative-only tools are not model-callable.
- Attempted execution is blocked with observation/event.
- Proposal-generating executable tools remain callable.

Verification:

- Unit/integration tests for prompt filtering and execution blocking.

Non-goals:

- No new tools.
- No bash.

## P1-2: PromptStack Skeleton

Prerequisite:

- ADR 0002 accepted.

Affected primitive:

- `PromptStack`

Goal:

Introduce `PromptBlock` and `PromptStack` without migrating all prompts at once.

Allowed edit areas:

- new `openlife-core/src/agent/prompt_stack.rs`
- `openlife-core/src/agent/mod.rs`
- tests under `openlife-core/src/agent/`

Expected behavior:

- Can assemble a stack from blocks.
- Blocks have IDs, versions, privacy metadata, cloud policy.
- Can produce cloud-filtered stack.
- Can output block ID/version trace.

Verification:

- Unit tests for assembly.
- Unit tests for cloud filtering.
- Unit tests for trace metadata.

Non-goals:

- No immediate rewrite of all existing prompts.
- No final system prompt wording required.

## P1-3: Migrate AgentRole Prompt Into PromptStack

Prerequisite:

- P1-2 complete.

Affected primitive:

- `PromptStack`
- `AgentSpec` preparation

Goal:

Move existing role prompt behavior into a PromptBlock-backed path.

Allowed edit areas:

- `openlife-core/src/agent/agent_loop.rs`
- `openlife-core/src/agent/prompt_stack.rs`
- tests under `openlife-core/src/agent/`

Expected behavior:

- Generalist/Planner role behavior is preserved.
- Role prompt block ID/version is traceable.
- No new role semantics added.

Verification:

- Existing AgentRole tests pass.
- New tests verify role prompt block inclusion.

Non-goals:

- No PlanMode.
- No SubAgentSpec.

## P1-4: LifeModel Field Risk Matrix

Prerequisite:

- ADR 0004 accepted.

Affected primitive:

- LifeModel governance

Goal:

Implement a field risk classifier or config table.

Allowed edit areas:

- `openlife-core/src/life_model/`
- `openlife-core/src/agent/`
- tests under `openlife-core/src/`

Expected behavior:

- Can classify a LifeModel path as high/medium/low risk.
- High-risk fields are identifiable by proposal generation/application logic.
- No behavior change unless used by a separate task.

Verification:

- Unit tests for representative paths.

Non-goals:

- No MemoryEvidence engine yet.
- No proposal auto-apply changes yet.

## P1-5: MemoryEvidence Schema Skeleton

Prerequisite:

- ADR 0005 accepted.

Affected primitive:

- `MemoryEvidence`

Goal:

Add schema and in-memory generation helpers for evidence without changing LifeModel.

Allowed edit areas:

- new `openlife-core/src/agent/memory_evidence.rs`
- `openlife-core/src/agent/mod.rs`
- tests under `openlife-core/src/agent/`

Expected behavior:

- Can create MemoryEvidence from accepted memory IDs and a claim.
- Can mark evidence type, affected path, confidence, recency, contradictions.
- Does not create proposals yet.

Verification:

- Unit tests for schema creation and validation.

Non-goals:

- No LifeModel mutation.
- No scheduled evidence generation.

## P1-6: MemoryEvidence to Proposal Draft

Prerequisite:

- P1-4 and P1-5 complete.

Affected primitive:

- `LifeModelEvolutionEngine`
- `ProposalEngine`

Goal:

Generate LifeModel evolution proposals from evidence with risk classification.

Allowed edit areas:

- new or existing evolution/proposal modules under `openlife-core/src/agent/`
- tests under `openlife-core/src/agent/`

Expected behavior:

- Evidence can generate a proposal.
- Proposal includes evidence links.
- High-risk proposals are never auto-applied.
- Contradictions produce clarification/low-confidence result.

Verification:

- Tests for repeated preference.
- Tests for high-risk field requiring explicit review.
- Tests for contradiction path.

Non-goals:

- No frontend evidence UI yet.
- No background scheduler.

## P1-7: Implement Execution Path Convergence Facade

Prerequisite:

- P0-3 complete.
- P0-1 and P0-2 complete enough to record trace events.
- ADR 0001 accepted or explicitly accepted for P1 implementation.

Affected primitive:

- runtime convergence
- `AgentRunEvent`

Goal:

Implement the internal execution facade specified by P0-3 so formal agent runs share one backend entry model.

Allowed edit areas:

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/`
- `src-tauri/src/scheduler_runner.rs`
- `openlife-core/src/agent/`
- tests under `src-tauri/src/` and `openlife-core/src/agent/`

Expected behavior:

- Chat and streaming chat share the same core runtime semantics.
- Fallback behavior is routed through a traceable facade.
- Scheduled/proactive execution has a clear runtime mode even if implementation remains thin.
- The facade returns a consistent result structure for reply, run id, proposals, trace summary, and fallback status.

Suggested internal shape:

```rust
async fn run_agent_task(
    task: AgentTask,
    mode: AgentExecutionMode,
    stream: Option<Arc<dyn StreamingCallback>>,
    deps: AgentExecutionDeps,
) -> Result<AgentExecutionOutcome>
```

Verification:

- Existing chat tests pass.
- Test normal non-streaming chat path.
- Test streaming chat path.
- Test fallback records events.
- Test proposal generation still links to run.

Non-goals:

- Do not remove all legacy code in the same task.
- Do not rewrite ChatPage.
- Do not add PlanMode or SubAgentRuntime.

## Suggested P0 Order

1. P0-3: Execution Entry Point Map and Facade Spec
2. P0-1: AgentRunEvent Store Skeleton
3. P0-2: Trace Current AgentLoop Milestones
4. P0-5: Existing Data Bridge Audit
5. P0-4: Tool Metadata Audit and Enforcement Spec

Reason:

The entrypoint map prevents event work from attaching to the wrong places, while the event store gives later runtime convergence a trace substrate.
