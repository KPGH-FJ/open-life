# Current OpenLife Product Development Plan

Status: blocked

## Objective

Complete S6 by proving the accepted report task path as one behavior matrix on
the exact current source: controlled tests for every safety and recovery
contract, an exact native Tauri build and product-path review, and only the
external-live provider/Web checks required to prove real execution.

## Product path

```text
Workspace request
  -> canonical Task / Run / Item execution
  -> document.read and/or Web read
  -> user-selected provider synthesis
  -> ArtifactVersion + Review checkpoint
  -> confirmed materialization
  -> Results / Changes / Preview / Verification
```

## In scope

1. Define one report behavior matrix covering document-only, Web-only, and
   combined reports plus their failure boundaries.
2. Prove steering consumption, inline approval continuation, restart recovery,
   cancellation, and bounded concurrency without creating a second lifecycle.
3. Prove proposed, materialized, drifted, missing, failed, and effect-unknown
   artifact projections through backend product truth.
4. Build the exact current native Tauri application and review the Workspace,
   Tasks, and Review product path with an isolated data profile.
5. Run a bounded external-live provider/Web report case only through the
   explicit live-eval gate. Never credit local HTTP, fixtures, or scripted
   providers as external-live evidence.
6. Keep the evidence summary concise and tied to commands and current commit;
   do not create an evidence registry or store user content in planning docs.

## Out of scope

- new connectors, computer-use, shell, provider auto-routing, Memory, or
  LifeModel capability;
- rich editors, PDF/image preview, or additional artifact formats;
- S7 old-path deletion and release cleanup;
- touching the default OpenLife profile or using historical native/live runs
  as proof for the current build.

## Behavior matrix

| Scenario | Required product truth | Required evidence |
| --- | --- | --- |
| Document-only report | exact bound document read before provider; reviewable report | controlled command-surface + exact native |
| Web-only report | observed Web evidence and verified citations before proposal | controlled command-surface + external-live |
| Combined report | ordered document and Web Items feed one provider synthesis | controlled command-surface + external-live |
| Missing/drifted document | no Web/provider dispatch and no Artifact | controlled negative test |
| Missing/forged Web citation | one bounded provider retry, then no Artifact | controlled negative test + external-live valid case |
| Steering | authenticated in-scope input consumed once at checkpoint | restart/integration test + native UI |
| Scope-expanding steering | blocked without capability or policy expansion | controlled negative test |
| Review approval | exact Artifact materializes, then same task may continue | integration test + native UI |
| Restart recovery | no duplicate reads, proposal, effect, or steering consume | file-backed/restart test |
| Concurrency/cancellation | admission before mutation; one owner; bounded parallelism | integration test |
| Result surfaces | backend-owned Result, Change, Preview, Verification agree | projection/UI test + native UI |
| Drift/missing/effect unknown | no delivered claim, no fabricated preview, no blind replay | controlled negative test |

## Evidence boundaries

- Source/unit/integration tests prove deterministic contracts only.
- Browser-shell tests prove React behavior only.
- Native evidence must use the exact current bundle and an isolated profile.
- External-live evidence must use the configured user-selected provider and
  real Web access through `scripts/live-eval.zsh`.
- A missing key, unavailable account, or provider/network refusal stops S6 as
  blocked; it is never replaced by a fixture.

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

S6 closes only when every matrix row has its required current-source evidence,
the exact native path is reviewed from an isolated profile, the required live
case succeeds or is truthfully blocked on user-supplied credentials, all gates
pass, and the working tree is clean. Then move to S7 old-path deletion and a
clean release baseline.

## Current result

- The controlled report matrix passes for document/Web execution, typed retry,
  steering, approval ownership, cancellation, recovery, concurrency, artifact
  materialization, and backend result projections.
- Full Rust and frontend gates pass. The exact current release bundle builds.
- The explicitly gated external-live document + Web report completes through
  one pending Review item and materializes once after acceptance.
- The exact native bundle was reviewed with a fresh isolated data profile and
  correctly fails closed when its credential store is unavailable.

## Current blocker

Completing the report workflow inside that exact isolated native profile needs
macOS Keychain initialization/recovery. That creates or accesses persistent
credentials, so it requires the user's confirmation at action time. Do not use
the default OpenLife profile or substitute fixture credentials.
