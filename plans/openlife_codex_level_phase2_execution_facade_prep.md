# OpenLife Codex-Level Phase 2 Execution Facade Prep

Date: 2026-05-24

Status: ready-for-agent-assignment

Scope: Post-Beta Phase 2 / Codex-level execution path convergence.

## 1. Why This Phase Exists

OpenLife has completed the main governance hardening work for replay, MCP target governance, fake MCP fallback removal, scheduler task reliability, and trace contract drift checks. The next Codex-level gap is not another isolated tool fix. The gap is runtime authority:

```text
All formal Agent behavior must enter one governed, traceable execution harness.
```

Current formal or near-formal paths still live across separate Tauri functions:

- `src-tauri/src/lib.rs::send_message`
- `src-tauri/src/streaming.rs::start_stream_message`
- `src-tauri/src/scheduler_runner.rs::execute_scheduled_task`
- `src-tauri/src/lib.rs::execute_tool_call`
- `src-tauri/src/commands/agent.rs::replay_agent_action`
- Builder / Calibration direct LLM paths, to be migrated later

This phase starts the Tauri-side `ExecutionFacade`, but it must be incremental. Do not rewrite chat, streaming, builder, calibration, or replay in one patch.

## 2. Current Baseline

Recent verified state:

- `make ci` passed on 2026-05-24 after scheduler lease repair.
- `cargo test -p openlife-tauri scheduler -- --nocapture`: 12 passed.
- `cargo test -p openlife-tauri proposal -- --nocapture`: 34 passed with unrestricted filesystem access; normal workspace sandbox blocks app-data writes.
- `cargo test -p openlife-tauri replay -- --nocapture`: 15 passed.
- `openlife-tauri` full tests in `make ci`: 116 passed.

Important current files:

- `src-tauri/src/execution_deps.rs`
  - already has `build_loop_config`
  - already has `build_agent_loop`
  - already has `build_agent_task`
  - already has `assemble_action_context`
- `src-tauri/src/lib.rs`
  - non-stream chat currently builds AgentLoop and ActionContext directly
  - direct tool execution currently builds ActionContext directly
  - `handle_agent_loop_fallback` still lives here
- `src-tauri/src/streaming.rs`
  - stream chat duplicates large parts of non-stream AgentLoop setup
  - stream fallback still has separate handling
- `src-tauri/src/scheduler_runner.rs`
  - has safe task claim / merge semantics
  - still builds AgentLoop directly for scheduled tasks

## 3. Phase 2 Hard Invariants

The next Agent must preserve these invariants:

- Do not weaken `AgentSpec` resolution. Formal paths must fail closed when required spec resolution fails.
- Do not pass `agent_spec: None` in formal Agent execution paths.
- Do not bypass `ExecutionSandbox`, `NetworkPolicy`, `ToolPermissionStore`, or `PrivacyPolicy`.
- Do not remove scheduler lease semantics:
  - active `running` tasks are not reclaimed
  - `running` without `running_started_at` is skipped
  - stale reclaim requires the 30-minute threshold
- Do not convert tool failures into fake successes.
- Do not expose declarative-only or stub tools as executable.
- Do not remove `AgentRunEvent` recording.
- Do not rewrite ChatPage or frontend UX in this phase.

## 4. Target Architecture For This Phase

Create a Tauri-side facade module:

```text
src-tauri/src/execution_facade.rs
```

The facade should initially centralize shared runtime assembly and outcome shape, not migrate every caller at once.

Recommended first-stage types:

