# Life Model

## Status

Source-backed description of the current LifeModel implementation and write
governance under ADR 0016. LifeModel is the user-owned long-term model; it is
not Agent Memory, business state, policy, audit, or a general heuristic system.

The current repository still contains a YAML model and a governed
proposal/write-gateway path. Canonical truth promotion remains proposal-first
and gateway-bound.

## Authority

Authority remains with `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and current
source.

Historical backend completion and maturation plans are background only. They do
not permit ordinary Main Chat to write durable LifeModel truth directly.

## Last verified

2026-08-06 during Phase 5 architecture-boundary implementation.

## Source map

- `plans/adr/0016-agent-memory-lifemodel-domain-boundaries.md`
- `openlife-core/src/life_model.rs`
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

`openlife-core/src/life_model.rs` defines the current LifeModel structure:
metadata, identity, goals, capabilities, state, relationships, preferences, and
evolution rules. It also defines a `LifeModelHSCompatibilityView` and provenance
fields that explicitly mark the compatibility view as not accepted source of
truth and not durable truth materialization.

The manager still loads and saves `life_model.yaml`. Structured accepted change
records and gateway checks govern mutations; YAML is the deterministic
human-readable representation and must not become a second independently
writable truth. The complete structured-store/YAML migration remains later
Phase 5 work.

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
