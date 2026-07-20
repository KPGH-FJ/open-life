# Phase 4D Governed-Action Spine QA Report

Status: `PASS_WITH_BOUNDED_REAL_TAURI_ACTION_LIMITATION`
Date: 2026-07-20

## Scope

This report covers the desktop-only candidate journey
`Workspace -> Permission -> Review -> Refresh -> Resume` in the isolated Phase
4D harness. It does not cover production routes, mobile, durable
materialization, or a fixture-free live approval/resume action.

## Automated Gates

| Gate | Result |
| --- | --- |
| `git diff --check` | pass |
| `cargo fmt --check` | pass |
| frontend typecheck | pass |
| focused governed/read-only tests | 6 files, 29 tests passed |
| full frontend tests | 59 files, 595 tests passed |
| targeted Rust Phase 4D authority guard | 1 passed |
| `cargo test -p openlife-tauri single_system -- --nocapture` | 44 passed |
| frontend format check | pass |
| production frontend build + release absence scan | pass |
| Phase 4D dev build | pass |
| Phase 4D browser QA | 165 assertions passed |
| Phase 4D package attempt | expected rejection from `beforeBuildCommand` |

The full frontend suite emitted existing React Router future warnings and
intentional error-path logs. No test failed. Browser QA recorded no console
error or warning.

## Browser Interaction Coverage

The browser harness uses typed fixtures selected only in the QA toolbar outside
the product Shell. Tests covered:

- opening a permission request without dispatching approval;
- ReviewAction id/kind/effect/enabled/disabledReason/target/confirmation and
  no-completion attributes;
- title-first dialog focus, visible keyboard path, focus trap, and confirmation;
- approval command followed by refreshed same-item verification;
- manual refresh after a delayed projection without replaying the prior command;
- approved permission labelled as not resumed and not completed;
- exact TaskControl identity/effect/target checks;
- resume command followed by refreshed same-task verification;
- running remaining distinct from completed;
- stale Review disabling decisions while preserving evidence access;
- incomplete permission scope disabling approval with a visible reason;
- Inspector structure and focus restoration;
- settings context/back focus restoration;
- Today/Tasks stale, error, and empty fail-closed behavior;
- production route rejection and QA selector isolation.

## Desktop Layout Results

| Viewport | Page overflow | Review overflow | Sidebar | Review queue | Inspector | Primary decision visible |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 1440x900 | 0px | 0px | 232px | 248px | 344px | yes |
| 1280x800 | 0px | 0px | 232px | 248px | 344px | yes |
| 1024x720 | 0px | 0px | 232px | 248px | 344px | yes |

The sticky Review action area keeps the primary decision visible at the minimum
desktop size while the permission details remain scrollable. Body reading text
is at least 14px and metadata at least 12px.

Measured text contrast ratios were 18.88:1, 7.51:1, 5.74:1, 5.90:1,
6.35:1, and 4.83:1 across tested semantic pairs. Control-boundary and focus
contrast measured 3.69:1 and 5.17:1.

## Real Tauri Probe

The dev-only Tauri overlay started against a fresh isolated
`OPENLIFE_DATA_DIR`. The harness selected `真实 Tauri 后端`; no browser fixture
fed the probe. A dev-only status sink returned envelope status and warning codes
only, not task/review content or sensitive values.

Observed probe:

```json
{
  "today": "stale",
  "tasks": "error",
  "workspace": "error",
  "review": "empty",
  "journeyTasks": "error",
  "diagnostics": [
    { "id": "workspace_view_model", "status": "loaded" },
    { "id": "review_center_view_model", "status": "loaded" },
    { "id": "tasks_view_model", "status": "loaded" }
  ]
}
```

Relevant task warnings were
`main_chat_task_summaries_unavailable` and `agent_run_store_unavailable`.
`diagnostics=loaded` means the Tauri commands returned envelopes; it does not
mean those envelopes were ready. The UI therefore had no real pending
ToolPermission to approve and no exact resume control to execute.

This proves real command connectivity and fail-closed composition. It does not
prove fixture-free approval, rejection, postponement, task resume, or successful
Workspace/Tasks ready state. The isolated directory was removed and the native
process stopped after the probe.

## Artifacts

Machine-readable result:

- `artifacts/phase4d-browser-qa.json`

Key screenshots:

- `artifacts/phase4d_1440x900_workspace_permission_pending.png`
- `artifacts/phase4d_1440x900_review_permission_pending.png`
- `artifacts/phase4d_1440x900_review_approved_not_resumed.png`
- `artifacts/phase4d_1440x900_workspace_resumed_running.png`
- `artifacts/phase4d_1440x900_review_incomplete_scope_blocked.png`
- `artifacts/phase4d_1440x900_today_stale.png`
- `artifacts/phase4d_1440x900_tasks_error_fail_closed.png`
- corresponding Today, Tasks, Workspace, and Review captures at 1280x800 and
  1024x720.

## Conclusion

The candidate frontend sequencing, fail-closed behavior, desktop layout, and
release isolation are ready for human review. Fixture-free governed-action
dogfood remains explicitly unproven and must be completed before production
authority can switch; this limitation does not justify inventing backend state
or weakening the refresh checks.
