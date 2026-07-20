# Phase 4D Read-Only Spine QA Report

Status: `PASS_WITH_BOUNDED_REAL_TAURI_LIMITATION`
Date: 2026-07-20

## Scope

This report covers the desktop-only Shell + Today + Tasks read-only journey in
the isolated Phase 4D harness. It does not cover mobile, production routing,
task controls, review decisions, configuration writes, or durable state writes.

## Automated Gates

| Gate | Result |
| --- | --- |
| `git diff --check` | pass |
| `cargo fmt --check` | pass |
| targeted Phase 4D Rust authority guard | 1 passed |
| `cargo test -p openlife-tauri single_system -- --nocapture` | 44 passed |
| frontend typecheck and format check | pass |
| full frontend tests | 55 files, 574 tests passed |
| production frontend build and release absence scan | pass |
| Phase 4D dev build | pass |
| Phase 4D browser QA | 97 assertions passed |
| Phase 4D Tauri package/build attempt | expected rejection from `beforeBuildCommand` |

The full frontend suite emitted existing React Router v7 future warnings and
intentional error-path test logs. No test failed. Browser QA recorded no console
error or warning.

## Desktop Browser QA

The browser harness used typed fixtures outside the product Shell. Fixture
values were visibly labeled non-backend state.

| Viewport | Horizontal overflow | Sidebar | Context bar | Inspector | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| 1440x900 | 0px | 232px | 56px | 344px | pass |
| 1280x800 | 0px | 232px | 56px | 344px | pass |
| 1024x720 | 0px | 232px | 56px | 344px | pass |

Body reading text measured 15px and metadata 12px. Task rows did not overflow.
The three non-harness HTML paths returned 404 and did not load the product App.

Measured text contrast ratios were 18.88:1, 7.51:1, 5.74:1, 5.90:1,
6.35:1, and 4.83:1 across the tested semantic pairs. Control boundary and
focus contrast measured 3.69:1 and 5.17:1.

Interaction checks covered:

- keyboard navigation and visible focus;
- one `aria-current="page"` product destination;
- search, filter, row selection, and Inspector focus;
- Inspector close focus restoration;
- Settings context and Back focus restoration;
- unavailable Workspace, Review, LifeModel, and Settings feedback;
- stale/unknown boundary states never rendering verified green;
- Review viewing never becoming approval or application;
- `error + empty payload` never becoming a confirmed zero-task state;
- product action `id`, `kind`, `enabled`, `disabledReason`, and `targetRef`;
- absence of Review actions, task controls, and durable/external effects.

## Real Tauri Dogfood

The dev-only Tauri overlay started against a fresh isolated
`OPENLIFE_DATA_DIR`. The QA selector was set to `真实 Tauri 后端`; no browser
fixture was used. The temporary data directory and process were removed after
capture.

Observed command probe:

```text
Today stale
Tasks error [main_chat_task_summaries_unavailable, agent_run_store_unavailable]
```

Observed UI behavior:

- Today displayed amber Safe Mode protection and did not claim a fresh state;
- provider/privacy stayed `是否外传未知`, never green local certainty;
- Workspace remained disabled with a refresh requirement;
- Tasks displayed an unknown-count read failure, not `0 items` or a normal
  empty list;
- the Inspector exposed source metadata and limitations without enabling task
  controls;
- raw startup reasons and DTO fields stayed in Inspector/QA surfaces rather
  than the product work surface.

This is valid real-command and fail-closed evidence. It is not proof that the
isolated backend can currently return Today `ready` or Tasks `ready/empty`.
Source tracing shows the observed warnings are consistent with Safe Mode and
unavailable canonical task/AgentRun stores. Credential recovery was not invoked:
it is a confirmed, user-initiated security action and is outside this read-only
slice.

## Artifacts

- `artifacts/phase4d-browser-qa.json`
- `artifacts/phase4d_1440x900_today_ready.png`
- `artifacts/phase4d_1440x900_tasks_inspector.png`
- `artifacts/phase4d_1280x800_today_ready.png`
- `artifacts/phase4d_1280x800_tasks_inspector.png`
- `artifacts/phase4d_1024x720_today_ready.png`
- `artifacts/phase4d_1024x720_tasks_inspector.png`
- `artifacts/phase4d_1440x900_today_stale.png`
- `artifacts/phase4d_1440x900_tasks_error_fail_closed.png`
- `artifacts/phase4d_1440x900_review_unavailable.png`
- `artifacts/phase4d_real_tauri_today_safe_mode.png`
- `artifacts/phase4d_real_tauri_tasks_error_inspector.png`

## QA Conclusion

The desktop frontend slice is ready for human review. The real Tauri limitation
must remain explicit and should be rechecked after a reviewed, user-authorized
credential recovery or another approved environment can expose healthy stores.
It does not justify weakening the backend envelope or frontend fail-closed
rules.
