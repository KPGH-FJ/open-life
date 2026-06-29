# Sprint 2 Solution RFC: Runs, Trace, Recovery

Date: 2026-06-29

Status: ready for Slice 2A after Sprint 1 route evidence DTO is committed. Timeout representation is frozen below; do not add a competing lifecycle model during implementation.

## Scope

Raw issues: `OL-007`, `V4-002`, `V4-011`, `V5-007`, `V5-008`, `V6-002`, `V6-007`.

Primary source entrypoints:

- `src-tauri/src/main_chat_task_controls.rs`
- `src-tauri/src/main_chat_generation_support.rs`
- `src-tauri/src/main_chat_strategy.rs`
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/pages/AgentRunDetail.tsx`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/utils/runDisplaySummary.ts`

## Product Goal

Runs must be the trustworthy task ledger: what happened, what state it is in, what evidence exists, what failed, and what the user can do next.

## Non-Goals

- Do not expose raw sensitive transcript text.
- Do not replace the full AgentRun store.
- Do not implement full cloud-provider telemetry here; consume Sprint 1 route evidence and leave transmission details to Sprint 5.

## Lifecycle Contract

| State | Meaning | Allowed next states |
|---|---|---|
| `queued` | Task/session created, no work started. | `running`, `cancelled`, `failed` |
| `running` | Work is in progress. | `waiting_for_permission`, `blocked`, `timed_out`, `cancelled`, `failed`, `completed` |
| `waiting_for_permission` | User decision is required. | `running`, `cancelled`, `failed` |
| `blocked` | Runtime cannot proceed until a recoverable condition changes. | `running`, `cancelled`, `failed` |
| `timed_out` | Runtime exceeded timeout. | terminal, or linked retry |
| `cancelled` | User cancelled. | terminal, or linked retry |
| `failed` | Non-timeout failure. | terminal, or linked retry |
| `completed` | Final delivery exists. | terminal |

Schema guard: `timed_out` is a normalized product lifecycle target, not a verified existing `AgentTaskSessionStatus` or `AgentRunStatus` variant. Current source defines task statuses as `Running`, `WaitingPermission`, `Blocked`, `Completed`, `Failed`, and `Cancelled`, and run statuses as `Running`, `WaitingPermission`, `Completed`, `Failed`, and `Cancelled`.

Frozen implementation choice for Slice 2A: do not add a native `TimedOut` enum or migration in this slice. Persist timeout as:

- `AgentRun.status = Failed` with a metadata-safe `AgentRunError` whose code/kind identifies timeout.
- `AgentTaskSession.status = Failed` with `final_summary` containing a safe timeout summary.
- A transcript `Error` event with `failure_kind="timeout"`, `normalized_lifecycle_state="timed_out"`, `source_ref`, and `directWritesExecuted=false`.
- `RunEvidenceView.lifecycle_state = "timed_out"` only when the failed run/session has that typed timeout evidence.

Without the typed `failure_kind=timeout` transcript/error evidence, failed runs remain `failed`; the UI must not infer `timed_out` from stale `running`, duration, or error copy alone.

Every transition to terminal or blocked state must write:

- AgentRun status/error/finished_at if run exists.
- Task session status/final timestamp.
- Transcript event with metadata-safe reason.
- `next_recommended_control`.

## RunEvidenceView

Frontend should consume one normalized view.

| Field | Meaning |
|---|---|
| `run_id` | AgentRun id if available. |
| `task_session_id` | task session id if available. |
| `title` | safe objective/title. |
| `lifecycle_state` | normalized state above. |
| `route_evidence` | Sprint 1 `RuntimeRouteEvidence` subset. |
| `event_timeline` | transcript events first, AgentRun fallback second. |
| `action_count` | normalized count from task actions plus AgentRun fallback. |
| `observation_count` | normalized observations/blockers/final delivery. |
| `blockers` | blocker ids/reasons. |
| `proposals` | proposal ids/statuses. |
| `plan_refs` | plan/session/artifact ids. |
| `allowed_controls` | open_trace, refresh_context, cancel, retry, resume, review_permission. |
| `redaction_state` | none, partial, metadata_only. |

## Timeout / Error Finalizer

Create a shared backend helper:

`finalize_main_chat_task_failure(state, run_id?, task_session_id?, failure_kind, safe_reason, source_ref)`

Required behavior:

- Idempotent.
- Does not overwrite completed runs.
- Writes safe error to AgentRun.
- Blocks or fails task session with matching reason.
- Appends transcript event.
- Returns normalized `RunEvidenceView` or enough ids for frontend refresh.
- Treats `timeout` as `Failed + failure_kind=timeout` per the schema guard above.
- Is the only new writer for timeout/provider/tool/policy terminal failures in this slice.

`failure_kind` values:

- `timeout`
- `cancelled`
- `provider_error`
- `tool_error`
- `policy_blocker`
- `unknown_error`

## UI Contract

- Runs list: objective, state, route chip, last safe event, next action.
- Runs detail: event timeline, route evidence, blocker/proposal/plan refs, controls.
- Zero-count empty state is allowed only when no task transcript and no AgentRun evidence exists.
- Redaction must say what was redacted and what remains available.

## Tests

These tests must distinguish native session status from normalized product status so timeout is not accidentally claimed as an existing enum variant. Every command gate must record a non-zero matched/passed test count.

Backend:

- Provider timeout finalizes AgentRun and task session.
- Cancel running task writes terminal state and transcript event.
- Retry links to prior run/session and does not mutate old evidence.
- Blocker creates transcript event and appears in detail view.

Candidate command-level gates after adding the focused tests:

- `cargo test -p openlife-core main_chat_agent_v1`
- `cargo test -p openlife-tauri main_chat_task_control`
- `cargo test -p openlife-tauri main_chat_command_surface`

Frontend:

- Runs list uses normalized counts from task transcript.
- Runs detail timeline shows timeout/blocker/final events.
- Controls reflect allowed backend controls.

Candidate command-level frontend gates after adding/updating the focused tests:

- `cd frontend && corepack pnpm test -- RunsPage.test.tsx`
- `cd frontend && corepack pnpm test -- AgentRunDetail.test.tsx`

Current repo check: `frontend/src/pages/AgentRunDetail.tsx` exists, but `frontend/src/pages/AgentRunDetail.test.tsx` does not yet exist. Sprint 2 must add this focused test file or replace the command with another exact existing test file before claiming the gate passed.

Replay:

- v6 timeout run no longer remains `running`.
- v5 ambiguous request no longer runs indefinitely.
- v4 blocker case creates auditable event.

## Development Slices

1. Add failure finalizer.
2. Add `RunEvidenceView` backend builder or frontend normalizer.
3. Switch Runs list to normalized view.
4. Switch Runs detail timeline/stats to normalized view.
5. Replay stuck/timeout/blocker cases.

Exit only when timeout and blocker evidence are visible without DB inspection.

## Slice 2A Entry Checklist

- Sprint 1 `RuntimeRouteEvidence` is committed and consumed as the route field inside `RunEvidenceView`; do not rebuild provider/fallback truth from prose or raw labels.
- Timeout storage uses `Failed + failure_kind=timeout`; no native `TimedOut` enum or schema migration in this slice.
- New tests must assert both stored status and normalized lifecycle state, so `Failed` cannot be accidentally displayed as `timed_out` without typed evidence.
- Add `frontend/src/pages/AgentRunDetail.test.tsx` if detail behavior is changed; otherwise replace the candidate gate with an exact existing focused test before claiming completion.
- Replay evidence must include task session status, AgentRun status, transcript event, route evidence, Runs list/detail UI, and next recommended control.
