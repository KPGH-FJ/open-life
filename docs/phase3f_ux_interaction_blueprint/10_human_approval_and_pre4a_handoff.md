# Phase 3F Human Approval And Pre-4A Handoff

Status: `APPROVED_DIRECTION_PRE_4A_BLOCKED`
Date: 2026-07-18

## Approval Record

The user approved proceeding from the Phase 3F plan on 2026-07-18. This records
human approval for:

- the Codex/Cursor white workbench visual baseline;
- typography, spacing, line, radius, semantic-color, and icon direction;
- desktop and mobile information hierarchy;
- Workspace, Review, evidence, permission, and Settings interaction grammar;
- fail-closed unknown/stale behavior;
- the distinction between approved, applying, applied, failed, and unknown.

The approval does not claim that target fixture fields already exist in current
backend projections. It does not approve React implementation from the current
convergence branch.

## Repository Baseline Before Convergence

The pre-fetch local snapshot was:

```text
DESIGN_BASE_COMMIT = e1b43161f78a
LOCAL_ORIGIN_MAIN = 9da3908359f6 (2026-07-08)
ROADSHOW_FREEZE = c9e75c8cc904
CONVERGENCE_VS_LOCAL_ORIGIN_MAIN = 0 behind / 161 ahead
PHASE3D_UNTRACKED_FILES = 3
PHASE3E_UNTRACKED_FILES = 47
PHASE3F_UNTRACKED_FILES = 57
```

The configured convergence upstream did not have a local remote-tracking ref at
this snapshot. These facts are evidence inputs only and must be refreshed before
any push or merge decision.

## Required Pre-4A Gate

```text
PHASE3F_HUMAN_APPROVAL = YES
DESIGN_ASSETS_TRACKED_AND_REVIEWED = PENDING
REMOTE_STATE_FETCHED_AND_CLASSIFIED = NO
CONVERGENCE_MERGED_TO_MAIN = NO
MAIN_CI_GREEN = UNKNOWN
MAIN_REVERIFIED = NO
FRONTEND_REFACTOR_READY = NO
PHASE4A_BRANCH_CREATED_FROM_VERIFIED_MAIN = NO
```

Protected remote `main` is the only long-term product authority. The next work
must preserve these assets in a scoped design commit, fetch and classify remote
state, merge convergence through normal protected-main review, merge the design
authority, and reverify the resulting `origin/main`.

## Backend Boundary

The Phase 3F backend map uses the roadshow/convergence snapshot. It does not
include the 13 commits unique to the paused Backend Remediation v4 branch. V4
plans and findings remain backlog boundaries and are not completion evidence.

After mainline convergence, rerun the backend capability map and call the result
`BACKEND_CAPABILITY_AND_AUTHORITY_BASELINE`. Phase 4A may add read-only product
projections and executable contract tests; it must not silently change business
authority, authorization semantics, or durable-write policy.

## Deletion Rule For Phase 4A

Phase 4A must create the migration/deletion ledger before introducing a new
frontend owner. Each row must name:

- new owner;
- old owner;
- last product caller;
- deletion condition;
- absence guard;
- current state from `identified` through `guarded_absent`.

The production authority switch may wait until Phase 4E, but dependency mapping
and delete-readiness advance with every earlier journey slice.
