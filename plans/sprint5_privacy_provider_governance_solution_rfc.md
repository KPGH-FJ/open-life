# Sprint 5 Solution RFC: Privacy and Provider Governance

Date: 2026-06-29

Status: Slice 5A implemented in `083015d`. Ready for bounded Slice 5C1 implementation after source-level diagnosis in `plans/sprint5_danger_action_preflight_diagnosis_packet.md`. This is not approval for live-provider expansion, typed-confirmation Slice 5D, or broad danger-zone redesign.

## Scope

Raw issues: `OL-010`, `V4-005`, `V5-014`, `V6-006`, `V6-010`.

Primary source entrypoints:

- `frontend/src/pages/settings/tabs/PrivacyTab.tsx`
- `frontend/src/pages/settings/tabs/DataTab.tsx`
- `frontend/src/pages/settings/tabs/OverviewTab.tsx`
- Settings import/export tests and commands
- `src-tauri/src/main_chat_route_preview.rs`
- `src-tauri/src/main_chat_live_provider_harness.rs`
- provider preflight/final-gate code under `src-tauri/src/commands/agent_runtime/mod.rs`

## Product Goal

Users must know when data stays local, when it is sent externally, what category of data was sent, and what high-risk actions will do before they click.

## Source Reality Freeze

Slice 5A started from current route/run evidence:

- `RuntimeRouteEvidence.external_transmission` already supports `sent`, `not_sent`, `unknown`, and `not_instrumented` semantics.
- Existing runtime-facts tests prove cloud routes report `sent`, local/runtime routes report `not_sent`, and missing settings instrumentation reports `not_instrumented`.
- Runs/detail surfaces already display route evidence, but Privacy has no aggregated per-run transmission history.
- No dedicated ProviderTransmission table exists today.
- Live provider harness evidence is opt-in eval evidence, not default product transmission history.

Slice 5C1 must start from current Settings danger-action reality:

- `frontend/src/pages/SettingsPage.tsx` still owns direct handlers for data export/import, MCP audit export/cleanup, and audit key rotation.
- Data export and MCP audit export are read-only but privacy-sensitive because they serialize local personal/audit data.
- Import already requires a backend `GovernedDataImportRequest`, but Settings does not present a reusable preflight view before final execution.
- Cleanup and key rotation still rely on browser `confirm()` plus Safe Mode button disabling; that is not durable product preflight evidence.

## Non-Goals

- Do not expose API keys.
- Do not perform real import overwrite, deletion, cleanup, rollback, or key rotation during tests.
- Do not add cloud-provider comparison until sent/not-sent evidence is auditable.
- Do not claim backup/snapshot availability in preflight unless backend evidence proves it.

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
- Slice 5A storage choice is AgentRun-derived projection plus `RuntimeRouteEvidence` semantics. Do not add a new table unless implementation proves this source is insufficient.
- Future slices may add a dedicated SQLite table, AgentRun metadata extension, or audit-log file. That later choice must document retention, redaction, migration/backfill behavior, and how old runs are represented.
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

Slice 5C1 concrete fields:

| Field | Meaning |
|---|---|
| `data_categories` | bounded labels such as LifeModel, messages, vectors, MCP audit metadata |
| `writes_durable_state` | whether the final action mutates local durable state |
| `external_transmission` | `not_sent_externally`, `sent_externally`, `unknown` |
| `blocking_reasons` | Safe Mode or unsupported action blockers |
| `source_refs` | metadata-safe command/code/evidence refs, not raw payloads or paths |

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
- For Slice 5C1, first click shows preflight and must not execute final export/import/cleanup/key-rotation. A distinct continuation action may call the existing final command; typed phrases remain Slice 5D.

## Tests

These are schema/readiness tests for the proposed governance layer; they do not imply provider-transmission history exists today. Every command gate must record a non-zero matched/passed test count.

Backend:

- AgentRun-derived provider-transmission view records local route as `not_sent`.
- Cloud route or live-provider invocation evidence records `sent`.
- Blocked provider preflight with no model invocation records `blocked`.
- External preflight blocked run records `blocked`, not cloud success.
- Provider invocation log never serializes key.
- Import/delete/key rotation preflight does not execute final action.
- Old runs without instrumentation render `unknown` or `not_instrumented`, never retroactive `not_sent`.
- Missing provider log alone is rejected as evidence for `not_sent`.
- Danger-action preflight never serializes payload content, audit arguments/results, key material, or filesystem paths.

Command-level gates:

- `cargo test -p openlife-tauri live_provider`
- `cargo test -p openlife-tauri settings`
- `cargo test -p openlife-tauri provider_transmission`
- `cargo test -p openlife-tauri danger_action_preflight`

Slice 5A already added provider-transmission tests. Slice 5C1 must keep those passing and add explicit focused danger-action preflight tests:

- `provider_transmission_view_records_local_not_sent_with_positive_route_evidence`
- `provider_transmission_view_records_cloud_sent_with_cloud_route_evidence`
- `provider_transmission_view_records_blocked_preflight_without_model_invocation`
- `provider_transmission_view_marks_uninstrumented_old_runs_unknown`
- `provider_transmission_view_rejects_missing_log_as_not_sent_evidence`
- `provider_transmission_view_never_serializes_key_material`
- `danger_action_preflight_returns_safe_data_export_scope`
- `danger_action_preflight_marks_import_overwrite_as_critical_without_claiming_existing_snapshot`
- `danger_action_preflight_marks_cleanup_and_key_rotation_as_mutating`
- `danger_action_preflight_safe_mode_blocks_destructive_actions`
- `danger_action_preflight_never_serializes_payload_paths_or_key_material`

Frontend:

- Privacy tab renders sent/not-sent history.
- Slice 5C1 danger actions require visible preflight before final execution; typed confirmation remains Slice 5D.
- Safe Mode disables high-risk actions.

Candidate command-level frontend gates after adding/updating the focused tests:

- `cd frontend && corepack pnpm test -- SettingsPage.test.tsx`
- `cd frontend && corepack pnpm test -- PrivacyTab.test.tsx`
- `cd frontend && corepack pnpm test -- DataTab.test.tsx`

Replay:

- v6 provider route prompt includes external sent/not-sent evidence.
- v5/v4 export/import preflight inspection.
- API key masking in Settings/trace/report.

## Development Slices

1. Slice 5A: AgentRun-derived provider-transmission read model and Privacy history. Implemented in `083015d`.
2. Slice 5B: optional dedicated transmission store if the read model cannot represent required payload category / confirmation state semantics. Deferred; do not start unless 5A read model proves insufficient.
3. Slice 5C1: `DangerActionPreflightView` for Settings data export/import and MCP audit export/cleanup/key rotation, preflight-first UI, no typed phrase.
4. Slice 5C2: broader danger-zone consolidation if needed for rollback/delete surfaces.
5. Slice 5D: high-risk typed confirmation.
6. Slice 5E: replay cloud/provider contrast only after evidence exists.

Exit only when a user can prove whether a run left the machine.

## Slice 5A Implementation Contract

Goal: make external-transmission status visible in Privacy using existing durable route/run evidence, without adding live provider calls or dangerous actions.

Required implementation:

1. Add a backend command/read model, for example `list_provider_transmission_history`, backed by recent AgentRuns and `RuntimeRouteEvidence` status semantics.
2. The view must expose status, run id, task session id when available, provider/model/route type, reason, evidence id, confidence, data category, and safe source refs.
3. `not_sent` must require positive local/agent_runtime/scripted route evidence or explicit no-provider-invocation metadata.
4. `sent` must require cloud route or live-provider invocation metadata.
5. `blocked` must require provider preflight/live-provider blocker evidence and no model invocation.
6. Missing route metadata must produce `not_instrumented` or `unknown`.
7. Privacy tab must render recent history and must not hide unknown/not-instrumented rows.
8. No API keys, raw prompts, or sensitive excerpts may be serialized or rendered.

Blocked from this slice:

- Real cloud/provider invocation.
- Provider key changes or key rotation behavior changes.
- Import overwrite, log cleanup, delete, rollback, or final danger action execution.
- Retention-policy claims for third-party providers.
- A new table that bypasses `RuntimeRouteEvidence`.

## Slice 5C1 Implementation Contract

Goal: make Settings danger actions preflight-first, so the user sees scope, risk, durable-write effect, external-transmission status, Safe Mode blocker, and source refs before final execution.

Required implementation:

1. Add a backend read-only preflight command, for example `get_danger_action_preflight`, under the Settings/governance command surface.
2. Support only these action types in this slice: `data_export`, `data_import_overwrite`, `mcp_audit_export`, `mcp_audit_cleanup`, `mcp_audit_key_rotation`.
3. The command must return metadata-safe bounded labels only; no raw import payload, audit log arguments/results, API key, keyring material, or filesystem path.
4. The command must not call final action commands or trigger file dialogs.
5. Export actions must be marked privacy-sensitive but read-only: `writes_durable_state=false`, `external_transmission=not_sent_externally`.
6. Import overwrite, cleanup, and key rotation must be marked durable writes; Safe Mode must block final execution.
7. Import preflight may state that a pre-change snapshot will be created on execution; it must not state that a snapshot already exists.
8. Data and Privacy tabs must make the first click show preflight. Existing final commands can only run from a distinct continuation action after preflight is visible.
9. Existing `importAllData` governed request must remain intact.
10. Existing Slice 5A provider-transmission history behavior must not regress.

Blocked from Slice 5C1:

- Typed confirmation phrase or deletion/rollback flows.
- Live provider calls.
- Real dangerous action execution in tests outside isolated mocks/test state.
- Backup/snapshot claims that cannot be traced to backend state.
- Replacing backend import governance with frontend-only confirmation.
