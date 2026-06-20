# Main Chat Agent Product Maturity v2 Eval Scenarios

> Date: 2026-06-17
> Status: preparation artifact for Product Maturity v2
> Parent: `plans/main_chat_agent_product_maturity_v2_goal_spec.md`

## 1. Purpose

This document defines the first scenario matrix for Product Maturity v2.

The matrix focuses on gaps left after Productization v1:

- memory rollback,
- real event deltas,
- plan interaction,
- long task continuity,
- skills/tool product surface,
- external live product UI evidence.

## 2. Scenario Contract

Every scenario must define:

- `id`
- `capabilityGroup`
- `prompt`
- `preconditions`
- `expectedRoute`
- `requiredRuntimeEvidence`
- `requiredUiState`
- `requiredControls`
- `negativeAssertions`
- `expectedOutcome`: `pass`, `expected_blocker`, or `unsupported`
- `defaultGate`: true or false

The implementation must convert this document into a machine-readable fixture
or typed scenario builder before claiming v2 readiness. Markdown tables are not
the gate. The gate must fail if any scenario row below is missing from the
structured fixture.

For every table row below, the structured fixture must include all fields in
this contract. The "Required evidence" column is human shorthand, not the full
gate contract. The machine-readable fixture must expand each row into exact
preconditions, route, runtime evidence, UI states, controls, and negative
assertions. A row cannot pass merely because its id appears in a table.

Minimum structured fields:

```json
{
  "id": "MR-03",
  "capabilityGroup": "memory_lifecycle",
  "prompt": "Roll back the memory I just accepted.",
  "preconditions": {
    "acceptedMemoryId": "memory-mr-03",
    "proposalId": "proposal-mr-03",
    "materializedViewVersion": "before"
  },
  "expectedRoute": "task_control",
  "requiredRuntimeEvidence": [
    "memory_id",
    "rollback_event_id",
    "materialized_view_version"
  ],
  "requiredUiState": ["rollback_visible", "memory_inactive"],
  "requiredControls": ["rollback_memory"],
  "negativeAssertions": [
    "no_silent_memory_write",
    "rolled_back_memory_not_in_runtime_context"
  ],
  "expectedOutcome": "pass",
  "defaultGate": true
}
```

## 3. Memory Rollback And Lifecycle

| ID | Prompt | Expected outcome | Required evidence |
| --- | --- | --- | --- |
| MR-01 | "Remember that I prefer execution-first agents." | pass | memory proposal, evidence id, scope. |
| MR-02 | "Accept that memory." | pass | accepted memory lifecycle record. |
| MR-03 | "Roll back the memory I just accepted." | pass | rollback event, inactive memory, materialized view update. |
| MR-04 | "Roll back the memory about execution." with two matches. | expected_blocker | ambiguity blocker and choices. |
| MR-05 | "Roll back that memory again." | expected_blocker | already rolled back terminal state. |
| MR-06 | "Do not remember that." | pass | rejected proposal, no active memory. |
| MR-07 | "This applies only to this project." | pass | scoped memory proposal. |
| MR-08 | "Show why you proposed that memory." | pass | evidence/provenance visible. |

Required MR fixture coverage:

- `MR-01` must prove memory proposal route, pending proposal UI, evidence id,
  scope, and no direct memory write.
- `MR-02` must prove task-control or proposal-acceptance route, accepted
  lifecycle record, and explicit `pending_materialization` or `materialized`
  state.
- `MR-03` must prove rollback command, rollback event, inactive memory, and
  changed materialized context version.
- `MR-04` must prove ambiguity blocker and candidate choices without mutating
  any memory record.
- `MR-05` must prove already-rolled-back terminal handling without creating a
  second rollback event.
- `MR-06` must prove rejection control and absence from active memory/runtime
  context.
- `MR-07` must prove scoped proposal and no global materialization.
- `MR-08` must prove evidence/provenance panel visibility and no unsupported
  confidence claim.

## 4. Real Delta Event Stream

| ID | Prompt | Expected outcome | Required evidence |
| --- | --- | --- | --- |
| EV-01 | Simple direct answer. | pass | route.selected and final_delivery.created events. |
| EV-02 | Read a workspace file. | pass | action.queued, action.completed, observation.created. |
| EV-03 | Tool fails then blocker appears. | pass | action.failed and blocker.created. |
| EV-04 | Accept a proposal. | pass | proposal.accepted event. |
| EV-05 | Reconnect after missed events. | pass | replay since sequence. |
| EV-06 | Duplicate event received. | pass | UI ignores duplicate. |
| EV-07 | Sequence gap. | pass | UI requests replay or snapshot. |
| EV-08 | Streaming text mentions a tool. | pass | no action event created from text. |

Required EV fixture coverage:

- Passing event scenarios must compare emitted event ids/sequences with replayed
  event ids/sequences.
- `EV-05` must simulate disconnect and prove replay from last applied sequence.
- `EV-06` must send a duplicate event and prove idempotent UI/application
  state.
- `EV-07` must simulate a sequence gap and prove replay or snapshot recovery.
- `EV-08` must prove streamed assistant text cannot unlock action,
  observation, proposal, or final-delivery runtime objects.

## 5. Plan Interaction

