# Main Chat Memory Rollback Lifecycle Contract v1

> Date: 2026-06-17
> Status: preparation artifact for Product Maturity v2
> Parent: `plans/main_chat_agent_product_maturity_v2_goal_spec.md`

## 1. Purpose

This document defines how Main Chat memory proposals become accepted memory,
how accepted memory remains inspectable, and how rollback becomes a real
governed product action instead of an unsupported placeholder.

Productization v1 correctly kept `MP-06` as optional unsupported. Product
Maturity v2 should support rollback only if OpenLife can preserve provenance,
audit, materialized view consistency, and user-visible state.

## 2. Baseline

OpenLife already has:

- proposal-first memory and LifeModel update constraints,
- ProposalStore,
- evidence and maturation foundations,
- accepted guidance/materialized LifeModel read models,
- bounded knowledge files such as `USER.md`, `MEMORY.md`, and `SOUL.md`,
- Review Center concepts,
- no silent durable write requirements.

OpenLife does not yet have a clean Main Chat product lifecycle for:

- accepted memory as a first-class user-visible object,
- rollback command and rollback event,
- accepted memory provenance in Main Chat,
- rollback effect on materialized context surfaces,
- user-facing rollback history.

## 3. Benchmark Lessons

### Codex-style lesson

Memory/instruction state should be inspectable, bounded, and reversible by user
action. File-like knowledge is valuable because users and agents can read what
is active.

### Hermes-style lesson

Memory should affect future task execution, but the user must see when the
agent is proposing or applying durable knowledge changes.

### OpenLife constraint

OpenLife memory is not a generic vector cache. It is evidence-backed LifeModel
and knowledge governance. Rollback must preserve audit and cannot simply delete
raw history.

## 4. Lifecycle States

| State | Meaning | Durable effect |
| --- | --- | --- |
| `candidate` | Agent thinks a memory may be useful. | None. |
| `pending_review` | Proposal exists and awaits user action. | Proposal only. |
| `edited_pending_review` | User edited candidate but has not accepted. | Proposal only. |
| `accepted` | User accepted the proposal. | Accepted memory/guidance asset created. |
| `pending_materialization` | Accepted asset exists but active runtime surfaces are not rebuilt yet. | Lifecycle record exists, not active context. |
| `materialized` | Accepted asset appears in bounded context/read model. | Context surface updated. |
| `materialization_failed` | Materialized view update failed after acceptance. | Lifecycle record remains inactive with error. |
| `rejected` | User rejected the proposal. | Rejection audit only. |
| `deferred` | User postponed decision. | Proposal remains inactive or pending later. |
| `superseded` | Accepted memory replaced by a newer accepted memory. | Old asset inactive, history kept. |
| `rolled_back` | User reversed accepted memory effect. | Asset inactive, rollback event recorded. |

## 5. Required Objects

### 5.1 MemoryLifecycleRecord

Required fields:

- `memoryId`
- `proposalId`
- `sourceTaskSessionId`
- `sourceRunId`
- `content`
- `scope`: `global`, `workspace`, `conversation`, or `project`
- `category`: `preference`, `fact`, `workflow`, `correction`, or `boundary`
- `riskLevel`: `low`, `medium`, `high`, or `identity_value`
- `status`
- `materializationStatus`: `not_required`, `pending`, `materialized`, or `failed`
- `materializationErrorCode`
- `createdBy`
- `acceptedBy`
- `acceptedAt`
- `materializedViewId`
- `materializedViewVersion`
- `evidenceIds`
- `confidence`
- `conflictIds`
- `supersedesMemoryId`
- `replacementMemoryId`
- `rolledBackByEventId`
- `runtimeContextExcludedAt`

### 5.2 MemoryRollbackEvent

Required fields:

- `rollbackEventId`
- `memoryId`
- `proposalId`
- `requestedBy`
- `reason`
- `previousStatus`
- `nextStatus`
- `affectedMaterializedViewIds`
- `affectedRuntimeSurfaceIds`
- `createdAt`
- `auditDigest`

## 6. Command Surface

Minimum commands:

- `accept_memory_proposal(proposalId)`
- `reject_memory_proposal(proposalId)`
- `edit_memory_proposal(proposalId, patch)`
- `defer_memory_proposal(proposalId)`
- `rollback_memory_asset(memoryId, reason)`
- `supersede_memory_asset(memoryId, replacementContent, reason)`
- `list_memory_assets(scope?, status?, limit, offset)`
- `get_memory_asset(memoryId)`
- `get_memory_lifecycle_events(memoryId)`
- `rebuild_memory_materialized_view(scope?)`
- `retry_memory_materialization(memoryId)`

Existing proposal commands may be reused only if they return or expose the
memory lifecycle record id created by acceptance. A rollback command must refuse
ambiguous text-only targets; it must operate on a resolved accepted `memoryId`.

## 7. Acceptance And Materialization Rules

Accepting a memory proposal is not complete until lifecycle and materialization
state are explicit.

Required flow:

1. Load and validate pending proposal.
2. Create a `MemoryLifecycleRecord` with `status=accepted` and
   `materializationStatus=pending`.
3. Attempt to rebuild or update the relevant materialized runtime surfaces.
4. If materialization succeeds, update record to `status=materialized` and
   `materializationStatus=materialized`.
