# Main Chat Agent Product Maturity v2 Goal Spec

> Date: 2026-06-17
> Status: preparation artifact for the next Main Chat Agent phase
> Scope: close the real product gaps left after Productization v1 deterministic readiness

## 1. Goal

Build Main Chat Agent Product Maturity v2.

Productization v1 made Main Chat visibly runtime-backed: task state, actions,
observations, blockers, proposals, controls, final delivery, and deterministic
product accounting are now present. Product Maturity v2 must move the product
from "auditable task surface" to "usable agent workflow surface".

The user should be able to:

- roll back accepted memory updates with provenance,
- watch real task delta events rather than only snapshot-derived events,
- edit, confirm, skip, execute, and review plan steps,
- find and resume existing tasks across sessions,
- inspect and select skills/tools without bypassing policy,
- see product-level external-live evidence when explicitly opted in.

This phase is not a broad autonomy phase. It is not a marketplace phase. It is
not a self-evolution phase. It is a product maturity phase for Main Chat.

Implementation must be phase-gated. This document is the overall product
contract, not a request to implement all of v2 in one goal-mode run. Do not
develop all surfaces in parallel unless the earlier gate is already passing.
Each phase should normally be its own implementation goal, with its own runtime
tests, UI tests where applicable, and deterministic eval accounting. The required
order is:

1. Memory lifecycle and rollback.
2. Durable/replayable event delta stream.
3. Plan interaction objects and controls.
4. Long task continuity list/detail/resume safety.
5. Skills/tool product surface.
6. Opt-in external live product evidence.

Each gate must add focused runtime tests, UI tests when applicable, and
deterministic eval accounting before the next gate begins. A later gate may
extend earlier objects, but it must not weaken an earlier gate to pass.

## 2. Why This Phase Exists

Productization v1 is complete for its deterministic default scope, but it left
known gaps:

- `MP-06` rollback is optional unsupported.
- Events are `snapshot_derived_ordered_events_not_live_delta_stream`.
- Plan edit and skip-step are mostly contract/eval concepts, not mature product
  interactions.
- L3 Plan-Execute is an MVP slice, not a mature planner.
- L4 Memory is proposal/confirmation MVP, not a full memory lifecycle product.
- L5 task continuity uses existing task state, not a full task continuity
  product.
- Skills/tool selection has runtime proof but not a user-facing product surface.
- External live productization is opt-in and not fully mapped to product UI
  evidence.

## 3. Benchmark Lessons To Adopt

These are product patterns, not claims about private internals.

### 3.1 Codex-style lessons

- Instructions and skills should be file-based, inspectable, scoped, and bounded.
- Execution evidence should be concrete: command/action, output/observation,
  failure, retry, and final result are separate objects.
- Users should be able to inspect why a tool or file context was used.
- Long-running work should survive navigation and remain traceable.

### 3.2 Hermes/OpenClaw-style lessons

- The default experience should be "the agent is doing the task", not "the
  assistant is talking about the task".
- Planning, action, observation, blocker, and final delivery should be visible
  as one task frame.
- Tool use and recovery should be normal product behavior, not hidden preview
  mode.
- User approval should unblock exact work, not grant vague permission.

### 3.3 OpenLife-specific lesson

OpenLife must not copy these products blindly. OpenLife has a stronger
governance and LifeModel ambition. The product challenge is to expose that power
without letting governance freeze execution.

## 4. Required Reading

The implementation goal must require these documents:

- `plans/main_chat_memory_rollback_lifecycle_contract_v1.md`
- `plans/main_chat_event_stream_delta_contract_v1.md`
- `plans/main_chat_plan_execute_interaction_contract_v1.md`
- `plans/main_chat_long_task_continuity_contract_v1.md`
- `plans/main_chat_skills_hub_product_contract_v1.md`
- `plans/main_chat_external_live_productization_eval_v1.md`
- `plans/main_chat_agent_product_maturity_v2_eval_scenarios.md`
- `plans/main_chat_agent_control_plane_v2_ui_flow.md`
- `plans/main_chat_agent_productization_v1_goal_spec.md`
- `AGENTS.md`

## 5. In Scope

### 5.1 Memory rollback and lifecycle

- Turn accepted memory/proposal outcomes into inspectable memory lifecycle
  records.
- Implement rollback as a governed mutation with provenance.
- Make rollback visible in Main Chat and Review Center.
- Keep assistant text from becoming durable user fact.
- Because this phase includes memory maturity, `MP-06` must move from optional
  unsupported to supported before final v2 readiness can be claimed.

### 5.2 Real delta event stream

- Add a durable or replayable event stream for task updates.
- Keep snapshot as recovery source, but do not treat snapshot-derived events as
  real live delta proof.
- Support reconnect, dedupe, replay, and out-of-order protection.

### 5.3 Plan interaction maturity

- Make plan draft, confirm, edit, skip, blocked step, executed step, and review
  summary real objects with controls.
- Execute only supported read-only or proposal-first steps in this phase.

### 5.4 Long task continuity

- Add a user-facing task list/detail model for existing Main Chat tasks.
- Show stale, blocked, waiting permission, cancelled, terminal, and resumable
  states.
- Resume only when stored context and permissions are still valid.

### 5.5 Skills/tool product surface

