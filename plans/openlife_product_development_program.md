# OpenLife Reconstruction Plan

Status: active

## Objective

Reconstruct OpenLife as a capable local-first personal Agent for general
knowledge work. Preserve proven execution and safety assets, replace incorrect
or duplicated lifecycle ownership, and delete each retired production path as
its complete replacement lands.

This plan supersedes the S0-S7 program. Those stages remain historical Git
evidence; they are not current product-completion credit.

## Product contract

- The Workbench contains Projects and Conversations. A Conversation can use
  Chat for a direct response or Work for a durable outcome.
- Chat and Work share one canonical `Conversation -> Turn -> Item` spine.
- Work adds `Task -> Run -> Item -> ItemAttempt`, a canonical `FinalResult`,
  and optional independent `ArtifactVersion` objects.
- Planning, adaptive tool use, approval, recovery, and future subagents are
  phases or capabilities inside that spine, never independent product owners.
- The user-selected provider and model are bound to a Turn or Run and are not
  silently substituted.
- Memory and LifeModel participate only through bounded typed ports. They do
  not own execution, permission, or completion.

## Non-negotiable migration rules

1. A production concern has one canonical writer and recovery owner.
2. Temporary adapters may exist only while the production write owner remains
   unambiguous. There is no legacy runtime fallback.
3. Every migrated capability includes its backend owner, control and recovery
   path, ViewModel, usable frontend, behavior tests, and old-path deletion.
4. A schema, plan, proposal, streaming response, green unit test, or stable
   process launch is not product completion.
5. SQLite owns lifecycle and recovery metadata. Artifact files own their
   content. JSONL is diagnostic or export material only.
6. Missing, stale, failed, or uncertain effects remain unknown or blocked.

## Current stage: R0 - reconstruction baseline

### Outcome

Create a truthful, reproducible baseline from which the product can be rebuilt
without carrying S7 completion claims or unstable native identity forward.

### In scope

1. Replace S0-S7 as active authority with this plan and the accepted
   reconstruction ADR.
2. Align `PRODUCT.md`, architecture, and testing documentation with the
   accepted Conversation/Task model and reduced product surfaces.
3. Establish the stable `ai.openlife.desktop` macOS bundle identifier and explicit signing identity
   contract for exact-native QA; mechanically verify the signed bundle and
   Keychain round trip.
4. Make a fresh profile initialize required internal integrity credentials
   without an approval ritual. Existing unreadable or missing credentials stay
   typed recovery states and never rotate over protected data.
5. Back up the bounded user-owned configuration and personal-intelligence
   subset, then remove authorized legacy execution/test data without reading or
   exporting secret values.
6. Inventory reusable assets and the production consumers that must be
   migrated or deleted in R1-R7.
7. Establish the reconstruction behavior matrix and cost/performance baseline.

### Out of scope

- R1 Conversation schema or the new general Work runtime;
- new tools, connectors, Computer Use, arbitrary shell, scheduling, cloud
  execution, account sync, or broader LifeModel learning;
- Developer ID distribution, notarization, or public release;
- migrating historical TaskSession, AgentRun, ActionQueue, EventStream,
  PlanExecute, report Task, or test Proposal records.

### Acceptance matrix

| Scenario | Required result | Evidence |
| --- | --- | --- |
| Fresh signed profile | Internal keys initialize, stores open, Workbench reaches a usable empty state | exact signed bundle, fresh reconstruction profile, bounded reset of product-owned internal keys, restart |
| Existing accessible profile | Exact signing identity can read its previously created internal keys after restart | exact-native round trip |
| Existing key unavailable | Only affected capabilities are blocked and recovery is explicit; no key rotation or false provider warning | integration and native recovery test |
| Clean break | Legacy execution/test data is absent from the active profile; retained settings and personal intelligence are enumerated | dry-run manifest, backup, post-clean inspection |
| Authority | Product, ADR, architecture, tests, and this plan agree that reconstruction is active | documentation review and source guards |
| Repository | No secrets, generated bundles, or unrelated changes enter the commit | diff review and common checks |

### Checks

```sh
git diff --check
cargo fmt --check
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
OPENLIFE_CODESIGN_IDENTITY="<local signing identity>" \
  scripts/macos-exact-native.zsh
```

### Stop condition

R0 is complete only when the exact signed application passes bundle identity,
resource seal, fresh-profile credential initialization, restart access,
and usable-empty-state checks; the authorized clean break is complete; the
repository is clean; and the current plan points to R1.

### R0 result

Completed on 2026-08-13:

- ADR 0018 and this plan replaced S0-S7 as current product authority;
- macOS bundle/runtime/data identity converged on `ai.openlife.desktop` and the
  exact local-signed bundle passed strict signature and resource-seal checks;
- a provably empty profile now initializes OpenLife-owned internal credentials
  automatically, while existing, invalid, or unavailable authority remains
  fail-closed;
- authorized legacy execution and QA data were removed, while sanitized
  configuration, Memory, and LifeModel were retained through a private backup;
- exact-binary first start and restart opened every protected execution owner
  with no observed network socket; and
- full Rust, frontend, production-build, and browser-shell gates passed.

R0 does not claim Developer ID/notarization or live-provider evidence. Local
self-signed Keychain ACL continuity across a newly rebuilt binary is not stable
product proof and remains part of R8 release identity work.

## Reconstruction sequence

| Stage | Complete vertical outcome |
| --- | --- |
| R0 | Stable native identity, Keychain and clean reconstruction baseline |
| R1 | Canonical Conversation/Turn/Item, Provider Registry, and reliable Chat |
| R2 | General Task/Run/ItemAttempt, Goal, control, recovery, and FinalResult |
| R3 | Production document, Web, citation, Skill, and MCP capability loop |
| R4 | Artifact versions, Changes, Preview, Verification, approval, receipts, Undo, and effect reconciliation |
| R5 | Project scope, restart freshness, controlled concurrency, background work, and notifications |
| R6 | Bounded Memory and LifeModel ports with independent evolution proof |
| R7 | Final Workbench, conversation organization, results, onboarding, diagnostics, i18n, accessibility, and old frontend deletion |
| R8 | Golden behavior, performance/cost, profile migration, exact-native/live evidence, absence guards, and clean release baseline |

## Current stage: R1 - canonical Conversation and reliable Chat

R1's first vertical outcome is one canonical Conversation/Turn/Item lifecycle
for ordinary Chat, together with a Provider Registry and user-visible
provider/model binding. It must migrate controls, recovery, ViewModels, and the
Workbench conversation UI before deleting old TaskSession/Event/AgentRun
presentation owners.

## R1 entry condition

R1 starts only after R0 is committed and its native evidence is current for the
exact source. R1 must migrate ordinary Chat completely before deleting its old
session/event/presentation consumers; it must not add a parallel Chat runtime.
