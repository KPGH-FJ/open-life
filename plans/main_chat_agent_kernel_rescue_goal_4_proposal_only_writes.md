# Goal 4: Proposal-Only Writes

> Status: prepared for goal mode
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## Objective

Make all durable write-like Main Chat kernel outcomes proposal-only,
permission-required, or hard-blocked, including Memory, LifeModel, file,
external side effects, and dangerous shell requests, with Review Center evidence
for created proposals.

## System Position

This goal reconnects OpenLife governance after the kernel can answer and read.
It must preserve user sovereignty while making write-like requests productive:
proposal, permission interruption, or hard blocker.

## OpenLife Lessons Applied

- Proposal-first governance is valuable, but not if it hides basic agent
  execution problems.
- Ordinary chat auto-checkin/materialization must not be part of the kernel
  success path.
- Memory and LifeModel updates need inspectable provenance and user acceptance.

## Industry Practices Applied

- Human-in-the-loop approval should pause risky actions and preserve resumable
  state.
- Rejections are not successful tool results.
- Short-term run state and long-term memory/truth must remain separate.

## Scope

Allowed implementation scope:

- extend MainChatKernel write-intent handling;
- reuse proposal stores and proposal UI contracts;
- adjust ordinary chat auto-checkin/materialization so it does not silently
  persist accepted truth from the kernel path;
- add focused tests for Memory/LifeModel/file/external/dangerous cases.

Out of scope:

- applying proposals automatically;
- full Review Center redesign;
- background maturation;
- calendar/email real writes;
- dangerous shell execution.

## Required Mapping

| Request class | Required outcome |
| --- | --- |
| Memory write/archive | Memory proposal. |
| LifeModel patch | LifeModel proposal. |
| File write | External/file proposal or permission request. |
| Calendar/email/external write | Proposal or confirmation blocker. |
| Dangerous shell | Hard blocker. |

## Runtime Contracts

- Proposal contract: proposal id, type, source run/turn id, bounded payload
  summary, and review status.
- Permission contract: exact pending action, allowed decision types, and replay
  identity.
- Write-safety contract: `direct_writes_executed=false` unless the action is a
  deliberately accepted replay path outside ordinary kernel default.
- Blocker contract: dangerous action blockers are terminal and not replayable by
  ordinary approval.

## Acceptance Checklist

- [ ] "Remember this" creates a Memory proposal only.
- [ ] LifeModel update creates a LifeModel proposal only.
- [ ] File write does not write by default.
- [ ] External side effect is proposal/confirmation only.
- [ ] Dangerous shell is hard-blocked.
- [ ] Ordinary chat auto-checkin does not silently materialize truth in the
      kernel path.
- [ ] Review Center can inspect created proposal metadata.

## Verification

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri proposal -- --nocapture
```

## Stop Conditions

- A write-like request can complete without proposal, permission, or blocker.
- Proposal creation cannot preserve source evidence.
- Existing auto-checkin persistence cannot be isolated from the kernel path.
- UI or tests require treating proposal creation as accepted durable truth.
