# Main Chat Plan-Execute Interaction Contract v1

> Date: 2026-06-17
> Status: preparation artifact for Product Maturity v2
> Parent: `plans/main_chat_agent_product_maturity_v2_goal_spec.md`

## 1. Purpose

This document defines the Main Chat product contract for editable
Plan-Execute-Review.

Productization v1 covers a narrow PlanExecute MVP. Product Maturity v2 must make
plan draft, confirm, edit, skip, execute, block, and review into real
user-facing interactions backed by runtime state.

## 2. Baseline

OpenLife already has:

- PlanExecute foundations,
- AgentTaskSession,
- ActionQueue,
- ExecutionTranscript,
- proposal-first write safety,
- Productization v1 plan scenarios.

Missing:

- first-class Main Chat plan object lifecycle,
- real edit plan command,
- real skip step command,
- plan confirmation command,
- step-to-action linkage,
- review summary object,
- plan UI controls connected to backend commands.

Existing OpenLife command surface already includes `create_plan_execute_session`,
`get_plan_execute_session`, `list_plan_execute_sessions`,
`update_plan_execute_session_draft`, `finalize_plan_execute_session`,
`cancel_plan_execute_session`, and `execute_plan_execute_step`. Product Maturity
v2 should extend or adapt this command surface before adding new parallel Main
Chat plan commands.

## 3. Benchmark Lessons

### Codex-style lesson

Plans are useful only when they are tied to real actions and editable work.
Users should see the relationship between a plan step and the executed command
or observation.

### Hermes-style lesson

Planning should not be a static proposal. The agent should move through plan,
action, observation, blocker, and review in one visible task.

### OpenLife constraint

OpenLife must keep write-like work proposal-first. Plan execution in this phase
should focus on read-only execution and proposal generation, not broad writes.

## 4. Plan Object

Required fields:

- `planId`
- `taskSessionId`
- `runId`
- `status`: `draft`, `awaiting_confirmation`, `confirmed`, `executing`,
  `reviewing`, `completed`, `blocked`, `cancelled`
- `revision`
- `goal`
- `steps`
- `createdAt`
- `updatedAt`
- `confirmedAt`
- `reviewId`
- `sourceEvidenceIds`
- `supersededByPlanId`

## 5. Step Object

Required fields:

- `stepId`
- `planId`
- `index`
- `title`
- `description`
- `kind`: `read`, `proposal`, `ask_user`, `blocked`, `manual`, `unsupported`
- `status`: `draft`, `queued`, `running`, `completed`, `skipped`, `blocked`,
  `failed`, `cancelled`, `needs_reconfirmation`
- `revision`
- `basePlanRevision`
- `linkedActionIds`
- `linkedObservationIds`
- `linkedProposalIds`
- `blockerIds`
- `skipReason`
- `policyDecisionId`

## 6. Commands

Minimum backend behavior:

- `finalize_plan_execute_session` or equivalent confirmation behavior.
- `update_plan_execute_session_draft` or equivalent structured edit behavior.
- `execute_plan_execute_step` or equivalent read/proposal step execution.
- `cancel_plan_execute_session` or equivalent cancellation behavior.
- New skip/review commands only if existing PlanExecute APIs cannot represent
  those transitions.

Commands must return updated agent state or event deltas.

If new command names are introduced, the implementation must document why the
existing PlanExecute product commands cannot be extended safely. The default
approach is to reuse and harden the current PlanExecuteSession API.

Patch requirements:

- `patch` must be structured, not free-form assistant prose.
- Supported patch operations are `replace_goal`, `add_step`, `remove_step`,
  `replace_step`, `reorder_steps`, and `change_step_kind`.
- Patch application must return the new `revision`.
- Patch conflict must return a visible blocker/diagnostic, not silently merge.

## 7. Execution Rules

- Draft plan is not completion.
- Confirmed plan can execute only supported steps.
- Read steps may auto-execute if policy allows.
- Proposal steps create proposals; they do not apply durable changes.
- Write-like steps produce permission/proposal/blocker, not direct writes.
- Skip step must create durable step event with reason.
- Editing a confirmed plan must invalidate or explicitly preserve affected
  queued steps.
- Executing an old plan revision is forbidden.
- Step execution must verify `baseRevision` matches the current plan revision.
- If editing a confirmed plan affects a queued/running step, the implementation
  must either cancel that step or mark it `needs_reconfirmation`.
- A step can claim completion only when linked action, observation, proposal, or
  blocker evidence exists.

## 8. UI Contract

Plan panel v2 must show:

- goal,
- status,
- step list,
- active step,
- step controls,
- linked observations/proposals,
- blocked/skipped state,
- review summary,
- final delivery relationship.

Allowed controls:

- confirm plan,
- edit plan,
- execute next read step,
- skip step,
- cancel plan,
- open trace,
- review completed plan.

Controls render only when backend command exists and plan state allows it.

## 9. Review Summary

Review summary must separate:

- completed steps,
- skipped steps,
- blocked steps,
- proposals created,
- observations used,
- unresolved questions,
- recommended next action.

Review cannot claim work completed without linked step/action evidence.

## 10. Eval Scenarios

Minimum scenarios:

- create draft plan,
- confirm plan,
- edit draft plan before execution,
- reject stale edit against old revision,
- execute one read step,
- create proposal step,
- skip unsupported step,
- blocked write-like step,
- cancel remaining queued steps,
- produce review summary.

## 11. Acceptance

This contract is satisfied when:

- plan/step objects are runtime-backed,
- UI controls call real commands,
- step execution creates action/observation/proposal/blocker evidence,
- skipped steps are visible and auditable,
- stale plan revision execution is blocked,
- final delivery distinguishes plan, execution, and review.

## 12. Stop Conditions

Stop if:

- plan edit is only a frontend prompt,
- skip step does not create durable step evidence,
- step execution cannot link to action queue,
- write-like steps would silently execute,
- review summary is generated without runtime evidence.
