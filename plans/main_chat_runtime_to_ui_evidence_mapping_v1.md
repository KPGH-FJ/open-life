# Main Chat Runtime To UI Evidence Mapping v1

> Date: 2026-06-16
> Status: required preparation artifact before Main Chat Agent Productization v1
> Parent: `plans/openlife_agent_product_capability_matrix_v1.md`

## 1. Purpose

This document defines how runtime evidence becomes UI state.

The product must not display Agent execution unless the backend has evidence for
it. The mapping below is the contract between Main Chat runtime and frontend
Agent Control Plane.

## 2. Evidence Classes

The canonical strategy/status vocabulary is defined in
`main_chat_agent_control_plane_ui_contract_v1.md`. This document maps evidence
to those UI objects; it does not define a separate route taxonomy.

| Evidence class | Required source | UI object unlocked |
| --- | --- | --- |
| Task/run identity | AgentIngress / AgentTaskSession / AgentRun | TaskSessionView |
| Strategy route | StrategyRouter / task metadata | strategy badge and state |
| Context source | ContextCompiler / context loader | context source list |
| Provider route | ModelRouter / InferenceScheduler trace | provider/model trace |
| Plan | PlanExecute draft or AgentLoop plan trace | PlanView |
| Action | ActionQueue entry or AgentLoop action record | ActionView |
| Observation | ExecutionTranscript observation | ObservationView |
| Policy decision | ExecutionPolicy metadata | risk badge and permission state |
| Blocker | blocker metadata / policy error / runtime failure | BlockerView |
| Proposal | ProposalStore entry | ProposalView |
| Memory candidate | Proposal/evidence/memory pipeline | MemoryCandidate section |
| Final delivery | AgentRun finalization / transcript summary | FinalDeliveryView |

## 3. Mapping Rules

### 3.1 TaskSessionView

| UI field | Runtime source | Missing source behavior |
| --- | --- | --- |
| `taskId` | AgentTaskSession id | Show legacy/unknown trace, never task-complete badge. |
| `runId` | AgentRun id | Hide detailed run trace; show diagnostic gap. |
| `conversationId` | chat command input / message store | Required for Chat rendering. |
| `strategy` | StrategyRouter output | Use `blocked` or `legacy_fallback`, not guessed strategy. |
| `status` | session status + action queue + blocker state | Show conservative blocked/unknown. |
| `contextSources` | ContextCompiler selected sources | Show none or unknown, not inferred sources. |
| `controls` | runtime transition capability | Render only valid controls. |

### 3.2 PlanView

| UI claim | Required evidence |
| --- | --- |
| "Plan drafted" | PlanExecute draft id or AgentLoop plan trace. |
| "Plan confirmed" | User confirmation event or runtime auto-confirm policy. |
| "Step running" | action queue item tied to plan step. |
| "Step completed" | action success plus observation or explicit no-observation result. |
| "Step blocked" | blocker id tied to plan step or action. |
| "Review ready" | review summary generated after execution evidence. |

If a plan exists only in assistant prose, it may be rendered as text, not as a
structured PlanView.

### 3.3 ActionView

| UI claim | Required evidence |
| --- | --- |
| Tool planned | Candidate/plan/action queue evidence. |
| Tool selected | selected target/action metadata from AgentLoop or ActionExecutor. |
| Tool running | queued/running action state. |
| Tool succeeded | action success status and observation id or result id. |
| Tool failed | action failure status and reason code. |
| Retry available | action policy says retryable and no terminal state. |
| Cancel available | action is pending/running and cancellation is supported. |

Model text like "I will read the file" is not action evidence.

### 3.4 ObservationView

| Source kind | Required evidence |
| --- | --- |
| file | workspace resolver target plus file read observation. |
| memory | MemoryStore/search result id and source label. |
| session | session search result id and source label. |
| web | web action result with URL/source metadata. |
| mcp | registered MCP target, action metadata, and observation. |
| skill | selected skill id plus tool/action result. |
| provider | provider response trace, model identity, bounded preview. |

Observation previews must be bounded and source-labeled. If source metadata is
missing, show a diagnostics warning instead of a normal observation card.

### 3.5 BlockerView

| Blocker reason | Runtime source | Required user control |
| --- | --- | --- |
| missing input | router/runtime required input | ask user / cancel |
| permission required | ExecutionPolicy or ToolPermission proposal | approve_once / deny / defer |
| policy blocked | ExecutionPolicy deny | inspect / cancel |
| network unavailable | web policy or provider preflight | retry if safe / cancel |
| provider unavailable | scheduler/provider error | retry if safe / switch route if allowed |
| tool unavailable | MCP/tool registry | choose alternative / cancel |
| invalid model action | AgentLoop validation | retry with guidance / cancel |
| stale context | task resume validation | refresh context / cancel |
| unsafe manifest | MCP/skill safety filter | inspect / cancel |
| outside workspace | workspace resolver | edit path / cancel |

### 3.6 ProposalView

| UI claim | Required evidence |
| --- | --- |
| Proposal created | ProposalStore id. |
| Proposal pending | Proposal status pending review. |
| Proposal accepted | accepted proposal event. |
| Proposal rejected | rejected proposal event. |
| Proposal rolled back | rollback event and target state. |
| Memory candidate | proposal payload plus evidence ids. |
| ToolPermission proposal | pending permission proposal target and scope. |

