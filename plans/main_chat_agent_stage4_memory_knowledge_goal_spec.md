# Main Chat Agent Stage 4 Memory And Knowledge Goal Spec

> Date: 2026-06-20
> Status: prepared for CLI goal mode
> Depends on: Stage 3 execution UX commit `ed62752`

## 1. Objective

Implement **Main Chat Agent Stage 4: Memory and Knowledge Asset
Productization**.

The output should make OpenLife memory and knowledge assets user-visible,
governed, reversible, and consumed by the default Main Chat Agent path:

- memory proposal evidence;
- accepted memory asset inspection;
- edit/reject/defer/accept semantics that are explicit;
- rollback with active-context exclusion;
- context inventory for active/excluded memory;
- context inventory for loaded/skipped knowledge files;
- managed `USER.md` / `MEMORY.md` write lifecycle;
- final delivery durable memory and knowledge-file changes;
- Stage 4 MK4 coverage report.

Stage 4 is not limited-internal-trial readiness and must not fill or fabricate
S2-D manual dogfood rows.

## 2. Required Reading

Read before editing code:

- `AGENTS.md`
- `plans/main_chat_stage4_preparation_index.md`
- `plans/main_chat_stage4_memory_knowledge_best_practices.md`
- `plans/main_chat_stage4_current_gap_inventory.md`
- `plans/main_chat_stage4_memory_knowledge_product_contract.md`
- `plans/main_chat_stage4_memory_knowledge_eval_matrix.md`
- `plans/main_chat_memory_rollback_lifecycle_contract_v1.md`
- `plans/main_chat_stage2_memory_proposal_trial_flow.md`
- `plans/main_chat_agent_beta_v1_knowledge_assets_contract.md`
- `plans/main_chat_stage3_implementation_report.md`

## 3. Non-goals

- Do not create a second memory system.
- Do not create a second proposal format.
- Do not create a second task runtime or second AgentControlPlane.
- Do not implement full Skills Hub or skill marketplace.
- Do not implement automatic self-evolution.
- Do not implement cross-device sync or bulk import/export.
- Do not claim `ready_for_limited_internal_trial`.
- Do not run or fill S2-D01 through S2-D24 manual dogfood rows.
- Do not lower Stage 1, Stage 2, Stage 3, beta, final acceptance, or live
  provider gates.

## 4. Required Implementation Areas

### Phase 0: report skeleton and audit

Add a focused Stage 4 coverage surface, for example
`main_chat_stage4_memory_knowledge`, with MK4-01 through MK4-18 rows.

The report must preserve:

- `notAReadinessGate=true`;
- `readinessClaim=false`;
- Stage 2 readiness fail-closed semantics;
- no manual/live evidence fabrication.

### Phase 1: memory asset view

Compose pending memory proposals and accepted lifecycle records into a
user-facing memory asset view.

Requirements:

- show pending, accepted/materialized, materialization_failed, rejected,
  deferred, superseded, and rolled_back states;
- show memory id, proposal id, scope, category, risk, confidence, evidence ids,
  conflict ids, materialized view/version, rollback event ids;
- expose list/get/events through existing Tauri commands or focused wrappers.

### Phase 2: proposal edit semantics

Fix the current ambiguous edit behavior.

Minimum requirement:

- support draft-only edit for pending memory;
- keep the pending proposal pending until a separate explicit accept;
- preserve original provenance/evidence through draft edits;
- draft-only edit must not call `apply_proposal_to_state`, must not create a
  `MemoryLifecycleRecord`, and must not mark the proposal resolved;
- draft-only edit should use an explicit command/control such as
  `draft_edit_memory_proposal`, `update_memory_proposal_draft`, or a
  `proposal_revisions` record. It may update a draft field/revision, but the
  user-visible pending memory must remain pending until accept;
- if edit-and-accept remains, expose it as a separate explicitly named durable
  control.

Do not substitute edit-and-accept for draft-only edit. Do not let the UI imply
that editing is draft-only if it applies durable memory or LifeModel state.

### Phase 3: rollback exclusion across context paths

Ensure rolled-back and rejected memory cannot enter default runtime context.

Requirements:

- lifecycle active memory loader already uses active materialized records;
- linked `MemoryStore` / vector rows created from lifecycle-backed acceptance
  must be archived, excluded, or filtered after rollback/supersede/reject;
- context inventory must show excluded lifecycle memory ids or linked old rows
  where relevant;
- tests must prove rolled-back memory does not influence a later answer as
  accepted truth.

### Phase 4: knowledge asset inventory

Productize bounded context files already loaded by `main_chat_context_loader`.

Requirements:

