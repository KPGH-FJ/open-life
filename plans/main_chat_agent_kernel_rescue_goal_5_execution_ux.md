# Goal 5: Minimal Execution UX

> Status: prepared for goal mode
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## Objective

Update the Main Chat user experience so it displays real MainChatKernel evidence
for thinking, tool execution, observations, proposals, blockers, and permission
needs, without exposing readiness/final-gate noise as the primary user surface.

## System Position

This goal productizes the kernel evidence already created by Goals 1-4. It must
not invent a frontend-only task model. The UI should become a thin
representation of kernel events, proposal records, blockers, and final answers.

## OpenLife Lessons Applied

- The product currently exposes too much control-plane/debug complexity.
- User trust comes from visible evidence, not from readiness labels.
- UI must not claim execution unless runtime data proves it.

## Industry Practices Applied

- Agent result surfaces should teach users final output, pending approvals, and
  resumable state before exposing deep diagnostics.
- Trace/debug details are useful, but they should not be the first product
  surface.
- Human approval flows need clear approve/edit/reject style state.

## Scope

Allowed implementation scope:

- update Chat runtime-to-UI mapping;
- update relevant Chat page components/hooks;
- update frontend Tauri wrappers only for kernel-backed fields;
- add UI tests for the minimal execution states.

Out of scope:

- complete frontend shell rewrite;
- full Agent Control Plane v2;
- new product marketing surfaces;
- changing backend kernel semantics to satisfy UI convenience.

## Required States

- thinking;
- tool running;
- tool observation;
- proposal created;
- blocked;
- permission needed;
- final answer.

## Runtime Contracts

- UI evidence contract: every visible tool/proposal/blocker state maps to a
  kernel event or persisted proposal/permission record.
- Default chat contract: hide readiness/final-gate machinery from the normal
  answer flow.
- Recovery contract: blockers show reason and next action when available.
- No-fake-success contract: missing runtime evidence renders as unavailable or
  blocked, not as completed.

## Acceptance Checklist

- [ ] Direct answer appears without debug clutter.
- [ ] Tool read progress and observation are visible.
- [ ] Proposal-created state links to inspectable proposal.
- [ ] Blocker state shows reason and next action.
- [ ] UI never claims a read/write/remember action without runtime evidence.
- [ ] Readiness/final gate details are hidden from the default chat experience.

## Verification

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
npm --prefix frontend test -- --run
```

If the repository uses `pnpm` in the active environment, use the matching
frontend test command already established by the project.

## Stop Conditions

- UI needs a second frontend-only task model.
- UI claims execution without kernel events.
- The implementation requires a broad redesign before minimal evidence states
  can render.
- Readiness/final-gate debug output remains more prominent than kernel evidence
  in ordinary chat.
