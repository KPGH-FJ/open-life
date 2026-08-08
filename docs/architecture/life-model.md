# Life Model

## Status

Source-backed description of the current LifeModel implementation and write
governance under ADR 0016. LifeModel is the user-owned long-term model; it is
not Agent Memory, business state, policy, audit, or a general heuristic system.

The repository now contains a validated v2 document schema and append-only
SQLite version owner alongside the legacy YAML compatibility owner. Canonical
truth promotion remains proposal-first and gateway-bound; existing YAML data
has not been migrated or switched to the v2 write path.

## Authority

Authority remains with `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and current
source.

Historical backend completion and maturation plans are background only. They do
not permit ordinary Main Chat to write durable LifeModel truth directly.

## Last verified

2026-08-08 during Phase 5.2A structured canonical-owner implementation.

## Source map

- `plans/adr/0016-agent-memory-lifemodel-domain-boundaries.md`
- `openlife-core/src/life_model.rs`
- `openlife-core/src/life_model/v2.rs`
- `openlife-core/src/life_model/patch.rs`
- `openlife-core/src/life_model/patch_store.rs`
- `openlife-core/src/life_model_write_gateway.rs`
- `openlife-core/src/agent/proposal_store.rs`
- `openlife-core/src/agent/proposal_outcome.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `src-tauri/src/life_model_write_gateway.rs`
- `src-tauri/src/life_model_materializer_guard.rs`
- `src-tauri/src/commands/life_model.rs`
- `src-tauri/src/commands/proposal.rs`

## Current boundary

ADR 0013's broad LifeModel-HS target is superseded. EvidenceStore,
HeuristicStore, StateStore, PolicyStore, regression, and audit code may still
exist, but they are not jointly the canonical LifeModel. Existing code is
reviewed by its real owner and may be narrowed or removed in later slices.

## Current Model Shape

`openlife-core/src/life_model.rs` defines the legacy LifeModel structure:
metadata, identity, goals, capabilities, state, relationships, preferences, and
evolution rules. It also defines a `LifeModelHSCompatibilityView` and provenance
fields that explicitly mark the compatibility view as not accepted source of
truth and not durable truth materialization.

New empty legacy skeletons no longer invent a focus, health state, mood, stress,
fulfilment, or energy value. Existing YAML values continue to load exactly as
stored and are not silently rewritten.

`openlife-core/src/life_model/v2.rs` defines the narrower long-term user model:
identity and self-definition, values, long-term goal direction and meaning,
stable preferences, personal boundaries, important relationships, user
capabilities and stable resources, decision principles, and collaboration
preferences. Collection items have stable IDs, confirmation timestamps, and
minimal source refs. Operational goal progress/deadlines and unknown fields are
rejected by schema deserialization or validation.

The v2 SQLite owner stores immutable JSON documents with schema version, model
version, parent version/digest, document digest, materialization identity,
source refs, and creation time. Commits require the exact current parent and are
idempotent only for identical content. Reads revalidate schema and digest.
Merely opening `/life-model` does not create the database or an empty model.

`get_life_model_view_model` consumes an existing v2 head and gives canonical
credit only to a non-empty validated version. Without one, existing YAML remains
the compatibility owner. The v2 store does not yet accept product writes, YAML
has not been migrated, and the current proposal materializer still targets the
legacy governed gateway; those are later 5.2 slices.

## Patch And Proposal Path

`openlife-core/src/life_model/patch.rs` defines patch objects, patch status,
patch source, conflict handling, and conversion from proposals to patch inputs.
`openlife-core/src/life_model/patch_store.rs` persists patches and conflicts in
SQLite.

The former `openlife-core/src/agent/proposal_engine.rs` module is deleted. It
was a shipped second proposal authority: `AppState` and bootstrap owned it,
ordinary Main Chat finalization and AgentRun replay invoked it, and it could
construct proposals from raw run output without PolicyRouter authorization.
Those caller-shaped proposals were subsequently submitted to ReviewWorkflow,
but ReviewWorkflow had no observation-bound policy proof to validate. The
engine, its product consumers, and its public exports were therefore deleted
together. Main Chat proposals must now carry current PolicyRouter admission
into ReviewWorkflow. Builder, Calibration, ToolPermission, PlanExecute,
Maturation, and other remaining proposal writers are still tracked as separate
convergence work; their existence is not evidence that the single proposal
authority is complete.

`openlife-core/src/agent/proposal_store.rs` persists proposal lifecycle state,
including pending, accepted, rejected, edited, and postponed records.

`openlife-core/src/agent/proposal_outcome.rs` records maturation evidence only
for low-risk supported proposal domains. High-risk or unsupported domains do not
become maturation evidence through that helper.

## Write Gateway

`openlife-core/src/life_model_write_gateway.rs` classifies LifeModel write
intents. Accepted proposal materialization requires a proposal id and matching
base/current hashes. Manual and restore/import overrides require explicit
override evidence. Source-data compatibility writes are allowed only as
non-truth compatibility. Automatic learning is blocked.

`src-tauri/src/life_model_write_gateway.rs` is the Tauri-side enforcement path.
It materializes accepted LifeModel proposals by checking base hash, applying the
patch, saving the model through the gateway, recording patch state, and writing
metadata-safe audit details.

This gateway currently materializes the legacy YAML shape. It does not receive
v2 canonical-store credit until a later slice replaces its patch allowlist and
commit path with exact v2 document versions.

`src-tauri/src/life_model_materializer_guard.rs` limits allowed caller
contexts. Governed manual override, restore/import, source-data compatibility,
and accepted-proposal apply have explicit lanes. Unclassified caller contexts
are blocked.

## Review Center Application

`src-tauri/src/commands/proposal.rs` accepts, rejects, edits, and postpones
proposals. Accepting a LifeModel-affecting proposal calls the Tauri write
gateway materialization path. Editing a proposal keeps it pending and reports
that no durable write was executed.

ToolPermission and Memory proposal acceptance have their own application paths.
They do not imply that LifeModel truth was updated unless the proposal type and
gateway decision route through the LifeModel write gateway.

## Practical Rule For New Docs

Use "LifeModel" for the user's confirmed long-term model. Use "Agent Memory"
for working, project, episodic, semantic, procedural, Reflection, and Markdown
context. `LifeModel-HS` is a superseded historical term. Current durable
LifeModel writes remain proposal-first and gateway-enforced.
