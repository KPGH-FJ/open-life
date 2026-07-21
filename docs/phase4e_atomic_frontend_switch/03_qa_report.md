# Phase 4E QA Report

Date: 2026-07-21
Scope: production desktop Workbench authority switch

## Automated Gates

| Gate | Result |
| --- | --- |
| frontend TypeScript typecheck | pass |
| frontend format check | pass |
| frontend Vitest suite | pass, 35 files / 260 tests |
| production frontend build and absence guard | pass |
| Rust `single_system` authority suite | pass, 44 tests |
| Rust `main_chat_runtime_module` suite | pass, 30 tests |
| `cargo fmt --check` | pass |
| `git diff --check` | pass |

The final rerun commands and exact totals are recorded again in the Phase 4E
summary after the branch is ready for review.

## Production Browser QA

The browser target was the actual production Vite entry at
`http://127.0.0.1:4187/`, not the Phase 4D fixture harness. A normal browser has
no Tauri IPC, so these checks earn layout, route, accessibility-structure, and
fail-closed evidence only. They do not earn backend-truth credit.

| Viewport | Surface | Result |
| --- | --- | --- |
| 1440 x 900 | Today | Workbench and current navigation present; no overflow; failed read stays non-success |
| 1280 x 800 | Workspace | no overflow; empty payload is not interpreted as no task; only reread remains enabled |
| 1024 x 720 | Tasks | fixed 232px desktop sidebar; no overflow; failed count remains unknown rather than zero |

Additional checks:

- `/companion` stayed on the retired path, showed the explicit unavailable
  surface, and did not load the Workbench or redirect to another action;
- Review, LifeModel, and Settings each loaded their canonical route and remained
  fail closed without Tauri IPC;
- Settings made no local/private claim from default values;
- evidence inspector opened at the right edge, received focus, and restored
  focus to its trigger on close;
- one skip link and one `aria-current` item were present;
- console reported zero errors and zero warnings;
- no horizontal overflow was detected at the tested desktop widths.

Machine-readable evidence:
`docs/phase4e_atomic_frontend_switch/artifacts/phase4e-production-browser-qa.json`.

Screenshots:

- `artifacts/phase4e_production_1440x900_today_fail_closed.png`
- `artifacts/phase4e_production_1280x800_workspace_fail_closed.png`
- `artifacts/phase4e_production_1024x720_tasks_fail_closed.png`
- `artifacts/phase4e_production_1280x800_retired_route.png`
- `artifacts/phase4e_production_1280x800_today_inspector.png`

These screenshots and the machine-readable browser capture preceded a final
copy-only cleanup that removed Phase 4D/slice terminology and moved raw
ViewModel names out of the product work surface. No layout, route, color, or
control geometry changed in that cleanup. The screenshots remain layout and
fail-closed structure evidence; the final product copy is covered by the full
component suite and can be reviewed at the live production entry.

## Real Tauri Startup Probe

The production Tauri entry was launched from the repository root with a fresh
isolated `OPENLIFE_DATA_DIR`:

```sh
OPENLIFE_DATA_DIR=/tmp/openlife-phase4e-tauri.<isolated> \
  frontend/node_modules/.bin/tauri dev --config src-tauri/tauri.conf.json
```

Observed result:

- production Vite entry started at `127.0.0.1:5173`;
- `openlife-tauri` compiled and launched successfully;
- the isolated backend initialized its SQLite stores and LifeModel directory;
- no startup/runtime error was emitted before the read-only probe stopped;
- the process was stopped and the isolated directory was deleted.

The desktop automation runtime could not discover the unpackaged `tauri dev`
process as an addressable application. Therefore this is startup evidence, not
a claim that every production UI route or backend action was exercised through
the native window.

## Explicit Limits

- The in-app Browser keypress API did not advance Tab focus. Component tests
  cover the focus sequence, but complete manual keyboard traversal remains a
  Phase 4F check.
- Manual VoiceOver/screen-reader acceptance has not been performed.
- No real provider test/save, permission approval/resume, durable apply/fail/
  rollback, or live external-provider journey was triggered in this slice.
- No mobile viewport was tested because mobile is not an OpenLife product or
  backend target in the current plan.

`PRODUCTION_BROWSER_FAIL_CLOSED_QA=PASS`

`REAL_TAURI_PRODUCTION_STARTUP=PASS`

`REAL_TAURI_ACTION_E2E=NO`

`MOBILE_ACCEPTANCE_SCOPE=NOT_APPLICABLE`
