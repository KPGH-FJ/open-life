# Main Chat Next 6 Steps Acceptance Matrix

> Date: 2026-06-25
> Status: preparation artifact before the next Main Chat Agent development cycle
> Parent: `plans/main_chat_next_6_steps_master_spec.md`

## 1. Purpose

This matrix turns the next six steps into auditable acceptance rows. A row is
not complete unless the expected evidence is present in code, tests, and the
reported gate output.

## 2. Status Labels

- `baseline_passed`: current baseline already proves the row.
- `not_started`: row has not been implemented.
- `partial`: some evidence exists but the row cannot be credited.
- `blocked`: implementation cannot proceed without an external dependency.
- `complete`: future implementation has passed all evidence requirements.

## 3. Matrix

| ID | Step | Scenario | Current status | Required evidence | Negative assertions | Required commands |
| --- | --- | --- | --- | --- | --- | --- |
| S1-RF20 | 1 | User asks whether a blocked task completed. | complete | `taskStatus=blocked`, bounded `blockerCodes`, `uiStatus=restricted`, next control metadata. | Must not say completed; must not call model; must not parse assistant prose. | `cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture` |
| S1-RF21 | 1 | User asks status while a permission action is pending. | complete | `taskStatus=waiting_permission`, `pendingPermissionCount>0`, bounded permission target label, `uiStatus=waiting_for_user`. | Must not expose raw unsafe manifest; must not execute pending action; must not claim durable completion. | `cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture` |
| S1-B-STREAM | 1 | Provider route runtime facts work through stream. | complete | Slice B stream command-surface proof exists and no `slice_b_provider_route_stream_out_of_scope`. | Must not call model when answering pre-model blocked route facts. | Runtime facts test plus command-surface matrix |
| S1-C-STREAM | 1 | Tool availability runtime facts work through stream. | complete | Slice C stream command-surface proof exists and no `slice_c_tool_availability_stream_out_of_scope`. | Must not run active reachability probe; must not expose raw MCP manifest. | Runtime facts test plus command-surface matrix |
| S1-D-STREAM | 1 | Agent self-state runtime facts work through stream. | complete | Slice D stream command-surface proof exists and no `slice_d_agent_self_state_stream_out_of_scope`. | Must not use current self-state question task as the target task; must not infer from assistant prose. | Runtime facts test plus command-surface matrix |
| S1-READY | 1 | Runtime Facts full-layer readiness. | partial | Full report covers required RF rows or names blockers; `runtimeFactsReady` may become true only when full contract passes. | Must not flip `runtime_facts_ready` from slice-only success. | Runtime facts full report command when implemented |
| S2-DIRECT | 2 | External live DirectAnswer. | blocked | Credited direct external live report with provider/model/run/task trace and non-empty normalized response preview. | Must reject scripted, local, fixture, loopback, synthetic, local-test HTTP credit. | Opt-in live final acceptance command |
| S2-WEB | 2 | Provider-backed web AgentLoop. | blocked | Credited web AgentLoop report with governed `web.*` target, action status succeeded, no single-step fallback. | Must not overlap MCP success or ToolPermission proposal trace. | Opt-in live final acceptance command |
| S2-MCP | 2 | Provider-backed registered MCP AgentLoop. | blocked | Credited MCP report with multi-candidate registered MCP set, provider-ranked selection, safe labels, and successful governed action. | Must not accept deterministic-only or local-ranked selection as provider-backed credit. | Opt-in live final acceptance command |
| S2-PERM | 2 | Provider-backed MCP ToolPermission proposal. | blocked | Credited proposal-permission report with selected MCP candidate and pending permission proposal target match. | Must not also claim MCP read success; must not execute write. | Opt-in live final acceptance command |
| S2-READY | 2 | Live provider gate ready. | blocked | `live_provider_ready_count=4`, live provider coverage booleans true, acceptance live gate ready. | Must fail closed without opt-in, key, network, or external provider. | `cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture` and opt-in live command |
| S3-INV | 3 | Legacy fallback strategy inventory. | not_started | All `main_chat_kernel_supports_turn=false` paths have explicit disposition. | Must not silently route unsupported ordinary turns to legacy success. | focused kernel and command-surface tests |
| S3-REVIEW | 3 | `ReviewMaturation` disposition. | not_started | Either kernel support, governed blocker, or explicit non-default unreachable path. | Must not disappear into legacy generation. | kernel support test |
| S3-ZERO | 3 | Default command surface legacy count remains zero. | baseline_passed | Command-surface report legacy fallback count is zero. | Must not hide fallback by omitting metadata. | command-surface matrix and final acceptance tests |
| S4-SPLIT | 4 | Runtime Facts module split. | not_started | Registry, resolver, snapshot, reply, and eval report responsibilities are separated. | Must not create new catch-all file or duplicate fact definitions. | runtime facts tests and module-boundary tests |
| S4-KERNEL | 4 | Kernel consumes typed boundary only. | not_started | Kernel imports typed RuntimeFact answer APIs but not scenario/eval internals. | Must not move fact-specific rules into kernel. | module-boundary tests |
| S4-REGRESS | 4 | Refactor behavior preserved. | not_started | Existing RF-01 through RF-19 evidence unchanged. | Must not change readiness semantics during pure refactor. | runtime facts test plus command-surface matrix |
| S5-DEFAULT | 5 | Default UI shows task status without diagnostics. | not_started | User-visible status chip/banner for completed, waiting, restricted, blocked, trace gap, proposal pending, permission pending. | Must not require `showMainChatDiagnostics` for basic status. | ChatPage tests and visual/browser QA if UI changes are nontrivial |
| S5-ACTION | 5 | Default UI exposes safe next action. | not_started | Proposal review, permission review, retry, resume, cancel, refresh context actions map to backend allowed controls. | Must not show unsafe or impossible controls. | ChatPage tests |
| S5-TRACE | 5 | Developer trace remains bounded. | partial | ReasoningTracePanel shows structured evidence from `generation_result`. | Must not parse assistant prose; must not expose raw prompts, keys, manifests, or absolute paths. | ReasoningTracePanel tests |
| S6-E2E | 6 | Real task suite. | not_started | 8-12 user journeys pass with answer, state, UI, and trace evidence. | Must not accept screenshots alone; must not mark local fixture as live external proof. | E2E harness plus final acceptance |

## 4. Baseline Commands

These commands should be run before starting Step 1 and after each step unless a
step-specific command set supersedes them:

```bash
cargo fmt --check
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
pnpm --dir frontend format:check
pnpm --dir frontend typecheck
git diff --check
```

## 5. Hallucination Checks

Before marking any row complete, verify:

- the expected field exists in code or serialized report output;
- the test asserts the field, not only a prose answer;
- a negative assertion exists for the main failure mode;
- no out-of-scope row was silently removed;
- no ignored live test is counted as completed;
- no fixture, local, scripted, or synthetic path is credited as external live
  provider evidence.
