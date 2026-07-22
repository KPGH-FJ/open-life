# Phase 4F Defect Register

Status: `ACTIVE`
Date: 2026-07-21

## D-001: Safe Mode Recovery Was Not Reachable In The New Settings Owner

- Severity: `P0` before native acceptance.
- Status: `OPEN_FAIL_CLOSED_AFTER_REVIEW`.
- Evidence: the backend still ships `recover_required_credential_access` and
  `frontend/src/tauri.ts` still exposes its typed bridge, but the Phase 4D/4E
  Settings journey had no caller after the old Settings tree was deleted.
- User impact: a fresh or upgraded desktop app could enter Safe Mode and explain
  the blocker without offering the only governed user-initiated recovery path.
- Root cause: the Phase 4E deletion ledger tracked Settings test/save/boundary
  ownership but omitted this utility action from the migration row.
- Rejected first repair attempt:
  - independently load `LifeStateProjection.safeMode`;
  - the attempt enabled recovery from `safeMode.active` alone;
  - independent review proved that vector corruption, database degradation, or
    unrelated startup warnings can also activate that generic flag;
  - exposing the command could therefore inspect or initialize unrelated
    integrity credentials under an incorrect diagnosis.
- Current bounded repair:
  - independently load `LifeStateProjection.safeMode` for truthful protection
    state;
  - keep the credential action absent because the backend does not expose typed
    recovery eligibility/cause;
  - do not parse free-text Safe Mode reasons in the frontend;
  - require a separately reviewed backend contract before reintroducing the
    action.
- Automated evidence: direct Settings cold-route load and generic Safe Mode
  no-action regression tests.
- Historical native evidence: the rejected attempt exercised confirmation,
  cancellation, metadata-only report, and restart failure. Those screenshots
  remain evidence of the trial, not proof that the current UI ships recovery.
- Old owner restored: `NO`.
- Backend business behavior changed: `NO`.

## D-002: Main CI Windows MCP Fixture Timeout

- Severity: `P2` intermittent infrastructure observation.
- First observation: main CI run `29811958822`, attempt 1.
- Failure boundary: a Python-backed slow MCP fixture exceeded the existing
  10-second initialize handshake while the Windows runner was heavily loaded;
  the intended one-second tool-call timeout assertion had not started.
- Comparison: the same source and job passed in PR #63; the exact focused test
  passed locally in 1.07 seconds; all Windows steps passed on CI attempt 2.
- Product regression evidence: `NO`.
- Source change: `NONE` pending recurrence or a reproducible root cause.
- Follow-up: keep the event visible; do not weaken the transport timeout or mark
  a failed run green without a complete successful rerun.

## D-003: Sanitized AppConfig Shape Crashed Settings

- Severity: `P0` in the production desktop route.
- Native symptom: `/settings` entered the global render-failure boundary.
- Root cause: Rust declares `LlmConfig.openai_key` with `skip_serializing`, but
  the TypeScript `AppConfig` contract incorrectly required the field and the
  Settings presentation called `.trim()` unconditionally.
- Repair:
  - model `openai_key` as an optional write-only submission field;
  - model optional `openai_key_ref` as non-secret presence metadata;
  - render omitted secrets as an empty input without claiming a credential;
  - treat a backend reference as stored presence without exposing its value;
  - clear both secret input and reference when provider identity changes.
- Evidence: native Settings now renders; omitted-secret and identity-change
  regression tests pass.
- Backend behavior changed: `NO`.

## D-004: Structured Tauri Errors Rendered As `[object Object]`

- Severity: `P1` trust and supportability defect.
- Native symptom: the blocked LifeModel Builder showed
  `建立过程暂时不可用 [object Object]`.
- Root cause: product journeys duplicated `String(error)` although Tauri
  serializes `AppError` as `{ kind, detail: { message, hint } }`.
- Repair:
  - use one bounded `journeyErrorCode` parser across shipped journeys;
  - prefer stable `kind:hint` metadata and never stringify unknown objects;
  - show plain product copy in the Builder surface;
  - keep `Database:read_only_degraded` in collapsed Inspector diagnostics.
