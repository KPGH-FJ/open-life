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

Clean source `a278f199b96b32fd941541253029e9d4ab362726` was rebuilt into
`target/debug/bundle/macos/OpenLife.app` and launched in the foreground with a
fresh temporary QA data directory. The `/settings` route rendered the sanitized
configuration, focused `模型与供应商`, retained unknown transmission and Safe
Mode, and exposed no credential-recovery button. Provider test and save remained
disabled. The process was checked before and after the UI observation as the
same PID, QA profile, isolated data directory, and bundled executable hash. The
earlier `af4ba87` / `d54a130e...` recheck remains intermediate historical
evidence only.

| Source SHA | Binary SHA-256 | Root executable identity | Verification | Native observation | Credit |
| --- | --- | --- | --- | --- | --- |
| `a278f199b96b32fd941541253029e9d4ab362726` | `aaab9b3580b0041c85d6c9ff69d5898e00dae4d201ea18ac21f67f3830ad4f0a` | ad-hoc; CDHash `9e6046ff1bfa36a0c397eab90e6f907d619ce43a`; identifier `openlife_tauri-ed5917cd8eac010b` | root executable: `PASS`; identical bundled executable: `FAIL_RESOURCE_SEAL`; app bundle `--deep --strict`: `FAIL_RESOURCE_SEAL` | `/today` opened fail-closed; `/settings` rendered, focused correctly, kept boundary unknown, omitted recovery, and disabled test/save | `SETTINGS_ONLY_PASS_AT_EXACT_ARTIFACT` |

`--no-sign` was used intentionally. The root debug executable has a valid
ad-hoc signature, but neither the identical executable copied into the bundle
nor the whole app bundle passes strict resource-seal verification. This is
launch evidence for the exact local artifact, not distribution-signing credit.
No credential command, provider request, save, review decision, or durable
user-data action was executed. The temporary QA directory was moved to Trash
after the app exited; release/dev application data was not touched.

## Runtime Boundary

The production-config debug bundle was built and launched from
`target/debug/bundle/macos/OpenLife.app` with `OPENLIFE_PROFILE=qa`, an isolated
`OPENLIFE_DATA_DIR`, and A2A development autostart disabled. The product used
the shipped Tauri entry and canonical routes; no Vite harness, fixture selector,
preview route, old page, or mobile surface was used.

The earlier pre-review rebuild had cdhash
`9b021cbef41385690df6140c3e103bfb112fe5b0`. It remains historical identity
evidence only and is not the artifact credited by the review-repair recheck.

Historical pre-review runtime evidence: the canonical development entry was
checked separately during the original trial. The first invocation correctly
rejected an inherited custom `OPENLIFE_DATA_DIR`; after that variable was
removed, `make dev` used `OPENLIFE_PROFILE=dev` and
`~/Library/Application Support/ai.openlife.app.dev`. Two complete launches
both returned to the same backend Safe Mode. This is evidence for those
recorded identities only; it is not inherited by the current `a278f19`
artifact. It also does not prove that formal distribution signing is required
for current product development.

During that historical attempt, no real provider credential was entered and no
provider test, network request, settings save, review decision, or durable write
was executed. The internal credential recovery command was executed only after
explicit user approval; the frontend received metadata statuses and no key
material.

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
| Current reviewed Settings | `PASS_AT_A278F19_AAAB9B35_ARTIFACT` | exact current artifact opened Today and Settings; Settings stayed fail-closed with the reviewed controls |
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

`CURRENT_REVIEWED_TAURI_SETTINGS=PASS_AT_A278F19_AAAB9B35_ARTIFACT`

`CURRENT_REVIEWED_ROOT_EXECUTABLE_ADHOC_VERIFY=PASS`

`CURRENT_REVIEWED_APP_BUNDLE_STRICT_VERIFY=FAIL_RESOURCE_SEAL`

`CURRENT_REVIEWED_CREDENTIAL_RECOVERY_ACTION=ABSENT_FAIL_CLOSED`

`MAKE_DEV_ENTRY=HISTORICAL_PASS_AT_RECORDED_SHA`

`DEVELOPER_ID_REQUIRED_FOR_CURRENT_FRONTEND_DEVELOPMENT=NO`
