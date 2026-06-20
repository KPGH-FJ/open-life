# Main Chat Stage 3 Current Gap Inventory

> Date: 2026-06-20
> Stage: Stage 3 - Execution UX and Main Chat Internal Alpha Candidate
> Status: preparation inventory

## 1. Current Assets To Reuse

| Area | Current state | Reuse direction |
| --- | --- | --- |
| Agent runtime | `AgentIngress`, StrategyRouter, `AgentTaskSession`, `ActionQueue`, `ExecutionTranscript`, AgentLoop, ActionExecutor, ExecutionPolicy are already in the Main Chat path. | Reuse as source of truth. Do not create a parallel runtime. |
| Task controls | Resume, retry, cancel, permission-preserving resume, proposal controls, plan controls, and task detail helpers exist in `src-tauri/src/main_chat_task_controls.rs` and related command modules. | Tighten UI wiring and object-scoped control display. |
| Agent state payload | `src-tauri/src/main_chat_agent_state_payload.rs` builds typed snapshots with task, route, context, actions, observations, blockers, proposals, plan, provider, and final delivery. | Treat this as the main UI contract for Stage 3. |
| AgentControlPlane | `frontend/src/components/AgentControlPlane.tsx` renders task header, reviewer trace, event stream summary, context/provider/plan, plan interaction, actions, observations, blockers, proposals, controls, and final delivery. | Productize and simplify it into the primary Main Chat task surface. |
| ChatPage integration | `frontend/src/pages/ChatPage.tsx` renders `AgentControlPlane` when `currentAgentState` exists and still has a fallback execution task panel when only ingress/task state exists. | Remove duplication where possible and make missing-state fallback compact and diagnostic. |
| Stage 2 readiness | `run_main_chat_agent_stage2_readiness_gate` audits Stage 1/Beta, manual dogfood, live provider, control plane, memory, recovery, final delivery, and safety evidence. | Keep as final evidence gate. Stage 3 should not replace it. |
| Manual dogfood tooling | S2-D01 through S2-D24 artifact template, validator, and reviewer worksheet exist. | Do not fill fake rows in Stage 3. Preserve for post Stage 3/4/5 acceptance. |

## 2. Stage 3 Product Gaps

| Gap | Current symptom | Product risk | Stage 3 target |
| --- | --- | --- | --- |
| Duplicate execution surfaces | `AgentControlPlane` and the older compact `currentAgentIngress` / task-state fallback can both represent execution concepts. | Users and reviewers may not know which surface is authoritative. | One primary control plane; fallback only when typed state is missing and clearly diagnostic. |
| Active execution visibility during streaming | While sending/streaming, the UI can show a spinner or streaming reply before full task state appears. | The product still feels like chat completion first and agent execution second. | Show active task shell as soon as task/session/run identity is available from existing `MainChatAgentStateSnapshot`, task detail, event stream, or ingress/task ids. |
| Timeline readability | Actions, observations, blockers, proposals, event summary, and final delivery exist but are not yet a single coherent execution timeline. | Users cannot quickly answer "what is it doing now?" or "what did it actually observe?" | Timeline groups plan/action/observation/blocker/final delivery with current action emphasis. |
| Blocker next actions | Blockers exist and controls exist, but the relationship can be indirect. | Users see failure but not a clear recovery path. | Each blocker shows reason, affected action/proposal if any, and exact safe next controls or terminal explanation. |
| Scoped controls | Controls exist across task, proposal, permission, and plan surfaces. | Approve/retry/resume can feel broad or ambiguous. | Every control displays exact object scope: task id, action id, proposal id, plan step, target, or permission scope. |
| Final delivery trust | Final delivery sections exist, but Stage 3 must make them the terminal contract for all task outcomes. | Proposed/blocked/skipped work can be misread as completed. | Final delivery always separates completed, proposed, blocked, skipped, pending, durable changes, and next steps. |
| Reload and navigation recovery | Task continuity panel can inspect previous tasks, but current Main Chat task surface may depend on in-memory state. | Internal testers lose context after refresh or navigation. | Current conversation can reload most recent task state and event stream into the primary control plane using existing task session store, event stream, or conversation-linked task metadata only. |
| Reviewer trace quality | Reviewer trace strip exists with task/run/status/route/blockers. | Manual dogfood later still needs scenario/build/provider context and a stable copy format. | Add bounded one-line JSON copy payload suitable for S2-D rows without adding fake manual evidence. |
| Visual density and hierarchy | The panel is functional but can become dense, especially with plan steps, proposals, and final delivery. | Internal users may ignore the state surface and report "it just chatted." | Keep compact default, expand details on demand, and prioritize current state and next action. |
| Source/citation clarity | Observations show source previews; final answers may not always visibly connect to observation ids. | Users cannot verify what the agent used. | Actions and observations show source labels, bounded previews, and final delivery references. |

## 3. High-risk Files

Stage 3 is likely to touch:

- `frontend/src/components/AgentControlPlane.tsx`
- `frontend/src/components/AgentControlPlane.test.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/ChatPage.test.tsx`
- `frontend/src/tauri.ts`
- `frontend/src/test/mocks/tauri.ts`
- `src-tauri/src/main_chat_agent_state_payload.rs`
- `src-tauri/src/main_chat_task_controls.rs`
- `src-tauri/src/main_chat_event_stream.rs`
- `src-tauri/src/commands/agent_runtime/mod.rs`
- Stage 3 focused tests in `src-tauri/src/main_chat_agent_productization_tests.rs`
  or a new focused test module if extraction is cleaner.
- A focused deterministic Stage 3 UX coverage report/test surface for `UX3-01`
  through `UX3-13`. This must not replace or weaken Stage 2 readiness.

Shared runtime strategy files such as `src-tauri/src/main_chat_strategy.rs`
should only be touched when a UI state cannot be truthfully derived from
existing runtime evidence.

## 4. Out Of Scope For Stage 3

- Running or filling the 24 P0 manual dogfood rows.
- Redesigning memory lifecycle or knowledge asset management. That is Stage 4.
- Building final internal-trial release packaging and support process. That is
  Stage 5.
- Public Skills Hub or marketplace.
- Broad background autonomy.
- Arbitrary external writes.
- Replacing Stage 2 readiness gate.

## 5. Stage 3 Done Means

Stage 3 is done when a real internal tester can open Main Chat, issue common
task-like requests, watch the Agent execute or block, use the available
controls, understand final delivery, and copy trace evidence for later manual
dogfood.

It does not mean the product is already approved for limited internal trial.
That approval still requires Stage 4, Stage 5, S2-D01 through S2-D24 manual
dogfood, current-commit live evidence, and a passing Stage 2 readiness gate.
