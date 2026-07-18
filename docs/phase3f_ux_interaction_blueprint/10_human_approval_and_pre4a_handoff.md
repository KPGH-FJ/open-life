# Phase 3F Human Approval And Pre-4A Handoff

Status: `APPROVED_DIRECTION_DESIGN_PR_PENDING`
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
this snapshot. These values are retained as the historical pre-fetch snapshot;
the refreshed remote facts are recorded below.

## Convergence And Mainline Evidence

```text
CONVERGENCE_PR = https://github.com/KPGH-FJ/open-life/pull/50
CONVERGENCE_HEAD = e1b43161f78a
MERGED_MAIN = 79f613871c6b68c037be5d5e08e434c04446df96
MERGED_AT = 2026-07-18T11:39:26Z
ROADSHOW_FREEZE = c9e75c8cc904
BACKEND_REMEDIATION_V4 = PAUSED_SEPARATE_BRANCH
```

The protected-main push checks passed for commit `79f6138`:

| Check | Result | Run |
| --- | --- | --- |
| CI | `PASS` | `29642938351` |
| Step 6 Tauri Product Acceptance | `PASS` | `29642938335` |
| Stage 1 Tauri Dogfood | `PASS` | `29642938370` |

The deleted convergence remote branch is not a second authority. The fetched
`origin/main` merge commit is the sole code baseline.

## Local Reverification Of Merged Main

An isolated detached worktree at exact `origin/main` commit `79f6138` produced:

| Gate | Result |
| --- | --- |
| clean worktree and `git diff --check` | `PASS` |
| `cargo fmt --check` | `PASS` |
| `cargo check --workspace` | `PASS` |
| `cargo test -p openlife-tauri single_system -- --nocapture` | `PASS`, 39 passed |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | `PASS`, 30 passed |
| `corepack pnpm --dir frontend typecheck` | `PASS` |
| `corepack pnpm --dir frontend format:check` | `PASS` |
| `corepack pnpm --dir frontend test` | `PASS`, 43 files / 520 tests |
| `corepack pnpm --dir frontend build` | `PASS` |
| isolated `make dev` product start | `PASS` |

The startup used a temporary `OPENLIFE_DATA_DIR`, explicit permission for that
isolated dev profile, `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=0`, no A2A
autostart, and a temporary Vite port. The Tauri binary ran, Vite listened, and
the isolated stores were created; all processes and temporary data were then
removed. This is a local startup check, not external live-provider evidence.

## Required Pre-4A Gate

```text
PHASE3F_HUMAN_APPROVAL = YES
DESIGN_ASSETS_TRACKED_AND_REVIEWED = YES
DESIGN_AUTHORITY_COMMIT = beade1985b41
REMOTE_STATE_FETCHED_AND_CLASSIFIED = YES
CONVERGENCE_MERGED_TO_MAIN = YES
MAIN_CI_GREEN = YES
MAIN_REVERIFIED = YES
FRONTEND_REFACTOR_READY = YES
DESIGN_AUTHORITY_MERGED_TO_MAIN = NO
PHASE4A_BRANCH_CREATED_FROM_VERIFIED_MAIN = NO
```

Protected remote `main` is the only long-term product authority. The next work
is to publish and review the scoped design-authority commit, merge it through
protected main, then fetch and reverify the resulting `origin/main`. No Phase 4A
branch or contract implementation is allowed before that final gate.

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