5. If materialization fails, update record to `status=materialization_failed`,
   set `materializationStatus=failed`, keep the memory out of runtime context,
   and expose a visible retry/diagnostic path.

Transaction expectations:

- Proposal acceptance and lifecycle record creation should be atomic when they
  share a store.
- If materialized view rebuild happens in a separate store, failure must not
  leave the memory active in runtime context.
- A command response must distinguish `accepted_but_pending_materialization`,
  `materialized`, and `materialization_failed`.
- UI must not display "active memory" until materialization evidence exists.
- Rollback of a `materialization_failed` record should deactivate the lifecycle
  record and preserve the failure audit; it must not try to remove active
  runtime context that was never activated.

Phase A minimum materialized surface may be a typed Main Chat memory
materialized view/read model. File materialization to `MEMORY.md`, `USER.md`,
or `SOUL.md` can remain later unless the implementation explicitly supports it
with the same provenance, rollback, and exclusion guarantees.

## 8. Existing Store Mapping

Required mapping:

- `ProposalStore` remains the source of proposal status and proposal reason.
- Accepted memory lifecycle records are the source of truth for runtime memory
  state; raw transcript, vector rows, and materialized files are not source of
  truth.
- Evidence links may point to transcript entries, action observations, proposal
  source detail, or LifeModel-HS evidence assets.
- `MemoryStore` / vector store may index active accepted memory, but rollback
  must remove or exclude rolled-back memory from retrieval.
- `USER.md`, `MEMORY.md`, `SOUL.md`, and LifeModel materialized views are
  rebuildable runtime surfaces, not independent truth.
- Review Center must read lifecycle records rather than inferring memory state
  from proposal text.

## 9. Rollback Semantics

Rollback must not mean "delete all traces".

Allowed rollback forms:

- `deactivate`: accepted memory remains in history but no longer participates in
  runtime context.
- `supersede`: accepted memory is replaced by a corrected accepted memory.
- `reverse_materialization`: materialized view is rebuilt without the rolled
  back memory.

Disallowed rollback forms:

- deleting evidence required for audit,
- hiding the original proposal,
- mutating raw transcript,
- pretending the memory never existed,
- rolling back unrelated memories with the same text.

## 10. Runtime Context Rules

Runtime context compiler must obey:

- Only `materialized` active lifecycle records with
  `materializationStatus=materialized` can enter runtime memory context.
- `candidate`, `pending_review`, `edited_pending_review`, `rejected`,
  `deferred`, `pending_materialization`, `materialization_failed`,
  `rolled_back`, and `superseded` records are not runtime truth.
- `rolled_back` records may appear only in audit/history UI, not normal prompt
  context or retrieval.
- `superseded` records may appear only as provenance for the replacement.
- High-risk or identity/value memory rollback requires explicit confirmation or
  a reversal proposal; it must not be silently applied.
- Assistant-authored inference cannot become accepted memory unless user
  confirmation is linked in provenance.
- Task-local memory cannot be promoted to global scope without a separate
  proposal.

## 11. Main Chat UI Contract

Memory proposal card v2 must show:

- proposed memory text,
- scope,
- source evidence,
- confidence/conflict state,
- status,
- materialization failure or pending state,
- accept/reject/edit/defer controls when pending,
- rollback control only when accepted and rollback command exists,
- provenance link after acceptance,
- rollback history after rollback.

Rollback button must be hidden if no real rollback command exists.

## 12. Review Center Contract

Review Center must show:

- pending proposals,
- accepted memories,
- rejected/deferred proposals,
- rolled back memories,
- provenance and evidence,
- materialized view status.

Main Chat may link to Review Center, but Review Center cannot be the only place
where rollback state is visible.

## 13. Lifecycle Rules

- A user saying "remember this" creates a proposal, not memory.
- A user accepting a memory proposal creates or updates a lifecycle record.
- A user rejecting a memory proposal must prevent it from appearing in active
  memory or knowledge files.
- A rollback request must resolve to one accepted memory id.
- If multiple memories match, ask the user to choose.
- If memory was already superseded or rolled back, show terminal state.
- If memory acceptance is pending materialization, show pending state and retry
  materialization control instead of active memory.
- If materialization failed, show failure diagnostics and keep the memory
  excluded from runtime context.

## 14. Eval Scenarios

Minimum scenarios:

- create scoped memory proposal,
- accept memory and see provenance,
- reject proposal and prove absence from active memory,
- edit proposal before acceptance,
- defer proposal,
- show conflict evidence,
- rollback accepted memory,
- rollback ambiguous memory asks user,
- rollback already rolled back memory is blocked,
- accepted memory with materialization failure is not active,
- retry materialization succeeds or reports blocker,
- materialized context excludes rolled back memory.

## 15. Acceptance

This contract is satisfied when:

- `MP-06` passes with real rollback evidence,
- accepted memory has lifecycle record and provenance,
- accepted memory cannot be called active until materialized,
- materialization failure is visible and excludes memory from runtime context,
- rollback updates active materialized context,
- retrieval and bounded knowledge surfaces exclude rolled-back memory,
- raw transcript and evidence remain auditable,
- UI never shows rollback unless command exists,
- no silent memory writes occur.

## 16. Stop Conditions

Stop if:

- existing memory stores cannot identify accepted memory ids,
- materialized view cannot be rebuilt or invalidated safely,
- rollback would delete audit history,
- proposal acceptance cannot be linked to source evidence.
