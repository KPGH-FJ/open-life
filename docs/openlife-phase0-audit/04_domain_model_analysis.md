# Domain Model Analysis

## LifeModel

Finding: The LifeModel is a real structured domain model, not a placeholder.

Evidence:

- `LifeModel` includes metadata, identity, goals, capabilities, state,
  relationships, preferences, and evolution rules.
- `LifeModel::default_model` creates a fully shaped empty model.
- `LifeModelManager` supports load and save.
- Patch application and compatibility/provenance views exist.

File location:

- `openlife-core/src/life_model.rs`
- `openlife-core/src/life_model/patch.rs`
- `openlife-core/src/life_model/patch_store.rs`

Confidence: High.

Impact: Frontend v2 should preserve and explain this model rather than rebuild
life state as loose chat metadata.

## LifeModel-HS Compatibility and Provenance

Finding: The code includes a compatibility/provenance layer for source-backed
LifeModel views.

Evidence:

- `LifeModelMaterializedViewProvenance` tracks compatibility materialized view,
  accepted source of truth, durable truth materialized, proposal-first required,
  source proposal/evidence/patch/heuristic ids, and provenance digest.
- `LifeModelHSCompatibilityView` includes materialized state summary,
  collaboration summaries, asset refs, and source digest.

File location:

- `openlife-core/src/life_model.rs`

Confidence: High.

Impact: The product can expose provenance and trust, but current UI should not
claim canonical truth when compatibility views are the source.

## Memory Model

Finding: Memory is split across chat messages, memory rows, vector chunks,
memory lifecycle, evidence, and gateway decisions.

Evidence:

- `MemoryStore` creates `messages`, `memories`, FTS, snapshots, chat sessions,
  and state history tables.
- `save_message` writes both a message and a `chat_message` memory row.
- `MemoryGateway` separates turn context, episodic life event, semantic
  preference, procedural rule, evidence record, and canonical LifeModel truth.
- `MemoryLifecycleStore` records accepted, materialized, rolled back, and
  superseded lifecycle states.

File location:

- `openlife-core/src/memory.rs`
- `openlife-core/src/memory_gateway.rs`
- `src-tauri/src/memory_gateway.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`

Confidence: High.

Impact: Memory is not one table and should not be represented as one inbox or
one search box in the next design.

## Evidence and Life Events

Finding: Life events and evidence are implemented as separate auditable assets.

Evidence:

- `LifeEventStore` persists source type, source refs, privacy level, and event
  data.
- `EvidenceStore` exists and is used by LifeModel backend completion paths.

File location:

- `openlife-core/src/agent/lifemodel_backend_completion.rs`
- `openlife-core/src/agent/evidence_store.rs`

Confidence: High.

Impact: The UX should show evidence and provenance as product primitives, not
debug-only details.

## Domain Model Verdict

The domain model is strong enough to preserve. The major weakness is not domain
absence. It is product presentation and authority convergence: users need to
understand which facts are context-only, pending review, accepted memory, and
canonical LifeModel truth.
