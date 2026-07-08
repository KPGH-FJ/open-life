# Life Model

## Status

Stage3-A source-backed explainer. This document describes the current
LifeModel-HS implementation and write governance. It is not a declaration that
the canonical LifeModel-HS migration is finished.

The current repository still contains a YAML compatibility model and a governed
proposal/write-gateway path. Canonical truth promotion remains proposal-first and
gateway-bound.

## Authority

Authority remains with `AGENTS.md`, `plans/README.md`,
`plans/openlife_single_system_deletion_manifest.md`, and
`plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`. This page is a
readable map of source-backed behavior beneath those documents.

Historical backend completion and maturation plans are background only. They do
not permit ordinary Main Chat to write durable LifeModel truth directly.

## Last verified

2026-07-07 during Stage3-A source-map reading.

## Source map

- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `openlife-core/src/life_model.rs`
- `openlife-core/src/life_model/patch.rs`
- `openlife-core/src/life_model/patch_store.rs`
- `openlife-core/src/life_model_write_gateway.rs`
- `openlife-core/src/agent/proposal_engine.rs`
- `openlife-core/src/agent/proposal_store.rs`
- `openlife-core/src/agent/proposal_outcome.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `src-tauri/src/life_model_write_gateway.rs`
- `src-tauri/src/life_model_materializer_guard.rs`
- `src-tauri/src/commands/life_model.rs`
- `src-tauri/src/commands/proposal.rs`

## Inherited blocker

The source-of-truth ADR defines a target HS architecture, but current code still
uses a compatibility materialized view and proposal materialization gateways.
This page must not be read as proof that all HS assets are migrated or that
ordinary chat may apply LifeModel truth directly.

## Current Model Shape

`openlife-core/src/life_model.rs` defines the current LifeModel structure:
metadata, identity, goals, capabilities, state, relationships, preferences, and
evolution rules. It also defines a `LifeModelHSCompatibilityView` and provenance
fields that explicitly mark the compatibility view as not accepted source of
truth and not durable truth materialization.

The manager still loads and saves `life_model.yaml`. The compatibility view adds
source digests and asset references so runtime code can reason about provenance
without treating the YAML view as canonical HS truth.

The ADR in `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` states
the direction: evidence, state, heuristics, policies, regression state, and
audit should become governed HS assets. During migration, YAML remains a
compatibility/materialized surface.

## Patch And Proposal Path

`openlife-core/src/life_model/patch.rs` defines patch objects, patch status,
patch source, conflict handling, and conversion from proposals to patch inputs.
`openlife-core/src/life_model/patch_store.rs` persists patches and conflicts in
SQLite.

`openlife-core/src/agent/proposal_engine.rs` generates proposal objects for
memory writes, memory archives, tool permissions, and chat-derived LifeModel
updates. It creates proposal records, not direct durable writes.

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

Use "LifeModel" to describe the current model and compatibility surface. Use
"LifeModel-HS" only with the governance caveat that the ADR is the target
architecture and that current durable truth writes are proposal-first and
gateway-enforced.
