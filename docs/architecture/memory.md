# Memory

## Status

Source-backed description of current Memory storage, gateway, lifecycle,
retrieval, and Main Chat memory-candidate surfaces.

## Authority

Authority remains with `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and current
source.

ADR 0016 makes Agent Memory a first-class domain separate from LifeModel.
Procedural working rules, Project context, and eligible Reflection outputs
belong here unless an exact supported LifeModel field is proposed and accepted
through the governed bridge. Markdown files are ordinary Project files, not a
second Memory owner.

## Source map

- `openlife-core/src/memory.rs`
- `openlife-core/src/memory_gateway.rs`
- `openlife-core/src/agent/main_chat_memory_candidate.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `openlife-core/src/conversation.rs`
- `src-tauri/src/agent_memory_learning.rs`
- `src-tauri/src/canonical_chat_runtime.rs`
- `src-tauri/src/canonical_work_runtime.rs`
- `src-tauri/src/memory_gateway.rs`
- `src-tauri/src/main_chat_context_loader.rs`
- `src-tauri/src/commands/memory.rs`

## Inherited blocker

Memory context and Memory proposals are governed support surfaces. They do not
replace external-live evidence and do not allow ordinary chat to silently write
canonical LifeModel truth.

## Storage Surfaces

`openlife-core/src/agent/memory_lifecycle.rs` owns durable Agent Memory bodies,
scope, provenance and retrieval disposition. Its SQLite lifecycle store is the
single fact owner for accepted, corrected, archived, restored, superseded,
rolled-back and privacy-erased Memory.

`openlife-core/src/memory.rs` defines the separate `MemoryStore` used by
conversation persistence and explicit knowledge notes. It retains FTS support
for those non-lifecycle rows, but schema version 9 removes old lifecycle-body
copies and their projection markers. It is not a second Agent Memory owner.

`src-tauri/src/commands/memory.rs` owns direct correction, archive, restore and
privacy erase controls. Correction, archive and restore are exact-owner,
reversible canonical actions; privacy erase is destructive and requires native
confirmation. Knowledge-note, search and index-rebuild helpers are not release
product commands.

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
duplicate content, creating a lifecycle record and projecting only a
rebuildable semantic index. It does not copy the lifecycle body into
`MemoryStore`.

## Memory Lifecycle

`openlife-core/src/agent/memory_lifecycle.rs` records accepted Memory proposals
as lifecycle records with scope, category, risk, materialization state, evidence
ids, conflicts, materialized view ids, and rollback state.

Accepted memory proposals create `memory:<uuid>` ids and update a materialized
view. Rollback removes accepted memory from active runtime context. High-risk
or identity/value memory rollback requires explicit confirmation instead of
being silently rolled back.

The product keeps the following actions distinct:

- correction directly creates a canonical replacement bound to one exact prior
  `memory:<uuid>`; the old owner becomes superseded atomically and remains
  available to the existing rollback path;
- archive directly commits the `archived` disposition and keeps the canonical
  asset outside normal runtime retrieval;
- restore returns an archived asset to `active` without recreating content;
- rollback terminates one applied change while retaining its body and lifecycle
  history;
- privacy erase requires native confirmation, removes the canonical body,
  content-bearing provenance, and corresponding accepted Review proposal
  payload, then emits a tombstone that deletes the MemoryStore and VectorStore
  projections. Only body-free Memory audit metadata remains, and replay of the
  erased proposal cannot resurrect the body. This exact-Memory action does not
  silently delete a separate source conversation or workspace document; those
  retain their own explicit deletion controls.

`MemoryViewModel.items` is the product-facing projection for content, scope,
source explanation, recall state and allowed actions. Its summary contains only
total, active, archived and historical counts. The `/life-model` Memory area
does not expose lanes, tiers, materialization, linkage or vector telemetry, and
does not merge raw backend stores in the browser. A direct control is reported
as complete only after canonical commit and its exact rebuildable projection
receipt are confirmed.

## Retrieval And Context

The production Main Chat lifecycle retrieval path is
`main_chat_context_loader -> memory_gateway -> MemoryLifecycleStore`, with
`VectorStore` as an optional rebuildable semantic index. Bounded lexical
matching reads canonical lifecycle records directly, including CJK bigrams;
semantic matches are rejoined to the same lifecycle owners. Only owners admitted
for the active scope may be returned or receive access telemetry. The canonical
lifecycle store rechecks active/archive, supersede and rollback truth before any
content becomes prompt context.

Personal Memory is stored internally as the global scope and has no scope
owner. Project Memory carries an opaque `scopeOwnerRef` derived from the
canonical Project identity, never from cwd, a selected folder, or guessed
content. New product admissions support only these two scopes. Historical
Conversation or Workspace scoped records remain readable only through their
exact stored owner for compatibility; the current product does not create new
ones, and historical non-global records without an owner are excluded from
normal recall.

