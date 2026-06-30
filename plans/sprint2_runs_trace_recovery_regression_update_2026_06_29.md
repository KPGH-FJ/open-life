# Sprint 2 Runs / Trace / Recovery Regression Update

Date: 2026-06-29

Status: Slice 2 implementation update. Automated replay now proves durable failure/blocker evidence and normalized Runs display, but the raw audit issues remain `improved` until a fresh browser plus app DB replay is captured.

## Implemented Slice

- Added a shared Main Chat task failure finalizer for timeout, cancelled, provider error, tool error, policy blocker, and unknown error.
- Persisted timeout as `AgentRun.status=Failed` plus `AgentTaskSession.status=Failed` plus typed transcript `failure_kind=timeout`; no native `TimedOut` enum was added.
- Persisted cancel as terminal `Cancelled` on both AgentRun and task session, with a typed transcript event.
- Persisted policy blockers as task-session blocker evidence plus transcript timeline evidence, even when no tool call was executed.
- Added canonical `RunEvidenceView` so Runs list/detail consume one normalized view for lifecycle, route evidence, timeline, controls, redaction, blockers, proposals, and plan refs.
- Updated Runs list/detail to use `RuntimeRouteEvidence` for route chips and to avoid showing zero actions/observations unless both task transcript and AgentRun evidence are actually empty.
- Updated retry/cancel/resume display to use backend `allowed_controls`; terminal failed timeout runs default to trace-only controls.

## Slice 2 Automated Replay Evidence

Scope note: this replay is automated test replay from the current patch. No cloud provider expansion was added, no API key was read, entered, displayed, or tested.

### V6 Provider Timeout

| Field | Evidence |
|---|---|
| Scenario | Provider timeout leaves the durable run/session state running while UI claims timeout. |
| Current backend replay | `failure_finalizer_records_timeout_run_session_and_transcript_evidence` creates an AgentRun plus task session, finalizes `failure_kind=timeout`, and asserts AgentRun `Failed`, task session `Failed`, typed transcript event, `RunEvidenceView.lifecycle_state=timed_out`, and trace-only controls. |
| Current frontend replay | `AgentRunDetail.test.tsx` renders a timed-out detail view from `RunEvidenceView`, shows the timeout event and final event, uses `RuntimeRouteEvidence` for the route chip, and rejects stale legacy `AgentRun.modelRoute` provider text. |
| Verdict | Improved. A fresh browser/app DB replay is still required before marking the raw V6 timeout issue verified. |

### V5 Ambiguous Request: "帮我安排一下"

| Field | Evidence |
|---|---|
| Scenario | Ambiguous request can remain running without clear cancel/retry/resume recovery. |
| Current backend replay | `policy_blocker_finalizer_creates_auditable_detail_event_without_tool_call` proves a policy/clarification-style blocker becomes `RunEvidenceView.lifecycle_state=blocked`, creates transcript evidence without a tool call, and exposes backend controls. |
| Current UI replay | `AgentRunDetail.test.tsx` renders `blocked`, blocker timeline evidence, blockers/proposals/plan refs, and only backend-allowed controls. |
| Verdict | Improved. The original natural-language prompt still needs a fresh browser/app DB replay to confirm it no longer stays indefinitely running. |

### V4 File/Web/MCP Blocker

| Field | Evidence |
|---|---|
| Scenario | File/Web/MCP blocker appears as zero evidence in Runs detail. |
| Current backend replay | Kernel blocker paths call the shared failure finalizer with `failure_kind=policy_blocker`; the focused backend replay proves blocker transcript evidence enters the detail view even with zero executed actions. |
| Current command-surface replay | `main_chat_command_surface` continues to cover send/stream web policy blockers, missing MCP blockers, registered MCP read success, and AgentLoop no-fallback cases through ordinary command surfaces. |
| Current UI replay | Runs list/detail tests use normalized counts and timeline evidence from `RunEvidenceView`; zero-count empty state is not shown unless the canonical view has no evidence. |
| Verdict | Improved. Fresh browser/app DB replay is still pending before marking raw V4 blocker issues verified. |

## Regression Map

| Raw issue | Status | Evidence in this change | Remaining blocker |
|---|---|---|---|
| `OL-007` | improved | Canonical `RunEvidenceView` reconciles AgentRun/task-session/transcript state and routes UI through backend controls. | Needs fresh app DB replay for representative raw runs. |
| `V4-002` | improved | Policy blockers write transcript timeline evidence and enter detail view without relying on tool calls. | Needs fresh file/web/MCP blocker replay in browser plus DB. |
| `V4-011` | improved | Detail route chips use `RuntimeRouteEvidence`; raw transcript stays hidden behind metadata-safe summaries. | Needs raw scenario replay. |
| `V5-007` | improved | Runs list/detail use normalized counts and event timeline instead of misleading zero debug counts. | Needs fresh builder/web/PlanExecute replay. |
| `V5-008` | improved | Blocked lifecycle and backend allowed controls are surfaced; failed terminal states become trace-only unless retry evidence exists. | Needs fresh ambiguous request replay. |
| `V6-002` | improved | Timeout finalizer updates AgentRun/session/transcript and UI shows `timed_out` only from typed evidence. | Needs fresh provider-timeout browser/DB replay. |
| `V6-007` | improved | Detail redaction note and metadata-safe event summaries clarify hidden raw transcript vs retained evidence. | Needs raw scenario replay. |

## Gates Run

- `cargo test -p openlife-core main_chat_agent_v1` -> 31 passed.
- `cargo test -p openlife-tauri main_chat_task_control` -> 13 passed.
- `cargo test -p openlife-tauri main_chat_command_surface` -> 43 passed.
- `cd frontend && corepack pnpm test -- RunsPage.test.tsx AgentRunDetail.test.tsx` -> 2 files passed, 3 tests passed.
- `cd frontend && corepack pnpm typecheck` -> passed.
- `cargo fmt --check` -> passed.
- `git diff --check` -> passed.

## Deferred

- No real live external provider test was run.
- No API key was read, entered, displayed, or tested.
- No fresh browser plus app DB replay was captured in this patch.
- Raw audit issues are therefore not marked fixed/verified here.
