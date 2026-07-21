# Phase 4D Durable-Truth Execution Baseline

Status: `IMPLEMENTED`
Date: 2026-07-21

## Verified Starting Point

- Governed-action PR `#60` is merged at `5238ada4590636bfeb2dbfb8973d239a16e9655a`.
- Protected-main CI run `29795402563` completed successfully.
- This slice starts from that exact `origin/main` commit on
  `codex/phase4d-durable-truth-spine`.
- The existing Phase 4D entry remains development-only at `/dev/phase4d/`.

## Slice Goal

Add the desktop durable-truth journey to the candidate Shell:

```text
LifeModel / Memory read models
  -> source-backed current understanding
  -> exact ReviewItem decision
  -> refresh LifeModel + Memory + Review Center
  -> approved-not-applied | applying | applied | failed | rolled-back | unknown
```

The journey must prove that a decision and a durable result are separate. It
must not expose a materialization or rollback control unless a typed backend
action with an exact target exists.

## Allowed Changes

- `frontend/src/ui/journeys/durableTruth/**`
- the existing dev-only Phase 4D composition and fixtures
- focused frontend tests and desktop QA automation
- Phase 4D durable-truth documentation and artifacts
- release absence guards and the existing Rust source guard
- the Phase 4A migration/deletion ledger

## Forbidden Changes

- production `App.tsx`, `ProductShell.tsx`, route authority, or old page owners
- Rust/Tauri business authority or durable-write semantics
- raw proposal/store joins in the candidate page
- synthetic Apply or rollback buttons
- any claim that a fixture or command callback proves backend completion
- mobile implementation or mobile acceptance criteria

## Entry Gate

`DURABLE_TRUTH_IMPLEMENTATION_ALLOWED=YES` because the prior slice is merged,
main CI is green, and the branch was created from the verified main commit.

`PRODUCTION_AUTHORITY_SWITCH_ALLOWED=NO` until Phase 4E.
