# Goal 2: Send/Stream Convergence

> Status: prepared for goal mode
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## Objective

Route ordinary non-stream and stream Main Chat command surfaces through the same
MainChatKernel direct-answer path, preserving user-visible behavior while
removing duplicated runtime logic for the kernel slice.

## System Position

This goal moves the first kernel slice from isolated helper to command-surface
adapter. It must reduce divergence between `send_message` and
`start_stream_message` without migrating every legacy strategy at once.

## OpenLife Lessons Applied

- Existing send/stream duplication is a root cause of Main Chat drift.
- A transport command should not own business logic.
- Legacy fallback must remain explicit and measurable while migration is
  incomplete.

## Industry Practices Applied

- Keep the final answer, resumable state, and interruption surfaces distinct.
- Use streaming as transport over the same run semantics, not a second runtime.
- Trace first; broad readiness gates come after behavior is stable.

## Scope

Allowed implementation scope:

- update `src-tauri/src/main_chat_send.rs`;
- update `src-tauri/src/main_chat_streaming.rs`;
- add buffered and streaming event sinks;
- add focused command-surface tests for kernel-backed direct answer;
- keep legacy strategy path available only as an explicit fallback/legacy path
  while this goal migrates the direct-answer slice.

Out of scope:

- broad frontend redesign;
- read-only tools;
- proposals;
- HS maturation;
- final/live-provider evidence expansion.

## Required Behavior

```text
send_message
  -> BufferedEventSink
  -> MainChatKernel

start_stream_message
  -> StreamingEventSink
  -> MainChatKernel
```

Both surfaces must preserve:

- same final answer semantics;
- same no-direct-write guarantee;
- same route metadata shape;
- same blocker semantics for invalid input;
- no hidden success through legacy fallback.

## Runtime Contracts

- `BufferedEventSink` converts kernel events to `SendMessageResult`.
- `StreamingEventSink` emits the same event semantics over stream events.
- Direct-answer result fields must have the same meaning on send and stream.
- Adapter code must not re-run intent/layer/strategy logic separately for the
  kernel slice.

## Acceptance Checklist

- [ ] Direct-answer send path uses kernel.
- [ ] Direct-answer stream path uses kernel.
- [ ] Send/stream parity test passes.
- [ ] Invalid-input blocker appears on both surfaces.
- [ ] Existing command-surface tests either pass or have scoped updates that
      reflect the new kernel boundary.

## Verification

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
```

## Stop Conditions

- Stream and send cannot share the same kernel result/event model.
- Migration requires weakening blocker or no-direct-write semantics.
- The adapter starts rebuilding strategy routing inside both command files.
- Existing tests require preserving a duplicated behavior that conflicts with
  the kernel contract.
