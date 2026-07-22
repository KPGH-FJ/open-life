# Phase 4F Summary

Status: `READY_FOR_REVIEW_WITH_BLOCKERS`
Date: 2026-07-21

## Completed

- Reverified the Phase 4E merge on main and branched from exact verified SHA
  `7a167f4e50584524586c2350882e43df01b0da2b`.
- Exercised the single production desktop Workbench in a packaged Tauri app.
- Observed all canonical routes in the bounded native trial; this is not an
  exhaustive claim over every missing/stale/error combination.
- Kept generic Safe Mode visible but removed the attempted credential-recovery
  entry after review proved that the backend lacks typed recovery eligibility.
- Repaired the real sanitized AppConfig contract that crashed `/settings`.
- Repaired structured Tauri error handling and stale route live-region copy.
- Repaired direct `/settings` cold-route loading, task `remote_unknown`
  presentation, and post-save boundary fail-closed selection.
- Closed the follow-up Settings review blockers: exact post-save config
  attestation, bounded boundary-refresh retry, Projection-aware fail-closed
  state, loading locks, stable announcements, and dirty-draft re-entry.
- Prepared a SHA-bound evidence row for the final native Settings rebuild;
  code review alone does not populate or inherit that artifact credit.
- Captured native route, Inspector, Settings, dialog, defect, and repair
  evidence without provider credentials or secret material.
- Passed the complete committed-source frontend suite (37 files, 286 tests),
  production build/absence guard, formatting, type, and diff checks. Rust and
  cross-platform CI remain required on the final PR head before merge.
- Preserved the rejected recovery attempt and failed-restart evidence as
  historical trial evidence; it is not current product credit.
- Rechecked the canonical `make dev` entry twice with its isolated dev data
  profile and reproduced the same fail-closed credential boundary. This
  separates the current product-development blocker from future Developer ID
  distribution work.

## Not Yet Credited

- Credential recovery restart proof failed because the existing Keychain ACL
  remained bound to an old worktree executable rather than the current ad-hoc
  bundle or `make dev` identities. A development-safe credential identity or
  explicit persistent ACL recovery is required for durable-journey credit;
  formal Developer ID signing and notarization are deferred.
- The current backend product projection does not expose exact credential
  recovery eligibility/cause. The UI must remain unavailable until that
  authority exists; generic Safe Mode and free-text reasons are insufficient.
- Filesystem profiles are isolated, but the Keychain service and references are
  currently shared by release, dev, and qa. This needs a separately reviewed,
  non-rotating migration slice rather than an opportunistic Phase 4F backend
  change.
- Permission/review/resume and proposal/application E2E remain blocked because
  the isolated backend did not contain exact eligible state.
- External provider test/save remains blocked because no credential or external
  transmission was authorized.
- Manual VoiceOver remains blocked by the automation environment.
- Human merge review remains pending. PR CI must remain green after this
  evidence update; a prior green run is not treated as proof for a newer SHA.

## Authority Result

- Production frontend source changed: `YES`, bounded Phase 4F repairs only.
- Rust/Tauri backend source changed: `NO`.
- Product routes changed or added: `NO`.
- Old ProductShell/pages/routes restored: `NO`.
- Fixture selector added to production: `NO`.
- Unknown/stale/error still fail closed: `YES`.
- Approved still distinct from applied/completed: `YES`.
- Developer ID required for current frontend development: `NO`.
- Local durable-journey restart proof: `NO`.

Phase7 remains `red-until-trial-green`; blocked real journeys are not replaced
with fixture or unit-test claims.

`PHASE4F_COMPLETE=NO`

`PHASE7_TRIAL_GREEN=NO`
