# ADR 0013: LifeModel-HS Source Of Truth And Governance

Date: 2026-05-28
Status: accepted

## Context

OpenLife's current LifeModel is implemented primarily as a Rust `LifeModel`
struct serialized into a YAML current snapshot, surrounded by Proposal, Patch,
Snapshot, Memory, AgentRunEvent, PromptStack, ToolRuntime, and privacy
governance primitives.

This was sufficient for the Beta architecture, but it leaves LifeModel closer
to a maintainable profile file than a dynamic model:

- LifeModel is mostly consumed through broad PromptStack injection.
- Runtime context selection is category-level rather than field-level or
  task-level.
- Memory is useful for retrieval but is not yet a complete evidence layer.
- Signal extraction can produce false positives, false negatives, and memory
  corruption if treated as fact.
- Current YAML is not a good long-term source of truth for evidence, heuristics,
  policies, regression, compression, forgetting, or conflict resolution.
- Legacy evolution paths still coexist with proposal-first governance.

The current LifeModel direction should turn LifeModel into a user-governed Personal
Heuristic System (LifeModel-HS): a softwareized model system maintained through
Heuristic Learning over user-owned local data. The current product boundary is
documented in `PRODUCT.md`; the source-backed subsystem map is in
`docs/architecture/life-model.md`.

This ADR establishes the source-of-truth, governance, automatic update,
retention, deletion, policy/heuristic boundary, materialized-view, maintenance,
and MVP ownership defaults that must guide implementation.

## Decision

OpenLife will treat next-generation LifeModel as a Personal Heuristic System
made of canonical accepted HS assets, not as a single YAML profile.

Canonical HS assets include:

- evidence,
- state assets,
- heuristics,
- policies,
- regression scenarios,
- materialized view metadata,
- audit and maintenance records.

The current YAML LifeModel remains as a compatibility materialized view during
migration. It is not the long-term canonical source of truth for LifeModel-HS.

All risky HS mutation remains proposal-first and user-governed. Heuristic
Learning may generate events, signals, evidence, candidate heuristics, candidate
state patches, and candidate policy patches, but it must not directly mutate
canonical HS assets except for explicitly allowed low-risk transient metadata
and maintenance effects defined below.

## Source Of Truth Policy

### Target source of truth

The target canonical source of truth is the accepted HS asset layer:

```text
EvidenceStore
StateStore
HeuristicStore
PolicyStore
RegressionSuite
HSAuditLog / maintenance records
```

Materialized outputs are derived:

```text
lifemodel_yaml@compat
prompt_block.hs_runtime
ui.hs_overview
model_route_policy
RuntimeHSPacket
```

Materialized views must record source asset ids and content digests so they can
be rebuilt and audited.

### Migration source of truth

Migration must be phased:

1. Initially, existing YAML and `LifeModelManager` remain the runtime source for
   current code paths while EvidenceStore / HeuristicStore are additive.
2. Selector MVP may read accepted HS assets for a small set of runtime decisions
   while YAML remains compatibility state.
3. After materialization is deterministic and audited, canonical stores may feed
   the YAML compatibility view.
4. Product paths must eventually stop directly mutating YAML outside accepted
   Proposal / Governor / Materializer paths.

Implementation must not switch source of truth in one step.

## Automatic Update Policy

### Low-risk state auto-accept

Only transient `StateAsset` updates may be auto-accepted by default.

Eligible examples:

- temporary energy level,
- temporary mood or stress level,
- current focus for the current day or short window,
- short-lived task execution status,
- recent session-local state.

Required constraints:

- must have a TTL, defaulting to a short window such as 24 hours to 7 days,
- must record source, confidence, and privacy level,
- must not update identity, values, long-term goals, stable preferences, or
  policy,
- must not promote to durable state or preference without Evidence aggregation
  and Proposal review,
- user settings must be able to disable auto transient state updates.

### Medium-risk and high-risk updates

Medium-risk updates remain proposal-first by default. Related low/medium risk
updates may be batched to reduce review fatigue, but they must remain
inspectable.