- Evidence: parser and Builder tests pass; packaged app shows plain-language
  failure copy and no proposal or LifeModel write.

## D-005: Route Live Region Retained A Today Loading Message

- Severity: `P1` accessibility defect.
- Native symptom: Workspace, Tasks, Review, and LifeModel retained the hidden
  text `正在读取今日状态。` after route focus moved to the new heading.
- Root cause: route synchronization reset the shared announcement to a
  Today-specific initial string.
- Repair: derive the entry announcement from the canonical product surface and
  use the same source during navigation and route synchronization.
- Evidence: route regression test passes; packaged Workspace exposes
  `已进入工作区；当前执行与阻塞只取自后端读模型。`.

## D-006: Interactive Credential Access Did Not Survive Restart

- Severity: `P0` for durable-journey acceptance. It does not block frontend
  layout and read-only product work.
- Native symptom: credential recovery returned all four purposes as
  `available`, but both a complete packaged-app restart and two clean
  `make dev` launches returned to Safe Mode with
  `AgentRun receipt key is unavailable; AgentRun persistence is disabled`.
- Source boundary:
  - recovery uses interactive `KeyringSecretStore` and can prove only the
    current credential read;
  - bootstrap uses bounded, non-interactive `StartupKeyringSecretStore`;
  - therefore an interactive `available` result is not restart proof.
- Identity evidence: the AgentRun Keychain decrypt ACL still referenced the
  removed `open-life-backend-d050` executable and cdhash
  `05d890fc52f483d90744b8019b72ad2362e2dde6`. The tested bundle is ad-hoc
  signed with cdhash `b00d8e9c8edb21a674a1138189deb957a83a3579`, which was not added to that
  ACL after the authorized recovery attempt. A later pre-review copy-only
  rebuild had cdhash `9b021cbef41385690df6140c3e103bfb112fe5b0`,
  demonstrating that another unsigned debug rebuild changes identity again
  rather than repairing the ACL.
  The recorded `make dev` binary was also ad-hoc signed, identified as
  `openlife_tauri-4e33bcd58dc68447`, and has cdhash
  `2b5256b69515c0325ad30c7736e591d588b401b8`. All four values in this paragraph
  are historical trial identities, not current-artifact identities.
- Safety result: the backend remained in Safe Mode; the frontend did not infer
  recovery from the command report.
- Evidence status: `HISTORICAL-EVIDENCE` from the rejected recovery UI attempt;
  the current reviewed frontend no longer exposes that action.
- Bounded frontend repair: replace "ready" claims with "available in this
  check" and continue requiring restart proof.
- Current scope decision: Developer ID signing and notarization are deferred;
  they are not prerequisites for continuing frontend product development.
  Green credit for durable journeys still requires a development-safe identity
  or explicit persistent ACL recovery, followed by a fresh process proof. A
  formal Developer ID is only one future release option, not the required local
  development solution.
- Safety boundary: do not rotate or delete keys beside canonical data to make
  the test pass. The `make dev` recheck did not mutate Keychain ACLs or key
  material.
- Rust/backend behavior changed in Phase 4F: `NO`.

## D-008: Direct Settings Cold Route Did Not Load Its Data Source

- Severity: `P1`.
- Root cause: `initialMode="settings"` synchronized shell mode but only later
  click handlers called `settingsPrivacy.ensureLoaded()`.
- Repair: the journey now loads the exact Settings data source on a canonical
  Settings cold route and announces the Settings context.
- Evidence: a non-mocked journey regression test proves one initial backend
  read and a rendered `模型与传输边界` surface.

## D-009: Task `remote_unknown` Was Missing From The Frontend Contract

- Severity: `P1` fail-closed contract mismatch.
- Root cause: Rust serializes `TaskLifecycleStatus::RemoteUnknown`, while the
  TypeScript union and presentation switch omitted it.
- Repair: mirror the backend value and render `远端结果未知` as unverified
  unknown; include it in attention and terminal filtering.
