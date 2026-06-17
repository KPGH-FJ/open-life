# Main Chat Agent Beta v1 Preparation Index

> Date: 2026-06-18
> Status: preparation artifact for the next development stage
> Scope: define the next stage after the current verified Main Chat Agent
> productization baseline

## 1. Stage Definition

The next stage is **Main Chat Agent Beta v1: Execution-First Product
Integration**.

This preparation must not assume that every Product Maturity v2 Phase A-G item
is already implemented. Some items may exist as implementation, some may exist
only as contracts or partial slices, and some may still be missing.

Before Beta v1 development starts, the implementation agent must produce a
foundation inventory from the current repository:

- verified: implemented, covered by command-surface/runtime/UI evidence, and
  safe to build on;
- partial: implemented in one layer but missing product, UI, eval, or readiness
  proof;
- missing: still only a plan/contract or not present.

Beta v1 must not rebuild verified foundations under new names. If a required
foundation is partial or missing, the implementation must either complete the
minimum missing piece inside the current workstream or mark Beta readiness
blocked. It must not silently treat planned behavior as completed behavior.

The inventory must be written as an auditable artifact at:

- `plans/main_chat_agent_beta_v1_foundation_inventory.md`

Do not pre-fill that file with optimistic claims. It must be generated from the
current repository during implementation and must include, for each foundation:

- component name;
- status: `verified`, `partial`, or `missing`;
- runtime evidence;
- command-surface evidence;
- UI evidence;
- tests or readiness gates checked;
- blockers or gaps;
- development decision: reuse, extend, complete minimum missing slice, or block
  Beta readiness.

The inventory must cover every item in Section 3 and every readiness dimension
in `plans/main_chat_agent_beta_v1_hardening_readiness_contract.md`. It may add
more components, but it must not omit a listed foundation because the current
implementation does not touch it.

The core product claim for Beta v1 is:

> Chat remains the control surface, but the default path for work-like requests
> is an observable governed Agent task, not a legacy chat completion.

## 2. Five Workstreams

| Workstream | Product question | Preparation document |
| --- | --- | --- |
| 1. Default Agent Experience | Does Main Chat feel like an execution-first agent by default? | `plans/main_chat_agent_beta_v1_default_agent_experience_contract.md` |
| 2. Real Task Verticals | Can the agent complete realistic user tasks instead of only passing synthetic gates? | `plans/main_chat_agent_beta_v1_real_task_verticals_contract.md` |
| 3. Planner/Executor Quality | Does planning, tool use, recovery, and final delivery feel reliable? | `plans/main_chat_agent_beta_v1_planner_executor_quality_contract.md` |
| 4. Knowledge Assets | Are `AGENTS.md`/`USER.md`/`MEMORY.md`/`SOUL.md`/`SKILL.md` usable product assets rather than hidden prompt fragments? | `plans/main_chat_agent_beta_v1_knowledge_assets_contract.md` |
| 5. Beta Hardening | Can we ship this as a beta without hiding failures or overclaiming readiness? | `plans/main_chat_agent_beta_v1_hardening_readiness_contract.md` |

Shared benchmark notes live in:

- `plans/main_chat_agent_beta_v1_benchmark_lessons.md`

The future CLI goal-mode entrypoint is:

- `plans/main_chat_agent_beta_v1_goal_spec.md`

## 3. Current Foundation To Reuse

Beta v1 should reuse these foundations when the foundation inventory verifies
them. Each item must be classified as verified, partial, or missing before code
changes:

- governed Main Chat ingress and strategy routing;
- `AgentTaskSession`, `ActionQueue`, execution transcript, task controls;
- DirectAnswer, ReAct, Plan-Execute, proposal, memory, and blocker paths;
- execution-first UI events and task panel foundations;
- memory lifecycle and rollback;
- durable/replayable task delta events;
- plan edit/confirm/skip/execute/review objects;
- long task continuity list/detail/resume safety;
- skills/tool product surface and selected `SKILL.md` plumbing;
- external live product evidence gate;
- final readiness aggregation.

If an implementation needs to reintroduce a parallel task, memory, plan, skill,
or event object, that is a design smell. Prefer extending the existing object
and adding missing fields, tests, or UI states.

## 4. Industry Lessons To Apply

This stage should learn from first-class agents by behavior, not by copying
private internals.

Codex-style lessons:

- Durable instructions should be inspectable files with clear scope.
- Reusable workflows should be skills with progressive disclosure.
- Good agent work includes tests, review, and explicit verification.
- Execution evidence should be concrete enough for the user to inspect.

