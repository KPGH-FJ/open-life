# Life Model

## Status

Source-backed description of the current LifeModel implementation and write
governance under ADR 0016. LifeModel is the user-owned long-term model; it is
not Agent Memory, business state, policy, audit, or a general heuristic system.

The repository now contains a validated v2 document schema, append-only SQLite
version owner, a governed legacy-YAML migration path, and a bounded Main Chat
runtime projection. Canonical truth promotion remains proposal-first and
gateway-bound. Existing profiles are not silently migrated: the owner changes
only after an exact reviewed proposal, verified backup, and atomic v2 version
plus cutover receipt commit.

## Authority

Authority remains with `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and current
source.

Historical backend completion and maturation plans are background only. They do
not permit ordinary Main Chat to write durable LifeModel truth directly.

## Last verified

2026-08-10 during Phase 5.5F authority-convergence closeout.

## Source map

- `plans/adr/0016-agent-memory-lifemodel-domain-boundaries.md`
- `openlife-core/src/life_model.rs`
- `openlife-core/src/life_model/v2.rs`
- `openlife-core/src/agent/life_model_runtime_context.rs`
- `openlife-core/src/life_model/patch.rs`
- `openlife-core/src/life_model/patch_store.rs`
- `openlife-core/src/life_model_write_gateway.rs`
- `openlife-core/src/agent/proposal_store.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `src-tauri/src/life_model_write_gateway.rs`
- `src-tauri/src/life_model_materializer_guard.rs`
- `src-tauri/src/commands/life_model.rs`
- `src-tauri/src/commands/proposal.rs`
- `src-tauri/src/main_chat_kernel.rs`

## Current boundary

ADR 0013's broad LifeModel-HS target is superseded. HeuristicStore and the HS
asset-authority registry are no longer initialized, reconciled, or attached to
product state; existing database files remain inert historical data and are not
deleted during startup. Their selector, lifecycle, materializer and authority
registry source modules have also been removed. EvidenceStore, StateStore,
PolicyStore, and audit-event storage remain only under their current narrow
product owners. None of them is jointly the canonical LifeModel.

## Current Model Shape

`openlife-core/src/life_model.rs` defines the legacy LifeModel structure:
metadata, identity, goals, capabilities, state, relationships, preferences, and
evolution rules. The retired HS compatibility projection builder and decoder are
no longer part of the source tree. A small `legacy_hs_audit` DTO remains solely
to decode and minimize historical AgentRun selection-audit and behavior-check
metadata; it has no selector, provider capability, write path, or runtime
authority. New AgentRun constructors leave both historical fields empty. Its
exit condition is the removal or explicit migration of AgentRun rows that still
contain `hs_selection_audit_json` or `behavior_checks_json`.

New empty legacy skeletons no longer invent a focus, health state, mood, stress,
fulfilment, or energy value. Existing YAML values continue to load exactly as
stored and are not silently rewritten.

Startup and read paths do not create a missing legacy YAML file. A legacy YAML
that was previously manufactured from the fully expanded default model carries
no proven user content: migration preview retains only its provenance metadata
and does not turn default enum values or zero-valued state into user facts.

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
Only a profile with legacy YAML and no v2 head/cutover remains in bounded
migration mode. That profile exposes the migration preview, not the old 4D
current view, completion score, dimension cards, or MCP capability-gap
recommendation. A valid v2 owner suppresses the migration preview entirely; a
corrupt cutover relation fails closed.

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
than falling back to the legacy YAML. User add, replace, remove, rollback,
clear, and export actions create reviewed v2 proposals; the YAML body is never
saved as a second owner.

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
into ReviewWorkflow. ToolPermission, canonical Plan Items, learning candidates, and
other proposal producers remain separate domains and must carry their own
policy and source evidence.

The shipped legacy Builder and Calibration command modules are retired. A fresh
profile now uses the `/life-model` v2 schema-aware establishment panel. It asks
for one explicitly selected long-term user fact, defaults that fact out of the
review set, and creates only a typed-diff proposal. It does not collect current
state, task progress, Agent tool capability, or procedural work experience.

`openlife-core/src/agent/proposal_store.rs` persists proposal lifecycle state,
including pending, accepted, rejected, edited, and postponed records.

## Write Gateway

`openlife-core/src/life_model_write_gateway.rs` classifies LifeModel write
intents. Accepted proposal materialization requires a proposal id and matching
base/current hashes. Manual and restore/import overrides require explicit
override evidence. Source-data compatibility writes are allowed only as
non-truth compatibility. Automatic learning is blocked.