High-risk updates always require explicit user confirmation. High-risk examples
include identity, values, mission, long-term goals, sensitive relationship
definitions, and privacy boundaries.

## Retention Policy

Default retention policy:

| Asset | Default |
| --- | --- |
| Raw Life Data | Local long-term retention, user configurable by source. |
| Sensitive Raw Life Data | Shorter configurable retention, suggested default 30-180 days depending on source. |
| Transient StateAsset | 24 hours to 7 days by default. |
| Evidence | Long-term local retention with confidence decay, weakening, archival, and user deletion controls. |
| Active Heuristic | Retained until edited, weakened, deprecated, archived, rejected, or deleted by the user. |
| Archived Heuristic | Retain summary, lineage, and rollback metadata locally unless user deletes. |
| Regression Scenario | Retain long-term unless user deletes. |

This ADR does not mandate exact day counts for every source. Implementation
must expose source-specific retention policies and make sensitive-source
retention shorter by default than general local data.

## Raw Data Deletion And Forgetting

OpenLife will support three deletion/forgetting semantics:

### Delete raw only

Delete the original raw payload while preserving derived accepted assets when
appropriate. Linked evidence must mark the source as unavailable and reduce
support count.

### Forget this

Delete or archive raw data and weaken or archive linked evidence. If an evidence
record loses all support sources, it should become archived or contradicted
rather than remain active.

### Forget and prevent relearning

Create a tombstone that prevents the same deleted fact from being regenerated
from old imports, stale caches, or similar source material unless the user
explicitly allows relearning.

Deletion effects:

- linked evidence is weakened, archived, or tombstoned according to user intent,
- linked heuristics are re-evaluated,
- impactful heuristic archive/weaken actions become Proposals unless the effect
  is strictly low-risk maintenance,
- audit records should preserve that a deletion decision happened without
  retaining raw sensitive text.

## Policy vs Heuristic Boundary

OpenLife will separate hard policies from soft heuristics.

Policy is a hard runtime boundary. Heuristic is a learned collaboration
guidance unit.

Privacy belongs to Policy, not merely to Heuristic.

Examples:

```text
Policy:
Sensitive health, relationship, identity, finance, or private-file topics
default to LocalOnly unless the user explicitly overrides.

Heuristic:
When discussing health, the user prefers a short summary before action advice.
```

Rules:

- Heuristics cannot relax or override Policy.
- Heuristics may trigger stricter Policy.
- Privacy Policy changes require explicit user confirmation.
- Privacy Policy must be consumed by ModelRouter, PromptStack, ToolRuntime,
  selectors, and audit paths as a hard constraint.
- Privacy Policy must not be reduced to a natural-language prompt instruction
  as its only enforcement point.

## User-Facing Language

The internal term `Heuristic` may be used in code and architecture.

User-facing product language should avoid "heuristic" by default. Preferred
terms:

- "collaboration rule",
- "personal collaboration experience",
- "AI collaboration style",
- "the way your AI works with you".

Suggested UI labels:

- Review Center: "new collaboration rule suggestion"
- Heuristic Browser: "My AI collaboration style"
- Evidence Drawer: "Why OpenLife thinks this"
- Runtime Audit: "Collaboration rules used this run"
- Maintenance Inbox: "Collaboration experiences to organize"

Regression should also use user-facing language:

- `RegressionScenario` -> "behavior check" or "preference guard"
- `RegressionSuite` -> "behavior guard set"
- replay regression -> "replay check"

## Regression Visibility

Regression results should be visible to users in layers.

Default view:

```text
Passed 3 behavior checks.
1 preference guard needs attention.
```

Expanded user view:

- scenario title,
- purpose,
- pass/fail result,
- impacted collaboration rule,
- concise reason.

Advanced/developer view:

- scenario id,
- expected `must` / `must_not`,
- selected heuristics,
- selector audit,
- PromptStack metadata,
- run id.

MVP regression must start with deterministic checks and selector/prompt
assertions. LLM-judged behavior simulation is a later enhancement because it is
flaky and can create false confidence.

