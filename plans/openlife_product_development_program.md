# Current OpenLife Product Development Plan

Status: complete

## Objective

S2 is complete: the first provider-generated report path now has one canonical
Task/Run/Item/Artifact lifecycle through Plan, governed evidence, Review,
materialization, Verification, and FinalResult. Stop before S3 capability
expansion.

## S2 product lifecycle

```text
Task
  -> Run
    -> Instruction
    -> Plan
    -> (ToolCall -> Observation)*
    -> ProviderGeneration
    -> ArtifactDraft(s)
    -> ReviewCheckpoint(s)
    -> ArtifactMaterialized
    -> Verification
    -> FinalResult
```

The path may wait at Review. `FinalResult` exists only after every Artifact in
the current report result is materialized and its observed digest equals its
expected ArtifactVersion digest.

## Completed scope

- one SQLite canonical Task/Run/Item/Artifact metadata owner on the report path;
- Task and Artifact identities independent of Proposal;
- backend Task/Workspace projections;
- Instruction and ProviderGeneration facts;
- exact governed ToolCall and bound Observation facts;
- deterministic metadata-safe Plan contract before governed execution facts;
- canonical Verification bound to ArtifactVersion materialization evidence;
- canonical FinalResult as the only delivered completion fact;
- backend Task/Workspace projections that require Verification and FinalResult;
- transactional v1-v4 migrations and metadata-only persistence;
- exact replay, multi-Artifact, multi-Run, rejection, digest mismatch,
  effect-unknown, restart, and legacy-history coverage.

## Out of scope

- S3 expansion of local-document, Web, connector, or write capabilities;
- ItemAttempt and generalized Task contract ownership;
- steering, inline approval continuation, controlled concurrency, or subagents;
- Results/Changes/Preview UI redesign from S5;
- standalone PlanExecute deletion outside the migrated report path;
- Memory or LifeModel changes;
- computer use, arbitrary shell, provider routing, or silent fallback;
- native-desktop or external-live evidence unless required to prove an S2
  contract that cannot be established below that evidence level.

## Ownership

- `CanonicalTaskRuntimeStore` owns report Task, Run, Item order/status/digests,
  Artifact identity/version, Verification, and FinalResult.
- AgentRun, ToolGateway, provider receipt, ReviewWorkflow, and
  ArtifactMaterializer remain detailed execution/effect proof owners during
  migration. Canonical Items bind to those facts without copying bodies.
- Backend ViewModels remain the product-facing composition owner.
- Proposal acceptance is not materialization; materialization is not verified
  completion until the canonical Verification and FinalResult transitions
  succeed.

## Acceptance

| Scenario | Required canonical result |
| --- | --- |
| No-tool report | Instruction, Plan, ProviderGeneration, ArtifactDraft; no synthetic tool pair |
| Governed-read report | ordered ToolCall/Observation pairs between Plan and ProviderGeneration |
| Pending Review | checkpoint waits; no Verification or FinalResult |
| One verified Artifact | materialized and Verification Items are completed |
| Multi-Artifact partial acceptance | verified Item only for accepted Artifact; Task remains waiting |
| All Artifacts verified | one FinalResult for the completing Run and Task completed |
| Rejection | checkpoint/artifact/task blocked; no false Verification or FinalResult |
| Effect unknown | artifact/task unknown; no automatic replay or FinalResult |
| Digest mismatch | fail closed; no completed Verification or FinalResult |
| Exact replay/restart | no duplicate Items, versions, effects, or result facts |
| Later Run after completion | same Task, new Run/Plan/result lineage without rewriting history |
| Existing v1-v3 store | transactional migration preserves historical identities and fact versions |
| Product read model | delivered status requires canonical FinalResult plus verified Artifact evidence |

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

- Core store tests prove Plan ordering, exact evidence binding, partial and full
  multi-Artifact completion, replay idempotency, negative terminal paths, and
  v1-v3 migration into schema v4.
- The CC01 product test proves the complete local-document plus governed Web
  report path through Review, materialization, Verification, and FinalResult;
  its forged-citation path remains fail-closed.
- Backend read-model tests prove Artifact fields alone do not count as delivery
  when the canonical FinalResult is absent.
- Full Rust, frontend, production-build, and browser-shell gates pass on the
  completing source. Native-desktop and external-live evidence remain S6 work,
  so S2 makes no claim at those evidence levels.

## Next pointer

After S2 closes, begin S3: strengthen the real local-document and Web report
tool loop on this canonical foundation. Do not reopen S2 as a parallel runtime.