| ID | Prompt | Expected outcome | Required evidence |
| --- | --- | --- | --- |
| PI-01 | "Plan this work before executing." | pass | plan.created draft. |
| PI-02 | "Confirm this plan." | pass | plan.confirmed. |
| PI-03 | "Edit step 2 before running." | pass | plan revision and updated step. |
| PI-04 | "Run the first read-only step." | pass | step action and observation. |
| PI-05 | "Skip the unsupported step." | pass | step.skipped with reason. |
| PI-06 | "Run a write-like step." | expected_blocker | permission/proposal/blocker, no write. |
| PI-07 | "Cancel remaining steps." | pass | queued steps cancelled. |
| PI-08 | "Review what happened." | pass | plan.reviewed summary. |

Required PI fixture coverage:

- Plan scenarios must use or extend the existing PlanExecuteSession APIs unless
  a new command is explicitly justified in the implementation notes.
- Every row must include `planId`, `revision`, and stable step ids.
- Edit, execute, and skip scenarios must include `baseRevision`.
- A stale revision scenario must fail closed with a visible blocker.
- UI controls must be hidden or disabled if the backing command is absent.

## 6. Long Task Continuity

| ID | Prompt | Expected outcome | Required evidence |
| --- | --- | --- | --- |
| LT2-01 | Open task list. | pass | task summaries. |
| LT2-02 | Open blocked task detail. | pass | blockers, last observation, next control. |
| LT2-03 | Resume after exact permission acceptance. | pass | same action id/target replay. |
| LT2-04 | Resume after target changed. | expected_blocker | scope mismatch diagnostic. |
| LT2-05 | Retry failed safe read. | pass | retried action event. |
| LT2-06 | Continue stale task. | expected_blocker | stale context warning. |
| LT2-07 | Resume completed task. | expected_blocker | terminal no-resume. |
| LT2-08 | Reopen app and inspect task. | pass | persisted task detail. |

Required LT2 fixture coverage:

- Task list/detail must load from `AgentTaskSessionStore` or a documented read
  model, not raw chat text inference.
- Resume scenarios must include context digest, tool or skill digest,
  permission scope, action input hash, and terminal-state checks.
- Stale task scenarios must prove no automatic replay.
- Reopen scenarios must prove persistence across a fresh app/state instance.

## 7. Skills And Tool Surface

| ID | Prompt | Expected outcome | Required evidence |
| --- | --- | --- | --- |
| SK2-01 | Select a local skill. | pass | selected skill id, bounded preview. |
| SK2-02 | Ask why this skill was selected. | pass | selection reason. |
| SK2-03 | Execute safe read tool. | pass | candidate, policy allow, observation. |
| SK2-04 | Attempt write-like tool. | expected_blocker | permission/proposal/blocker. |
| SK2-05 | Unselected skill exists. | pass | not injected. |
| SK2-06 | Unsafe manifest in registry. | expected_blocker | excluded/blocked. |
| SK2-07 | Tool fails once. | pass | failure and retry/alternative. |
| SK2-08 | Clear selected skill. | pass | no selected skill in next task context. |

Required SK2 fixture coverage:

- Selected skill traces must include stable skill id and bounded instruction
  digest.
- Unselected skills must be absent from prompt/context evidence.
- Tool candidates must include policy decision and candidate digest.
- Write-like or unsafe tools must not be rendered as normal executable read
  tools.
- Clearing a skill must change the next task context evidence.

## 8. External Live Productization

These are opt-in only and excluded from default gate.

| ID | Prompt | Expected outcome | Required evidence |
| --- | --- | --- | --- |
| LIVE-PROD-01 | External provider direct answer. | pass | provider trace and final delivery. |
| LIVE-PROD-02 | External web read. | pass | live action/observation/source. |
| LIVE-PROD-03 | External MCP selection. | pass | candidate ranking and observation. |
| LIVE-PROD-04 | ToolPermission live proposal. | pass | exact action proposal, no read overlap. |
| LIVE-PROD-05 | Live failure recovery. | pass | blocker and safe controls. |
| LIVE-PROD-06 | Live event deltas. | pass | event sequence range. |

## 9. Default Gate Requirements

Default gate must:

- include MR, EV, PI, LT2, and SK2 deterministic scenarios,
- exclude LIVE-PROD scenarios,
- report per-capability pass/fail/expected-blocker counts,
- fail if rollback is claimed but not real,
- fail if UI controls are not backed by commands,
- fail if event deltas are synthetic frontend-only events,
- fail if skill selection bypasses policy,
- fail if task continuity replays stale or changed-target actions,
- fail if any scenario row lacks a structured fixture entry with complete
  fields,
- fail if a scenario uses only schema/assertion stubs without runtime object
  creation or UI-state proof.

## 10. Readiness Counts

Minimum deterministic target:

- memory lifecycle: 8 scenarios,
- event delta stream: 8 scenarios,
- plan interaction: 8 scenarios,
- long task continuity: 8 scenarios,
- skills/tool surface: 8 scenarios.

External live target:

- 6 opt-in product scenarios.

Recommended focused gate names:

- `main_chat_agent_product_maturity_v2_memory_tests`
- `main_chat_agent_product_maturity_v2_event_tests`
- `main_chat_agent_product_maturity_v2_plan_tests`
- `main_chat_agent_product_maturity_v2_task_continuity_tests`
- `main_chat_agent_product_maturity_v2_skills_tests`
- `main_chat_agent_product_maturity_v2_gate`

The final combined gate should call or aggregate the focused gates rather than
replacing them with schema-only checks.

## 11. Stop Conditions

Stop if:

- mandatory deterministic scenarios are marked unsupported,
- rollback cannot be linked to accepted memory id,
- plan edit/skip lacks runtime command,
- event stream lacks replay,
- task list depends on raw chat text inference,
- skill surface exposes unsafe tools.
