# Current OpenLife Product Development Plan

Status: active

## Objective

Create a clean, reviewable baseline for the agreed general Agent product before
changing its canonical runtime. Close the current mixed working-tree batch,
record the stable product and architecture decisions, verify the exact source,
and leave one unambiguous next pointer.

The next product path is:

```text
local documents + Web research
  -> sourced Markdown report
  -> preview, changes, steering, verification, and resumable result
```

## Current baseline

- Branch: `codex/phase5-native-closure`.
- The plan was activated on a dirty tree containing conversation and Agent
  Memory closure, source-bound generation, LifeModel review fixes, frontend
  presentation, tests, and documentation in one mixed batch.
- Automated Rust, frontend, build, and browser-shell gates were green on that
  mixed source snapshot. This is engineering evidence, not current exact-build
  native or external-live product credit.
- Current production Main Chat entrypoints converge on
  `OpenLifeTurnRuntime`. The tree still has multiple durable lifecycle owners
  and does not yet implement the accepted canonical Task Runtime.

Do not add bundle hashes, run IDs, proposal IDs, profile dumps, or iterative
trial narratives to this plan. Git history and isolated local evidence retain
that detail when needed.

## Product contract

- The user delegates an outcome and may supply resources, scope, constraints,
  and a desired deliverable.
- One Task may plan, use tools, pause for approval, accept steering, resume, and
  deliver artifacts without changing product identity.
- Work inside an explicit low-risk and recoverable scope proceeds autonomously.
  Scope expansion and consequential or destructive effects require a
  just-in-time decision.
- The selected provider and model stay bound to the Task. There is no silent
  model or provider fallback.
- Result completion requires canonical state, artifacts, and verification; a
  plan, streaming response, tool call, or approved proposal is not enough.
- Agent Memory and LifeModel are bounded collaborators, not task-runtime owners.

## Current batch: S0 stabilization and S1 authority

### In scope

1. Classify every existing changed file as retain, correct, extract, or delete.
2. Close only the already-implemented conversation/Memory and source-bound
   generation behavior needed to make the current batch self-consistent.
3. Extract the newly added source/evidence/output-validation responsibilities
   from `main_chat_kernel.rs` into a bounded module while retaining one runtime
   owner.
4. Remove stale phase pointers, raw fixed-snapshot audit links, duplicated plan
   history, and temporary report authority.
5. Keep `PRODUCT.md`, `docs/ARCHITECTURE.md`, ADR 0017, and this plan aligned.
6. Run proportional focused checks, full engineering gates, and one exact-build
   isolated native verification for the current source.
7. Split the stabilized work into reviewable semantic commits and finish with a
   clean working tree.

### Out of scope

- implementing Task/Run/Item/Artifact schemas or migrating a product path;
- building another runtime, dual-writing, or adding a fallback to an old path;
- expanding LifeModel learning or introducing AI coding for LifeModel;
- computer use, arbitrary shell, email send, calendar write, payment, or broad
  connector work;
- automatic model routing or cross-provider fallback;
- subagent orchestration or a workflow DSL;
- deleting old application profiles or touching the default Keychain service;
- creating a development ledger, evidence registry, task-packet system, or new
  governance platform.

## Source entrypoints

```text
frontend/src/tauri.ts
  -> src-tauri/src/lib.rs
  -> main_chat_send.rs | main_chat_streaming.rs
  -> main_chat_turn_runtime.rs
  -> main_chat_kernel.rs
  -> openlife-core/src/agent/main_chat_agent_v1.rs
```

Relevant supporting owners include ToolGateway, ReviewWorkflow,
ArtifactMaterializer, MemoryGateway, LifeModelWriteGateway,
PersistenceCoordinator, canonical stores, and backend ViewModels.

## Acceptance matrix

| Scenario | Required result | Evidence |
| --- | --- | --- |
| Existing conversation and scoped Memory path | Correct scope, no unrelated recall, no silent write | focused contracts + exact native |
| Source-bound answer with matching evidence | Answer bound to selected sources without internal IDs | focused contracts + exact native |
| Source-bound answer without evidence | Provider/tool not called; visible blocker or bounded unknown | focused contracts + exact native |
| Conflicting or unsupported source claims | No silent synthesis; limitation remains visible | focused contracts + exact native |
| LifeModel absent or not selected | Ordinary Agent path remains healthy; no false Review state | focused contracts + exact native |
| Provider/tool failure or uncertain effect | accurate blocked/failed/unknown state; no false completion | contracts + product projection checks |
| Documentation | one current product contract, architecture direction, ADR, and plan | source review + diff review |

## Checks

Use focused tests during editing. Before closing the batch run:

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

The exact-build native trial must use an explicit isolated data directory,
workspace, artifact directory, and trial Keychain service. External-live
provider or Web credit is required only for a contract that actually uses it;
otherwise it remains explicitly unverified.

## Stop condition

S0 and S1 are complete only when:

- all original dirty changes have a reviewed disposition;
- current source responsibilities are bounded and obsolete authority is gone;
- authority documents agree and this plan remains concise;
- engineering gates are green for the final source;
- the same exact source has current isolated native evidence;
- evidence levels and unverified live behavior are reported accurately;
- semantic commits are reviewable and the working tree is clean.

Do not begin S2 inside an unreviewed S0/S1 batch.

## Next pointer

After S0/S1 closes, begin S2 with a minimal canonical Task/Run/Item/Artifact
contract used immediately by the first vertical report path. Each later batch
must deliver user-visible value or be consumed by that path no later than the
following batch.

The accepted roadmap is:

1. S0 stabilization and dirty-tree closure.
2. S1 product, architecture, ADR, plan, and evaluation contract.
3. S2 canonical Task/Run/Item/Artifact foundation on the report path.
4. S3 real local-document and Web report tool loop.
5. S4 steering, inline approval, recovery, and controlled concurrency.
6. S5 Results, Changes, Preview, and Verification product surfaces.
7. S6 behavior matrix with exact native and required live evidence.
8. S7 remaining old-path deletion and clean release baseline.