```rust
pub enum TauriAgentExecutionMode {
    Chat,
    StreamChat,
    Scheduled,
    ToolExecution,
    Replay,
    Builder,
    Calibration,
}

pub struct TauriAgentExecutionInput {
    pub mode: TauriAgentExecutionMode,
    pub task: openlife_core::agent::AgentTask,
    pub life_model: openlife_core::life_model::LifeModel,
    pub tools_prompt: String,
    pub privacy_engine: openlife_core::privacy::PrivacyEngine,
    pub agent_spec: openlife_core::agent::types::AgentSpec,
    pub prompt_registry: openlife_core::agent::prompt_stack::PromptBlockRegistry,
}

pub struct TauriAgentExecutionOutcome {
    pub reply: String,
    pub run: openlife_core::agent::AgentRun,
    pub fallback_used: bool,
    pub fallback_reason: Option<String>,
    pub warnings: Vec<String>,
}
```

The exact names may vary, but the shape must be explicit and typed. Do not smuggle runtime state through strings.

## 5. Recommended Batch Plan

### Batch A: Facade Skeleton + Shared Runtime Assembly

Goal:

- Add `src-tauri/src/execution_facade.rs`.
- Move duplicated AgentLoop / ActionContext assembly into explicit facade helpers.
- Do not change user-visible behavior.
- Keep `send_message`, `start_stream_message`, and `execute_scheduled_task` behavior equivalent.

Expected changes:

- Add module declaration in `src-tauri/src/lib.rs` or `src-tauri/src/main module` as appropriate.
- Add helpers such as:
  - `build_governed_agent_loop(...)`
  - `build_governed_action_context(...)`
  - `resolve_default_agent_spec_fail_closed(...)`
  - `run_agent_loop_non_stream(...)`
- Prefer reusing `execution_deps.rs` helpers instead of duplicating logic.
- Keep old caller functions as thin wrappers for now.

Required tests:

- `execution_facade_builds_action_context_with_agent_spec`
- `execution_facade_builds_action_context_with_sandbox_from_config`
- `execution_facade_non_stream_chat_matches_existing_agentloop_config`
- `execution_facade_scheduled_mode_uses_restricted_toolset`

Validation:

```bash
cargo fmt --check
cargo clippy -p openlife-tauri -- -D warnings
cargo test -p openlife-tauri execution_facade -- --nocapture
cargo test -p openlife-tauri scheduler -- --nocapture
cargo test -p openlife-tauri replay -- --nocapture
make ci
```

### Batch B: Non-Stream Chat Through Facade

Goal:

- Route `send_message_with_agent_loop` through the new facade.
- Preserve current L1 reflex branch as an explicit pre-facade shortcut.
- Preserve fallback behavior and proposal generation.

Required tests:

- `chat_facade_non_stream_creates_agent_run`
- `chat_facade_non_stream_records_agent_spec`
- `chat_facade_non_stream_fallback_records_events`
- Existing chat tests still pass.

### Batch C: Scheduled Task Through Facade

Goal:

- Route `execute_scheduled_task` through the facade.
- Preserve scheduler lease / merge semantics unchanged.
- Preserve restricted Planner role and tool allowlist.

Required tests:

- `scheduled_facade_uses_planner_role`
- `scheduled_facade_uses_restricted_toolset`
- `scheduler_active_running_task_is_not_reclaimed`
- `scheduler_stale_running_task_is_reclaimed_after_threshold`

### Batch D: Stream Chat Through Facade

Goal:

- Route stream AgentLoop setup through the facade.
- Keep `TauriStreamingCallback` behavior stable.
- Preserve fallback chunk emission semantics.

Required tests:

- `stream_facade_uses_same_agent_spec_policy_as_non_stream`
- `stream_facade_agentloop_failure_fallback_emits_error_or_chunk`
- Existing frontend stream tests still pass.

## 6. Non-Goals For The Next Agent

The next Agent must not:

- migrate Builder and Calibration yet;
- change frontend components;
- change scheduler JSON schema except if a test proves it is necessary;
- alter `RUNNING_TASK_STALE_AFTER_SECONDS`;
- remove existing fallback behavior;
- convert fallback to a separate untracked run;
- introduce new model prompts outside PromptStack;
- change MCP target governance, replay governance, or tool permission semantics.

## 7. Review Checklist

Before accepting the next batch:

- Does every formal ActionContext include `agent_spec: Some(...)`?
- Does ExecutionSandbox come from config and safe paths?
- Does Scheduled mode keep `allow_writes: false` and restricted tool allowlist?
- Does Chat mode still use PromptStack and AgentSpec privacy policy?
- Does fallback remain traceable?
- Are tests behavior-focused rather than only constructor snapshots?
- Does `make ci` pass?

## 8. Agent Development Prompt

Use this prompt for the next implementation Agent:

```text
You are working in /Users/fujing/Desktop/偶来福.

Task: Codex-level Phase 2 Batch A — create the Tauri-side ExecutionFacade skeleton and shared runtime assembly helpers.

Read first:
- AGENTS.md
- plans/openlife_post_beta_roadmap.md
- plans/openlife_codex_level_upgrade_plan.md
- plans/openlife_codex_level_task_breakdown.md
- plans/openlife_vnext_execution_entrypoints.md
- plans/openlife_codex_level_phase2_execution_facade_prep.md

Context:
OpenLife is continuing the Codex / Claude Code-level Agent Runtime upgrade. Recent scheduler lease and merge-by-id repairs have passed validation. The next phase is execution path convergence. The goal is not a broad rewrite; the goal is to create a typed Tauri-side facade so formal Agent paths can be migrated safely in later batches.

Scope for this batch:
1. Add `src-tauri/src/execution_facade.rs`.
2. Add explicit mode/input/outcome types for the Tauri-side execution facade.
3. Centralize shared AgentLoop and ActionContext assembly logic currently duplicated across:
   - `src-tauri/src/lib.rs::send_message_with_agent_loop`
   - `src-tauri/src/streaming.rs::start_stream_message_with_agent_loop`
   - `src-tauri/src/scheduler_runner.rs::execute_scheduled_task`
4. Reuse `src-tauri/src/execution_deps.rs` where possible.
5. Keep public behavior unchanged in this batch. Existing caller functions may remain wrappers.

Hard constraints:
- Do not rewrite ChatPage or frontend UX.
- Do not migrate Builder/Calibration in this batch.
- Do not weaken AgentSpec governance.
- Do not pass `agent_spec: None` in formal Agent execution paths.
- Do not bypass ExecutionSandbox, NetworkPolicy, ToolPermissionStore, PrivacyPolicy, or PromptStack.
- Do not change scheduler lease semantics:
  active `running` tasks must not be reclaimed;
  `running` without `running_started_at` must not be reclaimed;
  stale reclaim requires the existing 30-minute threshold.
- Do not introduce fake success paths or MCP fallback.
- Do not remove existing fallback behavior.

Required tests:
Write tests before or alongside implementation. Add focused Rust tests for:
- `execution_facade_builds_action_context_with_agent_spec`
- `execution_facade_builds_action_context_with_sandbox_from_config`
- `execution_facade_non_stream_chat_matches_existing_agentloop_config`
- `execution_facade_scheduled_mode_uses_restricted_toolset`

Implementation guidance:
- Start with a skeleton facade and helper functions; avoid migrating all paths at once.
- A minimal acceptable facade can expose:
  - `TauriAgentExecutionMode`
  - `TauriAgentExecutionInput`
  - `TauriAgentExecutionOutcome`
  - helpers for governed loop/action context assembly
- Prefer typed structs/enums over stringly-typed mode flags.
- Keep functions small and testable.
- If a behavior is intentionally not migrated yet, document it in the module comment.

Validation commands:
Run all of these:
```bash
cargo fmt --check
cargo clippy -p openlife-tauri -- -D warnings
cargo test -p openlife-tauri execution_facade -- --nocapture
cargo test -p openlife-tauri scheduler -- --nocapture
cargo test -p openlife-tauri replay -- --nocapture
make ci
```

Final report must include:
- Files changed
- Facade types/helpers added
- Which paths were only prepared vs actually migrated
- Tests added and results
- `make ci` result
- Governance boundaries touched
- Residual risks
```

