# Memory

## Status

Source-backed description of current Memory storage, gateway, lifecycle,
retrieval, and Main Chat memory-candidate surfaces.

## Authority

Authority remains with `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and current
source.

ADR 0016 makes Agent Memory a first-class domain separate from LifeModel.
Procedural working rules, project context, Reflection, and bounded Markdown
memory belong here unless an exact supported LifeModel field is proposed and
accepted through the governed bridge.

## Last verified

2026-08-06 during Phase 5.1F user-control and native interface verification.

## Source map

- `openlife-core/src/memory.rs`
- `openlife-core/src/memory_gateway.rs`
- `openlife-core/src/memory_cache.rs`
- `openlife-core/src/agent/memory_service.rs`
- `openlife-core/src/agent/main_chat_memory_candidate.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `src-tauri/src/memory_gateway.rs`
- `src-tauri/src/main_chat_memory_proposals.rs`
- `src-tauri/src/main_chat_context_loader.rs`
- `src-tauri/src/markdown_memory.rs`
- `src-tauri/src/commands/memory.rs`

## Inherited blocker

Memory context and Memory proposals are governed support surfaces. They do not
replace external-live evidence and do not allow ordinary chat to silently write
canonical LifeModel truth.

## Storage Surfaces

`openlife-core/src/memory.rs` defines `MemoryStore`, backed by SQLite tables for
messages, memories, snapshots, chat sessions, and state history. It also creates
FTS support for memory text search. Chat messages are saved as messages and as
private `chat_message` memory rows.

`src-tauri/src/commands/memory.rs` exposes memory commands for chunk count,
chunk indexing, search, hot cache, archiving, tier stats, maintenance, and
rebuild. The rebuild path requires danger-action confirmation.

`src-tauri/src/memory_gateway.rs` connects those store operations to the app
state. It desensitizes search queries, uses privacy-aware embeddings, and falls
back when embedding generation is unavailable.

## Memory Gateway

`openlife-core/src/memory_gateway.rs` classifies memory writes by lane:
turn context, episodic life event, semantic fact/preference, procedural rule,
evidence record, or canonical LifeModel truth.

Chat turns are context-only. Episodic events, semantic facts/preferences, and
metadata-safe evidence can become local memory. Future procedural rules require
review unless they are being materialized after accepted proposal review.
Canonical LifeModel truth requests are routed to the LifeModel write gateway
instead of being written as ordinary memory.

`src-tauri/src/memory_gateway.rs` materializes accepted Memory proposals by
checking the gateway decision, rejecting canonical-LifeModel writes, checking
duplicate content, creating a lifecycle record, optionally inserting vector
memory, and saving a private `proposal_memory` record with proposal and lifecycle
tags.

## Memory Lifecycle

`openlife-core/src/agent/memory_lifecycle.rs` records accepted Memory proposals
as lifecycle records with scope, category, risk, materialization state, evidence
ids, conflicts, materialized view ids, and rollback state.

Accepted memory proposals create `memory:<uuid>` ids and update a materialized
view. Rollback removes accepted memory from active runtime context. High-risk
or identity/value memory rollback requires explicit confirmation instead of
being silently rolled back.

The product lifecycle now keeps the following actions distinct:

- correction creates a reviewed `MemoryWrite` replacement bound to one exact
  prior `memory:<uuid>`; the old owner becomes superseded only after acceptance;
- stop recall commits the `paused` retrieval disposition after Review and keeps
  the canonical asset outside normal runtime retrieval;
- archive commits the separate `archived` disposition after Review; paused and
  archived assets can both be restored to `active` without recreating content;
- rollback terminates one applied change while retaining its body and lifecycle
  history;
- privacy erase requires native confirmation, removes the canonical body,
  content-bearing provenance, and corresponding accepted Review proposal
  payload, then emits a tombstone that deletes the MemoryStore and VectorStore
  projections. Only body-free Memory audit metadata remains, and replay of the
  erased proposal cannot resurrect the body. This exact-Memory action does not
  silently delete a separate source conversation or workspace document; those
  retain their own explicit deletion controls.

`MemoryViewModel.items` is the product-facing owner for content, scope,
provenance explanation, recall state and allowed actions. The `/life-model`
Memory area consumes that ViewModel instead of merging raw lifecycle rows and
vector telemetry in the browser. It labels proposal creation as pending Review,
not as an applied Memory change.

`src-tauri/src/main_chat_memory_proposals.rs` supports draft edits for pending
Memory or Preference proposals. The edit report is draft-only, preserves the
original provenance, and reports that no durable write was executed.

## Retrieval And Context

