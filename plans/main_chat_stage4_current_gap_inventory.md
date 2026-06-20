# Main Chat Stage 4 Current Gap Inventory

> Date: 2026-06-20
> Stage: Stage 4 - Memory and Knowledge Asset Productization
> Status: preparation inventory

## 1. Current Assets To Reuse

| Area | Current state | Reuse direction |
| --- | --- | --- |
| Memory lifecycle store | `openlife-core/src/agent/memory_lifecycle.rs` has `MemoryLifecycleRecord`, materialized views, accept, rollback, list, get, events, and active-record filtering. | Treat as the governed memory source of truth. Do not replace it. |
| Proposal store | `openlife-core/src/agent/proposal_store.rs` stores proposal status and metadata; `src-tauri/src/commands/proposal.rs` applies accept/reject/edit/postpone. | Reuse for proposal-first memory confirmation, but clarify edit semantics. |
| Memory commands | `rollback_memory_asset`, `list_memory_assets`, `get_memory_asset`, `get_memory_lifecycle_events`, and `rebuild_memory_materialized_view` are registered Tauri commands. | Productize these commands through UI and eval coverage. |
| Context loader | `src-tauri/src/main_chat_context_loader.rs` loads bounded `AGENTS.md`, `SOUL.md`, `USER.md`, `MEMORY.md`, `memories/USER.md`, `memories/MEMORY.md`, selected `SKILL.md`, recent session metadata, and active memory lifecycle records. | Add context inventory/audit and ensure only active lifecycle memory becomes durable truth. |
| Legacy memory store | `openlife-core/src/memory.rs` and `openlife-core/src/agent/memory_service.rs` provide session messages, text search, vector search, and memory snippets. | Keep as search/evidence layer; do not let it bypass lifecycle rollback. |
| Agent state payload | `openlife-core/src/agent/main_chat_agent_productization_v1.rs` includes proposal memory lifecycle records and rollback controls. | Extend final delivery and UI state for durable memory changes and knowledge assets. |
| AgentControlPlane | `frontend/src/components/AgentControlPlane.tsx` shows memory proposal/rollback controls when lifecycle evidence exists. | Keep as inline task surface; add deeper management via Review/Knowledge asset UI. |
| Review/proposal UI | `frontend/src/pages/ProposalReviewPage.tsx`, `MailboxPage.tsx`, `ChatPage.tsx`, and `LifeModelPage.tsx` show proposals and some proposal controls. | Add accepted memory/rollback/provenance visibility, not just pending proposal rows. |
| Memory eval | `src-tauri/src/main_chat_memory_lifecycle_eval.rs` has MR-01 through MR-09 deterministic lifecycle scenarios. | Reuse as base, but Stage 4 needs broader Main Chat/product knowledge-asset coverage. |

## 2. Product Gaps

