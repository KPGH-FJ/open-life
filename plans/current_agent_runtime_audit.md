# Current Agent Runtime Audit

Date: 2026-05-10 (updated: Codex-level Final Closeout, 2026-05-26)

> **Updated note (2026-05-10)** : P0-P12 vNext primitives have all been implemented in code.
> The "gaps" described below (AgentRunEvent, PromptStack, ToolRuntime, MemoryEvidence,
> ExecutionSandbox, PlanMode, SubAgent) are now implemented and tested.
>
> **Closeout note (2026-05-26)** : Codex-level stabilization has now closed the runtime
> substrate boundaries. Chat / StreamChat, Scheduled, Direct Tool, Replay, and Plan execution
> use Tauri/core facade or wrapper paths; formal model entrypoints are PromptStack-governed;
> AgentSpec governance is fail-closed; Builder model helpers are Builder-specific PromptStack +
> LocalOnly; Calibration is deterministic / proposal-only / not applicable for PromptStack;
> Runtime fallback is a governed legacy compatibility retry for Runtime/model failure only.
> Remaining items below are non-blocking engineering debt or LifeModel-stage work unless they
> are explicitly marked as active blockers.

## Summary

OpenLife is not a broken project. The current codebase has a real Agent Framework spine:

- `AgentLoop` exists and executes an iterative ReAct-style loop.
- `ActionExecutor` exists and owns many core/execution tool paths.
- `ProposalEngine` and `ProposalStore` exist.
- Chat, Builder, Calibration, Memory, ToolPermission, and ExternalWriteAction flows are proposal-first for high-risk writes.
- AgentRun tracing uses append-only `AgentRunEvent` records with typed payload builders for governance, PromptStack metadata, context governance, replay, and runtime fallback events.

The main gap is no longer runtime substrate convergence. The remaining large work is the next LifeModel phase: evidence-backed LifeModel Evolution / Editor / Review depth, plus non-blocking engineering debt such as `lib.rs` size, ChatPage decomposition, release packaging, and platform validation.

## Current Execution Paths

Observed code paths:

- `src-tauri/src/lib.rs::send_message`
- `src-tauri/src/lib.rs::send_message_with_agent_loop`
- `src-tauri/src/lib.rs::start_stream_message`
- `src-tauri/src/lib.rs::start_stream_message_with_agent_loop`
- L1 reflex path in streaming mode
- Runtime fallback governed legacy compatibility retry after Runtime/model failure
- Scheduled task execution path in `src-tauri/src/scheduler_runner.rs`

Current status:

- Chat and StreamChat enter Tauri `ExecutionFacade` and then `AgentRuntime::execute_task_with_spec`.
- Scheduled execution uses a Scheduled-specific wrapper and does not inherit Chat fallback.
- Direct Tool, Replay, and Plan execution are no-model wrapper/action paths; they do not emit fake PromptStack traces.
- Skill runtime remains outside Chat ExecutionFacade by design, but model execution is Skill PromptStack-governed through stored `AgentSpec`.
- Runtime fallback is retained only as governed legacy compatibility retry: Runtime/model failures may fallback, Governance failures fail closed, and `fallback.started` / `fallback.completed` / `fallback.failed` payloads are metadata-only.
- `src-tauri/src/lib.rs` is still a large orchestration file, but size reduction is non-blocking for LifeModel phase entry.

Closeout conclusion:

All formal model paths that currently create AgentRun records have a governed prompt/runtime boundary or an explicit not-applicable classification. New LifeModel-stage model entrypoints must choose one of `governed`, `legacy compatibility`, or `not applicable` before implementation.

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

Legacy vNext direction (implemented as of P0-P12; keep for historical context):

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

Current residual concern:

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

- Calibration direct apply modes are default-disabled and remain explicit legacy compatibility gates.
- Proposal generation and proposal application are real; richer evidence linkage is still needed for the LifeModel phase.
- Proposal sources exist, but vNext needs richer evidence linkage, especially for memory-driven LifeModel evolution.

Next LifeModel direction:

- Treat proposal-first as a framework-level side-effect protocol, not as UI convention.
- High-risk LifeModel fields must never silently auto-apply.
- All proposal creation, edit, accept, reject, apply, fail, replay, and rollback should be represented as `AgentRunEvent` or linked audit events.

## Prompt / System Prompt

Current state (updated 2026-05-10):

- **PromptStack** (`agent/prompt_stack.rs`, 922行) is implemented with `PromptBlock` and `PromptBlockRegistry` (4 built-in blocks: base_system, planning, tool_discipline, privacy_rule).
- AgentLoop-governed model paths (Chat, StreamChat, Scheduled, Skill runtime, plus model helper boundaries where documented) assemble PromptStack via `execute_task_with_spec()` or a dedicated helper stack. Proactive suggestion generation is not applicable because it does not create an AgentRun or model PromptStack trace.
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

Current residual concern:

- `PromptBlockRegistry::built_in()` has expanded with AgentSpec, proposal, web, layered, builder, and skill-specific blocks. Future LifeModel-stage model entrypoints may still need dedicated LifeModel/Evidence/Editor blocks.
- Legacy `generate()` / `generate_stream()` compatibility paths in `scheduler.rs` and `llm.rs` still build LifeModel system prompts for explicitly non-governed callers only. Formal AgentRuntime / ExecutionFacade Chat, StreamChat, Scheduled, Skill, Plan, Replay, and governed fallback paths must not call them.
- Chat / StreamChat runtime fallback is intentionally retained as a governed legacy compatibility retry, not a first-class fallback mode. It only handles Runtime/model failures; Governance failures fail closed. `fallback.started` / `fallback.completed` / `fallback.failed` payloads are metadata-only and retain `agent_spec_id`, `privacy_policy`, `generation_path`, PromptStack source, and sanitized error summaries without raw prompt, raw user text, raw LifeModel, raw memory, or full model output.

Next-stage direction:

- Add LifeModel / Evidence / Editor / Review-specific PromptBlocks only when those model entrypoints are implemented.
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

Remaining gaps for LifeModel phase / non-blocking hardening:

- Broader full AgentLoop behavior tests for generated action execution.
- Additional streaming/non-streaming parity tests beyond the locked fallback/governance cases.
- ToolRuntime policy matrix tests for future tools.
- Memory evidence to LifeModel proposal tests.
- Sub-agent isolation tests before sub-agent feature work.

## Audit Conclusion

OpenLife has completed the Codex-level Agent Framework stabilization needed before LifeModel deep development. The safe next order is:

```text
Codex-level acceptance report
-> LifeModel phase entry gate
-> evidence-backed LifeModel Evolution proposals
-> LifeModel Editor / Review workflow hardening
-> broader product and release readiness
```

Sub-agents, bash expansion, and release packaging should remain out of the LifeModel entry slice unless a separate acceptance gate explicitly scopes them.
