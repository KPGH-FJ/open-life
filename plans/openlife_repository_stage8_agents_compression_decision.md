# OpenLife Repository Stage8 AGENTS Compression Decision

> Date: 2026-07-08
> Status: docs-only repository cleanup decision
> Authority: subordinate to `AGENTS.md`, `plans/README.md`, and the Phase7
> single-system deletion/product-trial contract.

## Inputs Read

Stage8 was executed only after reading the required inputs:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`
5. `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
6. `plans/openlife_repository_active_claim_audit.md`
7. `plans/openlife_repository_stage7_scope_reset_baseline_decision.md`

## Objective

Compress and stabilize root `AGENTS.md` so it can serve as a long-lived AI
coding entrypoint.

The target was to keep it within 250 lines if possible while preserving the
current Phase7 authority stack, Main Chat current source-map, no-silent-write
and proposal-first constraints, external live-provider non-closure, and retired
final-acceptance absence contract.

## Scope Boundary

Stage8 is documentation-only.

Files changed in this slice:

- `AGENTS.md`
- `plans/openlife_repository_stage8_agents_compression_decision.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_active_claim_audit.md`

Files and actions intentionally not touched:

- Rust, Tauri, React, frontend bridge, or product behavior code;
- `plans/README.md`;
- ADR files;
- `plans/archive` or any broad file move;
- Stage6E product RED repair work;
- retired command or deleted old final-acceptance test-owner restoration.

## Compression Result

| Item | Before | After |
| --- | ---: | ---: |
| `AGENTS.md` line count | 883 | 179 |

The compressed file is below the 250-line target, so no over-target exception is
needed.

## Content Decisions

Kept in `AGENTS.md`:

- active authority order and the Phase7 single-system contract;
- current ordinary Main Chat source-map from send/stream wrappers through
  `OpenLifeTurnRuntime`, `MainChatKernel`, and core agent/model-router areas;
- current non-completion boundaries for Main Chat Agent Execution v1 and
  external live-provider-backed scenarios;
- explicit distinction between local evidence and external live-provider credit;
- proposal-first, no silent durable write, bounded context, and product read
  model rules;
- retired `run_main_chat_agent_execution_v1_final_acceptance_gate` prohibition;
- expected-absent `src-tauri/src/main_chat_final_acceptance_tests.rs`
  prohibition;
- docs-only cleanup and dirty-worktree discipline.

Removed from `AGENTS.md`:

- long W-series historical流水;
- repeated roadmap/history summaries;
- stale module tables and deleted-route source paths;
- old product narrative sections better owned by README, architecture docs, or
  historical plans;
- long update log and detailed tool taxonomy that should not steer active
  coding work.

## Stage6E Boundary

Stage6E remains a product trial RED result and a product TODO boundary. Stage8
does not convert ToolPermission, LifeModel Mailbox/materialization, web/network
policy, native trial, or external provider gaps into repository cleanup
blockers, and it does not attempt to fix them.

## Validation Interpretation

The required validation commands were run after the Stage8 edits.

Expected interpretation:

- `wc -l AGENTS.md` must report 250 lines or fewer; it reports 179.
- `git diff --check` must pass.
- The completion-claim scan may still match historical/prohibited-command
  examples in older plan records, but the compressed `AGENTS.md` must not add a
  current completion claim.
- The retired-command/test-owner scan may match `AGENTS.md` and this decision
  only as explicit forbidden/expected-absent wording. It must not match shipped
  handler, product command, or frontend bridge surfaces.
- The stale current-module scan must not match `AGENTS.md`.

Stage8 does not claim Phase7 has finished, does not claim Main Chat Agent
Execution v1 has finished, does not claim external live-provider evidence is
closed, and does not promote any retired command or deleted owner back into
current authority.

## Stage8-Rework Source-Map Correction

Date: 2026-07-08

Stage8-rework corrected a source-map precision issue in the compressed
`AGENTS.md`.

Source checked before editing:

- `src-tauri/src/lib.rs`
- `src-tauri/src/main_chat_send.rs`
- `src-tauri/src/main_chat_streaming.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`
- `src-tauri/src/main_chat_kernel.rs`

Finding:

- `src-tauri/src/lib.rs` defines separate Tauri commands:
  `send_message` calls `main_chat_send::send_message_with_state`, and
  `start_stream_message` calls
  `main_chat_streaming::start_stream_message_with_state`.
- `main_chat_send.rs` calls `OpenLifeTurnRuntime::run_buffered`.
- `main_chat_streaming.rs` calls `OpenLifeTurnRuntime::run_streaming`.
- Both runtime methods then converge through
  `src-tauri/src/main_chat_turn_runtime.rs` and
  `src-tauri/src/main_chat_kernel.rs`.

Correction:

- `AGENTS.md` now shows two parallel branch entrypoints from
  `frontend/src/tauri.ts` through `src-tauri/src/lib.rs`, one for
  `send_message` and one for `start_stream_message`.
- It no longer represents `main_chat_send.rs` as flowing into
  `main_chat_streaming.rs`.
- After rework, `AGENTS.md` is 190 lines, still below the 250-line limit.

This rework is docs-only and does not change product behavior, Stage9 scope,
Phase7 status, Main Chat completion status, external live-provider evidence
status, or the retired-command/test-owner absence contract.
