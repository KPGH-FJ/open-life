# Sprint 5B Diagnosis Packet: Danger Action Preflight

Date: 2026-06-29

Status: ready for bounded Slice 5B implementation after Slice 5A commit `083015d`. This packet is not approval for live-provider calls, final destructive-action redesign, or typed-confirmation Slice 5D.

## Scope

Raw audit issues: `OL-010`, `V4-005`, `V5-014`.

Primary promise: before a user exports sensitive local data, imports over current data, cleans audit logs, or rotates an audit key, OpenLife must show what will happen, what data or state is affected, whether the action writes durable state, whether it sends anything externally, and whether Safe Mode blocks it.

Slice 5B covers these Settings actions only:

- `data_export`
- `data_import_overwrite`
- `mcp_audit_export`
- `mcp_audit_cleanup`
- `mcp_audit_key_rotation`

Rollback, agent-run deletion, memory rollback, and broader danger-zone consolidation are later slices.

## Verified Source Reality

Checked source entrypoints:

| Surface | Current reality | Risk |
|---|---|---|
| `frontend/src/pages/SettingsPage.tsx:217` | `handleExport` calls `exportAllData()` before the save destination is chosen. | Export is read-only but includes LifeModel, messages, and vector memories; the user sees no product-level scope/risk preflight before sensitive data is serialized in memory. |
| `frontend/src/pages/SettingsPage.tsx:241` | `handleImport` opens a file and stores `pendingImport`; `confirmImport` later calls `importAllData(pendingImport.payload)`. | There is a two-step UI, but the preflight is not a typed, reusable contract with payload scope, snapshot status, durable-write effect, or evidence refs. |
| `frontend/src/pages/SettingsPage.tsx:289` | `handleExportAudit` calls `exportMcpAuditLogs(30)` before destination selection. | Export can include local MCP audit arguments/results; the UI does not state sensitivity or external-transmission status before serialization. |
| `frontend/src/pages/SettingsPage.tsx:308` | `handleCleanupAudit` uses `window.confirm` then calls `cleanupMcpAuditLogs(90)`. | Browser confirm is not an auditable product preflight and does not expose dry-run, backup, scope, or evidence. |
| `frontend/src/pages/SettingsPage.tsx:327` | `handleRotateAuditKey` uses `window.confirm` then calls `rotateMcpAuditKey()`. | Key rotation is durable security state change but has no typed preflight, no source ref, and no separate confirmation contract. |
| `frontend/src/pages/settings/tabs/DataTab.tsx:87` | Data export/import buttons delegate directly to `SettingsPage` handlers. | The tab cannot render action-specific preflight details today. |
| `frontend/src/pages/settings/tabs/PrivacyTab.tsx:184` | Local audit export/cleanup/key-rotation controls are grouped but call direct handlers. | The surface looks like governance, but the first click can still enter execution flow for export/cleanup/rotation. |
| `frontend/src/tauri.ts:3404` | `exportAllData`, `importAllData`, `exportMcpAuditLogs`, `cleanupMcpAuditLogs`, and `rotateMcpAuditKey` are direct wrappers. | No frontend-facing preflight command exists. |
| `src-tauri/src/commands/settings.rs:59` | Import already fails closed unless a `GovernedDataImportRequest` is supplied. | Backend import has important guardrails, but Settings UI still lacks a preflight read model explaining them before execution. |
| `src-tauri/src/commands/settings.rs:208` | `export_all_data` returns full local export payload. | It is read-only but privacy-sensitive and should be described before serialization. |
| `src-tauri/src/commands/settings.rs:593` | MCP audit export/cleanup/key rotation are direct commands. | Cleanup and key rotation mutate state without a typed preflight command; export is sensitive read/export without product-level scope preview. |

## Root-Cause Hypotheses

1. The product has several separate safety mechanisms, but no common `DangerActionPreflight` read model for Settings actions.
2. UI risk copy is distributed across Data, Privacy, and browser confirm text, so users cannot compare action scope or understand backup/write/external-transmission semantics consistently.
3. Backend import is already governed, but the frontend does not surface the governed request fields as preflight evidence before execution.
4. Export operations are treated as harmless because they are read-only, but they still serialize sensitive local data and therefore need privacy preflight.

## Industry Benchmark Applied

| Benchmark | Product bar for Slice 5B |
|---|---|
| Granola privacy/sharing pattern | User sees whether content leaves the local/private boundary before sharing/exporting. |
| GitHub destructive settings pattern | Destructive state changes are separated from ordinary buttons and require explicit review/confirmation. |
| Apple/macOS privacy prompt pattern | System-level sensitive actions state the affected data category before permission is granted. |
| Codex/Cursor background-agent controls | Long-running or irreversible operations expose durable state, logs, and recovery expectations. |

## Slice 5B Frozen Scope

Implement only: read-only `DangerActionPreflightView` plus preflight-first Settings UI for the five scoped actions.

Required backend behavior:

- Add a metadata-safe command, for example `get_danger_action_preflight(actionType, safeMode, params)`.
- The command returns a read-only `DangerActionPreflightView`; it must not call `export_all_data`, `import_all_data`, `cleanup_mcp_audit_logs`, `rotate_mcp_audit_key`, or file dialogs.
- Supported action types: `data_export`, `data_import_overwrite`, `mcp_audit_export`, `mcp_audit_cleanup`, `mcp_audit_key_rotation`.
- The view must include `risk_tier`, `scope_summary`, `data_categories`, `writes_durable_state`, `external_transmission`, `dry_run_available`, `backup_status`, `requires_typed_confirmation`, `final_action_enabled`, `safe_mode_blocked`, `blocking_reasons`, and `source_refs`.
- Export actions must report `writes_durable_state=false`, `external_transmission=not_sent_externally`, and sensitive local data categories.
- Import overwrite must report `writes_durable_state=true`, `backup_status=will_create_on_execute` or equivalent; do not claim a snapshot exists before execution.
- Cleanup and key rotation must report `writes_durable_state=true`; Safe Mode blocks them.
- The view must never include raw payload content, file paths, audit log arguments/results, API keys, or keyring material.

Required frontend behavior:

- First click on scoped Data/Privacy danger actions opens or renders preflight; it must not execute the final command.
- Final command execution must be behind a distinct "continue/execute" action after preflight is visible.
- Browser `confirm()` must not be the only safety layer for cleanup or key rotation.
- Safe Mode must render as a product-level preflight block with the reason, not only a disabled button.
- Existing `importAllData` must continue to pass the governed import request; Slice 5B must not weaken backend import guards.

Non-goals:

- No real live-provider call.
- No API-key viewing, editing, rotation redesign, or key export.
- No rollback/delete implementation.
- No typed confirmation phrase; that is Slice 5D.
- No irreversible action in tests except through existing focused mocks or isolated backend test state.

## Anti-Hallucination Checks

- Do not claim a pre-change snapshot exists until the backend operation actually creates one.
- Do not infer external provider transmission from export or save dialogs; local file export is `not_sent_externally` unless a provider invocation record exists.
- Do not treat browser `confirm()` text as backend safety evidence.
- Do not claim cleanup is reversible unless a real backup/snapshot path is implemented.
- Do not serialize raw import payload, audit log arguments/results, keyring state, or filesystem paths into the preflight view.
- Do not mark `final_action_enabled=true` for Safe Mode blocked destructive actions.

## Slice 5B Acceptance Tests

Backend focused tests to add:

- `danger_action_preflight_returns_safe_data_export_scope`
- `danger_action_preflight_marks_import_overwrite_as_critical_without_claiming_existing_snapshot`
- `danger_action_preflight_marks_audit_export_as_sensitive_read_only`
- `danger_action_preflight_marks_cleanup_and_key_rotation_as_mutating`
- `danger_action_preflight_safe_mode_blocks_destructive_actions`
- `danger_action_preflight_rejects_unknown_action_type`
- `danger_action_preflight_never_serializes_payload_paths_or_key_material`

Frontend focused tests to add/update:

- `DataTab` first export/import click requests or displays preflight and does not call final export/import command.
- `PrivacyTab` first audit export/cleanup/key-rotation click requests or displays preflight and does not call final cleanup/key-rotation command.
- Preflight renders risk tier, data categories, durable-write status, external-transmission status, backup/snapshot status, and source refs.
- Safe Mode shows a blocked preflight state for import overwrite, cleanup, and key rotation.
- Existing provider-transmission history tests from Slice 5A remain passing.

Suggested gates:

- `cargo test -p openlife-tauri danger_action_preflight`
- `cargo test -p openlife-tauri settings`
- `cd frontend && corepack pnpm test -- DataTab.test.tsx`
- `cd frontend && corepack pnpm test -- PrivacyTab.test.tsx`
- `cd frontend && corepack pnpm test -- SettingsPage.test.tsx`
- `cd frontend && corepack pnpm typecheck`
- `cargo fmt --check`
- `git diff --check`

## Replay Scenarios

| Scenario | Expected evidence |
|---|---|
| Click "导出全部数据" | Preflight appears first; scope says LifeModel/messages/vectors; no external provider; no durable write; export command not called until explicit continuation. |
| Click "导入覆盖备份" in Safe Mode | Preflight/blocker says Safe Mode blocks import overwrite; no file read or import command. |
| Click "清理旧日志" | Preflight appears first; scope says logs older than retention; durable write; no external provider; cleanup command not called until explicit continuation. |
| Click "轮换密钥" | Preflight appears first; scope says audit keyring/key epoch; durable security state write; no key material displayed. |
| Click "导出审计" | Preflight appears first; scope says recent MCP audit logs and possible tool metadata; no external provider. |

## Rework Triggers

Slice 5B must be returned for rework if any of these happen:

- First click executes export/import/cleanup/key-rotation before preflight is visible.
- Preflight claims a snapshot/backup exists without backend evidence.
- Safe Mode blocked actions expose `final_action_enabled=true`.
- Raw import payload, audit arguments/results, API key, keyring material, or filesystem path appears in the preflight view.
- Backend import governed request is weakened or bypassed.
- Existing Slice 5A provider-transmission tests regress.