- Evidence: presentation regression test; no green/completed state is emitted.

## D-010: Failed Post-Save Refresh Could Reuse An Unproven Boundary

- Severity: `P2`.
- Root cause: after `boundary_refresh_failed`, the reducer correctly marked the
  saved revision unknown, but the view selector could still return the latest
  ready envelope from the failed refresh snapshot.
- Repair: when the saved revision is not attested, return an explicit unknown
  envelope rather than reuse the envelope.
- Evidence: regression test with a ready envelope plus missing refreshed config.

## D-011: Settings Lifecycle Could Overwrite Announcements And Dirty Drafts

- Severity: `P1`.
- Root causes:
  - Settings entry forced a backend reload on every re-entry;
  - a dirty draft could therefore be replaced by the stored config while the
    reducer still reported unsaved changes;
  - changing callback identity under StrictMode could also replace terminal
    load/save announcements with a new pending announcement.
- Repair:
  - use a stable cold-entry loader that reuses an existing snapshot;
  - make cold entry and explicit reload share one active read promise;
  - invalidate old requests and every test/save/retry continuation with a data
    source generation token;
  - prevent an old continuation from releasing a replacement-source operation
    lock;
  - cancel route announcements when leaving Settings;
  - retain an unsaved draft without a read or write when returning.
- Evidence: StrictMode cold-route, slow-load return, dirty re-entry, concurrent
  load, explicit-reload re-entry, data-source replacement, and in-flight
  test/save/retry replacement regression tests.

## D-012: Loading Or Missing LifeStateProjection Could Leak Old Certainty

- Severity: `P1` for fail-closed product truth.
- Root cause: the Settings surface could continue interpreting an old ready
  config/boundary while a refresh was in flight, and it did not independently
  gate actions when the current LifeStateProjection was missing.
- Repair: treat loading, active Safe Mode, and unknown protection state as
  separate fail-closed states; disable fields, test, and save; replace the
  boundary and Inspector conclusion with loading/unknown evidence.
- Evidence: loading-lock, missing-projection, and active-Safe-Mode regression
  tests.

## D-013: Post-Save Readback Was Not An Exact Configuration Attestation

- Severity: `P1`.
- Root cause: a ready envelope and non-null config were insufficient to prove
  that the readback represented the exact submitted settings revision. A failed
  boundary refresh also had no bounded retry path.
- Repair:
  - retain the previous and submitted sanitized config as an in-memory
    attestation only for the current save attempt;
  - require exact canonical config equality plus the single expected credential
    generation before accepting the refreshed boundary;
  - keep unknown on mismatch and expose a read-only retry that reuses the same
    attestation;
  - never convert a command return into durable completion by itself.
- Evidence: exact-match, stale credential generation, missing projection,
  retry-success, and retry-still-unknown regression tests.

## D-007: Dev Profile Isolation Stops Before The Credential Namespace

- Severity: `P1` development isolation defect; release migration risk remains
  unassessed.
- Evidence: filesystem storage separates `release`, `dev`, and `qa` profiles,
  but `src-tauri/src/secret_store.rs` hard-codes the same
  `com.openlife.desktop` service and `keychain://com.openlife.desktop/...`
  references for every profile. `tauri.dev.conf.json` does not provide a
  separate credential identity.
- Observed impact: the shared Keychain items currently trust an executable from
  a removed worktree, so current dev and packaged processes can both fail
  against credentials attached to otherwise separate data directories.
- Non-claim: the checked-in ignored real-Keychain unit test uses a random MCP
  account and cleans it up. The current source does not prove which historical
  command created the four retained authority keys, so that origin remains
  `UNKNOWN`.
- Required repair slice: design profile-scoped credential ownership and an
  explicit, non-rotating migration for existing canonical stores; add tests
  proving test binaries cannot claim product credential slots. Do not fold this
  migration into the Phase 4F frontend PR without a separate backend review.
- Status: `OPEN`, deliberately not repaired in this frontend acceptance branch.
