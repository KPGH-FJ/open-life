# Current OpenLife Product Development Plan

Status: complete

## Objective

Complete the next S2.3 report-runtime slice. A provider-generated report must
record the authenticated user instruction and the exact completed provider
generation as canonical typed Items before any ArtifactDraft or Review
checkpoint is admitted.

## Product result

```text
authenticated instruction digest
  -> canonical Instruction Item
validated durable provider receipt
  -> canonical ProviderGeneration Item
  -> ArtifactDraft Item
  -> ReviewCheckpoint / materialization
```

The canonical store persists metadata-safe digests and identities only. It does
not copy user prompts, provider responses, report bodies, or credentials.

## In scope

1. Add `instruction` and `provider_generation` to the canonical report Item
   contract with a safe schema-v1 to schema-v2 migration.
2. Create one instruction and one completed provider-generation Item for each
   newly admitted report Run.
3. Bind the instruction to the Policy-authorized user-message digest and the
   generation to one exact provider request and terminal receipt digest.
4. Move production ArtifactDraft admission after provider receipt validation
   and durable provider-event persistence.
5. Preserve exact replay, multiple Artifact drafts per Run, multiple Runs per
   Task, and existing schema-v1 report records.
6. Project the new typed Items through the existing backend ViewModels without
   frontend store joins or report-body copies.

## Out of scope

- real Web or local-document capability expansion from S3;
- canonical Plan, tool-call, Observation, Verification, or FinalResult Items;
- ItemAttempt, steering, concurrency, or subagents;
- replacing report task controls in this slice;
- migrating non-report Main Chat routes;
- Memory or LifeModel changes;
- external-live or native-desktop evidence credit.

## Ownership

- Policy owns the authenticated instruction authorization digest.
- provider lifecycle and durable event owners prove the completed provider
  attempt before canonical report admission.
- `CanonicalTaskRuntimeStore` owns the new typed Item identities and ordering.
- `AgentRunStore` remains the detailed execution and receipt owner during this
  vertical migration, but it cannot override canonical report product state.

## Acceptance

| Scenario | Required result |
| --- | --- |
| New report Run | Instruction, ProviderGeneration, then ArtifactDraft Items |
| Two Artifacts in one Run | one instruction/generation pair and two drafts |
| Exact replay | no duplicate Item, Run, Task, or Artifact |
| Same Run with changed instruction or receipt | fail closed with zero partial mutation |
| Later Run in same Task | new instruction/generation pair under the same Task |
| Invalid or missing completed provider receipt | no canonical report admission |
| Existing schema-v1 store | opens through transactional migration with prior identities and facts intact |
| Product read model | typed Items appear in backend Task/Workspace projections |

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

## Stop condition

Close this slice only when provider-generated report admission cannot precede
its validated durable provider receipt, the new Item order and replay contract
survive restart and migration, backend ViewModels expose the facts, all gates
pass, commits are reviewable, and the tree is clean.

## Closure

- New provider-generated report Runs record exactly one authenticated
  Instruction and one completed ProviderGeneration before ArtifactDraft Items.
- Missing, failed, or conflicting provider truth creates no canonical report
  Task or Artifact; changed Run facts roll back without partial mutation.
- Multiple Artifacts reuse one execution-fact pair, exact replay remains
  idempotent, and later Runs receive their own pair under the same Task.
- Existing schema-v1 report stores migrate transactionally; legacy identities
  remain readable without inventing historical execution facts.
- Backend Task and Workspace projections expose the typed Items. No prompt,
  response, Artifact body, or credential is copied into the canonical store.
- Rust, frontend, production-build, absence-guard, and browser-shell gates pass.
  This slice claims no native-desktop or external-live evidence.

## Next pointer

Continue S2.3 with the next report execution facts: Plan, governed tool call,
Observation, Verification, and FinalResult. Do not begin S3 capability
expansion until those facts have one canonical lifecycle owner on the report
path.
