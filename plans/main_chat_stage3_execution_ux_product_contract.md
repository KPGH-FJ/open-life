# Main Chat Stage 3 Execution UX Product Contract

> Date: 2026-06-20
> Stage: Stage 3 - Execution UX and Main Chat Internal Alpha Candidate
> Status: preparation contract

## 1. Objective

Make Main Chat feel like an Agent control console, not a legacy chat completion
surface with hidden backend state.

The user should be able to answer five questions from the screen:

1. What goal is the Agent working on?
2. What is it doing now?
3. What did it observe?
4. Why did it stop or ask for approval?
5. What is completed, still pending, or blocked?

## 2. Product Boundary

Stage 3 owns execution UX only:

- active task surface;
- action/observation/blocker timeline;
- permission/proposal/plan/recovery controls as rendered controls;
- final delivery presentation;
- reload/resume visibility;
- reviewer trace export for future manual dogfood.

Stage 3 does not own:

- memory asset manager and long-term knowledge UX beyond displaying existing
  proposal/memory runtime state;
- broad tool/Skill marketplace;
- final internal trial operations and release process;
- changing readiness semantics.

## 3. Required Runtime Sources

| UX object | Source of truth | UI may infer? |
| --- | --- | --- |
| Task identity | `AgentTaskSession` / `agentTaskSessionId` / AgentRun | No |
| Route | StrategyRouter / AgentIngress route | No |
| Plan | PlanExecute runtime payload | No |
| Action | `ActionQueue` entry | No |
| Observation | `ExecutionTranscript` observation / action observation metadata | No |
| Blocker | ExecutionPolicy, task pending blockers, action failure metadata | No |
| Permission | ToolPermission proposal and pending action metadata | No |
| Proposal | `ProposalStore` / linked proposal metadata | No |
| Memory accepted/rolled back | Memory lifecycle store and proposal provenance | No |
| Event stream | Main Chat event stream | No |
| Final delivery | AgentRun finalization / final delivery payload | No |

If the source is missing, the UI must show pending, unknown, blocked, or
diagnostic state. It must not fabricate action or observation evidence.

## 4. Required Main Chat States

| State | Required UI behavior | Required control behavior |
| --- | --- | --- |
| Direct answer | Compact task trace with route/provider/context when available; no fake action list. | No task control unless runtime provides one. |
| Planning | Plan summary, steps, revision, editable/confirm/review controls when supported. | Controls include exact plan session and revision. |
| Executing | Current action emphasized; queued/running status visible. | Cancel may be visible if task is non-terminal. |
| Observed | Bounded observation preview with source/action linkage. | Retry only if action failed and runtime marks safe retry. |
| Blocked | Reason code, affected action/proposal, and safe next action or terminal explanation. | Resume/retry/cancel only when runtime allows. |
| Waiting for permission | Exact target, action type, risk/scope, approve once, deny, defer controls. | Approval must apply only to pending scoped action. |
| Proposal pending | Proposal type, evidence/source, status, accept/reject/edit/defer/open-review controls. | Durable write cannot be shown until accepted/materialized evidence exists. |
| Cancelled | Terminal cancelled state and stopped queued actions. | No resume unless runtime explicitly supports a new task path. |
| Failed | Failure reason and recovery controls if available. | No hidden fallback to completed. |
| Completed | Final delivery anchored at task end. | Shows completed/proposed/blocked/skipped/pending separation. |

## 5. P0 User-visible Requirements

- Main Chat shows a task shell as soon as task/session/run identity is known.
  The shell must use existing snapshot/task detail/event stream/ingress data;
  it must not introduce a parallel frontend-only task model.
- The primary task surface is `AgentControlPlane`; older fallback surfaces are
  diagnostic only and must not duplicate primary claims.
- Every action row shows action id, type, target/source, status, and linked
  observation/blocker when available.
- Every observation row shows source kind, source label, bounded preview, and
  action linkage.
