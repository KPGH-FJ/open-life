# Main Chat Stage 2 Agent Control Plane Product Requirements

> Date: 2026-06-19
> Stage: Main Chat Agent Stage 2 - Internal Trial Readiness
> Status: preparation requirements

## 1. Purpose

Stage 1 proved the browser can observe D01-D36. Stage 2 must make the Agent
Control Plane understandable enough for internal users. The target is not visual
polish alone; it is trust, recoverability, and clear task ownership.

## 2. Existing Surfaces To Reuse

- `frontend/src/components/AgentControlPlane.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/components/ToolCallCard.tsx`
- `src-tauri/src/main_chat_task_controls.rs`
- `src-tauri/src/main_chat_event_stream.rs`
- `src-tauri/src/main_chat_runtime_support.rs`
- `src-tauri/src/main_chat_generation_support.rs`

Do not create a second task panel or parallel runtime state model.

## 3. Required User-visible Regions

| Region | Purpose | Required data source |
| --- | --- | --- |
| Task header | Goal, status, task/session/run identity, route. | `AgentTaskSession`, AgentRun trace. |
| Plan/action timeline | What the agent intends to do and what is happening now. | ActionQueue, PlanExecute runtime, transcript entries. |
| Observations | What the agent actually saw from tools/sources. | ExecutionTranscript observations. |
| Blockers | Why execution paused or failed. | ExecutionPolicy/blocker metadata. |
| Controls | Continue, retry, cancel, approve, reject, edit, defer, rollback. | Task controls, ProposalStore, Memory lifecycle. |
| Final delivery | What was completed, proposed, blocked, skipped, and pending. | AgentRun finalization, final delivery sections. |
| Trace details | Expandable diagnostics for reviewers. | Metadata-safe trace/report payloads. |

## 4. State Requirements

| State | UI behavior | Must not do |
| --- | --- | --- |
| `direct_answer` | Show compact answer and optional trace. | Do not show fake actions. |
| `planning` | Show draft plan and editable/confirm controls. | Do not call draft a completed task. |
| `executing` | Show current action and running indicator. | Do not hide active tool/action state. |
| `observed` | Show bounded observation preview and source. | Do not expose sensitive raw payloads. |
| `blocked` | Show reason, affected action, and next controls. | Do not convert blocker into generic apology. |
| `waiting_for_permission` | Show exact scope/risk/target and controls. | Do not approve broader future actions. |
| `proposal_pending` | Show proposal, evidence, and review controls. | Do not materialize memory/file changes. |
| `retry_available` | Show retry only for safe retryable action. | Do not replay stale/terminal actions. |
| `cancelled` | Show terminal cancelled state. | Do not continue queued actions. |
| `completed` | Show final delivery with sources/actions/proposals/blockers. | Do not claim proposed or blocked work is done. |

## 4.1 Runtime Mapping

Stage 2 implementation must map each UI state to existing typed runtime
payloads. It should not introduce a second state model.

| UI state | Primary runtime source | Required proof |
| --- | --- | --- |
| `direct_answer` | AgentRun / DirectAnswer transcript route | no tool/action transcript entries, provider route trace present. |
| `planning` | PlanExecute session / plan draft state | draft/steps/revision id and edit/confirm controls. |
| `executing` | ActionQueue entry | current action id, action type, status running/queued. |
| `observed` | ExecutionTranscript observation entry | observation id, source/tool/action linkage. |
| `blocked` | ExecutionPolicy or blocker transcript metadata | blocker id, reason code, affected action. |
| `waiting_for_permission` | ToolPermission proposal / pending action | exact target/action/scope and approve/deny/defer controls. |
| `proposal_pending` | ProposalStore record | proposal id, type, evidence refs, review controls. |
| `retry_available` | Task controls / retryability metadata | original action id and safe retry reason. |
| `cancelled` | AgentTaskSession terminal status / event stream | cancel event and no later queued execution. |
| `completed` | AgentRun finalization / final delivery sections | completed/proposed/blocked/skipped/pending sections. |

## 5. Product Acceptance

P0 acceptance:

- every Stage 2 P0 task shows a task header or compact DirectAnswer trace;
- every tool action has visible action and observation states;
- every blocker shows reason and at least one safe next step or terminal
  explanation;
- every proposal shows accept/reject/edit/defer/open-review controls when
  applicable;
- every final delivery separates completed, proposed, blocked, skipped, and
  pending work;
- internal reviewers can copy/report task id, run id, scenario id, and blocker
  code.

P1 acceptance:

- reload/replay status is visible after navigation or refresh;
- trace expansion is readable without overwhelming normal users;
- selected skill/tool reason is visible;
- final delivery can link to Review Center proposal or task detail.

## 6. Non-fake Rules

- UI cannot infer executed work from assistant prose.
- UI cannot show an observation without transcript/action evidence.
- UI cannot show provider-backed live success without credited live evidence.
- UI cannot show memory accepted/materialized unless the memory lifecycle says
  accepted/materialized.
- UI cannot hide fallback, legacy path, or policy blocker when those occurred.

## 7. Design Direction

Stage 2 should keep the interface dense and operational. This is an agent task
console inside Chat, not a marketing surface.

Recommended shape:

- compact task header by default;
- timeline with current action emphasized;
- observation previews collapsed after final delivery;
- blocker/proposal controls always visible while pending;
- diagnostics behind an expandable reviewer section;
- final delivery anchored at the bottom of the task frame.
