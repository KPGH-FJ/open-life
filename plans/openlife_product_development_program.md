# Current OpenLife Product Development Plan

Status: complete

## Objective

Complete the S2 report read-model slice. The existing backend-owned
`TasksViewModel` and `WorkspaceViewModel` must project canonical report Task,
Run membership, typed Item, Artifact, and ArtifactVersion truth without the
frontend joining raw stores or treating compatibility TaskSession completion as
the report Task authority.

## Product result

```text
task_runtime.db
  -> backend TasksViewModel composition
  -> canonical report task status + typed item timeline + artifact summaries
  -> existing Tauri ViewModel command
  -> Tasks and Workspace product surfaces
```

The frontend receives bounded metadata and materialized file references only.
Artifact bodies remain in their files.

## In scope

1. Add one consistent read snapshot for canonical report Task, Run membership,
   Items, Artifacts, and current ArtifactVersions.
2. Overlay the canonical report Task onto its current compatibility execution
   session by exact Run membership, avoiding duplicate product tasks.
3. Derive report lifecycle and delivery proof from canonical Artifact state;
   compatibility completion cannot override it.
4. Project bounded typed Item and Artifact summaries through
   `TasksViewModel`, inherited by `WorkspaceViewModel`.
5. Show canonical report results/status in the existing Tasks and Workspace
   surfaces without a broad visual redesign.
6. Keep report rejection and effect-unknown states truthful in the canonical
   owner and product read model.
7. Add store, builder, Tauri composition, TypeScript contract, and product UI
   tests.

## Out of scope

- S3 Web or local-document capability expansion;
- new Results/Changes/Preview information architecture;
- provider/model routing, fallback, steering, concurrency, or subagents;
- migrating non-report Main Chat routes;
- deleting compatibility stores before their remaining read and control uses
  are replaced;
- Memory or LifeModel changes;
- external-live provider or Web credit.

## Ownership

- `CanonicalTaskRuntimeStore` owns migrated report Task, Run membership, Item,
  Artifact, version, and lifecycle truth.
- `AgentRunStore` remains the execution/receipt owner for current Runs.
- compatibility TaskSession supplies controls and old event activity only; it
  cannot override canonical report completion or Artifact state.
- backend ViewModels perform the only product composition. Frontend code only
  renders the ViewModel contract.

## Acceptance

| Scenario | Required result |
| --- | --- |
| Report waiting for Review | one product Task with `waiting_review`, exact checkpoint and Artifact |
| Compatibility session says completed | canonical waiting/unknown state still wins |
| Confirmed ArtifactVersion | Task is completed only with matching observed digest and file reference |
| Multiple Runs or Artifacts | one Task; all memberships/items visible; partial materialization is not completed |
| Rejected Review | canonical report Task becomes blocked and does not claim delivery |
| Effect unknown | Task remains remote/effect unknown and never completed |
| Store unavailable/read failure | ViewModel is stale/error with warning; no compatibility fallback for migrated truth |
| Frontend | existing Tasks/Workspace surfaces render canonical artifact status from the backend contract |

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

Close this slice only when one backend product task represents the canonical
report lifecycle, Item/Artifact metadata reaches both product ViewModels, the
frontend renders it without raw-store joins, recovery states remain
conservative, all gates pass, commits are reviewable, and the tree is clean.

## Closure

- Canonical report Task snapshots now reach both backend product ViewModels.
- Compatibility completion cannot override canonical Review, rejection,
  failure, or effect-unknown truth.
- Tasks and Workspace render canonical Artifact status and materialized file
  references; task controls remain bound to the compatibility execution session
  until that control path migrates.
- Store, builder, rejection reconciliation, Tauri composition, frontend
  contract, product UI, production build, and browser-shell checks pass.
- No external-live or native desktop trial credit is claimed by this slice.

## Next pointer

Continue S2 with one bounded slice that moves the next report execution fact
into canonical Items and removes the corresponding compatibility read
dependency. S3 remains blocked until the report runtime no longer depends on a
parallel product lifecycle owner for its execution truth.
