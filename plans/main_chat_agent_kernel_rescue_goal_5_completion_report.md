# Main Chat Kernel Rescue Goal Completion Report

> Goal: 5 - Minimal Execution UX
> Branch: rescue/main-chat-kernel-goal-5
> Date: 2026-06-22
> Base commit: 749608f672de7165d03300b220587eb308a1f48a
> Final commit: recorded in branch history after this report is committed.
> Author/agent: Codex

## Objective

Update the Main Chat user experience so it displays real MainChatKernel evidence
for thinking, tool execution, observations, proposals, blockers, and permission
needs, without exposing readiness/final-gate noise as the primary user surface.

## Scope Actually Changed

| File | Change type | Why it was needed |
| --- | --- | --- |
| `frontend/src/components/MainChatExecutionEvidence.tsx` | New UI component | Adds the default evidence-first Main Chat surface backed only by kernel events, typed agent state, task state, tool calls, proposals, blockers, and final delivery evidence. |
| `frontend/src/pages/ChatPage.tsx` | Runtime-to-UI mapping | Subscribes to `main-chat-kernel-event`, keeps kernel events scoped to the active session, renders execution evidence before diagnostics, hides reasoning trace/tool cards/control-plane/runtime disclosure behind a diagnostics toggle, and renders cancel state from task evidence. |
| `frontend/src/tauri.ts` | Frontend runtime contract type | Adds the typed `MainChatKernelEvent` discriminated union matching the Rust event serialization used by the stream event bridge. |
| `frontend/src/pages/ChatPage.test.tsx` | UI acceptance tests | Adds Goal 5 tests for direct answer, thinking/tool-running/observation, proposal/permission/blocker, cancel, and no-fake UI behavior; updates existing diagnostics assertions to open the diagnostics toggle first. |
| `plans/main_chat_agent_kernel_rescue_goal_5_completion_report.md` | Added report | Records Goal 5 acceptance, verification evidence, safety evidence, fallback/direct-write evidence, and residual risk. |

## Acceptance Checklist

- [x] Direct answer appears without debug clutter.
- [x] Tool read progress and observation are visible.
- [x] Proposal-created state links to inspectable proposal.
- [x] Blocker state shows reason and next action.
- [x] UI never claims a read/write/remember action without runtime evidence.
- [x] Readiness/final gate details are hidden from the default chat experience.

## Acceptance Matrix Rows

| ID | Evidence |
| --- | --- |
| K5-01 | `shows direct answer execution evidence without default diagnostics clutter` passes; direct answer renders `Execution evidence` and `Final answer`, while `Agent Control Plane` and `Reviewer trace` are absent from the default answer flow. |
| K5-02 | `renders kernel-event thinking, tool running, and tool observation states` passes; emitted `turn_started`, `context_loaded`, and `tool_decision` events render `Thinking`, `Tool running`, and the governed target `AGENTS.md`. |
| K5-03 | The same kernel-event test passes after a `tool_observation` event; the UI renders `Tool observation` with the bounded preview `Main Chat evidence mapping found in AGENTS.md.`. |
| K5-04 | `renders proposal, permission, and blocker evidence with review navigation` passes; pending proposal evidence renders `Proposal created` and an `Open proposal` link to `/review` with proposal/task navigation state. |
| K5-05 | The proposal/permission/blocker test passes; permission evidence renders `Permission needed`, blocker evidence renders `Blocked`, and the UI shows next-step copy plus Review Center navigation. |
| K5-06 | `cancels an in-progress stream task and renders canceled state from task evidence` passes; `cancel_main_chat_agent_task` is invoked with the task session id, the UI renders `Canceled` and the task final summary, and no `Final answer` is claimed. |
| K5-07 | `does not infer productized actions or observations from assistant text` passes; assistant text containing fake tool/action words does not create action/observation UI unless typed runtime evidence exists. |

## Verification Commands

