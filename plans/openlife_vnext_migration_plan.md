# OpenLife vNext Migration Plan

Date: 2026-05-06

This plan turns the Agent Framework upgrade into phased work that can be implemented safely with AI coding support. It is intentionally ordered to avoid adding sub-agents or bash before the runtime is ready to govern them.

## Migration Goal

Move OpenLife from:

```text
working Agent Framework beta with multiple growing paths
```

to:

```text
LifeModel-governed Agent Runtime with unified execution, prompt architecture, tool policy, memory evidence, proposal-first side effects, and durable trace
```

## Phase 0: Current Runtime Audit

Status: prepared by `plans/current_agent_runtime_audit.md`.

Deliverables:

- Current execution path map.
- AgentLoop responsibility map.
- ToolRuntime/ActionExecutor inventory.
- Proposal and Memory evolution inventory.
- Current test coverage notes.

Exit criteria:

- The team agrees on current facts.
- Old or branch-specific claims are separated from current code reality.

## Phase 1: vNext Architecture Baseline

Deliverables:

- `openlife_vnext_architecture_principles.md`
- `openlife_vnext_architecture_diagrams.md`
- `openlife_vnext_core_primitives_and_boundaries.md`
- `adr/README.md`
- `openlife_vnext_p0_p1_task_specs.md`
- `openlife_vnext_test_and_acceptance_matrix.md`

Exit criteria:

- Definition of a legal OpenLife agent behavior is accepted.
- `PromptStack`, `AgentRunEvent`, `ToolRuntime`, `AgentPlan`, `AgentSpec`, and `MemoryEvidence` are accepted as first-class primitives.
- High-risk ADR topics are identified.
- P0/P1 tasks are small enough for AI coding under governance.
- Each phase has acceptance tests or manual review gates.

## Phase 2: Execution Path Convergence

Goal:

Reduce behavioral divergence between chat, streaming, fallback, proactive, scheduled, builder, and calibration flows.

Tasks:

- Map all current Tauri entrypoints to runtime modes.
- Define a single internal execution facade for formal agent runs.
- Preserve L1 reflex as a clearly marked non-AgentLoop shortcut or convert it into a lightweight AgentRun mode.
- Ensure fallback behavior creates trace events instead of silent alternate behavior.
- Add tests before deleting or rerouting legacy behavior.

Suggested implementation shape:

```text
run_agent_task(task, mode, stream_adapter)
```

Modes:

- chat
- stream_chat
- scheduled
- proactive
- calibration
- builder
- replay

Exit criteria:

- There is one primary backend runtime entry model for formal agent behavior.
- Streaming and non-streaming share core semantics.
- Fallbacks are traceable.
- No proposal or tool path is silently skipped in fallback.

## Phase 3: AgentRunEvent and ToolRuntime Hardening

Goal:

Make runtime trace and tool policy durable enough to support PlanMode, sub-agents, and bash later.

Tasks:

- Add `AgentRunEvent` type and store.
- Append events for model route, prompt assembly, tool calls, blocks, observations, proposal creation, fallback, repair, replay, and completion.
- Formalize tool metadata fields.
- Add policy matrix tests for executable/declarative-only/risk/permission behavior.
- Refactor `ActionExecutionContext` toward smaller dependency groups if needed.

Exit criteria:

- Every tool attempt creates an event.
- Declarative-only tools cannot enter model-callable prompt.
- Tool block and permission request behavior is test-covered.
- UI status can be derived from events for new surfaces.

## Phase 4: PromptStack and System Prompt Architecture

Goal:

Make system prompt design a framework primitive.

Tasks:

- Define `PromptBlock` and `PromptStack`.
- Create base prompt blocks:
  - base system
  - LifeModel usage
  - memory evidence usage
  - planning discipline
  - tool discipline
  - proposal-first rule
  - privacy rule
  - output format
- Record prompt block IDs/versions in AgentRunEvent.
- Add tests for prompt assembly, cloud filtering, and token budget behavior.

Exit criteria:

- No new major runtime feature can add ad hoc prompt fragments without a prompt block.
- Agent role instructions become prompt blocks or AgentSpec configuration.
- Cloud disallowed prompt blocks are filtered or summarized under policy.

## Phase 5: MemoryEvidence and LifeModel Evolution Design

Goal:

Upgrade memory from context store to LifeModel evolution evidence.

Tasks:

- Define `MemoryEvidence`.
- Define LifeModel field risk classification.
- Design `MemoryToLifeModelEngine` / `LifeModelEvolutionEngine`.
- Link evidence to accepted memories, feedback, rejected proposals, and contradictions.
- Generate evolution proposals without direct LifeModel mutation.
- Add tests for repeated preference, recurring goal, state trend, contradiction, and rejection feedback.

Exit criteria:

- Memory-driven evolution creates proposals only.
- Proposals include evidence links.
- High-risk fields never auto-apply.
- Rejected proposals influence future evidence scoring.

## Phase 6: AgentSpec and PlanMode

Goal:

Make planning and agent identity structured.

Tasks:

- Define `AgentSpec`.
- Define `AgentPlan`.
- Add PlanMode:
  - read-only exploration
  - structured plan generation
  - plan confirmation when needed
  - execute confirmed plan
