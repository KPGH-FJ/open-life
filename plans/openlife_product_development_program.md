# Current OpenLife Product Development Plan

Status: complete

## Objective

S2.4 is complete. A report Run that performs governed
reads must record each exact durable ToolCall and its bound Observation as
canonical typed Items before any ArtifactDraft or Review checkpoint is
admitted.

## Product result

```text
Instruction
  -> durable governed ToolCall
  -> bound Observation
  -> completed ProviderGeneration
  -> ArtifactDraft
  -> ReviewCheckpoint / materialization
```

A report without governed reads keeps the existing
`Instruction -> ProviderGeneration -> ArtifactDraft` path. Canonical Items
store bounded identities and digests only, never tool input/output bodies.

## In scope

1. Add `tool_call` and `observation` Item kinds with a transactional schema-v2
   to schema-v3 migration.
2. Derive the pair only from the exact durable ActionQueue/tool receipt and its
   run-bound Observation evidence after existing validation succeeds.
3. Record deterministic Item identities and metadata-safe digests for zero,
   one, or multiple governed reads in their actual execution order.
4. Move canonical report admission after durable tool evidence recording but
   before Artifact proposal creation.
5. Preserve exact replay, multiple Artifacts per Run, multiple Runs per Task,
   schema-v1 history, and schema-v2 execution-fact history.
6. Project the new typed Items through existing backend ViewModels and the
   TypeScript contract.

## Out of scope

- canonical Plan, Verification, or FinalResult Items;
- S3 expansion of Web, local-document, connector, or write capabilities;
- ItemAttempt, steering, concurrency, or subagents;
- copying transcript summaries, tool arguments, observations, or report bodies;
- replacing compatibility task controls;
- Memory or LifeModel changes;
- native-desktop or external-live evidence credit.

## Ownership

- ToolGateway, ActionQueue, receipt ledger, and durable tool events remain the
  execution/effect owners during migration.
- `CanonicalTaskRuntimeStore` owns Item identity, order, and digest bindings for
  the migrated report path; it does not become a second tool executor.
- Backend ViewModels remain the only product-facing composition owner.

## Acceptance

| Scenario | Required result |
| --- | --- |
| Report with no governed read | no synthetic ToolCall or Observation Item |
| One successful governed read | one ToolCall followed by its bound Observation |
| Multiple governed reads | deterministic ordered pairs before ProviderGeneration and ArtifactDraft |
| Missing, failed, or unbound tool proof | no canonical report admission |
| Exact replay or second Artifact | no duplicate execution Items |
| Same Run with changed tool/observation digest | fail closed with zero partial canonical mutation |
| Existing schema-v1/v2 store | transactional migration preserves prior identities without invented facts |
| Product read model | typed Items appear through backend Task/Workspace projections |

## Checks

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
```

## Completion evidence

- Schema v3 records deterministic `ToolCall -> Observation` pairs before
  ProviderGeneration and ArtifactDraft; zero-read reports add no synthetic
  pair.
- Existing v1 and v2 databases migrate transactionally without rewriting their
  historical execution-fact versions.
- Exact replay, changed fact conflicts, and multiple ordered reads are covered
  in the core store tests.
- The CC01 product path proves a governed Web read is durable before the
  canonical report Items and Review checkpoint; its forged-citation negative
  path remains blocked.
- Full Rust, frontend, production-build, and browser-shell gates passed on the
  current source. No native-desktop or external-live claim was made.

## Next pointer

Continue S2 with canonical Plan, Verification, and FinalResult facts on the
same report path. Do not begin S3 capability expansion first.
