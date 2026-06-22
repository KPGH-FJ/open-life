# Goal 1: Main Chat Kernel Foundation

> Status: prepared for goal mode
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## Objective

Create a small shared Main Chat kernel that can produce direct answers with
bounded context, provider/model route metadata, no tools, no durable writes, and
no legacy fallback success claim, verified by focused kernel tests and
`cargo check -p openlife-core` / `cargo check -p openlife-tauri`.

## System Position

This goal creates the new runtime spine but does not make it the default command
surface yet. It should be small enough to reason about independently from
`main_chat_strategy.rs`, final gates, live-provider harnesses, and HS
maturation.

## OpenLife Lessons Applied

- Do not let readiness gates define the first usable agent loop.
- Do not let HS materialization enter the first kernel turn.
- Do not duplicate the existing strategy dispatcher under a new name.
- Preserve bounded context and route metadata because they are useful product
  assets.

## Industry Practices Applied

- Start with one working run before adding orchestration.
- Treat result shape as a product contract, not incidental return data.
- Emit traceable events early so later evals can inspect real behavior.

## Scope

Allowed implementation scope:

- add `src-tauri/src/main_chat_kernel.rs`;
- add narrow helper types for `MainChatTurnInput`, `MainChatTurnResult`,
  `MainChatKernelEvent`, and event sinks;
- add focused kernel tests;
- wire module declarations only where required for compilation;
- reuse scheduler/context-loading helpers when feasible.

Out of scope:

- changing default `send_message` / `start_stream_message` behavior;
- tool execution;
- proposal application;
- frontend UI changes;
- final acceptance/live-provider gate changes;
- HS maturation or accepted-truth materialization.

## Required Behavior

- valid user turn returns one assistant response;
- empty or invalid user turn returns a named blocker;
- selected skill id is sanitized before context use;
- provider/model route metadata is bounded and audit-safe;
- direct writes are always false;
- legacy fallback is always false for successful kernel results.

## Runtime Contracts

- `MainChatTurnInput`: session id, messages, optional selected skill id.
- `MainChatTurnResult`: assistant message, blockers, proposals, tool calls,
  route metadata, `direct_writes_executed`, `legacy_fallback_used`.
- `MainChatKernelEvent`: at minimum start, route selected, final answer,
  blocker.
- Persistence is optional in this goal, but any persisted record must be
  metadata-safe and no-write with respect to durable LifeModel/Memory truth.

## Acceptance Checklist

- [ ] Kernel module exists and compiles.
- [ ] Direct-answer kernel test passes.
- [ ] Empty-input blocker test passes.
- [ ] Selected-skill context test passes.
- [ ] No-direct-write assertion exists.
- [ ] No final/live/readiness gate is required for kernel success.

## Verification

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
```

If the focused test target does not exist yet, this goal must create it.

## Stop Conditions

- Direct answer cannot run without calling final acceptance machinery.
- Kernel needs to duplicate most of `main_chat_strategy.rs`.
- Direct answer requires ordinary chat auto-checkin materialization.
- The result/event surface cannot represent blocker vs final answer cleanly.