The production Main Chat retrieval path is
`main_chat_context_loader -> memory_gateway -> MemoryStore/VectorStore ->
MemoryLifecycleStore`. It queries the existing FTS and Vector projections, but
only lifecycle owners admitted for the active scope may be returned or receive
access telemetry. The canonical lifecycle store rechecks active/paused/archive,
supersede and rollback truth before any content becomes prompt context.

Global Memory has no scope owner. Conversation, Workspace and Project Memory
uses an opaque `scopeOwnerRef` derived from the canonical conversation or the
user-selected root. The owner is part of fact identity, so identical text in
two projects remains two facts. A trusted runtime binds proposal materialization
to the current selected scope; a forged or mismatched owner is rejected.
Historical non-global records without an owner remain visible but are excluded
from normal recall rather than being assigned from cwd or guessed by content.

Eligible candidates are deduplicated by lifecycle owner and ranked by combined
retrieval relevance, freshness, conflict state, confidence and source quality.
Unresolved conflicts are excluded. Every selected block contains its
`memory:<uuid>` source ref, scope owner, freshness and selection reason. A turn
injects at most four Memory blocks and 4,800 body characters. Embedding failure,
unknown profile, rebuild or Vector query failure keeps FTS results and adds an
explicit degraded marker; it never reports text fallback as complete hybrid
retrieval.

## User control surface

The `/life-model` route is the Personal Intelligence workspace, not a claim
that Agent Memory belongs to LifeModel. It presents `LifeModelViewModel` and
`MemoryViewModel` as peer domains. Each domain remains readable when only the
other owner fails; reviewed Memory actions also require a current
`ReviewCenterViewModel` before the UI enables them.

Each Memory item exposes its canonical lifecycle state, why it was remembered,
how it is eligible for per-turn recall, and backend-owned source references.
Correction, pause and archive create Review proposals. Restore and rollback
require exact canonical owners and verified projection receipts. Privacy erase
also requires native confirmation. UI counts never prove a single action was
materialized.

`openlife-core/src/memory_cache.rs` builds a hot cache from the current
LifeModel for identity, values, current goals, recent state, refresh time, and
LifeModel version. The cache is a prompt/context support surface, not a
canonical truth write path.

`src-tauri/src/main_chat_context_loader.rs` can include accepted lifecycle
memory snippets and bounded Markdown working-memory surfaces in Main Chat
context. It labels them as bounded memory context and not trusted raw memory.

Since R1, ordinary Chat history is canonical only in
`ConversationStore.conversation_items`. `MemoryStore.messages` remains a
compatibility input for the Work runtime until R2 and is not read by the Chat
ViewModel or canonical Chat runtime. The runtime reconstructs a bounded
provider context from the canonical Conversation Items through
`agent/conversation_context.rs`; its deterministic summary is a derived
projection with a source range and digest, not long-term Memory.

## Workspace And Project Markdown Memory

`src-tauri/src/markdown_memory.rs` owns the file and scope contract. Workspace
and Project are two explicit, user-selected directory roots stored in config.
They are not inferred from the process working directory or from the list of
generic knowledge roots. If both scopes select the same physical directory,
the directory is loaded once rather than receiving two competing identities.

Within each selected root, the active readable files are limited to
`MEMORY.md` and one-level `memories/*.md` topic files. Symbolic links, nested
paths, disabled `*.disabled.md` files, oversized files, and every other root are
excluded. Runtime selection is task-relevant and capped by both file count and
total character budgets. Each context block exposes its scope, relative source,
and selection reason, and says explicitly that working memory is not identity,
permission, or completion evidence.

The Workspace editor reads through a backend ViewModel. Creating or editing a
file only creates an `ExternalWriteAction` Review proposal with an exact target
precondition. Deactivation is a reviewed move to `*.disabled.md`. Approval and
artifact materialization still use the existing proposal and artifact gateway;
neither the editor nor the context loader writes files directly.

## Main Chat Memory Candidates

`openlife-core/src/agent/main_chat_memory_candidate.rs` extracts candidate
memory claims from user text. It routes candidates to session-only, life-event,
Memory proposal, LifeModel proposal, or no-op destinations based on explicit
memory markers, future-rule language, identity/preference signals, and life
event expressions.

Low-confidence candidates are not routed into durable paths. Personal future
working rules route to Agent Memory proposal candidates. Identity and stable
preference candidates can reach the LifeModel bridge only when the runtime can
produce an exact supported field path and typed value; otherwise the candidate
remains blocked and no fake proposal is created.

Memory lifecycle and vector retrieval enrich the Agent. If an optional
enrichment store is unavailable, Main Chat carries an explicit degraded marker
and continues with healthy base context; exact reads and writes against the
missing store remain unavailable.
