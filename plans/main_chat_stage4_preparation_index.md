# Main Chat Agent Stage 4 Preparation Index

> Date: 2026-06-20
> Stage: Stage 4 - Memory and Knowledge Asset Productization
> Status: preparation draft

## 1. Direction

Stage 3 made execution visible. Stage 4 should make memory and knowledge
assets controllable and trustworthy.

OpenLife already has real memory lifecycle primitives. The problem is now
productization and runtime integration:

- accepted memory must be inspectable;
- rejected and rolled-back memory must stay out of runtime context;
- knowledge files must be visible as bounded context assets;
- proposal/edit/accept/rollback semantics must be unambiguous;
- final delivery must state durable memory changes.

Stage 4 does not make OpenLife ready for limited internal trial by itself. It
prepares the memory/knowledge layer needed before final internal dogfood.

## 2. Preparation Documents

| Document | Purpose |
| --- | --- |
| `plans/main_chat_stage4_memory_knowledge_best_practices.md` | Source-backed principles from Codex, Claude Code, ChatGPT memory, Gemini Enterprise, LangGraph, and OpenAI Agents tracing/guardrails. |
| `plans/main_chat_stage4_current_gap_inventory.md` | Current OpenLife memory/proposal/context assets and Stage 4 product gaps. |
| `plans/main_chat_stage4_memory_knowledge_product_contract.md` | Product contract for lifecycle, knowledge assets, context consumption, UI states, and non-fake rules. |
| `plans/main_chat_stage4_memory_knowledge_eval_matrix.md` | MK4-01 through MK4-18 scenarios and minimum test plan. |
| `plans/main_chat_agent_stage4_memory_knowledge_goal_spec.md` | CLI goal-mode implementation entrypoint. |

## 3. Stage 4 Target

Stage 4 must produce a Main Chat memory/knowledge product layer where:

- memory proposals are evidence-backed and controllable;
- accepted memory is a first-class asset;
- rollback is real and visible;
- old text/vector memory retrieval cannot bypass rollback;
- `USER.md`, `MEMORY.md`, `SOUL.md`, `AGENTS.md`, and selected `SKILL.md` are
  inspected as bounded context surfaces;
- `USER.md` and `MEMORY.md` writes are managed, proposal-backed, auditable,
  reloadable, and rollback-capable;
- `SOUL.md`, `AGENTS.md`, and `SKILL.md` direct writes are high-risk or blocked,
  not silently applied;
- DirectAnswer/ReAct/Plan flows consume active memory through a visible context
  inventory;
- final delivery reports durable memory changes, managed knowledge-file changes,
  and pending memory/knowledge work.

## 4. Recommended Development Order

1. Add a Stage 4 report skeleton and scenario rows MK4-01 through MK4-18.
2. Compose pending proposals and accepted lifecycle records into a user-facing
   memory asset view.
3. Fix proposal edit semantics: support draft-only edit for pending memory.
   If an edit-and-accept command remains, it must be a separate explicitly
   named durable control and cannot replace draft edit.
4. Ensure rollback excludes linked lifecycle-backed `MemoryStore` / vector
   records from default context retrieval.
5. Add context inventory for active/excluded memory and loaded/skipped
   knowledge assets across DirectAnswer, ReAct, and PlanExecute flows.
6. Implement mature proposal-backed managed write paths for both `USER.md` and
   `MEMORY.md`: proposal, draft/diff, validation, explicit confirmation, atomic
   write, audit, context reload, and rollback/snapshot. This must have its own
   MK4 proof, not be hidden inside a direct-write blocker scenario.
7. Populate final delivery durable memory and managed knowledge-file changes.
8. Productize Review Center / Main Chat UI for accepted memory, rollback
   history, conflicts, materialization failure, and knowledge asset inventory.
9. Add focused tests and keep Stage 1/2/3/final acceptance semantics intact.

## 5. CLI Goal Prompt

Use this short prompt for CLI goal mode after review:

```text
Implement Main Chat Agent Stage 4 Memory and Knowledge Asset Productization.
Read plans/main_chat_agent_stage4_memory_knowledge_goal_spec.md and the
preparation docs it lists. Keep scope to Stage 4. Reuse existing ProposalStore,
MemoryLifecycleStore, MemoryStore/vector search, main_chat_context_loader,
AgentControlPlane, Review Center pages, final delivery, and Stage 1/2/3 gates.
Do not create a parallel memory system, proposal format, task runtime, or
readiness gate, and do not broadly rewrite MemoryLifecycleStore, MemoryStore, or
vector search unless scoped rollback-exclusion tests prove it is necessary.
Make accepted memory inspectable, rollback real and visible, rolled-back/rejected
memory excluded from all default context paths, knowledge files visible as
bounded context assets, both USER.md and MEMORY.md proposal-backed managed write
capable, and final delivery report durable memory changes. Add
MK4-01 through MK4-18 Stage 4 coverage without claiming
ready_for_limited_internal_trial or filling manual dogfood rows.
```

## 6. Readiness To Start Stage 4 Development

Stage 4 development can start after:

- these preparation documents are reviewed;
- working tree is clean or intentionally staged;
- Stage 3 execution UX commit is present;
- the implementer accepts that lifecycle store is the governed memory source of
  truth;
- the implementer accepts that old `MemoryStore` / vector search is evidence or
  retrieval only, not accepted memory truth.

## 7. Non-negotiable Invariants

- No silent durable LifeModel, memory, file, external, plugin, or provider
  writes.
- No hidden legacy fallback.
- No fake manual, browser, live-provider, or knowledge-file write evidence.
- No duplicate memory system.
- No raw transcript or vector hit as accepted memory truth.
- No rolled-back/rejected memory in active runtime context.
- No knowledge file can override runtime privacy/tool/model policy.
- No direct write to `SOUL.md`, `AGENTS.md`, or `SKILL.md` in Stage 4 unless it
  is blocked or routed into an explicitly high-risk proposal/confirmation path.
- No claim that Stage 4 alone grants `ready_for_limited_internal_trial`.
- No broad rewrite of `MemoryLifecycleStore`, `MemoryStore`, or vector search
  unless a focused rollback-exclusion test proves the narrower metadata,
  filtering, archive-marker, or adapter path cannot work.