A proposal card cannot be synthesized solely from assistant text.

### 3.7 FinalDeliveryView

| UI section | Required evidence |
| --- | --- |
| Final answer | final generation or deterministic synthesis. |
| Completed actions | succeeded action ids. |
| Sources used | observation ids referenced by final answer. |
| Proposals created | proposal ids. |
| Blocked items | blocker ids not resolved. |
| Pending user actions | permission/proposal/missing-input states. |
| Next steps | finalizer output or structured follow-up. |

## 4. Fail-closed Rendering Rules

When evidence is missing:

- Missing task id: show normal chat answer with diagnostic trace unavailable.
- Missing strategy: show unknown/blocked, not inferred route.
- Missing action id: do not render action card.
- Missing observation id: do not render observation card.
- Missing proposal id: do not render proposal card.
- Missing policy decision: do not show safe/approved badge.
- Missing final delivery: show assistant text but no completed task badge.
- Missing source label: show source unknown warning.

## 5. Required Backend Payload

The next implementation should expose a Main Chat Agent state payload shaped like:

```ts
type MainChatAgentStatePayload = {
  task: TaskSessionEvidence;
  route: StrategyEvidence;
  context: ContextEvidence[];
  provider: ProviderRouteEvidence;
  plan?: PlanEvidence;
  actions: ActionEvidence[];
  observations: ObservationEvidence[];
  blockers: BlockerEvidence[];
  proposals: ProposalEvidence[];
  finalDelivery?: FinalDeliveryEvidence;
  diagnostics: EvidenceGap[];
};
```

The payload may be assembled from existing runtime objects, but the UI must
consume one coherent view model to avoid recreating runtime logic in React.

### 5.1 Snapshot And Delta Protocol

Main Chat should expose both snapshots and ordered events.

```ts
type MainChatAgentStateSnapshot = MainChatAgentStatePayload & {
  sequence: number;
  emittedAt: string;
};

type MainChatAgentEvent =
  | { type: "task.created"; sequence: number; task: TaskSessionEvidence }
  | { type: "task.updated"; sequence: number; task: TaskSessionEvidence }
  | { type: "route.selected"; sequence: number; route: StrategyEvidence }
  | { type: "context.selected"; sequence: number; context: ContextEvidence[] }
  | { type: "plan.updated"; sequence: number; plan: PlanEvidence }
  | { type: "action.queued"; sequence: number; action: ActionEvidence }
  | { type: "action.updated"; sequence: number; action: ActionEvidence }
  | { type: "observation.created"; sequence: number; observation: ObservationEvidence }
  | { type: "blocker.created"; sequence: number; blocker: BlockerEvidence }
  | { type: "proposal.created"; sequence: number; proposal: ProposalEvidence }
  | { type: "proposal.updated"; sequence: number; proposal: ProposalEvidence }
  | { type: "final_delivery.created"; sequence: number; finalDelivery: FinalDeliveryEvidence }
  | { type: "diagnostic.created"; sequence: number; diagnostic: EvidenceGap };
```

Rules:

- Events are ordered by monotonic `sequence`.
- Events are idempotent by object id and sequence.
- `task.updated` is required when task status, controls, terminal flag,
  `updatedAt`, or top-level delivery/proposal/blocker references change.
- A reconnect must request a fresh snapshot, then continue from the latest
  sequence.
- UI may optimistically show "receiving" or "loading", but not action,
  observation, proposal, or final delivery objects without evidence events.
- Streamed assistant text is not an action event.
- Partial stream failure must preserve partial text separately from final
  delivery.
- `final_delivery.created` is terminal unless a later user action creates a new
  task/run.
- UI must not infer durable top-level task status solely from child events. It
  may show temporary local loading state, but durable status changes require
  `task.updated` or a fresh snapshot.

## 6. Evidence Gap Diagnostics

The UI should receive metadata-safe diagnostics:

| Gap | Meaning |
| --- | --- |
| `missing_task_identity` | Cannot prove task/session/run identity. |
| `missing_strategy_route` | Cannot prove route. |
| `missing_action_evidence` | UI requested action but no action evidence exists. |
| `missing_observation_evidence` | UI requested observation but no observation exists. |
| `missing_proposal_evidence` | UI requested proposal but no proposal exists. |
| `missing_policy_evidence` | Risk/permission cannot be proven. |
| `missing_final_delivery` | Task lacks structured final delivery. |
| `legacy_fallback_visible` | Legacy fallback happened and must be shown. |

Diagnostics are not user-facing by default, but they must be available in trace
or developer mode.

## 7. Test Requirements

Each product UI test must assert:

- displayed task id matches runtime payload
- displayed action ids exist in runtime evidence
- displayed observation ids exist in runtime evidence
- displayed proposal ids exist in runtime evidence
- controls match valid state transitions
- no fake action/observation/proposal/final-delivery cards can render from text
  alone

This mapping is a hard gate. If the frontend cannot prove an object, it must not
display that object as completed execution.
