# Life Model

## Status

Source-backed description of the current LifeModel implementation and write
governance under ADR 0016. LifeModel is the user-owned long-term model; it is
not Agent Memory, business state, policy, audit, or a general heuristic system.

The repository now contains a validated v2 document schema, append-only SQLite
version owner, and a governed legacy-YAML migration path. Canonical truth
promotion remains proposal-first and gateway-bound. Existing profiles are not
silently migrated: the owner changes only after an exact reviewed proposal,
verified backup, and atomic v2 version plus cutover receipt commit.

## Authority

Authority remains with `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and current
source.

Historical backend completion and maturation plans are background only. They do
not permit ordinary Main Chat to write durable LifeModel truth directly.

## Last verified

2026-08-08 during Phase 5.2E governed migration and owner-cutover implementation.

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

`get_life_model_view_model` treats any validated v2 head, including an
authoritative empty version, as the canonical owner. A fresh profile with no
legacy source is canonical-empty without creating storage as a read side effect.
Only a profile with legacy YAML and no v2 head/cutover remains in compatibility
migration mode. A valid v2 owner suppresses legacy model, migration preview, and
legacy current-view input; a corrupt cutover relation fails closed.

Before cutover, `LegacyLifeModelMigrationPreviewV2` reads the exact YAML bytes
that produced the compatibility model and classifies every non-empty source
leaf. Long-term user fields are review-required candidates; current state,
tasks, Agent Memory, and Agent Runtime fields remain with their own owners;
scores and ambiguous fields are not silently reshaped. Unknown fields,
non-finite numbers, oversized sources, and unsupported YAML constructs make the
preview unavailable without changing data. The product shows this classification
only while no canonical v2 owner exists. The preview itself remains a pure read.
The migration editor requires an explicit include/exclude decision for every
candidate, leaves sensitive candidates unselected, and requires a separate
acknowledgement for fields owned by another domain. Submitting the editor creates
only a pending Review proposal.

For a non-empty validated v2 version, `LifeModelHumanProjectionV2` carries the
deterministic YAML plus its exact model id, model version, item count, document
digest, YAML content digest, and projection digest. The backend regenerates and
validates this binding before granting canonical `LifeModelViewModel` credit;
the frontend only renders the backend-owned projection. A changed YAML body,
version transplant, item-count drift, or document mismatch fails closed rather
than falling back to the legacy YAML. The product view is collapsed and
read-only: 5.2C introduced no YAML editor, import, proposal, or v2 write path.

`LifeModelTypedDiffV2` is the only v2 mutation contract. It allows item-level
add, replace, and remove operations in schema-owned sections; it does not expose
an arbitrary JSON path or whole-document replacement. Every diff binds the
model, base version and document digest, expected result digest, and for replace
or remove the exact stable item id and before-item digest. Review Center renders
the backend-owned operation summary and exact values before approval.

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

Legacy proposal paths continue to materialize the legacy YAML shape. The exact
`$lifemodel_v2` path instead parses a `LifeModelTypedDiffV2`, verifies proposal
`base_hash`, acquires the existing canonical-write admission, checks the current
v2 head, and appends one SQLite version before reporting success. Stale base,
before-item drift, result-digest drift, section mismatch, and materialization-id
reuse fail closed. A database error whose effect cannot be proven remains
unknown and is not automatically retried.

Until legacy owner cutover is complete, a typed remove may not produce an empty
v2 head. After cutover, the persisted receipt authorizes an empty canonical head;
the read model never falls back to old YAML based on item count.

The `$lifemodel_v2_migration` proposal binds the exact legacy source digest,
every candidate decision, the typed result, and its expected document digest.
Acceptance reloads and reclassifies the exact source under the canonical write
coordinator, creates an exact read-only byte backup, rechecks the source digest,
then commits version 1 and an immutable cutover receipt in one SQLite transaction.
The receipt binds model, legacy and backup digests, v2 version/document digest,
proposal id, and cutover time. Source drift, an existing v2 owner, validation
failure, or backup failure is a definite pre-effect failure; ambiguous database
commit failures remain unknown and are not automatically retried. After a v2
owner exists, shipped legacy read and proposal-write paths reject normal product
use. The original YAML and verified backup are evidence only and are not queried
by the normal product ViewModel. Main Chat, scheduled execution, proactive
suggestions, and A2A also stop injecting the legacy model after cutover. They use
no LifeModel enrichment until the v2 runtime-context integration planned for
Phase 5.4; this prevents a hidden stale-YAML fallback without prematurely moving
5.4 into migration work.

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