`src-tauri/src/life_model_write_gateway.rs` is the Tauri-side enforcement path.
The exact `$lifemodel_v2` path parses a `LifeModelTypedDiffV2`, verifies proposal
`base_hash`, acquires the existing canonical-write admission, checks the current
v2 head, and appends one SQLite version before reporting success. Stale base,
before-item drift, result-digest drift, section mismatch, and materialization-id
reuse fail closed. A database error whose effect cannot be proven remains
unknown and is not automatically retried.

Legacy 4D proposal and patch-batch materializers are no longer shipped. A
persisted old proposal remains visible so the user can reject it, but approval
and generic editing are disabled or fail before effect with an explicit retired
path result. The old YAML manager is read only for a not-yet-migrated profile's
bounded migration preview and for governed recovery; it is not a normal product
write owner.

The former patch/snapshot-backed current-view DTO and its frontend 4D dimension
contract are deleted. Materialization credit now comes from the current Review
item and canonical v2 version evidence. The unshipped feedback-evolution and
calibration implementation, its release commands, and its frontend contracts
were removed in Phase 5.5D. `FeedbackStore` is retained as a narrow audit-event
store for current LifeModel gateway and proposal receipts; it has no authority
to learn or mutate LifeModel. Fresh profiles create only its current `analytics`
table. Existing legacy feedback and conversation-inference tables remain inert
and are not modified during startup.

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
by the normal product ViewModel. Main Chat, scheduled execution, generic
AgentRuntime, and the development A2A reasoning bridge no longer read legacy
YAML for personalization. The uncalled release Proactive command is retired.
None of these paths receives Main Chat v2 capability credit merely because the
v2 owner exists.

## Main Chat Runtime Influence

`LifeModelRuntimeContextV2` validates the current canonical version and selects
at most four task-relevant confirmed facts. The packet binds model and version
digests, stable item IDs, source references, confirmation times, selection
reasons, and an exact content digest. It contains no raw model and grants no
permission.

Canonical Chat and Work load this packet through `LifeModelContextPort` in
`src-tauri/src/personal_intelligence_ports.rs`. The packet is a distinct
ContextCompiler source; it is not an HS summary or Agent Memory. Confirmed goals and boundaries can add bounded
planning hints, eligible Memory results can receive a capped rerank bonus, and
confirmed collaboration preferences can affect communication style or order
already-legal equivalent tool candidates. Scope, lifecycle, privacy, Policy,
ToolGateway eligibility, risk and permission decisions remain owned by their
existing domains.

The turn result includes a product-facing influence receipt with model version,
selected item IDs, confirmation times, reasons and affected surfaces. It exposes
no hidden reasoning. An explicit current instruction can disable LifeModel use;
irrelevant, unavailable, invalid or tampered models contribute no facts, and
ordinary Agent work continues without personalization.

An explicit stable preference in canonical Work goes through
`PersonalIntelligenceSuggestionPort`. That boundary can capture a typed
LifeModel learning candidate, but it does not create a Proposal or mutate the
canonical version. Existing candidate maturation, typed-diff Review, and
`LifeModelWriteGateway` materialization remain the only durable path. The
successful capture appears as a canonical Observation Item without giving
LifeModel any Task, permission, Artifact, or terminal-state authority.

`src-tauri/src/life_model_materializer_guard.rs` limits allowed caller
contexts. Governed manual override, restore/import, source-data compatibility,
and accepted-proposal apply have explicit lanes. Unclassified caller contexts
are blocked.

## Review Center Application

`src-tauri/src/commands/proposal.rs` accepts, rejects, edits, and postpones
proposals. Accepted v2 typed diffs and governed migration proposals call their
exact materializer. Generic JSON editing is blocked for these schema-bound
proposals. Persisted old 4D and Builder proposals also cannot be edited or
approved; they may only be rejected. Editing another currently supported
proposal keeps it pending and reports that no durable write was executed.

ToolPermission and Memory proposal acceptance have their own application paths.
They do not imply that LifeModel truth was updated unless the proposal type and
gateway decision route through the LifeModel write gateway.

## Practical Rule For New Docs

Use "LifeModel" for the user's confirmed long-term model. Use "Agent Memory"
for working, project, episodic, semantic, procedural, Reflection, and Markdown
context. `LifeModel-HS` is a superseded historical term. Current durable
LifeModel writes remain proposal-first and gateway-enforced.
