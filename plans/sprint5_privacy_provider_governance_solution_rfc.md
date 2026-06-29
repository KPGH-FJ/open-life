# Sprint 5 Solution RFC: Privacy and Provider Governance

Date: 2026-06-29

Status: prepared for schema review; implement after Sprint 1 route truth is reliable.

## Scope

Raw issues: `OL-010`, `V4-005`, `V5-014`, `V6-006`, `V6-010`.

Primary source entrypoints:

- `frontend/src/pages/settings/tabs/PrivacyTab.tsx`
- `frontend/src/pages/settings/tabs/OverviewTab.tsx`
- Settings import/export tests and commands
- `src-tauri/src/main_chat_route_preview.rs`
- `src-tauri/src/main_chat_live_provider_harness.rs`
- provider preflight/final-gate code under `src-tauri/src/commands/agent_runtime/mod.rs`

## Product Goal

Users must know when data stays local, when it is sent externally, what category of data was sent, and what high-risk actions will do before they click.

## Non-Goals

- Do not expose API keys.
- Do not perform real import overwrite, deletion, cleanup, rollback, or key rotation during tests.
- Do not add cloud-provider comparison until sent/not-sent evidence is auditable.

## Data Contracts

`ProviderTransmissionLogEntry`:

| Field | Meaning |
|---|---|
| `entry_id` | stable id |
| `run_id` | AgentRun id |
| `task_session_id` | task id |
| `timestamp` | when decision/invocation occurred |
| `status` | `not_sent`, `sent`, `blocked`, `failed`, `unknown` |
| `provider` | metadata-safe provider name |
| `model` | metadata-safe model name |
| `route_type` | local, cloud, agent_runtime, unknown |
| `data_categories` | prompt, LifeModel summary, memory summary, file excerpt, tool metadata, none |
| `sensitivity` | low, personal, sensitive, local_only |
| `confirmation_state` | not_required, required_pending, accepted, rejected |
| `reason` | route/privacy reason |
| `retention_policy_ref` | optional provider/policy link or unknown |
| `key_material_exposed` | must always be false |

Storage and migration precondition:

- This is a proposed contract, not an existing persisted table.
- Before implementation, choose one durable store explicitly: SQLite table, AgentRun metadata extension, or a dedicated audit-log file. The choice must document retention, redaction, migration/backfill behavior, and how old runs are represented.
- Old runs without instrumentation must be shown as `unknown` or `not_instrumented`, never retroactively as `not_sent`.
- `not_sent` requires positive local-route evidence or explicit no-provider-invocation evidence; absence of a provider log is not enough.

`DangerActionPreflight`:

| Field | Meaning |
|---|---|
| `action_type` | export, import_overwrite, log_cleanup, key_rotation, rollback, delete |
| `risk_tier` | medium, high, critical |
| `scope_summary` | what will be affected |
| `dry_run_available` | boolean |
| `backup_status` | none, available, recommended, required |
| `requires_typed_confirmation` | boolean |
| `final_action_enabled` | boolean |
| `safe_mode_blocked` | boolean |

## UI Contract

Privacy page:

- Per-run history: sent/not sent externally, provider/model, data category, route reason.
- "Not sent externally" should be visible, not only absence of cloud call.
- API keys always masked and never copyable from diagnostics.

Danger zone:

- Export/import/log cleanup/key rotation/rollback grouped by risk tier.
- Show preflight scope before final action.
- Final action requires typed confirmation for high/critical actions.
- Safe Mode disables destructive actions.

## Tests

These are schema/readiness tests for the proposed governance layer; they do not imply provider-transmission history exists today. Every command gate must record a non-zero matched/passed test count.

Backend:

- Local run records `not_sent`.
- External preflight blocked run records `blocked`, not cloud success.
- Provider invocation log never serializes key.
- Import/delete/key rotation preflight does not execute final action.
- Old runs without instrumentation render `unknown` or `not_instrumented`, never retroactive `not_sent`.
- Missing provider log alone is rejected as evidence for `not_sent`.

Candidate command-level gates after adding the focused tests:

- `cargo test -p openlife-tauri live_provider`
- `cargo test -p openlife-tauri settings`
- `cargo test -p openlife-tauri provider_transmission`

Current repo check: no `provider_transmission` source or test symbol exists yet. Sprint 5 must add explicit focused tests before using the filter above, for example:

- `provider_transmission_log_records_local_not_sent_with_positive_local_route_evidence`
- `provider_transmission_log_marks_uninstrumented_old_runs_unknown`
- `provider_transmission_log_rejects_missing_log_as_not_sent_evidence`
- `provider_transmission_log_never_serializes_key_material`
- `danger_action_preflight_does_not_execute_final_action_without_confirmation`

Frontend:

- Privacy tab renders sent/not-sent history.
- Danger action requires preflight and typed confirmation.
- Safe Mode disables high-risk actions.

Candidate command-level frontend gates after adding/updating the focused tests:

- `cd frontend && corepack pnpm test -- SettingsPage.test.tsx`
- `cd frontend && corepack pnpm test -- PrivacyTab.test.tsx`

Replay:

- v6 provider route prompt includes external sent/not-sent evidence.
- v5/v4 export/import preflight inspection.
- API key masking in Settings/trace/report.

## Development Slices

1. Add local-only `not_sent` log entries for Main Chat runs.
2. Render provider transmission history in Privacy tab.
3. Add `DangerActionPreflight` for import/export/log/key actions.
4. Wire high-risk typed confirmation.
5. Replay cloud/provider contrast only after evidence exists.

Exit only when a user can prove whether a run left the machine.