- Every blocker row shows reason code, affected object, recoverability, and
  exact next controls or terminal explanation.
- Every permission/proposal control shows what object/scope it will affect.
- Final delivery always separates completed, proposed, blocked, skipped,
  pending, durable changes, and next steps.
- Reloading the conversation can recover the most recent relevant task state
  into the primary task surface from existing task session store, event stream,
  or conversation-linked task metadata. It must not infer task identity from
  fuzzy message-text matching.
- Reviewer trace export includes task id, run id, status, route, blocker codes,
  provider/model when available, and final delivery status. The copied payload
  must be a bounded one-line JSON object with stable keys:
  `schemaVersion`, `taskId`, `runId`, `status`, `route`, `blockers`,
  `provider`, `model`, `finalDeliveryStatus`, and `timestamp`. Tests should
  validate timestamp presence/format/boundedness, not an exact wall-clock value.

## 6. P1 User-visible Requirements

- Timeline can be collapsed/expanded by section without hiding current blockers
  or pending approvals.
- Event stream can show more than the last few event labels when expanded.
- Source/citation references in final delivery link back to observations.
- The UI distinguishes deterministic/local execution, external live provider,
  and blocked/no-provider state without exposing secrets.
- The control plane remains readable on narrow desktop and mobile widths.

## 7. Non-fake Rules

- Do not show "running" after the task is terminal.
- Do not show "completed" for proposal-pending or blocked work.
- Do not show action success without action/observation evidence.
- Do not show memory accepted/materialized without memory lifecycle evidence.
- Do not show provider-backed success from local/mock/scripted/fixture evidence.
- Do not hide `legacyFallbackUsed` if it appears in state.
- Do not use S2-D manual dogfood artifact rows as Stage 3 generated output.

## 8. Acceptance Matrix

| ID | Scenario | Expected proof |
| --- | --- | --- |
| UX3-01 | Direct answer | Compact trace, no fake action, final delivery if available. |
| UX3-02 | File read success | Action and observation visible with file source preview. |
| UX3-03 | Missing file | Blocker visible with missing source reason and next action. |
| UX3-04 | Web policy blocker | Web/network blocker visible, no fake web observation. |
| UX3-05 | Registered MCP read | Selected target and observation visible. |
| UX3-06 | Tool permission proposal | Pending permission/proposal controls scoped to target/action. |
| UX3-07 | Plan draft | Plan steps and confirm/edit/skip/cancel/review controls visible when available. |
| UX3-08 | Memory proposal after read | Proposal pending visible; no materialized memory claim. |
| UX3-09 | Retry failed read | Retry control scoped to failed action and new observation/blocker visible after retry. |
| UX3-10 | Cancel task | Terminal cancelled state and no continued queued actions. |
| UX3-11 | Final delivery | Completed/proposed/blocked/skipped/pending sections remain separate. |
| UX3-12 | Reload recovery | Latest task state and event stream reload into primary control plane. |
| UX3-13 | Reviewer trace | Copy payload includes task/run/status/route/blockers and metadata-safe ids. |

## 9. Stage 3 Exit Criteria

- `UX3-01` through `UX3-13` have deterministic frontend or command-surface
  coverage.
- A focused Stage 3 UX coverage report/test surface lists every UX3 row as
  passed, failed, or blocked with named blockers. This is not a readiness gate
  and must not return `ready_for_limited_internal_trial`.
- The execution-first claim requires at least `UX3-02`, `UX3-03`, `UX3-04`,
  `UX3-06`, `UX3-09`, `UX3-11`, and `UX3-12` to pass.
- Stage 1, Beta, product maturity, command-surface, final acceptance, and Stage
  2 readiness tests still pass or fail only for documented external/manual
  evidence blockers.
- The implementation report states that Stage 3 is an internal alpha candidate
  UX milestone and does not grant `ready_for_limited_internal_trial`.
