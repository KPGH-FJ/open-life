# Memory

## Status

Source-backed description of current Memory storage, gateway, lifecycle,
retrieval, and Main Chat memory-candidate surfaces.

## Authority

Authority remains with `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and current
source.

## Last verified

2026-07-31 during repository cleanup source tracing.

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

`src-tauri/src/main_chat_memory_proposals.rs` supports draft edits for pending
Memory or Preference proposals. The edit report is draft-only, preserves the
original provenance, and reports that no durable write was executed.

## Retrieval And Context

`openlife-core/src/agent/memory_service.rs` retrieves memory context from text
search and optional vector search, merges and deduplicates hits, and formats a
bounded context string. Embedding failure falls back to text-only retrieval.

`openlife-core/src/memory_cache.rs` builds a hot cache from the current
LifeModel for identity, values, current goals, recent state, refresh time, and
LifeModel version. The cache is a prompt/context support surface, not a
canonical truth write path.

`src-tauri/src/main_chat_context_loader.rs` can include accepted lifecycle
memory snippets and bounded `MEMORY.md` surfaces in Main Chat context. It labels
them as bounded memory context and not trusted raw memory.

## Main Chat Memory Candidates

`openlife-core/src/agent/main_chat_memory_candidate.rs` extracts candidate
memory claims from user text. It routes candidates to session-only, life-event,
Memory proposal, LifeModel proposal, or no-op destinations based on explicit
memory markers, future-rule language, identity/preference signals, and life
event expressions.

Low-confidence candidates are not routed into durable paths. Future rules and
identity/preference style claims route to proposal destinations, not direct
truth mutation.
