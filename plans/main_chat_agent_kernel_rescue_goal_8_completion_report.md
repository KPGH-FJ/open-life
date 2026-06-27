# Main Chat Kernel Rescue Goal Completion Report

> Goal: 8 - Cleanup And Final Gate Realignment
> Branch: rescue/main-chat-kernel-goal-8
> Date: 2026-06-22
> Base commit: d4a6eeb
> Final commit: recorded in branch history after this report is committed.
> Author/agent: Codex

## Objective

Reduce legacy Main Chat runtime duplication and realign final/readiness gates so
they validate the new MainChatKernel path instead of preserving the old
over-orchestrated strategy path as the product default.

## Scope Actually Changed

| File | Change type | Why it was needed |
| --- | --- | --- |
| `openlife-core/src/agent/main_chat_agent_v1.rs` | Final acceptance evidence schema and gate logic | Added command-surface kernel evidence counters and blockers so core final acceptance requires kernel-backed DirectAnswer, read-only tools, proposal-only writes, PlanExecute draft, blockers, HS context, web, and MCP evidence. |
| `openlife-core/src/agent/tests/main_chat_agent_v1.rs` | Core gate fixture update | Updated final-acceptance fixtures to include complete local kernel command-surface evidence instead of relying only on old command-surface ready flags. |
| `src-tauri/src/main_chat_kernel.rs` | Kernel default path completion | Added kernel-backed PlanExecute draft handling, strategy-specific contract transcripts, and deterministic multi-read kernel read plans so ordinary PlanExecute, read, proposal, blocker, and direct-answer paths report kernel evidence without using legacy fallback as success. |
| `src-tauri/src/main_chat_command_surface_eval.rs` | Command-surface eval realignment | Extracts kernel metadata from responses, transcripts, and actions; counts kernel-backed cases by capability; recognizes kernel multi-read action/observation counts; requires full kernel coverage before command-surface readiness can credit final acceptance. |
| `src-tauri/src/main_chat_command_surface_tests.rs` | Command-surface gate test hardening | Added assertions that every 38-case command-surface eval case is kernel-backed and that local command-surface acceptance coverage is credited only after those counters are complete. |
| `src-tauri/src/main_chat_final_gate.rs` | Final gate realignment | Requires kernel command-surface counters when overlaying local command-surface evidence into the final acceptance report. |
| `src-tauri/src/commands/agent_runtime/mod.rs` | Eval command bridge | Populates the new kernel evidence fields in the no-command-surface placeholder evidence so missing local evidence remains fail-closed. |
| `src-tauri/src/main_chat_agent_beta_v1_default_experience.rs` | Readiness/default experience evidence | Surfaces kernel command-surface counters and adds blockers when default experience evidence is not fully kernel-backed. |
| `src-tauri/src/main_chat_agent_beta_v1_readiness.rs` | Readiness report realignment | Promotes kernel-backed command-surface evidence to readiness and blocks stale legacy-fallback or incomplete-kernel reports. |
| `src-tauri/src/main_chat_final_acceptance_tests.rs` | Final gate tests | Replaced stale local command-surface fixtures with a complete kernel-backed report so clean live overlays only pass when local kernel evidence is present. |
| `src-tauri/src/main_chat_runtime_module_tests.rs` | Runtime boundary guard | Tightened the guard so send and stream prove `MainChatKernel` is attempted before legacy strategy and remaining stream fallback paths. |
| `src-tauri/src/main_chat_agent_productization_tests.rs` | Beta readiness regression wording | Updated B22 multi-read assertions to describe the kernel read loop evidence they now consume. |
| `AGENTS.md` | Authority docs | Updated the project-level agent context to name Goal 8 and `MainChatKernel` as the current default-path authority. |
| `plans/README.md` | Authority docs | Updated the authoritative plan map, runtime boundary wording, current entry points, and progress table so new work starts from Goal 8 instead of stale stabilization/productization entries. |
| `plans/main_chat_agent_kernel_rescue_goal_8_completion_report.md` | Added report | Records Goal 8 acceptance, verification evidence, safety evidence, and residual risks. |

