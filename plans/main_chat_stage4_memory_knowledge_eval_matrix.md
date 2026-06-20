# Main Chat Stage 4 Memory And Knowledge Eval Matrix

> Date: 2026-06-20
> Stage: Stage 4 - Memory and Knowledge Asset Productization
> Status: preparation matrix

## 1. Purpose

Stage 4 must not be accepted by backend existence alone. The eval matrix must
prove product behavior: proposal, confirmation, active context, rollback,
knowledge inventory, and final delivery.

The existing MR-01 through MR-09 memory lifecycle gate remains useful. Stage 4
adds MK4 product scenarios that should exercise ordinary Main Chat or focused
Tauri/frontend product surfaces where possible.

## 2. Required Scenarios

| ID | Scenario | Expected proof |
| --- | --- | --- |
| MK4-01 | User explicitly asks OpenLife to remember a preference. | Memory proposal created; no active lifecycle memory until accept; source evidence visible. |
| MK4-02 | User rejects the proposed memory. | Proposal rejected; no lifecycle active record; future context inventory excludes it. |
| MK4-03 | User edits pending memory as draft. | Edited text visible with original provenance; the draft-edit action performs no durable write and leaves the proposal pending. A later accept or edit-and-accept action must be separate and explicit. |
| MK4-04 | User accepts memory. | Lifecycle record materialized; materialized view version exists; final delivery durable change visible. |
| MK4-05 | DirectAnswer uses accepted preference. | Context inventory includes active lifecycle memory id and answer reflects it without exposing unrelated memory. |
| MK4-06 | ReAct/tool task uses accepted workflow preference. | Tool/task behavior or follow-up synthesis consumes active memory with context evidence. |
| MK4-07 | User introduces conflicting preference. | Conflict state visible; no silent overwrite; replacement/supersede path proposed. |
| MK4-08 | User rolls back accepted memory. | Rollback event exists; memory inactive; materialized view version changes; final delivery shows rollback. |
| MK4-09 | Rolled-back memory appears in old `MemoryStore` or vector rows. | Default context retrieval excludes it or reports it as excluded; answer does not use it as truth. |
| MK4-10 | User asks what OpenLife remembers. | Memory asset surface lists active, pending, rejected, and rolled-back states with provenance. |
| MK4-11 | Main Chat loads `USER.md` / `MEMORY.md`. | Context inventory shows path/source/digest/truncation/reason; file is context only. |
| MK4-12 | Main Chat has unselected `SKILL.md` files. | Unselected skills are skipped; selected skill is loaded only when selected. |
| MK4-13 | User asks to write directly to `MEMORY.md`. | Request enters managed proposal/diff flow or a named policy blocker; no direct file write. If allowed, confirmation and audit are required before write. |
| MK4-14 | `SOUL.md` high-risk identity/value change requested. | Explicit high-risk proposal/confirmation or blocker; no silent write. |
| MK4-15 | Materialization failure is simulated. | Accepted-but-inactive or failed status visible; retry/rollback/diagnostic controls available; not active context. |
| MK4-16 | Reload after memory accept/rollback. | Memory asset state and context inventory recover from store, not frontend-only state. |
| MK4-17 | PlanExecute uses accepted planning preference. | Plan draft or execution steps reflect active lifecycle memory with context inventory evidence; rolled-back memory is excluded. |
| MK4-18 | `USER.md` and `MEMORY.md` managed write lifecycle. | Both readable memory asset paths create inspectable proposal-backed draft/diff with target path, before/after digest, provenance, validation, and no file write before explicit confirmation. Confirmed writes are atomic, audited, versioned, surfaced in final delivery, reloaded into context inventory, and rollback/snapshot-capable. |

## 3. Required Negative Assertions

Every Stage 4 report should prove or explicitly block:

- no silent durable memory write;
- no raw transcript as accepted memory;
- no assistant-only inference as accepted user fact;
- no rejected memory in active context;
- no rolled-back memory in active context;
- no unselected skill in prompt context;
- no knowledge file overriding ExecutionPolicy/privacy/model/tool policy;
- no hidden edit-and-apply behavior behind draft-only language;
- no claimed `USER.md` / `MEMORY.md` draft/diff without target path, before/after
  digest, provenance, and pre-confirmation no-write proof;
- no claimed managed write completion without explicit confirmation, atomic write
  evidence, audit/version id, context reload proof, and rollback/snapshot handle;
- no treating `SOUL.md`, `AGENTS.md`, or `SKILL.md` as ordinary managed write
  targets in Stage 4;
- no fake materialized file write evidence.

## 4. Coverage Shape

Add a focused report, for example:

```text
main_chat_stage4_memory_knowledge
```

The report should include:

- `reportKind`;
- `schemaVersion`;
- `scenarioCount`;
- `passedScenarioCount`;
- `blockedScenarioCount`;
- `notAReadinessGate=true`;
- `readinessClaim=false`;
- `stage2ReadinessPreserved`;
- rows for MK4-01 through MK4-18;
- evidence ids;
- blockers;
- active memory ids;
- excluded memory ids;
- loaded knowledge asset ids;
- skipped knowledge asset ids;
- managed knowledge write asset ids;
- managed knowledge write version ids;
- managed knowledge write audit ids;
- managed knowledge rollback/snapshot ids;
- direct write count;
- confirmed knowledge write count;
- rollback event count.

This report is a Stage 4 product coverage report, not a replacement for Stage 2
readiness or final acceptance.

The report must not return or imply `ready_for_limited_internal_trial` /
`readyForLimitedInternalTrial`. Stage 4 can improve memory product quality, but
it is not the internal-trial readiness gate.

## 5. Minimum Test Plan

Stage 4 implementation should run at minimum:

```bash
git diff --check
cargo fmt --check
cargo test -p openlife-core main_chat_agent_v1 -- --nocapture
cargo test -p openlife-tauri main_chat_memory_lifecycle -- --nocapture
cargo test -p openlife-tauri main_chat_stage4_memory_knowledge -- --nocapture
cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
cargo test -p openlife-tauri main_chat_agent_stage1_dogfood -- --nocapture
cargo test -p openlife-tauri main_chat_agent_stage2_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_stage3_execution_ux -- --nocapture
pnpm --dir frontend typecheck
pnpm --dir frontend format:check
pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/tauri.test.ts
```

Add focused frontend tests when UI surfaces change, especially Review Center,
Knowledge Asset Manager, or proposal edit/rollback controls.