| Gap | Current symptom | Product risk | Stage 4 target |
| --- | --- | --- | --- |
| Lifecycle exists but is not a user-facing asset manager | Users can trigger some inline controls, but there is no complete memory asset surface showing active, pending, rejected, superseded, rolled back, provenance, and context status together. | Users cannot understand or repair what the Agent remembers. | Build a memory/knowledge asset surface using existing lifecycle and proposal commands. |
| Pending proposal is not a lifecycle record | `MemoryLifecycleRecord` starts at acceptance/materialization; pending state primarily lives in `ProposalStore`. | UI/eval may describe lifecycle states that do not exist as records yet. | Document the split and either expose a composed pending-memory view or create candidate records deliberately. |
| Edit semantics are ambiguous | `edit_proposal_with_state` applies the proposal payload before marking the proposal edited. | "Edit proposal" can behave like "edit and apply", which is unsafe for memory unless the UI clearly says so. | Support draft-only edit for pending memory. If edit-and-accept remains, expose it as a separate explicitly named durable control. |
| Rollback does not clean old search/vector memory | Memory acceptance saves lifecycle-backed content into `MemoryStore` and vector store. `rollback_memory_asset` updates lifecycle/materialized view but does not update old `MemoryStore` rows. | Rolled-back memory may still appear via text/vector retrieval even though lifecycle says inactive. | Retrieval must exclude lifecycle-backed rolled-back/superseded records, or mark/archive linked `MemoryStore` rows during rollback. |
| Final delivery durable changes are empty | `FinalDeliveryEvidence.durable_changes` is currently `Vec::new()`. | Users cannot see accepted/rolled-back memory or confirmed knowledge-file writes as durable work in the terminal contract. | Populate durable changes for memory accept/materialize/rollback and managed `USER.md` / `MEMORY.md` write/rollback outcomes. |
| Knowledge files are loaded but not managed | `USER.md`, `MEMORY.md`, `SOUL.md`, `AGENTS.md`, and selected `SKILL.md` can be loaded as bounded context, but there is no product surface for loaded/skipped/truncated/digest/source status. | Knowledge looks like hidden prompt stuffing. | Add knowledge asset inventory and traceable context-source display. |
| Knowledge file writes are not productized | Contract says proposal-first edits, but there is no focused writer/diff/materializer for `USER.md` / `MEMORY.md`. | OpenLife cannot honestly claim Codex/Claude-like readable memory assets. A preview-only draft is also insufficient because users need confirmation, write audit, reload, and rollback. | Implement mature proposal-backed managed write paths for both `USER.md` and `MEMORY.md`: proposal, draft/diff, validation, explicit confirmation, atomic write, audit, context reload, and rollback/snapshot proof. |
| `SOUL.md` risk boundary is not enforced as a product flow | Loader treats `SOUL.md` as private context; editing/writing policy is not productized. | High-level identity/value memory could become a jailbreak or over-personalization surface. | Keep `SOUL.md` read-only or high-risk proposal-first with explicit confirmation in Stage 4; do not treat it like ordinary `USER.md` / `MEMORY.md` managed writes. |
| Context inventory is not complete enough for memory debugging | Stage 3 shows some context state, but accepted lifecycle memory, file surfaces, skipped surfaces, and old memory hits are not unified in a user-readable inventory. | Tester cannot tell why the Agent used or ignored a memory. | Add context inventory fields and UI/test coverage for loaded, skipped, active lifecycle, and excluded rolled-back memory. |
| Existing MR eval is backend-heavy | MR-01 through MR-09 prove store/lifecycle behavior, but not enough ordinary Main Chat UI/context/knowledge-asset behavior. | Stage 4 could pass tests without the user seeing or benefiting from memory. | Add Stage 4 report with product scenarios MK4-01 through MK4-18. |
| Review Center is proposal-centric | Existing pages are good for proposals but not for accepted memory lifecycle history. | Accepted memory becomes invisible after confirmation. | Review Center/Knowledge surface must show accepted and rolled-back memory assets and lifecycle events. |

## 3. High-risk Files

Stage 4 is likely to touch:

- `openlife-core/src/agent/memory_lifecycle.rs`
- `openlife-core/src/memory.rs`
- `openlife-core/src/agent/memory_service.rs`
- `openlife-core/src/agent/main_chat_agent_productization_v1.rs`
- `src-tauri/src/commands/proposal.rs`
- `src-tauri/src/commands/memory.rs`
- `src-tauri/src/main_chat_context_loader.rs`
- `src-tauri/src/main_chat_memory_lifecycle_eval.rs`
- `src-tauri/src/main_chat_agent_productization_eval.rs`
- `src-tauri/src/main_chat_agent_stage2_readiness.rs`
- `src-tauri/src/main_chat_stage3_execution_ux.rs`
- `frontend/src/components/AgentControlPlane.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/ProposalReviewPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/tauri.ts`
- `frontend/src/test/mocks/tauri.ts`
- a new focused Stage 4 report/test module, if extraction is cleaner.

Avoid broad edits to planner, live-provider final gate, or Stage 1/2/3 gates
unless Stage 4 evidence must be wired into existing aggregate reports.

Also avoid broad rewrites of `MemoryLifecycleStore`, `MemoryStore`, or vector
search. Prefer linked metadata, filtering, archive markers, or scoped adapters
unless a focused rollback-exclusion test proves those paths cannot satisfy the
contract.

## 4. Out Of Scope For Stage 4

- Public Skills Hub or marketplace.
- Full self-evolution of memory, skills, or prompts.
- Cross-device sync.
- Bulk import/export.
- Replacing Stage 2 readiness gate or filling manual dogfood rows.
- Arbitrary direct writes to knowledge files.
- Mature write management for `SOUL.md`, `AGENTS.md`, or `SKILL.md`; Stage 4
  should inspect/block or route these through high-risk confirmation rather than
  expanding into a general workspace-file manager.
- Treating old vector memory as accepted user truth.
- Broad background autonomy.

## 5. Stage 4 Done Means

Stage 4 is done when an internal tester can:

- see pending memory proposals with evidence and conflict state;
- accept, reject, defer, edit safely, and rollback accepted memory;
- inspect accepted and rolled-back memory later;
- see which knowledge assets affected a Main Chat task;
- confirm `USER.md` and `MEMORY.md` managed writes through proposal, diff,
  confirmation, audit, reload, and rollback;
- confirm that rolled-back/rejected memory no longer enters runtime context;
- see durable memory and managed knowledge-file changes in final delivery;
- use accepted memory in ordinary DirectAnswer/ReAct/Plan flows without hidden
  prompt stuffing.

Stage 4 still does not grant limited internal trial readiness by itself.
Stage 5 and the later real S2-D manual dogfood/current-commit live evidence
remain required.
