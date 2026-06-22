# Main Chat Kernel Rescue Goal Completion Report

> Goal: 2 - Send/Stream Convergence
> Branch: rescue/main-chat-kernel-goal-2
> Date: 2026-06-22
> Base commit: ab44265
> Final commit: recorded in branch history after this report is committed
> Author/agent: Codex

## Objective

Route ordinary non-stream and stream Main Chat command surfaces through the same
MainChatKernel direct-answer path, preserving user-visible behavior while
removing duplicated runtime logic for the kernel slice.

## Scope Actually Changed

| File | Change type | Why it was needed |
| --- | --- | --- |
| `src-tauri/src/main_chat_kernel.rs` | Shared adapter/runtime support | Added command-surface direct-answer adapter result, streaming event sink, kernel-backed direct reflex/scheduler model client, route/generation metadata, durable event materialization, and task-session resolver for adapter evidence. |
| `src-tauri/src/main_chat_send.rs` | Adapter migration | Routes `DirectAnswer` through `BufferedMainChatEventSink -> MainChatKernel`; leaves non-direct strategies on the existing scoped legacy/strategy path. |
| `src-tauri/src/main_chat_streaming.rs` | Adapter migration | Routes `DirectAnswer` through `StreamingMainChatEventSink -> MainChatKernel`; emits the shared kernel result over current stream start/chunk/done events. |
| `src-tauri/src/main_chat_command_surface_tests.rs` | Focused tests | Adds send/stream kernel-backed success parity and invalid-input blocker parity, strengthens existing direct-answer command-surface assertions, and fixes MCP fallback/blocker command-surface cases to use deterministic scripted scheduler output instead of ambient local model behavior. |
| `src-tauri/src/main_chat_command_surface_eval.rs` | Eval assertion update | Requires DirectAnswer command-surface eval evidence to be kernel-backed with kernel event evidence and makes the MissingMcpBlocker scenario deterministic. |
| `plans/main_chat_agent_kernel_rescue_goal_2_completion_report.md` | Added report | Records Goal 2 acceptance and verification evidence. |

## Acceptance Checklist

- [x] Direct-answer send path uses kernel.
- [x] Direct-answer stream path uses kernel.
- [x] Send/stream parity test passes.
- [x] Invalid-input blocker appears on both surfaces.
- [x] Existing command-surface tests either pass or have scoped updates that reflect the new kernel boundary.

## Acceptance Matrix Rows

| ID | Evidence |
| --- | --- |
| K2-01 | `send_message_with_state` routes `DirectAnswer` through `BufferedMainChatEventSink` and `run_main_chat_kernel_direct_answer_with_state`; `send_message_direct_answer_records_main_chat_run_and_completes_task` and provider trace tests pass with `kernelBackedDirectAnswer=true`. |
| K2-02 | `start_stream_message_with_state` routes `DirectAnswer` through `StreamingMainChatEventSink` and the same shared helper; stream direct-answer tests pass with kernel-backed generation metadata and emitted kernel events. |
| K2-03 | `main_chat_kernel_direct_answer_send_stream_success_metadata_parity` passes, comparing reply, route metadata, fallback flag, direct-write flag, model-generation flag, and scheduler-generation flag across send and stream. |
| K2-04 | `main_chat_kernel_direct_answer_invalid_input_blocks_send_and_stream_with_same_metadata` passes with the same `invalid_session_id` blocker, no model generation, no scheduler call, no legacy fallback, and no direct writes on both surfaces. |
| K2-05 | DirectAnswer kernel metadata records `legacyFallbackUsed=false`; command-surface eval gate passed with `legacy_fallback_count=0`. Non-direct paths keep explicit existing fallback metadata. |
| K2-06 | DirectAnswer runtime logic is centralized in `run_main_chat_kernel_direct_answer_with_state`; send/stream only select buffered vs streaming sinks and transport the shared result. |

## Verification Commands

| Command | Result | Notes |
| --- | --- | --- |
| `cargo fmt --check` | Passed | No formatting drift. |
| `cargo check -p openlife-core` | Passed | Core crate unchanged but verified per Goal 2 requirement. |
| `cargo check -p openlife-tauri` | Passed | Tauri adapter/kernel changes compile. |
| `cargo test -p openlife-tauri main_chat_kernel -- --nocapture` | Passed | 9 passed, 0 failed. |
| `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` | Passed | 26 passed, 0 failed; includes the 24-case command-surface eval gate. |
| `git diff --check` | Passed | No whitespace errors after implementation; the untracked report file was also checked for trailing whitespace with `rg`. |

## Safety Evidence

| Invariant | Evidence |
| --- | --- |
| No silent durable LifeModel/Memory write | DirectAnswer adapter uses `LifeModel::default()` instead of `LifeModelManager::load()` and records `directWritesExecuted=false`; command-surface eval reports `silent_write_count=0`. |
| No unsafe file/calendar/email/provider/plugin/shell side effect | Goal 2 DirectAnswer adapter exposes no tool/proposal execution and returns empty tool calls; read/write/tool paths remain outside this slice. |
| Unsupported capabilities fail closed | Non-direct tool/proposal/PlanExecute requests still route to existing governed strategy paths; invalid direct-answer input returns named kernel blockers. |
| Send/stream parity preserved where applicable | Successful direct-answer parity and invalid-input parity tests pass across send and stream. |
| UI claims backed by runtime evidence where applicable | No frontend changes; stream emits kernel events and send/stream responses include kernel-backed generation/transcript metadata. |

## Legacy/Fallback Evidence

```text
legacy_fallback_used: false for kernel-backed DirectAnswer
legacy_fallback_count: 0 in main_chat_command_surface eval gate
why_still_needed: Non-direct legacy/fallback paths remain explicit for later goals; Goal 2 only migrates the DirectAnswer slice.
```

## Direct Write Evidence

```text
direct_writes_executed: false for kernel-backed DirectAnswer
direct_write_count: 0 / silent_write_count=0 in main_chat_command_surface eval gate
proposal_or_permission_records: none created by Goal 2 DirectAnswer; proposal/tool paths are out of scope and remain on existing governed paths.
```

## Source And Practice Consistency Check

Confirmed the implementation does not conflict with:

- `plans/main_chat_agent_kernel_rescue_industry_practices.md`
- `plans/main_chat_agent_kernel_rescue_spec_coding_contract.md`
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`
- `AGENTS.md`

No external source was used for this implementation. Goal 2 did not expand into
read-only tools, proposal-only writes, HS reintegration, frontend redesign, or
final/live-provider gate changes.

## Residual Risk

| Risk | Blocks next goal? | Follow-up |
| --- | --- | --- |
| The command-surface direct-answer adapter still preserves existing chat message persistence for successful user/assistant messages. | No | Goal 4 can further separate proposal-only write semantics; Goal 2 only asserts no silent durable LifeModel/Memory truth write. |
| DirectAnswer context is bounded through existing context transcript plus kernel context candidates; broader HS reintegration remains absent by design. | No | Goal 6 owns HS reintegration. |
| Non-direct ReAct, PlanExecute, proposal, web, and MCP paths remain on existing strategy/fallback infrastructure. | No | Goals 3, 4, 7, and 8 own those migrations and cleanup. |