## Acceptance Checklist

- [x] Default Main Chat path is kernel-backed.
- [x] Legacy strategy path is isolated or explicitly marked legacy.
- [x] Duplicate send/stream code is reduced.
- [x] Final/readiness gates consume kernel evidence.
- [x] Documentation authority map is updated.
- [x] No safety regression in no-silent-write, permission, or blocker behavior.

## Acceptance Matrix Rows

| ID | Evidence |
| --- | --- |
| K8-01 | `main_chat_send.rs` and `main_chat_streaming.rs` attempt `MainChatKernel` before `try_run_main_chat_agent_strategy`; `ordinary_chat_entrypoints_try_kernel_before_legacy_strategy_paths` passed. Command-surface eval now requires `kernel_backed_case_count == total_cases` across all 38 eval cases, including B22 multi-read. |
| K8-02 | Command-surface and readiness/final gates continue to count `legacy_fallback_count`; readiness adds `legacy_fallback_detected`, and command-surface readiness requires `legacy_fallback_count == 0`. |
| K8-03 | PlanExecute draft is now kernel-backed, removing one old strategy-default success path. Remaining send/stream differences are transport/result envelope behavior or explicitly guarded legacy fallback paths. |
| K8-04 | Core final acceptance, Tauri final gate overlay, default-experience readiness, and beta readiness all consume the new kernel evidence counters and block incomplete kernel evidence. |
| K8-05 | `AGENTS.md` and `plans/README.md` now name Goal 8 / `MainChatKernel` as the active authority and demote stale stabilization/productization docs to audit or post-rescue planning references. |
| K8-06 | Required Rust and frontend suites passed; command-surface/final-acceptance suites still cover no silent writes, proposal-only writes, permission proposal behavior, blockers, web/MCP blockers, and fail-closed live-provider evidence. |
| K8-07 | Historical core/runtime/live/final-gate tests are preserved. Stale final-acceptance fixtures were replaced with kernel-backed reports rather than deleting audit coverage. |

## Verification Commands

| Command | Result | Notes |
| --- | --- | --- |
| `cargo check -p openlife-core` | Passed | Final output: `Finished dev profile ... target(s) in 0.62s`. |
| `cargo check -p openlife-tauri` | Passed | Final output: `Finished dev profile ... target(s) in 11.93s`. |
| `cargo test -p openlife-core main_chat_agent_v1 -- --nocapture` | Passed | Output: 31 passed, 0 failed, 0 ignored, 540 filtered out; finished in 10.58s. |
| `cargo test -p openlife-tauri main_chat_kernel -- --nocapture` | Passed | Output: 32 passed, 0 failed, 0 ignored, 696 filtered out; finished in 15.90s. |
| `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` | Passed | Output: 39 passed, 0 failed, 0 ignored, 689 filtered out; finished in 122.34s. |
| `cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture` | Passed | Output: 86 passed, 0 failed, 1 ignored, 641 filtered out; finished in 148.97s. |
| `cargo test -p openlife-tauri main_chat_agent_beta_v1 -- --nocapture` | Passed | Extra readiness regression proof after hallucination check found B22 multi-read missing kernel evidence; output: 15 passed, 0 failed, 0 ignored, 713 filtered out; finished in 203.11s. |
| `pnpm --dir frontend test -- --run` | Passed | Package-manager equivalent for `npm --prefix frontend test -- --run`; output: 38 files passed, 461 tests passed; duration 41.46s. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Passed | Extra K8 boundary proof; output: 35 passed, 0 failed, 0 ignored, 693 filtered out; finished in 0.04s. |
| `git diff --check` | Passed | No whitespace errors reported after the completion report was added. |

If a command was not run: no Goal 8 minimum verification command was skipped.
The frontend uses `packageManager: pnpm@9.1.0` and `pnpm-lock.yaml`, so the
pnpm command is the repository-equivalent frontend test command.

## Hallucination Check

