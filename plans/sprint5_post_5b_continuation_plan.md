# Sprint 5 Post-5B Continuation Plan

Date: 2026-06-30

Status: prepared after Slice 5B commit `27367ef`. This document defines the remaining Sprint 5 sequence; it is not permission to run live providers, execute destructive actions in real data, or start all slices in parallel.

## Current Verified Baseline

- Slice 5A (`083015d`) implemented AgentRun-derived provider-transmission history in Privacy.
- Slice 5B (`27367ef`) implemented Settings danger-action preflight for `data_export`, `data_import_overwrite`, `mcp_audit_export`, `mcp_audit_cleanup`, and `mcp_audit_key_rotation`.
- No dedicated `ProviderTransmissionLogEntry` table exists. This is acceptable until a real payload-category, confirmation-state, or retention-proof gap is found.
- Live external-provider replay remains blocked unless the user explicitly authorizes low-sensitivity external calls and route/transmission evidence is checked first.

## Development Order

1. Optional 5A.1 only if needed: dedicated provider-transmission store.
2. Slice 5C: danger-zone consolidation for remaining destructive/governance surfaces.
3. Slice 5D: high-risk typed confirmation and backend confirmation enforcement.
4. Slice 5E: cloud/provider replay with privacy-transmission evidence.

Do not run 5C, 5D, and 5E in parallel. Each slice must preserve 5A/5B tests.

## Optional 5A.1: Dedicated Provider Transmission Store

Only start this if at least one of these is proven with source/runtime evidence:

- AgentRun-derived history cannot represent data categories needed for cloud/provider privacy review.
- Confirmation state for external provider calls cannot be recovered from existing run metadata.
- Provider retention/policy refs need durable per-call metadata that cannot fit the run read model.
- Old-run backfill needs explicit `unknown` / `not_instrumented` rows not representable from AgentRun.

Non-goals:

- No live-provider expansion.
- No backfilling old runs as `not_sent`.
- No table that becomes a second route-truth source independent of `RuntimeRouteEvidence`.

Required tests if started:

- Store records `sent`, `not_sent`, `blocked`, `unknown`, and `not_instrumented` without contradicting `RuntimeRouteEvidence`.
- Old runs remain `unknown` / `not_instrumented`.
- Missing log never proves `not_sent`.
- Key/prompt/raw payload redaction.

## Slice 5C: Danger-Zone Consolidation

Goal: make destructive/governance actions outside the Slice 5B Settings actions use the same preflight-first pattern or an explicitly equivalent governed preflight.

Source inventory to check before coding:

- `frontend/src/pages/RunsPage.tsx`: bulk run deletion currently uses browser `confirm`.
- `frontend/src/pages/AgentRunDetail.tsx`: single run deletion currently uses browser `confirm`.
- `frontend/src/pages/MemorySearch.tsx`: archive/restore actions currently use browser `confirm`.
- `frontend/src/pages/McpPage.tsx`: MCP server deletion currently uses browser `confirm`.
- `frontend/src/pages/VersionControl.tsx`: rollback/version actions currently use browser `confirm`.
- `frontend/src/pages/settings/tabs/OverviewTab.tsx`: vector rebuild currently uses browser `confirm`.

Required behavior:

- Create an inventory table before implementation: action id, current handler, final command, durable write, privacy sensitivity, rollback/backup status, Safe Mode behavior, first slice decision.
- Extend or generalize the preflight DTO without breaking existing `DangerActionPreflightView` consumers.
- First click on selected actions shows preflight; final execution is a distinct continuation action.
- If a surface is intentionally deferred, record why and keep the existing action unchanged.
- No raw memory content, run transcript, file path, MCP server secret, provider key, or audit result text in preflight.

Suggested first implementation subset:

- `agent_run_delete`
- `agent_run_bulk_delete`
- `vector_rebuild`

These are visible governance/data actions and are easier to prove than memory rollback/version rollback.

Acceptance tests:

- First click does not call final delete/rebuild command.
- Preflight lists durable write, affected object count/id digest, Safe Mode status, and source refs.
- Bulk delete count is bounded and metadata-safe.
- Existing Slice 5A/5B tests remain passing.

Rework triggers:

- A browser `confirm()` remains the only safety layer for a selected action.
- Preflight includes raw transcript/memory/server config/path/key material.
- Final commands can execute while preflight is blocked.
- Existing Settings preflight regresses.

## Slice 5D: Typed Confirmation Enforcement

Goal: high/critical actions require explicit typed confirmation, and the backend can tell whether a final action was executed from a confirmed preflight path.

Required behavior:

- Add confirmation fields to the preflight contract, for example `confirmation_phrase`, `confirmation_required`, `confirmation_scope_digest`, and `preflight_id`.
- Critical mutating actions require typed confirmation before continuation.
- Final mutating commands must receive confirmation evidence or an equivalent governed request.
- Frontend-only typing is insufficient for import, cleanup, key rotation, delete, rollback, or rebuild actions.
- Existing `importAllData` governed request must remain stronger than or equal to the new confirmation model.

Initial action set:

- `data_import_overwrite`
- `mcp_audit_key_rotation`
- `mcp_audit_cleanup`
- any selected Slice 5C delete/rebuild action

Non-goals:

- No live-provider calls.
- No broad redesign of Settings IA.
- No weakening of existing typed `IMPORT` import dialog until backend confirmation evidence replaces it.

Acceptance tests:

- Confirm button disabled until phrase matches.
- Wrong phrase does not call final command.
- Backend rejects final mutating command without confirmation evidence where enforcement is added.
- Confirmation evidence contains no raw payload/path/key/audit text.
- Safe Mode still blocks even with a correct phrase.

Rework triggers:

- Typed confirmation is frontend-only for a backend-mutating action.
- Confirmation phrase contains private content or filesystem path.
- Confirmation bypass exists through direct tauri wrapper call.

## Slice 5E: Cloud / Provider Replay

Goal: rerun the cloud/provider contrast only after 5A/5B/5D evidence can prove route and transmission boundaries.

Preconditions:

- User explicitly authorizes low-sensitivity external-provider calls.
- No API key is viewed, copied, logged, or committed.
- `ProviderTransmissionHistoryItem` and route evidence can prove sent/not-sent/blocked status for the replay runs.
- If external route cannot be verified, mark `cloud_route_not_verified` and stop quality comparison.

Required replay cases:

- Runtime route truth prompt.
- Cloud-requested direct answer.
- Sichuan Museum current-fact request with no web/tool confusion.
- Low-pressure planning prompt.
- Memory/preference proposal prompt.
- Provider failure/fallback blocker.

Acceptance artifacts:

- `route_evidence.md`
- `privacy_transmission_log.md`
- `cloud_vs_local_comparison.md`
- issue updates for any cloud-only or product-general regressions

Rework triggers:

- UI claims cloud while route evidence says local/unknown.
- External provider transmission is not visible in Privacy/run evidence.
- Cloud LLM is treated as web/current-fact tool.
- Any key/raw secret appears in logs, screenshots, reports, or test fixtures.

## Cross-Slice Gates

Every remaining Sprint 5 slice must run:

- Slice-specific backend tests.
- Slice-specific frontend tests.
- `cargo test -p openlife-tauri provider_transmission`
- `cd frontend && corepack pnpm test -- PrivacyTab.test.tsx`
- `cd frontend && corepack pnpm test -- SettingsPage.test.tsx`
- `cd frontend && corepack pnpm typecheck`
- `cargo fmt --check`
- `git diff --check`

If a candidate command matches zero tests, it is not a gate.