- Add planner output schema and tests.

Exit criteria:

- High-risk and multi-step tasks can enter PlanMode.
- Planner cannot write external state.
- Plan confirmation path is traceable.
- Execute mode follows the plan or records deviations.

## Phase 7: SubAgentRuntime

Goal:

Introduce specialized agents without losing governance.

Tasks:

- Define `SubAgentSpec`.
- Implement `call_as_tool` first.
- Add parent/child AgentRun linkage.
- Add isolated context policy.
- Add role-specific tool policy.
- Add reviewer mode after call-as-tool is stable.
- Add parallel mode last.

Suggested first sub-agents:

- `PlannerAgent`
- `CodebaseExplorerAgent`
- `MemoryCuratorAgent`
- `LifeModelGuardianAgent`
- `ReviewAgent`

Exit criteria:

- Sub-agent runs are isolated and traceable.
- Sub-agent tools are policy-limited.
- Sub-agent output has schema.
- Main agent can explain how sub-agent output influenced final response.

## Phase 8: Compaction

Goal:

Support long-running conversations and runs without context collapse.

Tasks:

- Define `CompactionSummary`.
- Trigger compaction by token threshold or run length.
- Use summarizer under privacy policy.
- Preserve decisions, proposals, memory evidence, unresolved tasks, and tool observations.
- Record compaction in AgentRunEvent.

Exit criteria:

- Long chats can continue with stable context.
- Compaction summaries are auditable.
- Important proposal and tool state is not lost.

## Phase 9: Bash / Shell / Sandbox

Goal:

Introduce shell execution only after policy and trace are ready.

Tasks:

- Define `ExecutionSandbox`.
- Add deny-read defaults.
- Add command allowlist and dangerous command denylist.
- Add cwd, timeout, env allowlist, output limit.
- Start with explicit user-triggered or scheduled safe operations only.
- Keep write operations proposal-first.

Exit criteria:

- Bash is default-off.
- Shell attempts are traceable.
- Secret paths and dangerous commands are blocked.
- No shell execution can bypass ToolRuntime.

## Phase 10: Frontend Agent Workspace

Goal:

Make the UI reflect the framework: runs, plans, tools, proposals, memory evidence, and context.

Tasks:

- Split ChatPage incrementally.
- Add Plan view.
- Add Run event timeline.
- Add Tool/Observation panel.
- Add Memory evidence display for LifeModel proposals.
- Add Proposal review improvements.

Exit criteria:

- Users can see what the agent understood, planned, used, called, proposed, and changed.
- Frontend state remains stable under streaming.
- No backend runtime migration is bundled with large UI rewrites.

## Work Ordering Rules

- Do not start sub-agent implementation before ToolRuntime and AgentRunEvent hardening.
- Do not start bash implementation before ExecutionSandbox design is approved.
- Do not rewrite ChatPage wholesale.
- Do not remove legacy/fallback paths without tests.
- Do not add new prompt fragments outside PromptStack after Phase 4.
- Do not let memory-driven evolution directly mutate LifeModel.

## Suggested First Development Package

Package name:

```text
vNext P0: Runtime Trace and Path Convergence Prep
```

Scope:

- Add AgentRunEvent design and schema.
- Add tests for current fallback/tool/proposal trace gaps.
- Map all current execution entrypoints.
- Audit how existing AgentRun data bridges into the new event model.
- Refactor no more than one path after tests are in place.

Why first:

It creates the measurement layer needed to safely perform the rest of the upgrade.

Task source:

- See `plans/openlife_vnext_p0_p1_task_specs.md`.
- Start with P0-3, P0-1, P0-2, P0-5, then P0-4.

Verification source:

- See `plans/openlife_vnext_test_and_acceptance_matrix.md`.

Next planning source after P0/P1:

- See `plans/openlife_vnext_p2_p3_task_specs.md`.
- See `plans/openlife_vnext_p4_task_specs.md`.
- See `plans/openlife_vnext_p5_task_specs.md`.
- See `plans/openlife_vnext_agent_coding_prompts.md`.
- ADR 0007-0010 are accepted. P4 starts from confirmed plan execution and Chat trace UI integration.

## P5: Governed Plan Operations and Recovery

Goal:

Turn confirmed plan execution into a governed operational workflow.

Task source:

- `plans/openlife_vnext_p5_task_specs.md`

ADR source:

- `plans/adr/0011-plan-recovery-rollback-policy.md`

Scope:

- stable plan operation contracts
- cancellation
- whole-plan retry for failed / failed-review plans
- blocked action continuation through existing Permission / Proposal / Replay
- rollback policy ADR before rollback implementation
- read-only ReviewAgent integration
- minimal plan operations UI

Non-goals:

- no Bash/Shell
- no SubAgent parallel or handoff
- no automatic rollback executor before ADR 0011 is accepted
- no ChatPage rewrite
- no bypass of ToolRuntime, Proposal, PromptStack, AgentRunEvent, or ExecutionSandbox

Exit criteria:

- plan operation contracts are stable across Tauri and frontend
- cancellation and retry are traceable
- blocked action continuation uses existing permission/proposal policy
- rollback boundaries are accepted by ADR
- ReviewAgent remains read-only
- minimal UI operations preserve streaming and trace behavior