- Branch metadata was checked against the current git branch and corrected to
  `rescue/main-chat-kernel-goal-8`.
- Current runtime context source ids and system prompt were checked for stale
  Goal 6 labels and now use Goal 8 authority labels. Remaining Goal 6 matches
  are historical tests, historical reports, and the Goal-mode index.
- Active command-surface matrix wording was checked against the current 38-case
  runner. Earlier 39-case matrix wording was removed because 39 is the Rust
  test count, not the eval matrix case count.
- The beta readiness suite initially found a real B22 gap:
  `multi_read_agent_loop_success` was still missing kernel-backed evidence.
  The fix moved that deterministic multi-read path into MainChatKernel and
  hardened the command-surface eval test to reject any non-kernel-backed case.
- Final/readiness gates were checked in code, not only docs: missing kernel
  command-surface counters still block acceptance instead of crediting stale
  ready flags.

## Safety Evidence

| Invariant | Evidence |
| --- | --- |
| No silent durable LifeModel/Memory write | Kernel PlanExecute records draft/proposal-style metadata and `directWritesExecuted=false`; command-surface/final gates continue to require zero silent writes and zero direct writes. |
| No unsafe file/calendar/email/provider/plugin/shell side effect | Goal 8 added a governed kernel PlanExecute draft path and gate/report evidence wiring; no shell executor, provider/plugin mutation, email/calendar write, or unsafe external write path was added. |
| Unsupported capabilities fail closed | Missing command-surface kernel evidence blocks readiness/final acceptance; live-provider evidence remains explicit and fail-closed when opt-in/key/network/provider proof is missing. |
| Send/stream parity preserved where applicable | `main_chat_command_surface` passed and the command-surface report now counts kernel-backed send/stream cases before crediting readiness. |
| UI claims backed by runtime evidence where applicable | Default-experience and readiness reports expose kernel counters and blockers instead of relying on stale ready booleans. |

## Legacy/Fallback Evidence

```text
legacy_fallback_used: false for the required command-surface/final-acceptance passing evidence.
legacy_fallback_count: 0 required for command-surface readiness and default-experience readiness.
why_still_needed: Explicit legacy strategy/fallback paths remain for unsupported behavior and historical audit coverage, but they are attempted only after MainChatKernel and are not counted as default success.
```

## Direct Write Evidence

```text
direct_writes_executed: false in kernel-backed PlanExecute draft, proposal-only write, blocker, read-only tool, web/MCP, and final acceptance evidence.
direct_write_count: 0 new direct-write paths.
proposal_or_permission_records: Proposal-only writes and MCP ToolPermission proposal behavior remain covered by command-surface and final-acceptance suites; Goal 8 added no permission bypass.
```

## Live Provider Status

Goal 8 realigned the local final/readiness gates to kernel evidence, but it did
not claim external live-provider completion. The final acceptance gate still
requires explicit live-provider evidence where live-provider evidence is
required, and fail-closed blockers remain in place when external live-provider
proof is absent.

## Source And Practice Consistency Check

Confirmed the implementation does not conflict with:

- `plans/main_chat_agent_kernel_rescue_industry_practices.md`
- `plans/main_chat_agent_kernel_rescue_spec_coding_contract.md`
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`
- `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`
- `AGENTS.md`

No external source was used for this implementation. The change follows Goal 8
by making the eval/readiness surfaces track real kernel traces while preserving
explicit safety, blocker, proposal, and live-provider gates.

## Residual Risk

| Risk | Blocks next goal? | Follow-up |
| --- | --- | --- |
| External live-provider completion is still opt-in and not executed in this local run. | No | Keep final/live-provider gates fail-closed until a credentialed external run supplies complete evidence. |
| Legacy strategy code remains for unsupported or historical surfaces. | No | Continue shrinking it only after equivalent kernel evidence exists; do not delete useful audit coverage blindly. |
| Command-surface kernel coverage is metadata-driven. | No | Keep focused command-surface tests asserting the metadata is emitted from real kernel task/session/action traces. |
