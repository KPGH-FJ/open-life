# Main Chat Agent Productization v1 Goal Spec

> Date: 2026-06-16
> Status: next development goal spec
> Scope: Main Chat Agent Control Plane + L0-L2 product completeness + narrow L3/L4/L5 continuity

## 1. Goal

Build Main Chat Agent Productization v1.

The user must be able to give OpenLife a task in Main Chat, see the Agent route
the task, watch real runtime-backed actions and observations, handle blockers or
proposals inline, recover from common failures, and receive a structured final
delivery.

This goal is not a broad Agent rewrite. It is the productization layer on top of
the governed runtime already built in Main Chat Agent Execution v1.

## 2. Required Reading

Before coding, read these documents and treat them as the product contract:

- `plans/openlife_agent_product_capability_matrix_v1.md`
- `plans/main_chat_agent_product_eval_scenarios_v1.md`
- `plans/main_chat_agent_control_plane_ui_contract_v1.md`
- `plans/main_chat_runtime_to_ui_evidence_mapping_v1.md`
- `plans/main_chat_permission_proposal_memory_ux_contract_v1.md`
- `plans/main_chat_final_delivery_contract_v1.md`
- `AGENTS.md`

Do not re-interpret this goal as "match Hermes" or "build all future Agent
features". The documents above define the target.

## 3. In Scope

### 3.1 Agent Control Plane

- Add a runtime-backed Agent task state payload for Main Chat.
- Expose task/session/run, route, context, provider route, plan, actions,
  observations, blockers, proposals, diagnostics, and final delivery.
- Support snapshot and ordered event/delta semantics, including `task.created`
  and the complete minimum event set from
  `plans/main_chat_runtime_to_ui_evidence_mapping_v1.md`.
- Keep UI rendering fail-closed: no action, observation, proposal, or final
  delivery object may render without evidence.

### 3.2 Main Chat UI

- Add an execution-first task panel or equivalent Agent Control Plane surface in
  Main Chat.
- Render DirectAnswer as compact governed output with optional trace.
- Render read actions, ReAct actions, observations, blockers, proposals,
  permissions, and final delivery from runtime evidence.
- Add valid controls only when the backend state supports them: continue, retry,
  cancel, approve once, deny, defer, edit plan, accept/reject/edit proposal,
  rollback, open trace, open Review Center.
- Keep streaming states accurate. Streamed assistant text is not action evidence.

### 3.3 L0-L2 Product Completeness

- L0 DirectAnswer: low-noise answer, task/run trace, provider/model/context
  disclosure, no fake timeline.
- L1 Governed Read: visible file, memory/session, fixture web, and MCP read
  execution with source/observation and final synthesis.
- L2 Multi-step ReAct: visible plan/action/observation/follow-up timeline with
  blocker and retry/cancel behavior.

### 3.4 Narrow L3/L4/L5 Slice

- L3 MVP only: draft plan, confirm/edit plan, execute one read-only step, show
  one blocked/skipped step, and produce review summary. Do not build a broad
  planner.
- L4 MVP only: create memory proposal, show evidence/scope, accept/reject/edit
  or defer, link Chat to Review Center, and support rollback by removing or
  superseding accepted materialization when the rollback scenario is claimed as
  supported. If full rollback mutation is too large for this goal, the rollback
  scenario must be reported as an optional unsupported blocker and cannot count
  toward L4 MVP completion.
- L5 MVP only: visible resume/retry/cancel for existing task states, including
  stale, blocked, cancelled, and terminal no-resume cases. Do not build a new
  long-running task system beyond these state/control surfaces.

### 3.5 Product Eval

- Convert `plans/main_chat_agent_product_eval_scenarios_v1.md` into a
  machine-readable deterministic scenario fixture or equivalent structured test
  input.
- Add product-level assertions for runtime evidence and UI state.
- Default product gate must use deterministic fixtures and mock IPC/UI tests.
- External live scenarios are opt-in only and must not count toward default
  deterministic readiness.

## 4. Out Of Scope

- Broad autonomous background work.
- Dangerous external writes.
- Broad write tools.
- Marketplace-scale plugin or Skill ecosystem.
- Self-modifying prompts, hidden self-evolution, or automatic policy edits.
- Replacing runtime evidence with frontend-only timeline state.
- Calling a proposal completed execution.
- Treating memory files or `SKILL.md` as a way around privacy/model/tool policy.
- Making legacy default chat a hidden fallback path.

## 5. Non-negotiable Rules

- A proposal is not completion.
- A plan is not execution.
- A final answer is not proof of tool use.
- A UI timeline is invalid unless backed by runtime evidence.
- A memory candidate is not durable memory.
- A knowledge file is not higher priority than privacy/model/tool policy.
- A fallback answer must be labeled as fallback.
- A blocked task must remain visibly blocked until the blocker is resolved.
- A permission approval must be scoped to the exact pending action.
- A completed task must show what was done, not just what the model said.
- No silent durable writes to LifeModel, memory, files, external services, or
  plugin state.