## Active Heuristic Limits

To avoid expert-system-style rule bloat, each domain should default to:

```text
active heuristics per domain: 5
active + trial heuristics per domain: 8
```

Suggested initial domains:

- conversation,
- planning,
- privacy,
- tool_use,
- proactive,
- memory,
- goals,
- state.

When a domain exceeds the cap, MaintenanceEngine should prefer:

1. updating an existing heuristic,
2. merging duplicates,
3. weakening low-value heuristics,
4. archiving stale heuristics,
5. proposing a new heuristic only when it is clearly non-duplicate.

User-pinned heuristics may be exempt from normal caps, but excessive pinned
heuristics should trigger a clarity warning or organization suggestion.

## YAML Materialized View Policy

The current LifeModel YAML remains a compatibility view. It should not become a
dump of all HS details.

Allowed in YAML compatibility view:

- current state summary,
- existing identity/goals/capabilities/state/preferences fields,
- concise preference or collaboration summaries where compatibility requires,
- source asset ids and digest metadata,
- compact references to accepted HS assets.

Not allowed in YAML compatibility view:

- full heuristic list,
- raw evidence,
- opposing evidence,
- raw source text,
- regression internals,
- privacy-sensitive reasoning,
- full audit history.

Recommended compatibility sections:

```text
runtime_context_summary
collaboration_style_summary
hs_asset_refs
```

The full HS must remain queryable through canonical stores and purpose-built UI,
not through a bloated YAML profile.

## Ownership And Multi-User Policy

MVP supports a single local user LifeModel-HS.

MVP must not:

- merge multiple users,
- infer shared identity,
- auto-promote shared-device data into personal evidence,
- create relationship-level shared memory.

Implementation should still reserve ownership fields:

- `owner_id`,
- `profile_id`,
- `device_scope`,
- `data_source_user`,
- `asset_owner`.

If current user identity cannot be confirmed on a shared device, OpenLife should
default to a temporary or anonymous session and avoid writing personal HS
assets.

Multi-user, family, delegated assistant, and relationship-level shared memory
require future ADRs.

## Maintenance Automation Policy

Maintenance is part of correctness, but automatic maintenance must remain
conservative.

### May run automatically

- low-risk confidence decay,
- access count updates,
- `last_used_at` updates,
- transient state expiration,
- selector cache rebuild,
- materialized view rebuild,
- metadata-only diagnostics,
- low-risk audit consistency checks.

### May generate recommendations or Proposals

- merging heuristics,
- archiving active heuristics,
- weakening medium/high confidence heuristics,
- deleting evidence,
- changing retention policy,
- changing privacy or model route policy,
- promoting transient state to stable preference,
- compression that changes user-visible behavior.

### Must require explicit confirmation

- deleting high-risk evidence,
- modifying identity, values, mission, long-term goals, or sensitive
  relationships,
- relaxing privacy policy,
- disabling LocalOnly on sensitive domains,
- auto-accepting high-risk heuristics,
- permanently forgetting broad memory categories.

## Options Considered

### Option A: Keep YAML LifeModel as the canonical model

Pros:

- Minimal implementation disruption.
- Easy to inspect manually.
- Fits current code paths.

Cons:

- Keeps LifeModel static and profile-like.
- Does not support evidence, heuristic lifecycle, regression, compression, or
  deletion lineage well.
- Encourages broad prompt injection and prompt pollution.

Rejected as long-term architecture.

### Option B: Replace current LifeModel immediately with HS canonical stores

Pros:

- Clean architecture.
- Avoids dual-source complexity.

Cons:

- Too risky for current codebase.
- Existing Chat, Builder, Calibration, PromptStack, Proposal, Patch, and UI
  flows depend on current LifeModel shape.
- High migration blast radius.

Rejected for MVP.

### Option C: Additive HS layer with YAML compatibility view

Pros:

- Preserves current runtime while adding EvidenceStore, HeuristicStore,
  Selector, Regression, and Materializer incrementally.
