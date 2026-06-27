# Main Chat Kernel Rescue Goal Completion Report

> Goal: 3 - Minimal Read-Only Tools
> Branch: rescue/main-chat-kernel-goal-3
> Date: 2026-06-22
> Base commit: c8b8dd6
> Final commit: recorded in branch history after this report is committed
> Author/agent: Codex

## Objective

Add a minimal governed read-only tool loop to MainChatKernel for workspace file
read, session search, memory search, and explicit web/network blocker behavior,
verified on both send and stream surfaces without silent writes or fake tool
success.

## Scope Actually Changed

| File | Change type | Why it was needed |
| --- | --- | --- |
| `src-tauri/src/main_chat_kernel.rs` | Kernel read-only tool loop | Added deterministic minimal read-tool planning, governed input construction, ActionExecutor-backed read execution, tool decision/observation events, action-queue/transcript evidence, read synthesis, unsupported/web blockers, and a guard that keeps broad legacy ReAct/final/live dependencies out of the kernel slice. |
| `src-tauri/src/main_chat_send.rs` | Command-surface routing | Routes kernel-supported ReAct read-only turns through the shared kernel path while leaving unsupported non-direct strategies on their existing explicit paths. |
| `src-tauri/src/main_chat_streaming.rs` | Command-surface routing | Applies the same kernel-supported read-only routing to stream startup so send and stream share outcome semantics. |
| `src-tauri/src/main_chat_command_surface_tests.rs` | Focused command-surface tests | Added send/stream tests for workspace file read, path traversal blocker, session search, memory search, web read unavailable blocker, and unknown-tool blocker with task-session/action evidence assertions. |
| `src-tauri/src/main_chat_command_surface_eval.rs` | Eval assertion update | Allows the existing file/session eval rows to accept the new kernel read-only loop evidence while still requiring completed queue actions, read evidence, and no direct writes. |
| `src-tauri/src/main_chat_react_runtime.rs` | Shared read metadata support | Classifies `web.read` as a governed read observation so unavailable web reads can be represented without network execution or fake success. |
| `openlife-core/src/agent/main_chat_agent_v1.rs` | Execution policy classification | Marks `web.read` as read-only for queue policy classification; it still blocks in the minimal kernel when governed web execution is unavailable. |
| `plans/main_chat_agent_kernel_rescue_goal_3_completion_report.md` | Added report | Records Goal 3 acceptance, verification evidence, safety evidence, and residual risks. |

## Acceptance Checklist

- [x] File read success case passes.
- [x] Path traversal blocker case passes.
- [x] Session search case passes.
- [x] Memory search case passes.
- [x] Unknown tool blocker case passes.
- [x] Send and stream surfaces produce equivalent outcomes.
- [x] Tool observations feed follow-up synthesis.
- [x] Model-provided arguments cannot bypass governed executor input.

## Acceptance Matrix Rows

| ID | Evidence |
| --- | --- |
| K3-01 | `main_chat_kernel_goal_3_workspace_file_read_send_stream_records_observation` passes; send returns a `file.read` tool call with successful output and stream records the same completed queue action with `file_system_read` evidence. |
| K3-02 | `main_chat_kernel_goal_3_path_traversal_send_stream_blocks_filesystem_read` passes; `../AGENTS.md` produces `filesystem_path_traversal_blocked`, failed read action evidence, and no outside-workspace read. |
| K3-03 | `main_chat_kernel_goal_3_session_search_send_stream_uses_bounded_prior_context` passes; seeded prior context is retrieved through bounded `session.search` and appears in the synthesized answer. |
| K3-04 | `main_chat_kernel_goal_3_memory_search_send_stream_is_read_only` passes; `memory.search` records `memory_read` evidence with `directWritesExecuted=false` and does not mutate memory. |
| K3-05 | `main_chat_kernel_goal_3_web_read_unavailable_send_stream_blocks_without_fake_success` passes; `web.read` returns a named unavailable/network blocker rather than a fabricated web result. |
| K3-06 | `main_chat_kernel_goal_3_unknown_tool_send_stream_blocks_without_fallback` passes; unknown tool requests return `unsupported_tool` evidence with `legacyFallbackUsed=false`. |
| K3-07 | `main_chat_kernel_read_tool_ignores_model_supplied_arguments` passes; malicious model-supplied arguments are ignored and executor input comes from governed kernel planning. |