## 6. Implementation Phases

### Phase A: Contracts And Fixtures

1. Add typed shared contracts for Agent strategy route, task status, delivery
   status, proposal status, control names, and evidence ids.
2. Create deterministic product scenario fixtures from the scenario document.
3. Add tests that verify the scenario fixture routes use only canonical strategy
   values.
4. Add tests that verify external live scenarios are excluded from default gate.
5. Add tests that verify every `task_control` scenario fixture has explicit
   preconditions and target references: prior task/session/run id, target action
   id or proposal id when applicable, control action, expected state transition,
   and negative assertions.

### Phase B: Runtime State Payload

1. Build a Main Chat Agent state assembler that maps existing runtime evidence
   into a single payload.
2. Include task/session/run ids, strategy route, context sources, provider route,
   actions, observations, blockers, proposals, final delivery, and diagnostics.
3. Add snapshot support and ordered event support for the minimum event set:
   `task.created`, `task.updated`, `route.selected`, `context.selected`,
   `plan.updated`, `action.queued`, `action.updated`,
   `observation.created`, `blocker.created`, `proposal.created`,
   `proposal.updated`, `final_delivery.created`, and `diagnostic.created`.
4. Add `task.updated` whenever top-level task status, controls, terminal state,
   or top-level references change.
5. Add fail-closed diagnostics for missing evidence.
6. Phase B is a hard gate: do not implement Phase C UI rendering until payload
   assembly, snapshot generation, event ordering, and evidence-gap diagnostics
   have passing tests.
7. `task_control` routes must be resolved against existing runtime objects. If a
   prior task/action/proposal cannot be found, the route must return a visible
   blocker or diagnostic rather than creating a fake control result.

### Phase C: UI Control Plane

1. Add Agent task panel rendering in Main Chat.
2. Render compact DirectAnswer trace.
3. Render action timeline only when action evidence exists.
4. Render observation cards only when observation evidence exists.
5. Render proposal cards only when proposal evidence exists.
6. Render final delivery block from canonical final delivery object.
7. Add tests proving fake action/observation/proposal/final-delivery cards cannot
   render from assistant text alone.

### Phase D: L0-L2 Product Flow

1. Complete DirectAnswer product trace.
2. Complete visible read-only execution for file, memory/session, fixture web,
   and MCP read scenarios.
3. Complete visible multi-step ReAct timeline and follow-up synthesis.
4. Ensure explicit tool requests do not become DirectAnswer unless the router
   records a no-tool-needed reason.
5. Keep legacy fallback visible and non-completing.

### Phase E: Proposal, Permission, Memory, And Recovery

1. Add permission request cards for exact pending actions.
2. Add ToolPermission proposal card and exact-action resume behavior.
3. Add memory proposal card with evidence, scope, confidence/conflict where
   available, and accept/reject/edit/defer controls.
4. Add Review Center linkage for proposals.
5. Add provenance visibility for accepted memory/proposal outcomes. Add rollback
   only if MP-06 is claimed supported; otherwise report MP-06 as optional
   unsupported blocker.
6. Add resume/retry/cancel UI and state assertions for long-task scenarios.

### Phase F: Final Delivery And Gate

1. Add canonical final delivery generation/rendering.
2. Ensure final delivery separates executed, proposed, blocked, pending, durable
   changes, and next steps.
3. Add product eval gate that runs deterministic scenarios.
4. Keep existing runtime/final/live-provider gates passing.
5. Produce a final readiness report that lists passed scenarios, unsupported
   scenarios, blockers, and remaining gaps.
6. The readiness report must include route-based counts for
   `direct_answer`, `read_action`, `react_tool_execution`, `plan_execute`,
   `memory_proposal`, `permission_request`, `task_control`, and `blocked`,
   split by passed, failed, expected blocker, and unsupported.
7. The readiness report must explicitly state rollback support status:
   implemented or optional unsupported blocker, with the scenario id and reason.

## 7. Acceptance Criteria

This goal is complete only when all of the following are true:

- Main Chat has a runtime-backed Agent Control Plane surface.
- The UI can show DirectAnswer, read action, ReAct, PlanExecute, proposal,
  permission, blocked, failed, cancelled, and completed task states.
- Every displayed action maps to runtime evidence.
- Every displayed observation maps to runtime evidence.
- Every displayed proposal maps to ProposalStore evidence or equivalent test
  fixture evidence.
- Every displayed final delivery maps to canonical final delivery evidence.
- No UI claims execution from assistant text alone.
- Default deterministic product scenarios run and report pass/fail.
- External live scenarios are opt-in only and not required for default readiness.
- DirectAnswer scenarios pass.
- Every deterministic scenario row in
  `plans/main_chat_agent_product_eval_scenarios_v1.md` is accounted for exactly
  once as passed, expected blocker, or explicit unsupported blocker. A scenario
  cannot disappear from acceptance because its route is `task_control`,
  `blocked`, or `permission_request`.
