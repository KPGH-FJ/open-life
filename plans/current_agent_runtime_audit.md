# Current Agent Runtime Audit

Date: 2026-05-10 (updated: P0-P12 implementation complete)

> **Updated note (2026-05-10)** : P0-P12 vNext primitives have all been implemented in code.
> The "gaps" described below (AgentRunEvent, PromptStack, ToolRuntime, MemoryEvidence,
> ExecutionSandbox, PlanMode, SubAgent) are now implemented and tested.
> The remaining concern is **execution path convergence** — lib.rs still has 5+ entry
> paths that need to converge to the unified `ExecutionFacade` model.

## Summary

OpenLife is not a broken project. The current codebase has a real Agent Framework spine:

- `AgentLoop` exists and executes an iterative ReAct-style loop.
- `ActionExecutor` exists and owns many core/execution tool paths.
- `ProposalEngine` and `ProposalStore` exist.
- Chat, Builder, Calibration, Memory, ToolPermission, and ExternalWriteAction flows are partially unified through proposals.
- AgentRun tracing exists, but still needs a stronger append-only event model.

The main gap is not missing modules. The main gap is runtime authority: every meaningful agent behavior should be forced through one traceable path with one prompt architecture, one tool runtime, one permission/proposal protocol, and one audit model.

## Current Execution Paths

Observed code paths:

- `src-tauri/src/lib.rs::send_message`
- `src-tauri/src/lib.rs::send_message_with_agent_loop`
- `src-tauri/src/lib.rs::start_stream_message`
- `src-tauri/src/lib.rs::start_stream_message_with_agent_loop`
- L1 reflex path in streaming mode
- AgentLoop fallback path after runtime/model failure
- Scheduled task execution path in `src-tauri/src/scheduler_runner.rs`

Current concern:

- The primary path is AgentLoop-oriented, but fallback and streaming branches still create multiple behavioral surfaces.
- Fallback handling can produce valid user output while losing parts of the normal action/tool trace semantics.
- `src-tauri/src/lib.rs` is still a large orchestration file, even after bootstrap extraction.

vNext audit conclusion:

All chat, scheduled, proactive, builder, calibration, and future sub-agent executions should converge on one `AgentRuntime`/`AgentLoop` entry model, with mode-specific adapters rather than separate behavioral paths.

## AgentLoop

Current state:

- `openlife-core/src/agent/agent_loop.rs` contains the main iterative loop.
- `AgentLoopConfig` controls step budget, tool call budget, timeout, allowlist, and role.
- `AgentRole` currently provides prompt-level role instruction.
- `run_single_step` handles model generation, JSON envelope parsing, JSON repair, tool filtering, tool execution, status updates, and step completion.
- There are unit and integration tests, especially around parsing, follow-up construction, config, budgets, allowlists, and proposal tools.

Current concern:

- `run_single_step` has too many responsibilities.
- Tests exist, but vNext needs behavior tests for full step execution, streaming/non-streaming parity, fallback trace preservation, tool failure observation, and proposal generation under error conditions.
- AgentLoop status updates are useful but are not a complete append-only event trace.

Needed vNext direction:

- Split AgentLoop internals into smaller phases:
  - `generate_model_reply`
  - `parse_action_envelope`
  - `repair_if_needed`
  - `filter_and_authorize_actions`
  - `execute_actions`
  - `build_observations`
  - `decide_next_step`
- Add `AgentRunEvent` as the durable record, with status updates as a UI projection.

## Tool Runtime / ActionExecutor

Current state:

- `openlife-core/src/agent/action_executor/` is split into:
  - `core_os_tools.rs`
  - `execution_tools.rs`
  - `declarative_stubs.rs`
  - `tool_executor.rs`
  - `helpers.rs`
  - `mod.rs`
- Core tools include LifeModel, goal, state, memory, proposal, permission, and agent run lookup style operations.
- Execution tools include file, web, calendar/email/task/a2a-related capabilities depending on current implementation.
- Proposal-generating tools are treated specially so they can create reviewable side effects instead of being blocked as arbitrary writes.

Current concern:

- `ActionExecutionContext` is an 11-field dependency carrier and has Service Locator pressure.
- Tool policy, sandbox policy, and executor boundaries are not yet first-class enough for sub-agent and bash expansion.
- P1/P2/declarative-only policy must remain impossible to bypass.

Needed vNext direction:

- Promote `ToolRuntime`, `ToolPolicy`, `ExecutionSandbox`, and `ToolObservation` as explicit concepts.
- Keep `ActionExecutor` as implementation, but make policy and observation contracts more formal.
- Every tool call should generate an `AgentRunEvent`, even when blocked, skipped, repaired, or replayed.

## Proposal / Permission / Audit

Current state:

- `AgentProposal`, `ProposalStore`, and `ProposalEngine` exist.
- Proposal application supports LifeModel-like patches, MemoryWrite, MemoryArchive, ToolPermission, ExternalWriteAction, ScheduledTask, and DataExport-style flows.
- LifeModel proposal application creates before/after snapshots and stores patches.
- MemoryWrite proposal application deduplicates and writes memory/vector records.
- ToolPermission acceptance can unlock replay behavior.

Current concern:

- Some calibration paths still allow direct apply modes.
- Proposal generation and proposal application are real, but the event trace around them is not yet strong enough to be the single source of runtime truth.
- Proposal sources exist, but vNext needs richer evidence linkage, especially for memory-driven LifeModel evolution.

Needed vNext direction:

- Treat proposal-first as a framework-level side-effect protocol, not as UI convention.
- High-risk LifeModel fields must never silently auto-apply.
- All proposal creation, edit, accept, reject, apply, fail, replay, and rollback should be represented as `AgentRunEvent` or linked audit events.

## Prompt / System Prompt

Current state (updated 2026-05-10):

- **PromptStack** (`agent/prompt_stack.rs`, 922行) is implemented with `PromptBlock` and `PromptBlockRegistry` (4 built-in blocks: base_system, planning, tool_discipline, privacy_rule).
- AgentLoop-governed paths (send_message, start_stream_message, scheduled, proactive) assemble PromptStack via `execute_task_with_spec()` and inject as `messages[0]`.
- PromptStack assembly records `PromptStackAssembled` events with typed PromptBlock metadata: id, version, purpose, privacy_level, cloud_allowed, token_budget, applies_to, and estimated_tokens. It does not store raw prompt content, raw LifeModel, raw memory, or raw user content.
- **Critical gap fixed (2026-05-10)**: `scheduler.rs:generate_governed()` previously caused dual system prompts — the PromptStack one at `messages[0]` and a second LifeModel YAML prompt from `build_system_prompt()`. Now detects existing system message and uses `_raw` variants to avoid double-injection.

Remaining prompt boundary status (updated 2026-05-26):

| Category | Count | Risk | Mitigation |
|----------|-------|------|------------|
| System prompt double-build (llm.rs/ollama.rs) | 3 | Fixed in governed paths | `InferenceScheduler::generate` / `generate_stream` and `llm::build_system_prompt` are legacy compatibility only; formal AgentRuntime / ExecutionFacade and runtime fallback use governed APIs |
| Reasoning prompts (layered.rs) | 0 | Fixed | LayeredReasoner meaning / strategy / generation / safety prompts are PromptStack-governed internal blocks with metadata-only `ReasoningTrace` traces |
| Builder model-assisted extraction prompts (builder/engine.rs) | 0 | Fixed for model calls | Builder signal extraction and draft-to-LifeModel extraction now use Builder-specific PromptBlocks and `generate_raw_governed(..., LocalOnly)`; UI/session prompts are deterministic text, not model prompts |
| Skills prompts (skills.rs) | 2 | Medium | Skill execution should contribute PromptBlocks |
| Proactive prompts (proactive.rs) | 5 | Low | Proactive is read-only suggestion generation |
| Runtime/dynamic prompts | 9 | Low | JSON repair, tool list, follow-up — inherently dynamic content |

Current concern:

- `PromptBlockRegistry::built_in()` has only 4 entries. Missing blocks: LifeModel, MemoryEvidence, Task, Proposal, OutputFormat, Role, SubAgent.
- Legacy `generate()` / `generate_stream()` compatibility paths in `scheduler.rs` and `llm.rs` still build LifeModel system prompts for explicitly non-governed callers only. Formal AgentRuntime / ExecutionFacade Chat, StreamChat, Scheduled, Skill, Plan, Replay, and governed fallback paths must not call them.

Needed Post-Beta direction:

- Expand `PromptBlockRegistry` with LifeModel/Memory/Task/OutputFormat blocks.
- Route any future Calibration model-assisted prompt through PromptStack before enabling model generation.
- Eventually remove ad-hoc system prompt construction from `llm.rs` and `ollama.rs` when all callers go through governed path.

## Memory and LifeModel Evolution

Current state:

- `ChatProposalGenerator` can create GoalUpdate, StateUpdate, CapabilityUpdate, and MemoryWrite proposals from conversation.
- `MemoryProposalGenerator` can create memory governance proposals from structured output.
- Feedback and micro-evolution flows can generate and apply LifeModel changes.
- MemoryWrite application writes memory records and vector chunks.

Current concern:

- Memory currently works as context and durable record, but not yet as a formal evidence layer for LifeModel evolution.
- There is no single `MemoryToLifeModelEngine` / `LifeModelEvolutionEngine` that aggregates accepted memories, detects patterns/contradictions/trends, and emits evidence-backed LifeModel proposals.
- Direct calibration apply modes should be reviewed under vNext governance.

Needed vNext direction:

Memory must be upgraded from retrieval store to evidence layer:

```text
Accepted Memories
-> Evidence Aggregation
-> Pattern / Contradiction / Trend Detection
-> LifeModel Impact Analysis
-> Evolution Proposal
-> User Review
-> Patch / Snapshot / Audit
```

## Test Coverage Notes

Current strengths:

- There are meaningful Rust tests around AgentLoop parsing, integration behavior, tool allowlists, proposal tools, and proposal application.
- The codebase does not appear to rely on large visible TODO/FIXME/HACK markers for known debt.

Current gaps for vNext:

- Full AgentLoop behavior tests for generated action execution.
- Streaming/non-streaming parity tests.
- Fallback event preservation tests.
- ToolRuntime policy matrix tests.
- PromptStack assembly tests.
- Memory evidence to LifeModel proposal tests.
- Sub-agent isolation tests before sub-agent feature work.

## Audit Conclusion

OpenLife is ready for an Agent Framework upgrade, but the safe order is:

```text
runtime convergence
-> event trace
-> tool policy hardening
-> prompt architecture
-> memory evidence/evolution
-> plan mode
-> sub-agents
-> bash/sandbox
```

Sub-agents and bash should not be the first implementation step. They should arrive after the framework has a stronger runtime authority.
