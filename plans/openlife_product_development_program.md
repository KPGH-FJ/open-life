# Current OpenLife Product Development Plan

Status: complete

## Objective

Complete S5 by turning the canonical report lifecycle into a reviewable product
result: users can see Results, proposed or applied Changes, a bounded Preview,
and independent Verification without the frontend reconstructing truth.

## Product path

```text
canonical Task/Run/Item/ArtifactVersion
  -> backend TasksViewModel presentation projection
  -> Result summary
  -> exact Change target and create/replace state
  -> bounded Markdown/CSV Preview
  -> expected versus observed digest Verification
```

## In scope

1. Extend `TaskArtifactViewModel` with backend-owned change, preview, and
   verification projections.
2. For a pending draft, read preview/change facts only from the exact proposal
   bound to the Artifact id, version, target, and content digest.
3. For a materialized Artifact, read preview bytes only from the exact regular
   file reference and expose them only when the observed digest matches the
   canonical content digest.
4. Keep preview bounded and UTF-8/text-only; never follow symlinks or infer
   content from filenames, status prose, or frontend state.
5. Show Results, Changes, Preview, and Verification as distinct sections on the
   Tasks product surface, including truthful pending, failed, and unknown states.
6. Preserve Review and task controls as separate actions; viewing a preview
   does not approve, apply, retry, or complete anything.

## Out of scope

- rich document editing, PDF rendering, image preview, or diff algorithms;
- opening arbitrary local paths or adding shell/computer-use capability;
- a second artifact store or frontend-owned lifecycle state;
- provider routing, Memory, LifeModel, connectors, or old-path deletion;
- native/external-live behavior-matrix closure, which belongs to S6.

## Ownership

- `CanonicalTaskRuntimeStore` remains Task/Run/Item/ArtifactVersion authority.
- ProposalStore may supply an exact pending draft and target precondition only
  after all canonical artifact bindings match.
- The verified materialized file may supply preview bytes only after type,
  size, path, and digest checks.
- `TasksViewModel` owns product presentation facts. React only renders its typed
  status and never compares raw stores or computes completion.

## Acceptance

| Scenario | Required result |
| --- | --- |
| Pending new report | Change says create; bounded draft Preview; Verification pending |
| Pending replacement | Change says replace and identifies expected prior digest |
| Materialized matching file | Result delivered; applied Change; verified Preview and digest |
| Materialized file drift | no Preview; Verification failed; no delivered claim |
| Missing materialized file | no Preview; Verification failed/unknown, never empty success |
| Effect unknown | Change and Verification remain unknown; no automatic replay |
| Failed artifact | failed result with no fabricated preview or verification |
| Oversized or non-UTF-8 content | preview unavailable/truncated by backend contract |
| Multiple artifacts | each version has independent Change, Preview, and Verification |
| Frontend refresh | same backend projection is rendered without local inference |

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

S5 closes only when all four surfaces are driven by backend truth, negative
preview/digest cases fail closed, full gates pass, docs agree, the commit is
reviewable, and the working tree is clean. Then move to S6 exact native and
required live evidence.

## Closure

- Each canonical report ArtifactVersion now exposes backend-owned Result,
  Change, Preview, and Verification projections.
- Pending preview content is accepted only from an exact proposal binding;
  materialized preview content is accepted only from a regular file inside the
  configured safe paths whose current digest matches canonical observation and
  Verification Item truth.
- Drift, missing files, unsafe file types, oversize content, and invalid UTF-8
  fail closed and remove delivered product credit.
- The Tasks surface renders all four states without reading files, joining raw
  stores, or treating Review acceptance as delivery.

## Next pointer

After S5 closes, begin S6: run the accepted behavior matrix with exact native
evidence and only the required explicitly authorized live-provider checks.
