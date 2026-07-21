# Phase 4D Durable-Truth QA Report

Status: `PASS_WITH_CONTRACT_LIMITS`
Date: 2026-07-21

## Automated Contract Evidence

- durable data-source/presentation/journey tests: `10/10` passed;
- complete Phase 4D focused suite: `49/49` passed;
- complete frontend suite: `605/605` passed;
- previous Phase 4D desktop browser regression: `165` assertions passed;
- durable desktop browser QA: `60` assertions passed;
- browser console/page errors: `0`;
- Phase 4D build: passed;
- production build and Phase 4B/4C/4D absence guard: passed;
- TypeScript typecheck: passed;
- Rust format check: passed;
- complete Rust `single_system` authority suite: `44/44` passed;
- Phase 4D Rust source-level authority guard: `1/1` passed.

The durable QA report is
`artifacts/phase4d-durable-browser-qa.json`.

## Desktop Viewport Evidence

| Viewport | Horizontal overflow | Sidebar | Context bar | Reading text | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| `1440x900` | `0px` | `232px` | `56px` | `15px` | PASS |
| `1280x800` | `0px` | `232px` | `56px` | `15px` | PASS |
| `1024x720` | `0px` | `232px` | `56px` | `15px` | PASS |

Screenshots cover pending and approved-not-applied at every viewport, plus the
1440px applying, applied, failed, and rolled-back matrix.

## Interaction Evidence

The browser QA verified:

1. the fixture selector remains outside the product Shell;
2. LifeModel is the single current navigation item;
3. current understanding, selected change, lifecycle, and Memory summary have
   stable desktop hierarchy;
4. `查看并决定` carries an open action id/kind/target and does not change state;
5. approval requires confirmation and then refreshes the exact ReviewItem;
6. returning to LifeModel reloads all durable owners;
7. approved-not-applied exposes a disabled backend Apply action and reason;
8. only exact applied proof receives green verified treatment;
9. applying, failed, rolled-back, stale, and error remain non-green and
   distinct;
10. Inspector exposes structured evidence metadata and raw identities after
    the product explanation;
11. LifeModel navigation and the primary review entry are reachable with a
    visible keyboard focus ring.

## Visual Review

- The work surface uses three unframed sections and thin dividers rather than
  a card dashboard.
- At 1024px the dimension list becomes one column; content continues vertically
  without horizontal compression or overlap.
- Amber communicates waiting/protective states; red is reserved for failure;
  green is limited to the exact applied proof and the separately verified local
  provider boundary.
- The QA toolbar is visibly outside the Shell and labels all fixtures as
  non-backend state.

## Real Tauri Read-Model Probe

The final isolated Tauri run used:

```sh
frontend/node_modules/.bin/tauri dev --config src-tauri/tauri.phase4d.conf.json
```

Observed result:

```text
Today=stale
Tasks=error
Workspace=error
Review=empty
LifeModel=empty
Memory=empty
DurableReview=empty
durable diagnostics: all three commands loaded
```

This proves the dev-only Tauri entry can call the three durable read-model
commands and preserves their empty state. It does **not** prove a real pending
durable proposal, approval, application, failure, or rollback flow. No real
decision or write command was triggered.

The first launch attempt used the frontend working directory and failed Tauri
project discovery. The second reached the correct command but found an existing
Phase 4D Vite process on port 4186. After verifying that process belonged to
this repository and stopping it, the clean root invocation above succeeded.

## Contract Limits

- `LifeModelViewModel` remains a `current_compatibility`/limited product view in
  the available fixture and cannot be described as complete canonical truth.
- `MemoryViewModel` does not project exact record details or a typed rollback
  action.
- the current Review apply action is disabled because no callable backend
  materialization request command is available for the item.
- fixture state transitions validate frontend sequencing only.
- production routes and page owners remain unchanged.

`REAL_DURABLE_ACTION_E2E=NO`

`FAIL_CLOSED_QA=PASS`

`APPROVED_DISTINCT_FROM_APPLIED=PASS`
