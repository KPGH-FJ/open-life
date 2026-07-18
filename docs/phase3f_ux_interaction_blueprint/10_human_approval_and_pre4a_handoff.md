# Phase 3F Human Approval And Pre-4A Handoff

Status: `APPROVED_DIRECTION_MAIN_FIX_PENDING`
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

## Post-Publication CI Finding

Design-authority PR #51 later ran the unchanged Rust baseline in a different
UTC time window. Linux and macOS both rejected two transient-state test
fixtures with `state_daily_task_due_at_out_of_range`. The fixtures encoded
`+08:00` while deriving their date from the runner-local zone, so UTC runners
after 16:00Z crossed the fixture's 24-hour TTL. The production StateStore
correctly failed closed; the tests were inconsistent.

The isolated test-only correction is draft PR #52. It reproduces the failure
under `TZ=UTC`, derives the fixture date from the same explicit `+08:00` zone,
and leaves production timing validation unchanged. Its complete remote matrix
is green: Linux, macOS, Windows, Rust coverage, security audit, frontend,
Smoke Test, Stage 1 dogfood, and Step 6 acceptance all passed. Until PR #52 is
merged and protected main is fetched and reverified, the current readiness gate
must remain closed.

```text
DESIGN_AUTHORITY_PR = https://github.com/KPGH-FJ/open-life/pull/51
DESIGN_AUTHORITY_PR_CI = BLOCKED_BY_MAINLINE_TIMEZONE_FIX
MAINLINE_TIMEZONE_FIX_PR = https://github.com/KPGH-FJ/open-life/pull/52
MAINLINE_TIMEZONE_FIX_CI = PASS_10_OF_10
MAINLINE_TIMEZONE_FIX_MERGED = NO
```

## Required Pre-4A Gate

```text
PHASE3F_HUMAN_APPROVAL = YES
DESIGN_ASSETS_TRACKED_AND_REVIEWED = YES
DESIGN_AUTHORITY_COMMIT = beade1985b41
REMOTE_STATE_FETCHED_AND_CLASSIFIED = YES
CONVERGENCE_MERGED_TO_MAIN = YES
MERGED_MAIN_PUSH_CI = PASS_AT_79f6138
LOCAL_MAIN_REVERIFICATION = PASS_AT_79f6138
CURRENT_MAIN_CI_STABILITY = BLOCKED_BY_PR_52
FRONTEND_REFACTOR_READY = NO
DESIGN_AUTHORITY_MERGED_TO_MAIN = NO
PHASE4A_BRANCH_CREATED_FROM_VERIFIED_MAIN = NO
```

Protected remote `main` is the only long-term product authority. The next work
is to review and merge PR #52, fetch and reverify protected main, then refresh,
review, and merge PR #51 before one final mainline revalidation. No Phase 4A
branch or contract implementation is allowed before those gates.

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
