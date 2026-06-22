# Main Chat Agent Kernel Rescue Preparation

> Date: 2026-06-22
> Status: preparation artifact for `rescue/main-chat-kernel-prep`
> Baseline: `b78d707`
> Verification baseline: `cargo check -p openlife-core` and
> `cargo check -p openlife-tauri` passed on 2026-06-22

## 1. Purpose

This document prepares the rescue pass before runtime code changes begin.

Eight goal-mode delivery specs are indexed in
`plans/main_chat_agent_kernel_rescue_goal_mode_index.md`. This preparation file
defines the shared boundary for those goals.

The rescue target is not another readiness gate. The target is a small,
observable Main Chat agent kernel that can answer, use a bounded tool set,
produce observations, create proposals for durable changes, and expose the same
behavior through send and stream surfaces.

The current system has valuable parts, but the product path is too broad:
AgentIngress, strategy routing, HS packet construction, action queues,
proposal governance, final acceptance gates, live-provider evidence, and
productization evals are all close to the ordinary chat path. The first rescue
pass must narrow the path before it adds capability.

## 2. Rescue Hypothesis

OpenLife should be recovered by rebuilding the Main Chat runtime shape inside
the current repository:

```text
MainChatTurnInput
  -> MainChatKernel
  -> model decision
  -> optional governed action
  -> observation
  -> final answer or proposal/blocker
  -> MainChatTurnResult + events
```

`send_message` and `start_stream_message` should become transport adapters over
the same kernel, not separate implementations.

HS remains part of OpenLife, but the early rescue treats it as bounded
read-only context and proposal policy until Goal 6 explicitly reintegrates HS.
HS maturation, accepted guidance runtime mutation, final live-provider proof,
and broad readiness aggregation are not first-order rescue work.

## 3. Non-Goals

Do not do these in the first kernel rescue pass:

- do not delete HS, proposal stores, ActionExecutor, ModelRouter, or
  ToolPermission foundations;
- do not start a new repository during the first rescue spike;
- do not rewrite the frontend shell before the backend kernel exists;
- do not add new provider-ranked MCP selection logic;
- do not expand final acceptance gates;
- do not claim Main Chat Agent Execution v1 complete;
- do not silently persist durable LifeModel, Memory, file, calendar, email,
  external provider, plugin, or dangerous shell state from ordinary chat.

## 4. Freeze List

These areas are frozen unless a compile error or adapter boundary requires a
small touch:

- `src-tauri/src/main_chat_final_gate.rs`
- `src-tauri/src/main_chat_live_provider_harness.rs`
- `src-tauri/src/main_chat_command_surface_eval.rs`
- `src-tauri/src/main_chat_agent_stage*_*.rs`
- `src-tauri/src/main_chat_agent_productization_*.rs`
- broad readiness/product maturity report aggregation
- external live-provider proof paths
- provider-ranked MCP preselection credit rules
- maturation proposal generation beyond proposal-only boundaries

Frozen does not mean obsolete. It means these files must not steer the first
kernel rescue pass.

## 5. Reuse List

The rescue pass should reuse these foundations instead of rebuilding them:

- `openlife-core/src/scheduler.rs` for model routing and generation;
- `openlife-core/src/agent/action_executor/` for governed action execution;
- `openlife-core/src/tool_permissions.rs` for permission decisions;
- `openlife-core/src/mcp*` and `ToolManifest` registries for tool discovery;
- `src-tauri/src/main_chat_context_loader.rs` for bounded knowledge context;
- workspace-scoped file resolver helpers already used by Main Chat;
- proposal stores and Review Center data model for durable changes;
- AgentRun / transcript / observation records where they can be used without
  pulling the whole legacy strategy path into the kernel.

## 6. Kernel Boundary

The new kernel should start with a narrow API:

```rust
pub struct MainChatTurnInput {
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
    pub selected_skill_id: Option<String>,
}

pub trait MainChatEventSink {
    fn emit(&mut self, event: MainChatKernelEvent);
}

pub struct MainChatTurnResult {
    pub assistant_message: ChatMessage,
    pub tool_calls: Vec<ToolCall>,
    pub blockers: Vec<String>,
    pub proposals: Vec<String>,
    pub direct_writes_executed: bool,
    pub legacy_fallback_used: bool,
}
```

The concrete shape can change during implementation, but the boundary must
preserve the intent:

- one runtime entrypoint;
- send and stream share that entrypoint;
- user-visible state comes from kernel events;
- durable writes are explicit proposal or permission outcomes;
- unsupported capability is a named blocker, not a fake success.

## 7. First Rescue Tool Set

The first rescue program should only expose these action classes before broader
web/MCP/provider restoration. Goal 1 starts direct-answer-only; read tools are
introduced in Goal 3 and proposal-only writes in Goal 4.

| Class | Behavior |
| --- | --- |
| Direct answer | Model response with bounded context and no tools. |
| `file.read` | Workspace/safe-path scoped read only. |
| `session.search` | Read-only session context retrieval. |
| `memory.search` | Read-only memory retrieval. |
| `proposal.create` | Durable Memory/LifeModel/file/external change proposal only. |
| `web.read` | Either governed read or explicit network-policy blocker. |
| unsupported/write-like action | Explicit blocker or proposal request. |

Everything else can remain registered but unavailable to the kernel until the
basic loop is stable.

## 8. HS Handling During Rescue

HS must be downgraded during the first rescue pass:

- read selected bounded HS summaries as context;
- use HS policy to decide proposal vs blocker;
- do not allow ordinary chat to materialize accepted truth;
- do not run maturation as part of the kernel turn;
- do not let HS packet construction make direct answer or tool execution fail
  unless policy genuinely blocks the request.

The goal is to protect the user model while the agent loop becomes reliable.

## 9. Branch And Baseline

Preparation branch:

```text
rescue/main-chat-kernel-prep
```

Current baseline:

```text
HEAD: b78d707
git status: clean before preparation docs
cargo check -p openlife-core: passed
cargo check -p openlife-tauri: passed
```

Before runtime code changes, capture:

- current branch and commit;
- focused baseline commands;
- the K0-K8 kernel acceptance matrix;
- the completion report template;
- the planned adapter strategy for send/stream;
- a rollback point for any runtime path change.

## 10. Implementation Order

1. Add the kernel module with direct-answer-only behavior and an event sink.
2. Add focused unit tests for kernel result shape and no direct writes.
3. Add send adapter behind a preview flag or isolated helper.
4. Add stream adapter over the same kernel events.
5. Add the first read-only tool action path through ActionExecutor.
6. Add proposal-only write handling.
7. Move ordinary auto-checkin/materialization out of the kernel path or behind
   explicit proposal acceptance.
8. Simplify UI around kernel events only after the backend kernel is stable.

## 11. Stop Conditions

Stop and reassess if any of these happen:

- send and stream cannot share the same kernel without broad rewrites;
- direct answer requires final acceptance/live-provider machinery to work;
- file/session/memory read cannot be executed through ActionExecutor or a
  narrow adapter;
- ordinary chat still performs durable writes without a proposal;
- the new path starts duplicating large parts of `main_chat_strategy.rs`.

If a stop condition is hit, the next decision is whether to recover from the
archived dev branch or start OpenLife-next on a Pi/Hermes-inspired kernel.
