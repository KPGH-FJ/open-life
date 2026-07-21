# Phase 4F Defect Register

Status: `ACTIVE`
Date: 2026-07-21

## D-001: Safe Mode Recovery Was Not Reachable In The New Settings Owner

- Severity: `P0` before native acceptance.
- Evidence: the backend still ships `recover_required_credential_access` and
  `frontend/src/tauri.ts` still exposes its typed bridge, but the Phase 4D/4E
  Settings journey had no caller after the old Settings tree was deleted.
- User impact: a fresh or upgraded desktop app could enter Safe Mode and explain
  the blocker without offering the only governed user-initiated recovery path.
- Root cause: the Phase 4E deletion ledger tracked Settings test/save/boundary
  ownership but omitted this utility action from the migration row.
- Repair:
  - independently load `LifeStateProjection.safeMode`;
  - keep recovery unavailable when that projection is missing or inactive;
  - show recovery even when editable config cannot load;
  - expose a complete Product Action Contract and prevent overlap with
    provider test/save operations;
  - require an application confirmation before the existing native confirmation;
  - display metadata-only results and require restart plus fresh backend proof;
  - keep errors, partial readiness, and command return fail-closed.
- Automated evidence: focused data-source, Hook, and rendered-view tests.
- Native evidence: Safe Mode entry, product confirmation, cancel path, dialog
  focus, and focus restoration passed in the packaged app. Native credential
  initialization remains pending explicit action-time user confirmation.
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

- Severity: `P0` acceptance and packaging blocker.
- Native symptom: credential recovery returned all four purposes as
  `available`, but a complete restart returned to Safe Mode with
  `AgentRun receipt key is unavailable; AgentRun persistence is disabled`.
- Source boundary:
  - recovery uses interactive `KeyringSecretStore` and can prove only the
    current credential read;
  - bootstrap uses bounded, non-interactive `StartupKeyringSecretStore`;
  - therefore an interactive `available` result is not restart proof.
- Signing evidence: the AgentRun Keychain decrypt ACL still referenced the
  removed `open-life-backend-d050` executable and cdhash
  `05d890fc52f483d90744b8019b72ad2362e2dde6`. The tested bundle is ad-hoc
  signed with cdhash `b00d8e9c8edb21a674a1138189deb957a83a3579`, which was not added to that
  ACL after the authorized recovery attempt. The final copy-only rebuild has
  cdhash `9b021cbef41385690df6140c3e103bfb112fe5b0`, demonstrating that another
  unsigned debug rebuild changes identity again rather than repairing the ACL.
- Safety result: the backend remained in Safe Mode; the frontend did not infer
  recovery from the command report.
- Bounded frontend repair: replace "ready" claims with "available in this
  check" and continue requiring restart proof.
- Required remediation before green credit: validate recovery from a stable
  signed package identity and decide whether the backend recovery contract
  needs an explicit transient-access state. Do not rotate or delete keys beside
  canonical data to make the test pass.
- Rust/backend behavior changed in Phase 4F: `NO`.
