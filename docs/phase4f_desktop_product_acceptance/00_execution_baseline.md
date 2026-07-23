# Phase 4F Desktop Product Acceptance Baseline

Status: `EXECUTED_WITH_BLOCKERS`
Date: 2026-07-21

## Entry Gate

- Human approval of Phase 4E: `YES`.
- Phase 4E merged to `main`: PR #63.
- Verified base SHA: `7a167f4e50584524586c2350882e43df01b0da2b`.
- Local post-merge frontend, Rust authority, formatting, build, and diff gates:
  `PASS`.
- Main CI run `29811958822`, attempt 2: `SUCCESS`.
- Phase 4F branch was created directly from that exact SHA.

The first CI attempt had one Windows-only MCP test-fixture handshake timeout.
The same source passed in PR #63, the exact focused test passed locally, and the
complete Windows job passed on attempt 2. No product or backend behavior is
credited from the retry; the event remains recorded as an intermittent CI
observation.

## Scope

Phase 4F validates the one production desktop Workbench and repairs only defects
found through source-backed or real product evidence. It may:

- repair migrated frontend access to an already shipped backend capability;
- add focused contract, interaction, accessibility, and absence tests;
- run the packaged Tauri application with an isolated QA data directory;
- record screenshots, keyboard/focus results, VoiceOver results, logs, and
  explicit blockers;
- update Phase 4 evidence and the Phase7 trial result without weakening its
  `red-until-trial-green` rule.

## Non-Goals

- no mobile UI, mobile viewport, or mobile acceptance;
- no second shell, V2 route, fixture route, or old frontend fallback;
- no new backend business authority or changed durable-write semantics;
- no page-local reconstruction of readiness, Safe Mode, provider/privacy,
  proposal, task, or materialization truth;
- no claim that an unavailable live provider, permission proposal, or durable
  application path passed;
- no secret material in screenshots, logs, reports, or Inspector data.

## Acceptance Order

1. canonical route and shell startup;
2. Safe Mode and fail-closed recovery access;
3. governed permission/review/refresh/resume journey when backend state exists;
4. durable proposal/decision/application journey when backend actions exist;
5. Settings test/save/boundary refresh within explicit privacy limits;
6. keyboard, focus, contrast, VoiceOver, release, authority, and Phase7 gates.

Unavailable prerequisites are recorded as `UNKNOWN` or `BLOCKED`, never as
fixture-backed product completion.

`PHASE4F_DESKTOP_ONLY=YES`

`PHASE4F_BRANCH_FROM_VERIFIED_MAIN=YES`

`PHASE7_TRIAL_GREEN=NO`
