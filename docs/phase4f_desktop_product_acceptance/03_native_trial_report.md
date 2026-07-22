# Phase 4F Native Desktop Trial Report

Status: `PASS_WITH_BLOCKED_AND_FAILED_ACTION_EVIDENCE`
Date: 2026-07-21

Review correction (2026-07-22): this report records the native trial performed
against the pre-review Phase 4F attempt. Credential-recovery interactions and
artifacts are `HISTORICAL-EVIDENCE`: independent review found that the attempt
used generic Safe Mode as recovery eligibility. The reviewed frontend keeps
the command unavailable until a typed backend eligibility contract exists.
Route observations are bounded observations, not exhaustive proof for every
unknown/stale/error state.

## 2026-07-22 Review-Repair Recheck

The final review-repair source is committed. Its replacement native artifact
and `/settings` observation must be rebuilt from a clean checkout before the
current row below can receive credit. The earlier `af4ba87` / `d54a130e...`
recheck is retained as intermediate historical evidence only because a later
async-generation repair changed the frontend source.

| Source SHA | Binary SHA-256 | Root executable identity | Verification | Native observation | Credit |
| --- | --- | --- | --- | --- | --- |
| `PENDING_FINAL_REBUILD` | `PENDING_FINAL_REBUILD` | `PENDING_FINAL_REBUILD` | `PENDING_FINAL_REBUILD` | `PENDING_FINAL_REBUILD` | `PENDING_FINAL_REBUILD` |

The final rebuild will use `--no-sign` intentionally and will record root
executable verification separately from copied executable and app-bundle
resource-seal verification. No intermediate artifact credit is inherited.

## Runtime Boundary

The production-config debug bundle was built and launched from
`target/debug/bundle/macos/OpenLife.app` with `OPENLIFE_PROFILE=qa`, an isolated
`OPENLIFE_DATA_DIR`, and A2A development autostart disabled. The product used
the shipped Tauri entry and canonical routes; no Vite harness, fixture selector,
preview route, old page, or mobile surface was used.

The earlier pre-review rebuild had cdhash
`9b021cbef41385690df6140c3e103bfb112fe5b0`. It remains historical identity
evidence only and is not the artifact credited by the review-repair recheck.

The canonical development entry was then checked separately. The first
invocation correctly rejected an inherited custom `OPENLIFE_DATA_DIR`; after
that variable was removed, `make dev` used `OPENLIFE_PROFILE=dev` and
`~/Library/Application Support/ai.openlife.app.dev`. Two complete launches
both returned to the same backend Safe Mode. This proves the unresolved
credential boundary is not limited to the packaged artifact. It does not prove
that formal distribution signing is required for current product development.

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

- In this recorded trial, all six canonical routes produced visible route
  feedback and moved focus to the route heading.
- Inspector open focused its heading; close returned focus to the trigger.
- Settings search reduced seven categories to the one matching `API 凭据`.
- Unmigrated Settings categories rendered an explicit unavailable state with
  working return actions rather than no-op controls.
- The rejected recovery attempt remained reachable from generic Safe Mode.
  Opening it showed exact scope before any command was invoked, but review later
  proved that the eligibility premise was too broad.
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
- A separate `make dev` launch and full relaunch reproduced the same Safe Mode
  result while retaining the isolated dev filesystem profile. No Keychain ACL,
  credential value, or canonical database was modified during that recheck.

## Journey Credit

| Journey | Result | Reason |
| --- | --- | --- |
| Shell and canonical routes | `HISTORICAL_PASS_AT_RECORDED_SHA` | the broad six-route walk belonged to the pre-review artifact |
| stale/unknown/error fail-closed | `HISTORICAL_PASS_FOR_OBSERVED_CASES` | observed broadly in the original trial; not exhaustive and not inherited by a newer SHA |
| Current reviewed Settings | `PENDING_FINAL_REBUILD` | code review is green; final native artifact and observation are not yet credited |
| approved versus applied | `PASS_AT_PRESENTATION_BOUNDARY` | empty-state and automated lifecycle tests preserve separation; no real approved item existed |
| permission -> review -> refresh -> resume | `BLOCKED` | isolated backend had no exact pending permission ReviewItem/task pair |
| proposal -> decision -> application | `BLOCKED` | Safe Mode blocked Builder start and no exact durable proposal existed |
| provider test -> save -> boundary refresh | `BLOCKED` | no test credential or authorized external transmission was supplied |
| credential recovery -> restart -> recheck | `HISTORICAL_FAILED_FAIL_CLOSED` | rejected UI attempt only; interactive reads succeeded but restart proof failed |

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
- `artifacts/16-make-dev-restart-safe-mode-persists.png`

Images 01-12 and 14-15 are `1228x768`; the focused report capture 13 is
`598x374`, and the `make dev` evidence capture 16 is `445x278`. Screenshots
contain no entered credential or secret material.

`REAL_TAURI_SHELL=HISTORICAL_PASS_AT_RECORDED_SHA`

`REAL_TAURI_FAIL_CLOSED=HISTORICAL_PASS_FOR_OBSERVED_CASES`

`REAL_TAURI_GOVERNED_ACTION_E2E=BLOCKED`

`REAL_TAURI_DURABLE_APPLICATION_E2E=BLOCKED`

`REAL_TAURI_EXTERNAL_PROVIDER_E2E=BLOCKED`

`REAL_TAURI_CREDENTIAL_RECOVERY_E2E=HISTORICAL_FAILED_FAIL_CLOSED`

`CURRENT_REVIEWED_TAURI_SETTINGS=PENDING_FINAL_REBUILD`

`CURRENT_REVIEWED_ROOT_EXECUTABLE_ADHOC_VERIFY=PENDING_FINAL_REBUILD`

`CURRENT_REVIEWED_APP_BUNDLE_STRICT_VERIFY=PENDING_FINAL_REBUILD`

`CURRENT_REVIEWED_CREDENTIAL_RECOVERY_ACTION=ABSENT_FAIL_CLOSED`

`MAKE_DEV_ENTRY=PASS_WITH_SAFE_MODE`

`DEVELOPER_ID_REQUIRED_FOR_CURRENT_FRONTEND_DEVELOPMENT=NO`
