# Main Chat Stage 4 Memory And Knowledge Product Contract

> Date: 2026-06-20
> Stage: Stage 4 - Memory and Knowledge Asset Productization
> Status: preparation contract

## 1. Objective

Make OpenLife memory and knowledge assets user-visible, governed, reversible,
and consumed by the default Main Chat Agent path.

The user should be able to answer seven questions:

1. What is OpenLife proposing to remember?
2. Why did it propose that memory?
3. What accepted memory is active right now?
4. Which memories or knowledge files influenced this task?
5. What is pending, rejected, conflicted, superseded, or rolled back?
6. What will happen if I accept, edit, reject, defer, or roll back?
7. Did the Agent actually use the accepted memory correctly?

## 2. Product Boundary

Stage 4 owns:

- memory proposal lifecycle productization;
- accepted memory asset inspection;
- rollback and active-context exclusion;
- knowledge asset inventory for `AGENTS.md`, `USER.md`, `MEMORY.md`,
  `SOUL.md`, and selected `SKILL.md`;
- context consumption and context inventory for accepted memory;
- final delivery durable memory and managed knowledge-file changes;
- Stage 4 memory/knowledge eval/report.

Stage 4 does not own:

- full public Skills Hub;
- automatic self-evolution;
- cross-device sync;
- external provider/live readiness completion;
- Stage 5 release/debug operations;
- filling the S2-D manual dogfood artifact.

## 3. Source Of Truth

| Object | Source of truth | Notes |
| --- | --- | --- |
| Pending memory proposal | `ProposalStore` plus linked evidence/action/task ids | Pending proposal is not active memory. |
| Accepted memory | `MemoryLifecycleStore` record with `status=materialized` and `materializationStatus=materialized` | Only active records can become runtime memory context. |
| Rolled-back memory | `MemoryLifecycleStore` record with `status=rolled_back` and rollback event | Historical/audit only; never normal runtime context. |
| Raw chat/session memory | `MemoryStore`, vector store, transcript, session search | Evidence/search context only; not durable user truth. |
| Knowledge files | Workspace/configured files loaded by `main_chat_context_loader` | Context surfaces, not policy authority. |
| `USER.md` / `MEMORY.md` managed assets | Generated or curated projection of accepted memory/guidance | Must be proposal-backed, versioned, auditable, reloadable, and rollback-capable when written. |
| `SOUL.md` | High-risk identity/value context surface | Read-only or explicit high-risk proposal-first in Stage 4. |
| `SKILL.md` | Workflow instruction context selected by user/task | Cannot grant tool permission or become user memory automatically. |

## 4. Lifecycle Rules

- Assistant-authored text cannot become accepted memory without user
  confirmation.
- Raw transcript, vector hits, and session search cannot become active memory by
  retrieval alone.
- Rejected memory cannot enter runtime context as accepted truth.
- Rolled-back memory must be excluded from lifecycle context and from linked
  `MemoryStore` / vector retrieval.
- Superseded memory is provenance for replacement only.
- High-risk or identity/value memory requires explicit confirmation for accept
  and rollback.
- Task-local memory cannot silently promote to project/workspace/global scope.
- `USER.md` and `MEMORY.md` direct edit requests must enter a managed write
  lifecycle; they must not silently mutate governed truth.
- `SOUL.md`, `AGENTS.md`, and `SKILL.md` direct edit requests must be blocked or
  routed into an explicitly high-risk proposal/confirmation path in Stage 4.
- Knowledge files cannot override ExecutionPolicy, privacy policy, model route,
  tool permission, or external write policy.

## 5. Required UI States

| State | Required UI behavior | Required controls |
| --- | --- | --- |
| Candidate/proposed memory | Shows proposed text, source evidence, scope, category, confidence, risk, conflicts, target surface. | Accept, reject, edit draft, defer, open review. |
| Edited pending memory | Shows original and edited text plus unchanged provenance. | Accept edited, reject, defer. |
| Accepted/materialized memory | Shows memory id, scope, category, source proposal, evidence, materialized view/version, context-active status. | Rollback, open events, open review. |
| Materialization failed | Shows accepted-but-inactive state and error code. | Retry materialization, rollback, open trace. |
| Rolled-back memory | Shows rollback reason, event id, affected surfaces, inactive status. | Open history only unless replacement proposal exists. |
| Conflict detected | Shows conflicting active memory and proposed replacement/correction. | Accept replacement, reject, compare, defer. |
| Knowledge asset loaded | Shows file path/source, digest, size/truncation, reason, selected skill id if relevant. | Open/inspect; write only via proposal. |
| Knowledge asset skipped | Shows reason: missing, unsafe path, unselected skill, oversized, policy, privacy. | No silent load. |
| Managed knowledge draft | Shows target file, source proposal, before/after digest, validation result, previewable diff, and no-write-before-confirmation proof. | Confirm write, revise draft, reject, defer. |
| Managed knowledge write applied | Shows target file, version id, audit id, write timestamp, new digest, context reload status, and rollback/snapshot handle. | Rollback file version, open audit, open context inventory. |
| Managed knowledge rollback | Shows restored version id, prior applied version id, audit id, restored digest, and context exclusion/reload proof. | Open history only unless replacement proposal exists. |
| Final delivery | Separates durable memory changes, managed knowledge-file changes, proposals, blockers, pending user actions, and rollback availability. | Object-scoped controls only. |

