# Main Chat Kernel Rescue Goal Completion Report

> Goal: 4 - Proposal-Only Writes
> Branch: rescue/main-chat-kernel-goal-4
> Date: 2026-06-22
> Base commit: 5c56428
> Final commit: recorded in branch history after this report is committed
> Author/agent: Codex

## Objective

Make all durable write-like Main Chat kernel outcomes proposal-only,
permission-required, or hard-blocked, including Memory, LifeModel, file,
external side effects, and dangerous shell requests, with Review Center evidence
for created proposals.

## Scope Actually Changed

| File | Change type | Why it was needed |
| --- | --- | --- |
| `src-tauri/src/main_chat_kernel.rs` | Kernel write-safety outcomes | Added proposal-only / confirmation / hard-block write intent outcomes, command-surface proposal creation, permission blocker metadata, dangerous shell terminal blockers, read-plus-memory-proposal follow-up handling, and no-direct-write evidence. |
| `src-tauri/src/main_chat_command_surface_tests.rs` | Focused send/stream tests | Added Goal 4 send + stream coverage for Memory, LifeModel, file write, external side effect, dangerous shell, and ordinary auto-checkin isolation. |
| `src-tauri/src/main_chat_command_surface_eval.rs` | Existing eval compatibility and stable stream verification | Allows existing knowledge asset proposal rows to accept the new kernel-backed `proposal.create` LifeModel proposal evidence while preserving task linkage and no silent writes; waits for persisted stream web-policy blocker evidence before asserting the eval row. |
| `plans/main_chat_agent_kernel_rescue_goal_4_completion_report.md` | Added report | Records Goal 4 acceptance, verification evidence, safety evidence, and residual risks. |

## Acceptance Checklist

- [x] "Remember this" creates a Memory proposal only.
- [x] LifeModel update creates a LifeModel proposal only.
- [x] File write does not write by default.
- [x] External side effect is proposal/confirmation only.
- [x] Dangerous shell is hard-blocked.
- [x] Ordinary chat auto-checkin does not silently materialize truth in the kernel path.
- [x] Review Center can inspect created proposal metadata.

## Acceptance Matrix Rows

| ID | Evidence |
| --- | --- |
| K4-01 | `main_chat_kernel_goal_4_remember_this_send_stream_creates_memory_proposal_only` passes; send and stream create pending `MemoryWrite` proposals linked to the task session, with active memory record count unchanged. |
| K4-02 | `main_chat_kernel_goal_4_lifemodel_update_send_stream_creates_proposal_only` passes; send and stream create pending `LifeModelUpdate` proposals, and the loaded LifeModel remains unchanged. |
| K4-03 | `main_chat_kernel_goal_4_file_write_send_stream_creates_proposal_without_writing_file` passes; send and stream create pending `ExternalWriteAction` proposals and the requested file path is not written. |
| K4-04 | `main_chat_kernel_goal_4_external_write_send_stream_requires_confirmation_only` and `main_chat_kernel_goal_4_calendar_and_generic_external_write_send_stream_require_confirmation_only` pass; email, calendar, and generic external write intents enter `WaitingPermission` with `external_confirmation_blocker`, no proposal auto-acceptance, and no external write. |
| K4-05 | `main_chat_kernel_goal_4_dangerous_shell_send_stream_hard_blocks_without_proposal` passes; dangerous shell intent is terminally blocked, not replayable, and creates no proposal. |
| K4-06 | `main_chat_kernel_goal_4_ordinary_auto_checkin_does_not_materialize_truth` passes; ordinary chat remains direct-answer only and does not create accepted memory or LifeModel truth. |
| K4-07 | Goal 4 proposal tests inspect proposal source detail, proposal type, pending status, affected path, no-direct-write flags, task action metadata, source run id, source task-session id, payload summary, and review status. |

## Verification Commands

| Command | Result | Notes |
| --- | --- | --- |
| `cargo fmt --check` | Passed | No Rust formatting drift after implementation. |
| `cargo check -p openlife-core` | Passed | Core package compiles without changes. |
| `cargo check -p openlife-tauri` | Passed | Kernel, command-surface, and test changes compile. |
| `cargo test -p openlife-tauri main_chat_kernel -- --nocapture` | Passed | 25 passed, 0 failed. |
| `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` | Passed | 39 passed, 0 failed; includes Goal 4 send/stream cases and the command-surface matrix. |
| `cargo test -p openlife-tauri proposal -- --nocapture` | Passed | 63 passed, 0 failed; long-running suite with local file/socket access in this environment. |
| `git diff --check` | Passed | No whitespace errors in tracked diffs; the untracked report was checked separately for trailing whitespace. |

If a command was not run: none.

## Safety Evidence

| Invariant | Evidence |
| --- | --- |
| No silent durable LifeModel/Memory write | Memory and LifeModel write intents produce pending proposals with `directWritesExecuted=false`; tests verify active memory count and LifeModel serialization remain unchanged. |
| No unsafe file/calendar/email/provider/plugin/shell side effect | File writes create pending external-write proposals without touching the target path; calendar, email, and generic external writes create confirmation blockers; dangerous shell requests are hard-blocked and non-replayable. |
| Unsupported capabilities fail closed | Dangerous shell and external side-effect paths produce explicit blocker metadata instead of tool success; write-like outcomes cannot complete as ordinary direct answers. |
| Send/stream parity preserved where applicable | Each Goal 4 command-surface test covers both `send_message` and `start_stream_message` and verifies equivalent task-session/proposal/blocker outcomes. |
| UI claims backed by runtime evidence where applicable | No frontend copy was changed; Review Center-backed proposal records carry source, affected path, payload summary, status, source run id, and source task-session id for inspection. |

## Legacy/Fallback Evidence

```text
legacy_fallback_used: false for kernel-backed Goal 4 write-safety turns
legacy_fallback_count: 0 in the required command-surface eval gate
why_still_needed: Existing non-Goal-4 broad ReAct, live-provider, and later-goal paths remain explicit; Goal 4 only migrates write-like kernel outcomes to proposal, permission, or hard blocker.
```

## Direct Write Evidence

```text
direct_writes_executed: false for Goal 4 proposal, confirmation, and hard-block outcomes
direct_write_count: 0 in Goal 4 send/stream tests and command-surface eval gate
proposal_or_permission_records: MemoryWrite, LifeModelUpdate, and ExternalWriteAction proposals are pending Review Center records; external side effects create permission blockers; dangerous shell creates no replay proposal.
```

## Source And Practice Consistency Check

Confirmed the implementation does not conflict with:

- `plans/main_chat_agent_kernel_rescue_industry_practices.md`
- `plans/main_chat_agent_kernel_rescue_spec_coding_contract.md`
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`
- `AGENTS.md`

No external source was used for this implementation. Goal 4 did not add
automatic proposal acceptance, background maturation, real calendar/email writes,
dangerous shell execution, final live-provider proof, or a Review Center UI
redesign.

## Residual Risk

| Risk | Blocks next goal? | Follow-up |
| --- | --- | --- |
| Write-intent classification is deterministic and intentionally narrow, so broader natural-language coverage is not complete. | No | Expand coverage only with explicit tests for each new write-like surface. |
| External side effects currently stop at confirmation/blocker evidence in the kernel path. | No | Later goals can add scoped accepted replay paths with provider-specific governance tests. |
| Review Center inspectability is proven through runtime proposal records, not a frontend redesign. | No | UI work should consume the existing proposal metadata contract without treating pending proposals as accepted truth. |
