# Current OpenLife Product Development Plan

Status: complete

## Objective

Deliver the first S2 vertical slice of the canonical Task Runtime on the
existing generated-report path. A report draft must gain a stable Task, typed
Items, and an Artifact identity before Review; approval must materialize and
confirm that same Artifact rather than deriving artifact identity from the
Proposal.

## Product result

```text
report request
  -> existing governed generation and read evidence
  -> canonical Task + ArtifactDraft Item + ArtifactVersion
  -> ReviewCheckpoint Item + Proposal
  -> confirmed file materialization
  -> same ArtifactVersion points to the confirmed file
```

The actual file remains the artifact content authority. SQLite owns Task,
Item, review relation, artifact metadata, version, and recovery state.

## In scope

1. Add a bounded SQLite canonical Task Runtime store for stable Tasks, Run
   membership, typed Items, Artifact metadata, and ArtifactVersion metadata.
2. Make Task identity stable across multiple Run references without changing
   the existing AgentRun execution owner in this slice.
3. Integrate only the current provider-generated Markdown/CSV report path.
4. Mint Artifact identity before Proposal creation and bind Proposal as a
   Review checkpoint rather than artifact identity.
5. On confirmed file materialization, update the same ArtifactVersion with the
   confirmed file reference and digest.
6. Preserve failed, blocked, and effect-unknown states without reporting Task
   completion.
7. Add restart/idempotency and production-path contract tests.

## Out of scope

- general Web or local-document capability expansion from S3;
- migrating every Main Chat route in this batch;
- changing provider/model selection or adding fallback;
- steering, concurrency, subagents, or workflow DSLs;
- broad Results/Changes/Preview redesign;
- deleting TaskSession, AgentRun, ActionQueue, PlanExecute, or transcript
  compatibility surfaces before the report path proves its replacement;
- changing Memory or LifeModel ownership;
- old profile or default Keychain cleanup.

## Ownership during this slice

- `OpenLifeTurnRuntime` remains the production application runtime.
- `AgentRunStore` remains the execution and receipt owner for the current Run.
- The new Task Runtime store is the sole owner of the new stable Task,
  Task-to-Run membership, typed report Items, and independent Artifact records.
- `AgentTaskSessionStore` remains a compatibility execution-session owner; it
  is not copied into the new Task store as another Task status authority.
- ReviewWorkflow owns the approval decision. ArtifactMaterializer owns the
  filesystem effect. Neither owns Artifact identity or Task completion.

## Acceptance

| Scenario | Required result |
| --- | --- |
| First generated report draft | one stable Task, one Run membership, ArtifactDraft Item, ArtifactVersion v1 |
| Proposal staging | same Artifact is waiting on a ReviewCheckpoint; Proposal ID is only a relation |
| Replayed staging | no duplicate Task, Item, Artifact, version, or checkpoint |
| Same conversation, later Run | same Task accepts another Run membership |
| Confirmed materialization | same ArtifactVersion records confirmed file ref and observed digest |
| Failed or unknown effect | Artifact and Task remain failed/blocked/unknown, never completed |
| Restart | SQLite reopens to the same identities and states |
| Non-report route | no canonical report Task/Artifact is created |

## Checks

Use focused tests while editing, followed by:

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

Exact native evidence is required if the production report path or ViewModel
changes in a way that cannot be proven by source contracts. External-live Web
and provider credit remains S3/S6 work and must not be inferred from fixtures.

## Stop condition

S2 first slice closes only when the report path uses the new owner in
production code, Proposal-derived artifact identity is removed for that path,
all acceptance cases pass, documentation stays aligned, semantic commits are
reviewable, and the working tree is clean.

## Closure

The generated Markdown/CSV report path now creates its canonical Task,
Run membership, ArtifactDraft Item, and ArtifactVersion before Proposal
staging. Review is an exact checkpoint relation; confirmed materialization and
recovery update the same ArtifactVersion. Focused recovery/idempotency tests,
the full Rust suite, frontend unit/build checks, and browser-shell E2E are
green. No external-live provider or Web credit is claimed by this slice.

## Next pointer

Continue S2 by projecting canonical Task/Item/Artifact records into backend
ViewModels and moving the next report execution fact from compatibility stores.
Begin S3 only after the minimal report Task Runtime is recoverable and visible.