Eligible candidates are deduplicated by lifecycle owner and ranked by combined
retrieval relevance, freshness, conflict state, confidence and source quality.
Unresolved conflicts are excluded. Every selected block contains its
`memory:<uuid>` source ref, scope owner, freshness and selection reason. A turn
injects at most four Memory blocks and 4,800 body characters. Embedding failure,
unknown profile, rebuild or Vector query failure keeps canonical lexical results
and adds an explicit degraded marker; it never reports text fallback as complete
hybrid retrieval.

## User control surface

The `/life-model` route is the Personal Intelligence workspace, not a claim
that Agent Memory belongs to LifeModel. It presents `LifeModelViewModel` and
`MemoryViewModel` as peer domains. Each domain remains readable when only the
other owner fails. Ordinary reversible Memory controls depend on current Memory
truth, not on Review Center availability.

Each Memory item exposes its content, scope, why it was remembered, recall state
and backend-owned source references. Correction, archive and restore use exact
canonical owners and verified projection receipts. Privacy erase also requires
native confirmation. Lifecycle status, proposal state and storage diagnostics
stay out of the ordinary product surface. UI counts never prove a single action
was committed or projected.

Settings owns one global Agent Memory switch. Each Conversation then selects
`Use and learn`, `Use only`, or `Off`; the Conversation setting cannot override
a disabled global switch. Explicit user commands such as “remember” and
“forget” remain available as direct user controls and do not require a Work
Task or provider call. Requests for retired Conversation or Workspace Memory
scope are rejected with a product explanation instead of being silently
widened to Personal scope.

`src-tauri/src/main_chat_context_loader.rs` can include bounded accepted
lifecycle Memory through the typed Personal Intelligence port. It labels the
content as optional context, not policy, permission, or completion evidence.

Ordinary Chat and Work history is canonical only in
`ConversationStore.conversation_items`; the retired conversation-memory store
is not a release input. The runtime reconstructs a bounded provider context
from canonical Conversation Items through
`agent/conversation_context.rs`; its deterministic summary is a derived
projection with a source range and digest, not long-term Memory.

## Main Chat Memory Candidates

`openlife-core/src/agent/main_chat_memory_candidate.rs` extracts candidate
memory claims from user text. It routes candidates to session-only, life-event,
Memory proposal, LifeModel proposal, or no-op destinations based on explicit
memory markers, future-rule language, identity/preference signals, and life
event expressions.

Low-confidence candidates are not routed into durable paths. When global Memory
and the Conversation's `Use and learn` mode are both enabled, one eligible
stable, internal candidate may be checked after the latest completed Turn has
been idle for the bounded delay. The check uses only that Turn's already-bound
provider profile and model; a changed provider, newer Turn, disabled mode,
invalid JSON or low confidence produces no proposal. A retained candidate
creates one Review proposal and never writes Memory directly. Duplicate facts
are skipped, while a materially clearer fact may explicitly supersede one
matching owner after Review. Personal future working rules route to Agent
Memory proposal candidates. Identity and stable
preference candidates can reach the LifeModel bridge only when the runtime can
produce an exact supported field path and typed value; otherwise the candidate
remains blocked and no fake proposal is created.

Memory lifecycle and vector retrieval enrich the Agent. If an optional
enrichment store is unavailable, Main Chat carries an explicit degraded marker
and continues with healthy base context; exact reads and writes against the
missing store remain unavailable.

## Canonical Chat And Work Port

Release Chat and Work depend on `AgentMemoryContextPort` in
`src-tauri/src/personal_intelligence_ports.rs`, not on execution-runtime
tables. The port retrieves bounded lifecycle
Memory, may use confirmed LifeModel terms only for reranking, and returns
context candidates that grant no permission or completion authority. A missing
optional port degrades context without making the Conversation or Task owner
unavailable.

For an explicit, policy-authorized, low-risk fact, canonical Chat or Work invokes
`PersonalIntelligenceSuggestionPort`, which delegates to the existing
reversible Memory gateway. Chat completes its Conversation Turn without
creating a Task; Work records a completed Observation Item. Neither path invokes
a model or creates a Proposal. Failure remains fail-closed instead of leaving a
running owner. Stable identity or preference statements are not misclassified
into this lane.

When Work actually selects Memory into provider context, the canonical Run
records only the `memory:<uuid>` id, product scope, content digest and selection
reason. It does not copy the Memory body into Task metadata. Vector and lexical
hits for one lifecycle owner are merged by canonical source identity so one
Memory cannot produce conflicting Run receipts merely because projection
session metadata differs.

LifeModel remains a separate optional context port. Agent Memory cannot grant
LifeModel write authority, and background Memory learning never creates or
updates LifeModel candidates or versions.