- Add a minimal local Skills/Tools surface: list, inspect, select, explain, and
  use under policy.
- Do not build marketplace-scale discovery.
- Do not let `SKILL.md` override privacy/model/tool policy.

### 5.6 External live productization evidence

- Keep live product scenarios opt-in.
- Prove that external live task traces appear in the same product UI model as
  deterministic task traces.
- Do not lower existing live-provider/final gates.

## 6. Out Of Scope

- Broad background autonomy.
- Proactive task execution without user-defined permission.
- Dangerous writes.
- Full marketplace plugin ecosystem.
- Self-modifying prompts or automatic policy edits.
- Replacing evidence-backed objects with frontend-only UI.
- A broad planner that edits files, calendars, email, or external services.
- Full multi-user or cloud sync.

## 7. Non-negotiable Rules

- For v2 memory maturity, rollback must be a real governed mutation. If this
  cannot be implemented safely, Phase A must stop and report a blocker instead
  of claiming maturity.
- A delta event must come from runtime state transition evidence, not UI local
  reconstruction.
- A plan step is not executed until an action/observation or blocker exists.
- A skipped step needs an explicit event and reason.
- A task resume must validate stale context and permission scope.
- A selected skill is context, not authority.
- A tool candidate is not executable until policy allows it.
- External live opt-in proof cannot replace deterministic default readiness.
- No silent durable writes.

## 8. Phases

### Phase A: Memory lifecycle gate

- Add accepted memory lifecycle records, rollback events, provenance, active
  context exclusion, and materialized-view invalidation/rebuild behavior.
- Add backend commands and UI controls only after rollback can be tied to a
  concrete accepted memory id.
- Gate passes only when `MP-06` has real rollback evidence.

### Phase B: Event delta gate

- Add durable or replayable task events with monotonic per-task sequence,
  replay, dedupe, and snapshot recovery.
- Tauri event emission may be the transport, but durable/replayable event
  records are the proof.

### Phase C: Plan interaction gate

- Add plan/step runtime objects, revisions, confirm/edit/skip/execute/cancel
  commands, and step-to-action/observation/proposal/blocker links.
- UI controls render only when the backing command and valid plan revision
  exist.

### Phase D: Task continuity gate

- Add task list/detail/read model, stale diagnostics, last-safe-resume point,
  and resume/retry/cancel/refresh behavior.
- Resume must revalidate context, tool/skill digests, permission scope, action
  input hash, and terminal state.

### Phase E: Skills/tool surface gate

- Add local skill/tool list/detail/select/clear/candidate surfaces.
- Selected skills must be bounded, digested, traceable, and lower priority than
  privacy/model/tool policy.

### Phase F: External live product evidence gate

- Add opt-in product-level live tests only after deterministic memory, event,
  plan, task, and skill gates pass.
- External live evidence must map into the same UI/event/final-delivery model
  without weakening existing live-provider gates.

### Phase G: Final readiness

- Produce a readiness report that lists supported, blocked, unsupported, and
  future scenarios.

## 9. Acceptance Criteria

Product Maturity v2 is complete only when:

- `MP-06` passes as real rollback with lifecycle, provenance, inactive memory,
  and materialized context update evidence.
- Real delta events are emitted, replayable, and consumed by Main Chat UI.
- Plan edit/confirm/skip/execute/review controls are backed by runtime objects.
- Existing tasks are discoverable and resumable from a task continuity surface.
- Skills/tools are inspectable and selectable without bypassing policy.
- Product-level external live scenarios can run opt-in and map to the same UI
  evidence model.
- The readiness report lists per-gate passed/failed/expected-blocker counts and
  cannot mark mandatory deterministic scenarios unsupported.
- Existing Productization v1, final acceptance, ReAct, live-provider, and
  frontend gates do not regress.

## 10. Stop Conditions

Stop and report blockers if:

- rollback would require unsafe deletion without provenance,
- real delta events cannot be emitted without corrupting current streaming,
- plan controls would be fake frontend-only controls,
- long task continuation would replay stale or permission-sensitive actions,
- skill/tool surface would expose unsafe tools as normal tools,
- live productization needs weaker final/live-provider evidence gates.

## 11. Goal Mode Handoff Rule

When this phase is handed to Codex CLI goal mode, do not paste all preparation
documents into the prompt. Use a short prompt that points to this file as the
authority and names exactly one phase gate.

Default next prompt shape for the first implementation goal:

```text
PLEASE IMPLEMENT Phase A from plans/main_chat_agent_product_maturity_v2_goal_spec.md.

Read the required documents listed in that spec, especially
plans/main_chat_memory_rollback_lifecycle_contract_v1.md and
plans/main_chat_agent_product_maturity_v2_eval_scenarios.md. Keep scope to the
Memory lifecycle gate only: accepted memory lifecycle records, materialization
state, rollback events, provenance, active context exclusion, deterministic
MR-* eval coverage, and UI controls only where backed by real commands. Do not
start event stream, plan interaction, task continuity, skills/tool surface, or
external live product work. Stop and report blockers if rollback cannot be tied
to a concrete accepted memory id and materialized context update evidence.
```

Later goal-mode prompts should replace "Phase A" with the next gate only after
the previous gate has passed. This prompt is intentionally short. The detailed
contract lives in the plans.
