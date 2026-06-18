# Main Chat Stage 1 Manual Dogfood Report

> Date: 2026-06-18
> Status: blocked in this environment; manual dogfood not attempted

## Summary

Manual Stage 1 dogfood has not been run in this implementation pass.

Automated deterministic Rust evidence can exercise the Stage 1 command and
control paths. The Playwright harness now preserves the blocked non-Tauri path
and includes a Tauri-capable branch that must drive the real Chat composer and
visible task controls before it can write passing browser evidence. That branch
also calls an explicit Stage 1 browser-prep command to create real
task-continuity state in the same AppState the Chat UI reads; this prep report is
not pass evidence. Both Tauri-capable browser paths now reject prep reports that
contain prep blockers, direct writes, durable LifeModel writes, file/external
writes, or missing seeded task ids for D13, D14, D15, D19, D20, D27, and D28.
This environment only exercised the non-Tauri Chromium path, which correctly
wrote `not_ready_browser_e2e_blocked`.
The checked-in Playwright command starts the Vite dev server only, so it is
intentionally a blocked non-Tauri smoke in this environment. Its blocked branch
now preflights the Tauri WebDriver prerequisites that a supported-platform run
would need: `tauri-driver`, the native WebDriver binary, and the debug Tauri app
binary. The checked-in `pnpm --dir frontend test:e2e:tauri` entrypoint performs
the same fail-closed preflight and writes blocked browser evidence instead of
pass evidence when the Tauri WebDriver environment is unavailable. On a
supported platform, that entrypoint starts `tauri-driver`, creates a real `wry`
WebDriver session with the debug Tauri app binary, starts or reuses the Vite
dev server at `http://127.0.0.1:5173`, navigates to Chat, calls the Stage 1
browser-prep command, and drives the shared D01-D36 matrix through real composer
and visible-control DOM interactions. It uses native input/textarea value
setters so React controlled composer and selected-skill state are updated by the
WebDriver path, and selected-skill scenarios now fail with explicit browser
blockers if the `SKILL.md` composer field is missing or does not retain the
requested selected skill id. It reads control-plane/task-continuity DOM evidence
plus Tauri runtime snapshots, task details, and event streams to map each row
back to the expected UI states, visible blockers, final delivery sections,
task/session/run ids, and visible-control events, writes browser evidence to
the repository-level `frontend/test-results/main-chat-stage1-dogfood-report.json`
path consumed by the Rust gate regardless of launch working directory, and it
rejects mismatched entry points, unsafe labels, placeholder task/run identities,
and reused browser task/run identities before writing a pass report. For the
icon-only task-continuity buttons, the Tauri-capable Playwright branch and
standalone WebDriver runner both preserve visible text, `aria-label`, and
`title` before recording visible-control events. Seeded visible-control
validation uses the same scenario-specific prefix matching as the Rust gate, so
real button labels that normalize to expected control-event variants are not
rejected by the browser harnesses. It also re-runs
`run_main_chat_agent_stage1_dogfood_gate` after writing any pass report and
requires `ready_for_engineering_dogfood` before the runner can exit
successfully; it still fails closed unless all D01-D36 observations complete
and the Rust gate accepts the browser evidence. If that final gate rejects the
report, the runner emits an explicit final-gate blocker instead of treating the
failure as successful browser evidence. The
official Tauri WebDriver docs state that desktop WebDriver support is available
only on Windows and Linux because macOS does not have a WKWebView driver tool.
The repository now includes a Linux GitHub Actions workflow at
`.github/workflows/stage1-tauri-dogfood.yml` for the supported-platform path:
it installs the Linux WebKit/WebDriver dependencies, installs `tauri-driver`,
builds the debug Tauri app binary, runs `pnpm --dir frontend test:e2e:tauri`
under Xvfb, asserts that the browser report is real
`tauri_command_surface_browser_observed` evidence with 36 observed scenarios,
reruns the Stage 1 Rust gate tests, and uploads the browser report artifact.
That CI path has not been executed in this environment. The real Tauri Chat UI
D01-D36 pass still needs the Linux CI run, another supported Tauri/WebDriver
runner, or a manual dogfood run.

The current recommendation is:

```text
not_ready
```

This is not a `ready_for_internal_trial` recommendation.

## Required Manual Scope

Before internal trial, reviewers still need to run:

- every P0 Stage 1 scenario;
- at least 8 P1 scenarios;
- at least 4 seeded task-control scenarios;
- at least 3 memory/proposal scenarios;
- at least 2 plan scenarios;
- at least 3 failure/recovery scenarios.

## Current Blockers

- `manual_dogfood_not_attempted`
- `not_ready_browser_e2e_blocked`
- `real_tauri_chat_ui_d01_d36_not_executed_in_this_environment`
- `real_tauri_browser_prep_and_visible_control_path_not_executed_in_this_environment`
- `stage1_linux_ci_tauri_dogfood_not_run_yet`
- `tauri_webdriver_environment_not_ready`
- `playwright_default_runner_is_vite_only`
- `tauri_webdriver_macos_not_supported_by_tauri_driver`
- `external_live_provider_not_attempted_opt_in_separate`

## Notes

External live provider dogfood remains opt-in and separate from default
deterministic readiness. No provider credential was used for this report.
The blocked web Playwright smoke must not be used as successful browser
readiness evidence. Stage 1 must not be presented as internal-trial-ready until
a real Tauri Chat UI run observes D01-D36, including the prep-backed visible
task-control rows, and the manual protocol is completed.

Reference: https://v2.tauri.app/develop/tests/webdriver/