- Mandatory L0-L2 and MVP L3/L4/L5 success scenarios cannot be marked
  unsupported. Unsupported blockers are allowed only for explicitly optional
  scenarios, and every unsupported scenario must be listed by id with a concrete
  justification.
- Deterministic route-based scenario accounting passes:
  - all `direct_answer` scenarios pass
  - all expected `read_action` success scenarios pass, including at least one
    file, memory/session, fixture web, and registered MCP read success
  - all expected `react_tool_execution` success scenarios pass, including at
    least one multi-action ReAct task with two observations
  - all expected `plan_execute` MVP success scenarios pass for draft, confirm,
    one executed read step, one blocked/skipped step, and review summary
  - all expected memory proposal flow MVP scenarios pass for create, edit,
    accept, reject/defer, evidence/scope, and Review Center link, even when the
    user-turn route is `task_control`; MP-06 rollback must either pass as real
    rollback or be reported as optional unsupported blocker, and cannot count
    toward L4 MVP completion unless implemented
  - all expected negative `blocked` scenarios pass as blockers
  - all expected `permission_request` scenarios pass as permission/proposal
    flows, not as read success
  - all expected `task_control` scenarios pass with prior-object references,
    exact target validation, and expected state transitions
  - all external live opt-in scenarios are excluded from default readiness
- 8 permission/blocker scenarios pass.
- 8 long-task recovery scenarios pass.
- 8 skill/tool selection scenarios pass, except explicitly unsafe/write-like
  tool cases must pass as expected blockers.
- 8 final delivery scenarios pass.
- No silent durable writes occur.
- No proposal is rendered as completed durable change before acceptance.
- No blocked task is rendered as completed.

## 8. Test Plan

Run at minimum:

```bash
git diff --check
cargo test -p openlife-core main_chat_agent_v1 -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance_tests -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface_tests -- --nocapture
cargo test -p openlife-tauri main_chat_live_provider_tests -- --nocapture
cargo test -p openlife-tauri main_chat_react_boundary_tests -- --nocapture
cargo test -p openlife-tauri main_chat_react_unit_tests -- --nocapture
pnpm --dir frontend typecheck
pnpm --dir frontend test -- --run
```

Add and run new tests for:

- product scenario fixture validation
- Agent state payload assembly
- snapshot/event ordering
- `task.updated` top-level state updates
- complete minimum event set coverage
- UI state rendering
- anti-fake action/observation/proposal/final delivery rendering
- permission/proposal/memory card behavior
- final delivery section rendering

Add at least one auditable main productization gate command. Prefer these names
unless the implementation requires a clearly documented alternative:

```bash
cargo test -p openlife-tauri main_chat_agent_productization_v1 -- --nocapture
pnpm --dir frontend test -- --run AgentControlPlane
```

The final report must list the exact new gate command names that were added and
run.

If live-provider tests are affected, keep them opt-in and do not weaken existing
live-provider final gate.

## 9. Deliverables

- Runtime state payload and event/snapshot support.
- Main Chat Agent Control Plane UI.
- Product scenario fixture or structured eval input.
- Product eval runner/report.
- Permission/proposal/memory UI minimum slice.
- Final delivery UI and evidence mapping.
- Updated tests.
- Updated docs if implementation changes the contracts.

## 10. Stop Conditions

Stop and report blocker rather than marking complete if:

- UI needs to display actions without backend evidence.
- Runtime cannot expose task/action/observation/proposal/final delivery evidence
  without large unrelated refactor.
- Phase B runtime payload/snapshot/event/evidence-gap tests cannot pass before
  UI rendering work.
- Product eval cannot distinguish deterministic scenarios from live opt-in
  scenarios.
- Product eval cannot separate expected success routes from expected blocker or
  permission routes.
- Product eval cannot prove `task_control` scenarios target existing prior
  tasks/actions/proposals with exact state transitions.
- Mandatory scenarios are being marked unsupported to make the gate pass.
- Existing stabilization/final acceptance gates regress.
- Scope pressure pushes into broad autonomy, broad write tools, marketplace
  plugins, or self-evolution.

## 11. Suggested CLI Goal Prompt

Use this short prompt for goal mode:

```text
PLEASE IMPLEMENT plans/main_chat_agent_productization_v1_goal_spec.md.

Follow the required-reading documents referenced in that spec. Keep scope limited
to Main Chat Agent Productization v1: Agent Control Plane, runtime-backed
task/action/observation/blocker/proposal/finalDelivery payload, L0-L2 product
completion, narrow L3/L4/L5 MVP slice, deterministic product eval, and final
delivery.

Do not implement broad autonomy, broad write tools, marketplace-scale Skill
ecosystem, self-evolution, or fake frontend-only execution UI. Do not weaken
existing final/live-provider gates. Do not start UI rendering before the runtime
payload/snapshot/event/evidence-gap gate is passing. Do not mark mandatory
success scenarios unsupported. `task_control` scenarios must reference existing
prior objects and prove exact state transitions. Stop and report blockers if
runtime evidence cannot support the UI contract.
```
