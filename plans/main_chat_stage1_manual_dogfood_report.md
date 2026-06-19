# Main Chat Stage 1 Manual Dogfood Report

> Date: 2026-06-19
> Status: automated Stage 1 engineering dogfood passed in Linux CI; manual dogfood not attempted

## Summary

Manual Stage 1 dogfood has not been run in this implementation pass. Manual
review remains a separate internal-trial activity.

The automated deterministic Stage 1 engineering dogfood gate has now passed on
the supported Linux Tauri WebDriver path in GitHub Actions. The local macOS
environment still correctly fails closed because official Tauri desktop
WebDriver support is available on Windows and Linux, not macOS WKWebView.

The current automated Stage 1 engineering dogfood recommendation is:

```text
ready_for_engineering_dogfood
```

This is not a `ready_for_internal_trial` recommendation, because the manual
dogfood protocol below has not been completed.

## Automated CI Evidence

- Workflow: `Stage 1 Tauri Dogfood`
- Run id: `27807633105`
- Result: `success`
- Evidence source: `tauri_command_surface_browser_observed`
- Browser environment ready: `true`
- Smoke passed: `true`
- Observed scenarios: `36`
- Passed journeys: `36`
- Failed journeys: `0`
- Blockers: `[]`
- Browser report digest:
  `bytes:25422 hash:sha256:b53415fe64b623298be32b93fe55d3c45b7941c65d94e1ce6f3c716db8ade678`

The same CI run also passed the Stage 1 Rust dogfood gate tests and the isolated
report command test. The companion pull-request CI run `27807633186` completed
successfully.

## Required Manual Scope

Before internal trial, reviewers still need to run:

- every P0 Stage 1 scenario;
- at least 8 P1 scenarios;
- at least 4 seeded task-control scenarios;
- at least 3 memory/proposal scenarios;
- at least 2 plan scenarios;
- at least 3 failure/recovery scenarios.

## Remaining Non-Default Items

- `manual_dogfood_not_attempted`
- `ready_for_internal_trial_not_claimed`
- `external_live_provider_not_attempted_opt_in_separate`
- `local_macos_tauri_webdriver_unsupported_fail_closed`

## Notes

External live provider dogfood remains opt-in and separate from default
deterministic readiness. No provider credential was used for this report.
The local macOS fail-closed browser evidence must not be used as successful
browser readiness evidence. The successful browser evidence for Stage 1 default
readiness is the Linux CI artifact listed above.

Stage 1 must not be presented as internal-trial-ready until the manual protocol
is completed.

Reference: https://v2.tauri.app/develop/tests/webdriver/