Claude/Hermes/OpenClaw-style lessons:

- Tool execution must be the normal path for work-like requests.
- Prompt/context assembly should separate stable identity, project context,
  volatile memory, selected skills, and ephemeral turn guidance.
- Skills teach workflows; tools perform actions; plugins/integrations add new
  runtime capability.
- Memory should be bounded, inspectable, editable, and not treated as an
  unlimited prompt dump.
- Browser/external automation should use isolated, permissioned surfaces.

OpenLife-specific lesson:

- OpenLife's governance is an advantage only if it unlocks reliable execution.
  If governance only blocks product behavior, the product will still feel worse
  than ordinary agent systems.

## 5. Non-duplication Rules

- Do not rebuild Product Maturity v2 Phase A-G under new names.
- Do not create a second event stream if the completed delta stream can be
  extended.
- Do not create a second memory product if rollback/proposal records already
  exist.
- Do not add a parallel skill registry if the skills/tool surface can support
  the requirement.
- Do not add a frontend-only task state that is not backed by runtime evidence.
- Do not label unsupported behavior as beta-ready.

## 6. Evidence Rules

Every key product claim needs one of these proof types:

- runtime object proof: session, action, event, plan, proposal, memory, skill, or
  final delivery record;
- command-surface proof: ordinary `send_message` and `start_stream_message`
  path, not hidden test-only path;
- UI proof: component renders the state from runtime data and exposes valid
  controls;
- eval proof: deterministic scenario assertions, plus external live proof only
  where explicitly opted in;
- negative proof: unsupported or risky behavior fails closed with a named
  blocker.

No product surface may claim that the agent read, executed, remembered, resumed,
approved, or delivered something unless the matching runtime evidence exists.

## 7. Suggested Development Order

0. Foundation inventory and prerequisite reconciliation.
1. Default Agent Experience integration.
2. Real task vertical harness and fixtures.
3. Planner/Executor quality pass.
4. Knowledge asset manager and inspection flows.
5. Beta hardening and release gate.

For CLI goal mode, each step is a checkpoint. If a checkpoint cannot be completed
without broad redesign or unverified assumptions, stop, report Beta v1 as
incomplete, and leave the next checkpoint as follow-up work. Do not expand the
scope silently.

This order is intentional. Without the foundation inventory, the goal can build
on false assumptions. Without the default experience and real tasks,
planner/executor improvements can optimize the wrong target. Without knowledge
assets, OpenLife will keep hiding its memory advantage. Without hardening, the
stage will overclaim readiness.

## 8. Out Of Scope For Beta v1

- Broad background autonomy.
- Full marketplace or ClawHub-scale plugin ecosystem.
- Dangerous writes.
- Self-evolution or automatic skill rewriting as a shipped capability.
- Multi-agent swarm orchestration.
- Full cloud sync or multi-device collaboration.
- Replacing deterministic gates with external live-only evidence.

## 9. Entry Criteria For Development

Before starting implementation, the developer must read:

- this file;
- all five workstream contracts;
- `plans/main_chat_agent_beta_v1_benchmark_lessons.md`;
- `plans/main_chat_agent_product_maturity_v2_goal_spec.md`;
- `plans/main_chat_agent_product_maturity_v2_eval_scenarios.md`;
- `plans/openlife_agent_product_capability_matrix_v1.md`;
- `AGENTS.md`.

It must then run or inspect the strongest available readiness path and produce
`plans/main_chat_agent_beta_v1_foundation_inventory.md`. If a referenced Product
Maturity v2 gate does not yet exist, that is a finding, not permission to invent
a passing result. Beta v1 implementation may continue only for workstreams whose
dependencies are verified or explicitly completed in the same change.

## 10. Exit Criteria For Beta v1

Beta v1 can be considered complete only when:

- the foundation inventory is attached to the final report and no unverified
  foundation is claimed as complete;
- ordinary Main Chat routes work-like tasks into visible governed task sessions;
- direct answers remain lightweight but traceable;
- at least 28 default-readiness real task scenarios run with structured product
  evidence;
- real task evals cover direct answer, read tools, ReAct, plan-execute-review,
  memory proposal/rollback, task continuity, skills/tool selection, permission,
  failure recovery, and final delivery;
- UI states are backed by runtime objects, not local inference;
- final delivery clearly separates completed work, proposed work, blocked work,
  skipped work, and next user action;
- deterministic gates pass;
- external live gates remain opt-in and auditable;
- no silent durable writes occur.
