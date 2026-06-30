# Sprint 0 Diagnosis: Runs, Trace, State Lifecycle, Recovery

Date: 2026-06-29

Status: Diagnosis packet and RFC outline. Not implemented.

## Raw Issues

Primary issues: `OL-007`, `V4-002`, `V4-011`, `V5-007`, `V5-008`, `V6-002`, `V6-007`.

Highest severity:

- `V4-002` P1: File/Web/MCP blockers do not create auditable observations.
- `V5-007` P1: Builder, web blocker, and PlanExecute Runs show zero useful trace.
- `V5-008` P1: ambiguous request remains running for minutes with no cancel/retry/resume.
- `V6-002` P1: UI shows DeepSeek timeout while DB run/session remains running.

## Observed Evidence

- Several Runs detail pages showed zero actions/tools/observations even when task sessions or transcripts should contain blocker/final-delivery evidence.
- v5 ambiguous request stayed running for more than six minutes without a clear product-level cancel/retry/resume path.
- v6 timeout evidence showed UI-level timeout while durable DB state remained `running`, creating false task status.

## Source Findings

| Area | Finding |
|---|---|
| `frontend/src/pages/RunsPage.tsx` | Loads both `listAgentRuns` and `listMainChatAgentTasks`, then builds summaries by run id. This is the right direction but still depends on both stores lining up. |
| `frontend/src/pages/AgentRunDetail.tsx` | Activity timeline can use task transcript when task detail exists, but stats still come from `run.stepCount`, `run.toolCallCount`, `run.actions`, and `run.observations`. |
| `frontend/src/utils/runDisplaySummary.ts` | List outcomes are derived from AgentRun status and action/observation counts, which can under-report task-session transcript evidence. |
| `src-tauri/src/main_chat_task_controls.rs` | Task summaries/details already expose transcript, blockers, proposals, finalDelivery, controls, stale diagnostics, and resume safety. |
| `src-tauri/src/main_chat_generation_support.rs` | Success finalization updates AgentRun and persists final output. Timeout around proposal/vector follow-up logs to stderr but does not represent a general timeout lifecycle contract. |
| `src-tauri/src/main_chat_strategy.rs` | Normal completion completes task session and AgentRun, but error/timeout paths need the same durable terminal-state guarantee. |

## Root-Cause Hypothesis

1. Runs currently has two partially overlapping truth stores: `AgentRun` and Main Chat task session/transcript. UI aggregation can show zero evidence when only one store has the useful event.
2. Timeout and cancellation are not modeled as first-class durable lifecycle states across both `AgentRun` and task session.
3. Error/blocker/fallback states are not always appended as transcript events and normalized into a product event timeline.
4. Existing task controls are stronger than what the product consistently surfaces; controls depend on diagnostic state and linkage to run id.

## Industry Comparison

- Codex cloud and other background-task tools make task status and logs central to trust. A long-running agent product cannot treat Runs as a debug afterthought.
- Cursor/agent workflows commonly distinguish "working", "needs input", "failed", and "complete"; OpenLife needs the same state vocabulary for user recovery.
- Claude Artifacts separates deliverable output from conversation. Runs should also separate final delivery, blockers, and raw trace.

## Solution RFC Outline

### Target State Machine

Use one product lifecycle vocabulary:

- `queued`
- `running`
- `waiting_for_permission`
- `blocked`
- `timed_out`
- `cancelled`
- `failed`
- `completed`

Every terminal state must write:

- AgentRun status/error/final timestamp
- task session status/final timestamp
- transcript event
- user-facing next control or no-control reason

### Product View Model

Add `RunEvidenceView` as the UI read model:

- run id
- task session id
- user objective/title
- lifecycle state
- provider/route evidence
- last safe event
- event timeline from task transcript plus AgentRun fallback
- action count and observation count from normalized evidence
- blockers/proposals/plan/artifact ids
- allowed controls
- redaction status

### UI Contract

- Runs list shows: objective, state, route, last event, next action.
- Runs detail shows a timeline with event type, safe summary, timestamp, and related ids.
- If content is redacted, show metadata-safe reason and what is still available.
- Timeout/cancel/retry/resume controls must be visible only when backed by allowed controls.

### Backend Contract

- Error and timeout paths must call a shared finalizer, not only success path.
- Cancel writes terminal `cancelled` to both stores.
- Retry/resume creates a linked new attempt or appends replay evidence, never silently mutates old evidence.
- Blocker creation writes a transcript event and appears in `RunEvidenceView`.

## Replay Tests

| Test | Expected |
|---|---|
| v6 provider timeout | AgentRun and task session are terminal `timed_out` or `failed`; Runs detail shows timeout event |
| v5 ambiguous request | Clarification or blocker appears; no indefinite running; cancel/retry state visible |
| Web/MCP unavailable | Blocker transcript event appears and action/observation counts are non-zero where appropriate |
| PlanExecute draft | Runs shows plan id, draft/artifact status, and final delivery status |
| Redacted run | User sees safe route/status/event metadata instead of empty zero-count page |

## Anti-Hallucination Checks

- Do not trust the run list status alone; verify task session and transcript.
- Do not infer completion from assistant output; require finalDelivery or terminal state.
- Do not infer timeout from UI toast only; require durable event/state.
- Do not count hidden redaction as absence of evidence; represent redacted evidence explicitly.

## Thin-Slice Implementation Proposal

1. Add backend helper that finalizes both AgentRun and task session for timeout/cancel/error.
2. Add focused regression test for a timed-out provider request leaving no `running` state.
3. Add `RunEvidenceView` builder using task transcript first and AgentRun second.
4. Update Runs list/detail to use normalized counts and event timeline.
5. Replay v5/v6 stuck/timeout cases before moving to LifeModel work.

## Open Questions

- Should stale running runs be auto-marked `timed_out` on app startup, or only when opened/refreshed?
- Should retry create a child run id or reuse the same task session with attempt numbering?
- How much redacted transcript metadata is enough for user trust without leaking sensitive content?