## 6. Context Consumption Rules

The Main Chat context compiler must expose and obey an inventory:

- active lifecycle memory ids loaded;
- lifecycle memory ids excluded because rejected, rolled back, superseded,
  failed, deferred, or pending;
- knowledge files loaded/skipped/truncated with digest/source;
- selected skill id and selected-skill source;
- old `MemoryStore` / vector hits used only as search/evidence context;
- policy decisions that prevented memory/context injection.

DirectAnswer, ReAct, and PlanExecute can use active memory differently, but all
must obey the same active/excluded memory rules.

Minimum Stage 4 target:

- active lifecycle memory can affect ordinary Main Chat response behavior;
- rolled-back lifecycle memory cannot affect response behavior through either
  lifecycle context or legacy memory/vector retrieval;
- confirmed `USER.md` / `MEMORY.md` writes appear in a later context inventory
  with the new digest/source, while rolled-back file versions are excluded from
  active context;
- context inventory is visible in AgentControlPlane or an inspectable trace
  drawer.

## 7. Knowledge Asset Rules

### `AGENTS.md`

- Project/workspace instruction context.
- Read/inspect only by default.
- Any write is a workspace write and must require explicit confirmation,
  validation, and audit.

### `SKILL.md`

- Workflow instruction context.
- Loaded only when selected.
- Unselected skills must not be injected.
- Skills cannot grant permission or write memory directly.

### `USER.md`

- Short user profile/preference projection.
- Stage 4 must manage this file through the proposal-backed write lifecycle.
- It should contain concise accepted user profile/preference projections, not
  raw transcript or assistant-only inference.

### `MEMORY.md`

- Curated memory summary/index.
- Must reflect accepted active memory only, not raw transcript.
- Rolled-back/superseded memory must not appear as active.
- Stage 4 must manage this file through the proposal-backed write lifecycle.

Both `USER.md` and `MEMORY.md` must support the mature managed write lifecycle in
Stage 4. A preview-only draft/diff is not enough. A valid lifecycle includes:

- proposal with provenance proposal id, source memory ids, risk, and target path;
- draft/diff with before digest, after digest, previewable diff, and validation
  result;
- proof that no file write occurs before explicit confirmation;
- atomic write on confirmation, with audit evidence and version id;
- context reload proof showing the new asset digest/source in the next Main Chat
  inventory;
- rollback/snapshot handle that can restore the prior version and remove the
  reverted content from active context.

### `SOUL.md`

- High-level identity/value surface.
- Stage 4 should keep it read-only unless an explicit high-risk proposal flow is
  implemented.

## 8. Non-fake Rules

- Do not claim a memory is active unless lifecycle status and materialization
  status prove it.
- Do not claim rollback succeeded unless a rollback event exists.
- Do not claim a knowledge file was written unless the write actually happened
  with proposal/audit evidence.
- Do not claim `USER.md` or `MEMORY.md` managed writes are complete if either
  file only has a blocker or preview-only draft path.
- Do not claim a managed knowledge write is applied unless the file write,
  audit/version id, and context reload evidence all exist.
- Do not claim managed knowledge rollback unless a prior snapshot/version is
  restored and the restored digest is reflected by context inventory.
- Do not show old vector/search hits as accepted memory.
- Do not let `edit_proposal` silently look like draft-only editing if it applies
  durable state.
- Do not treat an edit-and-accept operation as a substitute for draft-only
  pending memory editing.
- Do not count a direct-write blocker as the required `USER.md` / `MEMORY.md`
  managed write lifecycle.
- Do not hide materialization failure behind "accepted".
- Do not weaken Stage 1, Stage 2, Stage 3, beta, final acceptance, or live
  provider fail-closed semantics.

## 9. Stage 4 Exit Criteria

- Stage 4 eval/report covers MK4-01 through MK4-18.
- Memory accept/reject/draft-edit/defer/rollback semantics are explicit and
  tested.
- DirectAnswer, ReAct, and PlanExecute active-memory consumption are covered.
- Both `USER.md` and `MEMORY.md` support proposal-backed managed writes:
  draft/diff, validation, confirmation, atomic write, audit, context reload, and
  rollback/snapshot.
- Rolled-back lifecycle memory is excluded from all default runtime context
  paths, including legacy text/vector retrieval.
- User can inspect active and inactive memory assets.
- User can inspect loaded and skipped knowledge assets for a task.
- User can inspect `USER.md` and `MEMORY.md` managed draft/applied/rollback
  history.
- Final delivery includes durable memory and managed knowledge-file changes
  where applicable.
- Existing Stage 1/2/3 and final acceptance gates still pass or remain blocked
  only for documented manual/live evidence.