- show loaded/skipped/truncated/digest/source/reason for `AGENTS.md`,
  `USER.md`, `MEMORY.md`, `memories/USER.md`, `memories/MEMORY.md`, `SOUL.md`,
  and selected `SKILL.md`;
- prove unselected skills are not loaded;
- direct write requests to `USER.md`/`MEMORY.md`/`SOUL.md`/`AGENTS.md`/`SKILL.md`
  must create a proposal/diff or visible blocker, not silent file writes;
- both `USER.md` and `MEMORY.md` must support a mature proposal-backed managed
  write lifecycle. Preview-only draft/diff is not sufficient;
- each managed write path must prove: target path, source/provenance proposal id,
  linked memory ids where applicable, before digest, after digest, previewable
  diff, validation result, and no file write before explicit confirmation;
- explicit confirmation must perform an atomic write, record audit evidence and
  version id, reload the asset into context inventory, and expose a
  rollback/snapshot handle;
- rollback/snapshot must restore the previous file version and prove the
  reverted content no longer appears as active context;
- `SOUL.md` remains read-only or high-risk proposal-first;
- `AGENTS.md` and `SKILL.md` are inventory/inspect/blocker targets in Stage 4,
  not ordinary managed write targets.

### Phase 5: final delivery and UI productization

Extend Main Chat / AgentControlPlane / Review Center surfaces so users can
understand memory outcomes.

Requirements:

- final delivery includes durable memory changes for accept/materialize/rollback;
- final delivery includes durable managed knowledge-file changes for confirmed
  `USER.md` / `MEMORY.md` writes and rollbacks;
- accepted memory and rollback history are visible after refresh;
- `USER.md` / `MEMORY.md` draft, applied, audit, context reload, and rollback
  history are visible after refresh;
- materialization failure is not displayed as active memory;
- proposal and memory controls carry exact ids;
- managed knowledge write controls carry exact proposal id, target path, version
  id, audit id, and rollback/snapshot handle;
- context inventory is inspectable without dumping raw private memory into main
  chat.

### Phase 6: Stage 4 eval and implementation report

Add or update focused tests for MK4-01 through MK4-18.

Keep implementation scoped. Do not broadly rewrite `MemoryLifecycleStore`,
`MemoryStore`, or vector search unless rollback-exclusion tests prove that
metadata, filtering, archive markers, or a scoped adapter cannot satisfy the
contract.

Create `plans/main_chat_stage4_implementation_report.md` with:

- completed phases;
- changed files;
- MK4 scenario results;
- tests run;
- remaining blockers;
- explicit statement that Stage 4 does not grant limited internal trial
  readiness.

## 5. Test Plan

Run at minimum:

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

Add focused frontend tests for any changed Review Center, memory asset,
knowledge asset, PlanExecute context inventory, or proposal edit/rollback UI.

Do not mark Stage 4 complete if:

- rolled-back memory can still affect default context through old memory/vector
  retrieval;
- edit controls apply durable state while looking draft-only;
- draft-only pending memory edit is not implemented;
- active memory is claimed without lifecycle materialization evidence;
- final delivery hides durable memory or managed knowledge-file changes;
- knowledge-file writes happen silently;
- either `USER.md` or `MEMORY.md` lacks a positive managed write lifecycle;
- either `USER.md` or `MEMORY.md` is only blocked or preview-only;
- managed writes lack explicit confirmation, atomic write evidence, audit/version
  id, context reload proof, or rollback/snapshot;
- `SOUL.md`, `AGENTS.md`, or `SKILL.md` are treated as ordinary managed write
  targets instead of read-only/high-risk/blocker surfaces;
- Stage 2 readiness semantics are weakened.

## 6. Acceptance

Stage 4 can be accepted when:

- MK4-01 through MK4-18 are covered as passed or blocked with named blockers;
- active accepted memory is inspectable and used by Main Chat;
- draft-only pending memory edit is separate from accept/edit-and-accept;
- rejected/rolled-back memory is excluded from default context;
- knowledge assets are visible as bounded context surfaces;
- both `USER.md` and `MEMORY.md` support proposal-backed managed writes with
  draft/diff, validation, explicit confirmation, atomic write, audit/version id,
  context reload, and rollback/snapshot proof;
- final delivery reports durable memory and managed knowledge-file changes;
- existing gates still pass or remain blocked only for documented manual/live
  evidence;
- implementation report is complete and honest.

## 7. Required Final Response

After implementation, report:

- whether Stage 4 is complete;
- what changed;
- which MK4 scenarios passed or remain blocked;
- tests run and any tests not run;
- whether Stage 2 readiness remains `not_ready_for_limited_internal_trial`;
- whether it is appropriate to proceed to Stage 5 preparation.