| Command | Result | Notes |
| --- | --- | --- |
| `pnpm --dir frontend exec prettier --write src/components/MainChatExecutionEvidence.tsx src/pages/ChatPage.tsx src/pages/ChatPage.test.tsx src/tauri.ts` | Passed | Prettier reported the existing `jsxBracketSameLine` deprecation warning only. |
| `pnpm --dir frontend typecheck` | Passed | `tsc --noEmit` completed successfully. |
| `pnpm --dir frontend test -- --run src/pages/ChatPage.test.tsx --reporter=basic` | Passed | 56 passed, 0 failed. |
| `pnpm --dir frontend test -- --run --reporter=basic` | Passed | 38 test files passed, 461 tests passed. |
| `cargo check -p openlife-core` | Passed | Core package compiled successfully. |
| `cargo check -p openlife-tauri` | Passed | Tauri package compiled successfully. |
| `cargo test -p openlife-tauri main_chat_kernel -- --nocapture` | Passed | 25 passed, 0 failed, 696 filtered out. |

If a command was not run: `npm --prefix frontend test -- --run` was not run
because `frontend/package.json` locks `pnpm@9.1.0`; the equivalent active
frontend command was run with pnpm.

## Safety Evidence

| Invariant | Evidence |
| --- | --- |
| No silent durable LifeModel/Memory write | Goal 5 changes are frontend-only plus typed wrapper/report changes; no Rust kernel persistence or proposal semantics were changed. Existing `main_chat_kernel` tests still pass. |
| No unsafe file/calendar/email/provider/plugin/shell side effect | No backend executor or side-effect path was changed; UI cancel invokes the existing `cancel_main_chat_agent_task` command only when task evidence exposes cancel control. |
| Unsupported capabilities fail closed | The UI renders blockers from typed agent state, task pending blockers, or kernel `blocker` events; unsupported/fake action text is not promoted into success UI. |
| Send/stream parity preserved where applicable | Stream kernel events are consumed through the existing `main-chat-kernel-event` bridge, and the required kernel send/stream focused tests still pass. |
| UI claims backed by runtime evidence where applicable | Visible execution states are derived from `MainChatKernelEvent`, `MainChatAgentStateSnapshot`, `MainChatAgentTaskState`, `ToolCallResult`, proposal records, blockers, or final delivery records; tests cover the no-fake-success path. |

## Legacy/Fallback Evidence

```text
legacy_fallback_used: no new legacy fallback path added
legacy_fallback_count: 0 in the required main_chat_kernel focused test run
why_still_needed: Existing broader legacy/fallback paths remain outside Goal 5; this goal only changes the default Chat presentation of current kernel/task evidence and keeps diagnostics explicit.
```

## Direct Write Evidence

```text
direct_writes_executed: false for Goal 5 implementation
direct_write_count: 0 new direct-write paths
proposal_or_permission_records: UI displays existing proposal and permission records; it does not create or accept proposals by itself.
```

## Source And Practice Consistency Check

Confirmed the implementation does not conflict with:

- `plans/main_chat_agent_kernel_rescue_industry_practices.md`
- `plans/main_chat_agent_kernel_rescue_spec_coding_contract.md`
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`
- `AGENTS.md`

No external source was used for this implementation. The design follows the
Goal 5 contract by keeping the default surface focused on runtime evidence and
moving deep trace/debug/control-plane details behind an explicit diagnostics
toggle. The existing compact connectivity/readiness status remains a baseline
chat availability indicator; final acceptance/live-provider/runtime disclosure
details are not part of the primary answer flow.

## Residual Risk

| Risk | Blocks next goal? | Follow-up |
| --- | --- | --- |
| The default Chat still retains the pre-existing compact connectivity/readiness strip. | No | If Goal 8 redefines readiness surfaces more strictly, move that strip into the diagnostics disclosure or a Settings-only status area. |
| Evidence styling is intentionally minimal and not a full Agent Control Plane redesign. | No | Later UX work can refine layout without changing the runtime evidence contract. |
| Kernel events are scoped to the active stream session in the frontend, but historical kernel events are not persisted as a separate browsable timeline by this goal. | No | Persisted timeline/trace improvements belong to later runtime or diagnostics work. |
