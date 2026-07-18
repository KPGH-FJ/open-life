# Phase 3F Human Approval And Pre-4A Handoff

Status: `PRE4A_GATE_COMPLETE_AWAITING_START_DECISION`
Date: 2026-07-19

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

| Check                           | Result | Run           |
| ------------------------------- | ------ | ------------- |
| CI                              | `PASS` | `29642938351` |
| Step 6 Tauri Product Acceptance | `PASS` | `29642938335` |
| Stage 1 Tauri Dogfood           | `PASS` | `29642938370` |

The deleted convergence remote branch is not a second authority. The fetched
`origin/main` merge commit is the sole code baseline.

## Local Reverification Of Merged Main

An isolated detached worktree at exact `origin/main` commit `79f6138` produced:

| Gate                                                                   | Result                       |
| ---------------------------------------------------------------------- | ---------------------------- |
| clean worktree and `git diff --check`                                  | `PASS`                       |
| `cargo fmt --check`                                                    | `PASS`                       |
| `cargo check --workspace`                                              | `PASS`                       |
| `cargo test -p openlife-tauri single_system -- --nocapture`            | `PASS`, 39 passed            |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | `PASS`, 30 passed            |
| `corepack pnpm --dir frontend typecheck`                               | `PASS`                       |
| `corepack pnpm --dir frontend format:check`                            | `PASS`                       |
| `corepack pnpm --dir frontend test`                                    | `PASS`, 43 files / 520 tests |
| `corepack pnpm --dir frontend build`                                   | `PASS`                       |
| isolated `make dev` product start                                      | `PASS`                       |

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

The isolated test-only correction was merged through PR #52. It reproduced the
failure under `TZ=UTC`, now derives the fixture date from the same explicit
`+08:00` zone, and leaves production timing validation unchanged. Its pull
request matrix was green, and the resulting protected-main merge commit
`a58f4e2` passed Linux, macOS, Windows, Rust coverage, security audit,
frontend, Smoke Test, Stage 1 dogfood, and Step 6 acceptance.

The earlier PR #51 run also recorded one coverage-only wall-clock assertion
above 500 ms in `total_response_duration_is_bounded_even_when_chunks_keep_arriving`.
Although it passed locally and in the next fixed-main coverage run, it recurred
in PR #54. While validating the first correction, coverage then exposed the
same class of sub-second observation assumption in
`hanging_provider_records_local_adapter_start_without_inventing_terminal_truth`.
The repetition established a test-determinism defect rather than a one-off
runner-load event.

PR #55 changed only code inside Rust test modules. It replaced both
coverage-sensitive wall-clock assumptions with five-second test watchdogs while
retaining the product-owned assertions: the request must return its configured
timeout error, provider start must be observable, the hanging execution must
not finish, and abort must not invent a terminal receipt. Production request
timeouts, scheduler behavior, receipt semantics, and durable-write authority
were not changed. PR #55 passed all ten checks, merged as `974b416`, and its
protected-main CI run `29660063950` passed all eight jobs, including Rust
Coverage and Smoke Test.

```text
DESIGN_AUTHORITY_PR = https://github.com/KPGH-FJ/open-life/pull/51
DESIGN_AUTHORITY_PR_CI = PASS_8_OF_8_AT_fdd87fc
DESIGN_AUTHORITY_MERGED = YES_AT_8b3e493
DESIGN_MAIN_PUSH_CI = PASS_RUN_29655484846
MAINLINE_TIMEZONE_FIX_PR = https://github.com/KPGH-FJ/open-life/pull/52
MAINLINE_TIMEZONE_FIX_CI = PASS_10_OF_10
MAINLINE_TIMEZONE_FIX_MERGED = YES_AT_a58f4e2
FIXED_MAIN_PUSH_CI = PASS_RUN_29653861700
COVERAGE_TIMING_FIX_PR = https://github.com/KPGH-FJ/open-life/pull/55
COVERAGE_TIMING_FIX_CI = PASS_10_OF_10
COVERAGE_TIMING_FIX_MERGED = YES_AT_974b416
COVERAGE_TIMING_FIXED_MAIN_CI = PASS_RUN_29660063950
```

Targeted local checks against exact fixed main `a58f4e2` also passed:

| Gate                                            | Result               |
| ----------------------------------------------- | -------------------- |
| clean detached worktree and `git diff --check`  | `PASS`               |
| `cargo fmt --check`                             | `PASS`               |
| both transient-state regressions under `TZ=UTC` | `PASS`               |
| exact network total-response timeout test       | `PASS`, 0.12 seconds |

Targeted local checks after merging PR #55 into the handoff branch also
passed:

| Gate                                                           | Result               |
| -------------------------------------------------------------- | -------------------- |
| `cargo fmt --check`                                            | `PASS`               |
| exact network total-response timeout regression                | `PASS`, 0.12 seconds |
| exact hanging-provider start-observation regression            | `PASS`, 0.01 seconds |
| `cargo test -q -p openlife-core --lib` on the PR #55 candidate | `PASS`, 1,478 passed |

## Design Authority Mainline Closeout

The refreshed Phase 3 design authority passed all eight PR checks at
`fdd87fc`, including Linux, macOS, Windows, Rust coverage, frontend coverage,
security audit, and Smoke Test. PR #51 was merged as protected-main commit
`8b3e493` at `2026-07-18T18:14:45Z`. Its main push CI run `29655484846` also
passed all eight checks.

The merge commit and the locally verified PR candidate have identical file
trees. Local candidate verification included `TZ=UTC cargo test -p
openlife-tauri --lib` with 1,172 passed, zero failed, and 13 ignored, plus the
Phase 3E and Phase 3F data and interaction validators. PR #51 itself remained
strictly docs-only: 108 changed files under the Phase 3D, 3E, and 3F
directories, with no `frontend/src`, `src-tauri`, or `openlife-core` diff.

## Required Pre-4A Gate

```text
PHASE3F_HUMAN_APPROVAL = YES
DESIGN_ASSETS_TRACKED_AND_REVIEWED = YES
DESIGN_AUTHORITY_COMMIT = beade1985b41
REMOTE_STATE_FETCHED_AND_CLASSIFIED = YES
CONVERGENCE_MERGED_TO_MAIN = YES
MERGED_MAIN_PUSH_CI = PASS_AT_974b416
MAIN_REVERIFIED = YES_AT_974b416_CI_AND_TARGETED_TESTS
CURRENT_MAIN_CI_STABILITY = PASS
FRONTEND_REFACTOR_READY = YES
DESIGN_AUTHORITY_MERGED_TO_MAIN = YES_AT_8b3e493
PRODUCTION_COMPILED_PATH_MODIFIED = NO
TEST_ONLY_RUST_SOURCE_MODIFIED = YES_FOR_CI_DETERMINISM
PHASE4A_BRANCH_CREATED_FROM_VERIFIED_MAIN = NO
PHASE4A_START_DECISION = PENDING_USER_APPROVAL
```

Protected remote `main` is the only long-term product authority. The next work
is a user decision on whether to start Phase 4A from a newly created branch at
the then-current verified main. This readiness state authorizes Contract
Closure planning and implementation only after that decision. It does not make
the rich Review or exact Permission presentation contracts complete, and it
does not authorize a React page migration yet.

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
