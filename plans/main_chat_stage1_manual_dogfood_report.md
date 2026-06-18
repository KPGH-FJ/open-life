# Main Chat Stage 1 Manual Dogfood Report

> Date: 2026-06-18
> Status: not attempted

## Summary

Manual Stage 1 dogfood has not been run in this implementation pass.

Automated deterministic Rust evidence can exercise the Stage 1 command and
control paths, but true Tauri browser E2E evidence is not currently available.
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
- `external_live_provider_not_attempted_opt_in_separate`

## Notes

External live provider dogfood remains opt-in and separate from default
deterministic readiness. No provider credential was used for this report.
The blocked web Playwright smoke must not be used as successful browser
readiness evidence.