## Verification Commands

| Command | Result | Notes |
| --- | --- | --- |
| `cargo fmt --check` | Passed | No Rust formatting drift after implementation. |
| `cargo check -p openlife-core` | Passed | Core execution-policy update compiles. |
| `cargo check -p openlife-tauri` | Passed | Kernel, command-surface, and test changes compile. |
| `cargo test -p openlife-tauri main_chat_kernel -- --nocapture` | Passed | 16 passed, 0 failed. |
| `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` | Passed | 32 passed, 0 failed; includes the command-surface eval gate and all Goal 3 send/stream cases. |
| `git diff --check` | Passed | No whitespace errors, including the completion report. |

If a command was not run: none.

## Safety Evidence

| Invariant | Evidence |
| --- | --- |
| No silent durable LifeModel/Memory write | Kernel read loop metadata records `directWritesExecuted=false`; command-surface eval continues to assert `silent_write_count=0`; memory search is read-only observation evidence only. |
| No unsafe file/calendar/email/provider/plugin/shell side effect | Goal 3 only enables `file.read`, `session.search`, `memory.search`, and unavailable `web.read` blocker behavior; no calendar, email, provider, plugin, shell, or write tools were added. |
| Unsupported capabilities fail closed | Unknown tool requests produce an explicit unsupported-tool blocker; web reads without a governed path produce named network/unavailable blockers; MCP prompts remain outside this minimal planner. |
| Send/stream parity preserved where applicable | Each Goal 3 command-surface test exercises both `send_message` and `start_stream_message` and verifies equivalent task-session/action outcomes. |
| UI claims backed by runtime evidence where applicable | No frontend UI copy was changed; command responses, task sessions, queued actions, transcripts, and reasoning metadata carry the read observation/blocker evidence used for user-visible replies. |

## Legacy/Fallback Evidence

```text
legacy_fallback_used: false for kernel-backed Goal 3 read-only tool turns
legacy_fallback_count: 0 in main_chat_command_surface eval gate
why_still_needed: Existing non-Goal-3 ReAct, MCP, proposal, PlanExecute, and live-provider paths remain explicit for later rescue goals; Goal 3 only migrates the minimal read-only slice.
```

## Direct Write Evidence

```text
direct_writes_executed: false for Goal 3 read-only tool observations and blockers
direct_write_count: 0 / silent_write_count=0 in main_chat_command_surface eval gate
proposal_or_permission_records: none created by Goal 3; unsupported or unavailable capabilities block instead of creating write proposals.
```

## Source And Practice Consistency Check

Confirmed the implementation does not conflict with:

- `plans/main_chat_agent_kernel_rescue_industry_practices.md`
- `plans/main_chat_agent_kernel_rescue_spec_coding_contract.md`
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`
- `AGENTS.md`

No external source was used for this implementation. Goal 3 did not expand into
write tools, provider-ranked MCP, external live-provider proof, broad web/MCP
restoration, UI redesign, HS reintegration, or final acceptance completion.

## Residual Risk

| Risk | Blocks next goal? | Follow-up |
| --- | --- | --- |
| The read-tool planner is intentionally minimal and deterministic, so natural language coverage is narrow. | No | Later goals can add richer selection once the kernel contract is stable. |
| `web.read` is classified as read-only but still blocks in the minimal kernel unless a governed web path exists. | No | A later web/MCP goal should add auditable governed web execution instead of treating the blocker as success. |
| MCP prompts intentionally stay outside the Goal 3 kernel planner. | No | Provider-ranked MCP and registered manifest selection remain later-goal work. |
| Successful file reads still rely on the existing workspace resolver and safe-path setup. | No | Continue expanding resolver coverage only with tests that prove no traversal or outside-root read. |