- Lets MVP prove runtime usefulness with a few heuristics.
- Supports source-of-truth transition over phases.

Cons:

- Requires careful materialized view consistency.
- Temporarily has dual-read / compatibility complexity.

Accepted.

## Consequences

Positive:

- LifeModel can become a softwareized, learnable, user-governed model system.
- Memory can evolve beyond retrieval into evidence.
- Runtime behavior can improve through selected heuristics and policies rather
  than broad profile injection.
- Rejected proposals and maintenance actions become learning signals.
- Privacy boundaries become hard policy rather than prompt-only advice.

Tradeoffs:

- More stores, schemas, and migrations.
- Selectors become product-critical.
- Maintenance and compression become correctness responsibilities.
- UI must explain evidence, rules, and regression without overwhelming users.
- Source-of-truth migration must be phased carefully.

## Implementation Guardrails

- Raw Life Data must not directly mutate canonical HS assets.
- Signal extraction must produce weak Signals, not accepted facts.
- Evidence must include source refs and confidence metadata.
- Candidate heuristics and state/policy patches must go through Governor.
- High-risk identity, values, mission, long-term goals, sensitive relationships,
  and privacy boundaries require explicit user confirmation.
- Privacy is Policy. Heuristics cannot relax Policy.
- Low-risk auto-accept is limited to transient StateAssets with TTL.
- Rejected proposals must become negative evidence.
- Materialized YAML is compatibility output, not a full HS dump.
- Active heuristic caps must be enforced or monitored by maintenance.
- Deletion and forgetting must weaken, archive, or tombstone derived evidence
  according to user intent.
- Maintenance auto-actions are limited to low-risk metadata, expiration, cache,
  diagnostic, and materialized-view rebuild work.
- LLM-judged regression must not be required for MVP acceptance.

## MVP Boundaries

First implementation should prove runtime usefulness without broad rewrites.

Recommended MVP:

- EvidenceStore MVP.
- HeuristicStore MVP.
- Policy/Heuristic boundary enforced for privacy.
- ContextSelector / HeuristicSelector hard-filter MVP.
- Three active heuristics/policies:
  - sensitive topics prefer LocalOnly,
  - external writes require draft/proposal-first,
  - low-energy planning reduces intensity.
- One negative-evidence loop:
  - rejected reminders reduce proactive reminder frequency.
- Deterministic RegressionSuite checks for the three active decisions.
- Selection audit in run detail or trace metadata.
- Current YAML remains compatibility view.

MVP should not include:

- autonomous identity or value rewrite,
- multi-user support,
- complex heuristic editor,
- broad automatic compression,
- LLM-judge regression as a required gate,
- full source-of-truth migration in one step,
- cloud raw LifeModel extraction.

## Verification

Implementation should prove:

- A privacy-sensitive conversation selects LocalOnly due to Policy.
- A heuristic cannot relax a LocalOnly policy.
- An external write action becomes draft/proposal-first.
- A low-energy transient state causes smaller planning suggestions.
- A rejected reminder proposal creates negative evidence or suppression signal.
- Context/Heuristic selection audit lists included and excluded assets without
  raw sensitive payloads.
- YAML materialized view contains only allowed summaries and refs, not full
  heuristic/evidence internals.
- Deleting raw data weakens or archives linked evidence according to deletion
  mode.
- Active heuristic cap or warning works per domain.
- Deterministic regression catches a candidate that would violate LocalOnly
  privacy.

## Open Questions

1. Exact retention defaults per raw source still require product review.
2. Exact user-facing Chinese terms for "collaboration rule" and "behavior
   check" should be refined in UI copy review.
3. The first set of deterministic regression scenarios should be specified in a
   follow-up MVP task spec.
4. The storage location of EvidenceStore / HeuristicStore should be decided in
   implementation planning.
5. Multi-user, shared-device, family, delegated assistant, and shared memory
   scenarios require future ADRs.
6. Compression UX and maintenance inbox design require product interaction
   review before implementation beyond diagnostics.
