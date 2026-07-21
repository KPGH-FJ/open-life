# Phase 4F Native Desktop Trial Report

Status: `PASS_WITH_BLOCKED_AND_FAILED_ACTION_EVIDENCE`
Date: 2026-07-21

## Runtime Boundary

The production-config debug bundle was built and launched from
`target/debug/bundle/macos/OpenLife.app` with `OPENLIFE_PROFILE=qa`, an isolated
`OPENLIFE_DATA_DIR`, and A2A development autostart disabled. The product used
the shipped Tauri entry and canonical routes; no Vite harness, fixture selector,
preview route, old page, or mobile surface was used.

The final rebuilt artifact is ad-hoc signed with cdhash
`9b021cbef41385690df6140c3e103bfb112fe5b0`; it launches successfully and
continues to consume the fail-closed backend Safe Mode projection.

No real provider credential was entered. No provider test, network request,
settings save, review decision, or durable write was executed. The internal
credential recovery command was executed only after explicit user approval;
the frontend received metadata statuses and no key material.

## Route Results

| Route | Native result | Truth boundary |
| --- | --- | --- |
| `/today` | Safe Mode, no confirmed focus | stale/missing state remained read-only; privacy remained unknown |
| `/workspace` | state unavailable | missing Workspace state was not rendered as no task or completion |
| `/tasks` | read failed | count remained unknown rather than zero |
| `/review` | no current review items | empty did not mean all approved or applied |
| `/life-model` | current and Memory summaries absent | no old-object reconstruction; Builder failure created no proposal or write |
| `/settings` | sanitized config rendered, boundary unknown, Safe Mode active | secret field omission did not crash; no local/private certainty was inferred |

## Interaction Results

- All six canonical routes produced visible route feedback and moved focus to
  the route heading.
- Inspector open focused its heading; close returned focus to the trigger.
- Settings search reduced seven categories to the one matching `API 凭据`.
- Unmigrated Settings categories rendered an explicit unavailable state with
  working return actions rather than no-op controls.
- Safe Mode recovery remained reachable even though durable operations were
  blocked. Opening it showed exact scope before any command was invoked.
- Keyboard opening and Escape cancellation of the recovery dialog returned
  focus to `检查系统凭据` and announced that no credential access occurred.
- LifeModel Builder was attempted against the real backend and returned
  `Database:read_only_degraded`; the product surface stayed plain-language and
  the Inspector retained only the bounded diagnostic code.
- Authorized credential recovery returned all four purposes as available in
  the interactive process. The current session correctly stayed in Safe Mode.
- After a complete restart, the backend still reported the AgentRun receipt
  key unavailable. The UI remained fail-closed and did not treat the recovery
  report as durable proof.

## Journey Credit

| Journey | Result | Reason |
| --- | --- | --- |
| Shell and canonical routes | `PASS` | packaged production Tauri UI exercised |
| stale/unknown/error fail-closed | `PASS` | observed on Today, Workspace, Tasks, LifeModel, and Settings |
| approved versus applied | `PASS_AT_PRESENTATION_BOUNDARY` | empty-state and automated lifecycle tests preserve separation; no real approved item existed |
| permission -> review -> refresh -> resume | `BLOCKED` | isolated backend had no exact pending permission ReviewItem/task pair |
| proposal -> decision -> application | `BLOCKED` | Safe Mode blocked Builder start and no exact durable proposal existed |
| provider test -> save -> boundary refresh | `BLOCKED` | no test credential or authorized external transmission was supplied |
| credential recovery -> restart -> recheck | `FAILED_FAIL_CLOSED` | interactive reads succeeded, but the old Keychain ACL did not authorize the current ad-hoc bundle on non-interactive restart |

Blocked journeys are not credited from fixtures or Rust unit tests.

## Artifacts

- `artifacts/01-native-today-safe-mode.png`
- `artifacts/02-native-today-evidence-inspector.png`
- `artifacts/03-native-tasks-empty.png`
- `artifacts/04-native-review-center.png`
- `artifacts/05-native-lifemodel-safe-mode.png`
- `artifacts/06-native-settings-render-failure.png`
- `artifacts/07-native-settings-repaired.png`
- `artifacts/08-native-credential-recovery-confirmation.png`
- `artifacts/09-native-settings-unavailable.png`
- `artifacts/10-native-lifemodel-object-error-defect.png`
- `artifacts/11-native-lifemodel-fail-closed.png`
- `artifacts/12-native-keyboard-focus-ring.png`
- `artifacts/13-native-credential-recovery-report.png` (focused report capture)
- `artifacts/14-native-persistent-credential-authorization.png` (authorized
  attempt before the result-copy correction)
- `artifacts/15-native-restart-safe-mode-persists.png`

Images 01-12 and 14-15 are `1228x768`; the focused report capture 13 is
`598x374`. Screenshots contain no entered credential or secret material.

`REAL_TAURI_SHELL=PASS`

`REAL_TAURI_FAIL_CLOSED=PASS`

`REAL_TAURI_GOVERNED_ACTION_E2E=BLOCKED`

`REAL_TAURI_DURABLE_APPLICATION_E2E=BLOCKED`

`REAL_TAURI_EXTERNAL_PROVIDER_E2E=BLOCKED`

`REAL_TAURI_CREDENTIAL_RECOVERY_E2E=FAILED_FAIL_CLOSED`
