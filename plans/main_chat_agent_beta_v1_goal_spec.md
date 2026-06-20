# Main Chat Agent Beta v1 Goal Spec

> Date: 2026-06-18
> Status: implementation entrypoint for future CLI goal work
> Depends on: `plans/main_chat_agent_beta_v1_preparation_index.md`

## 1. Objective

Implement **Main Chat Agent Beta v1: Execution-First Product Integration**
against the current verified repository state.

This goal must start with a foundation inventory. Do not assume Product Maturity
v2 Phase A-G is already implemented unless the repository proves it through
runtime, command-surface, UI, or readiness evidence. If a dependency is missing,
either complete the minimum required slice in the relevant workstream or mark
Beta readiness blocked. A planned document is not implementation evidence.

## 2. Required Reading

Read these files before editing code:

- `AGENTS.md`
- `plans/main_chat_agent_beta_v1_preparation_index.md`
- `plans/main_chat_agent_beta_v1_benchmark_lessons.md`
- `plans/main_chat_agent_beta_v1_default_agent_experience_contract.md`
- `plans/main_chat_agent_beta_v1_real_task_verticals_contract.md`
- `plans/main_chat_agent_beta_v1_planner_executor_quality_contract.md`
- `plans/main_chat_agent_beta_v1_knowledge_assets_contract.md`
- `plans/main_chat_agent_beta_v1_hardening_readiness_contract.md`
- `plans/main_chat_agent_product_maturity_v2_goal_spec.md`
- `plans/main_chat_agent_product_maturity_v2_eval_scenarios.md`
- `plans/openlife_agent_product_capability_matrix_v1.md`

## 3. Implementation Order

Use phase-gated execution. Each phase must leave auditable artifacts before the
next phase starts:

1. Phase 0: produce `plans/main_chat_agent_beta_v1_foundation_inventory.md`
   with verified / partial / missing status.
2. Phase 1: integrate default execution-first Main Chat experience.
3. Phase 2: add real task vertical fixture/harness and scenario reports.
4. Phase 3: improve Planner/Executor quality using existing runtime objects.
5. Phase 4: productize knowledge assets and context inventory.
6. Phase 5: add Beta v1 readiness aggregation and release evidence.

Do not skip ahead if an earlier step fails. Do not replace existing runtime
objects with parallel objects. If CLI goal-mode time or stability makes the full
scope unsafe, stop at the last verified phase and report Beta v1 as incomplete;
do not relabel a partial phase as full Beta completion.

For CLI goal mode, this file is the stage goal. The implementation may complete
multiple phases in one run only if each previous phase passes its own evidence
checks. If Phase 0 or Phase 1 reveals broad missing foundations, stop after
documenting the blocker and produce a smaller follow-up plan.

## 4. Non-negotiable Constraints

- Main Chat remains the entry surface; legacy chat completion must not be the
  hidden default execution path for work-like tasks.
- UI states must be backed by runtime events or records.
- DirectAnswer must remain lightweight but traceable.
- Tools act; skills instruct; knowledge files provide context; policy remains
  authoritative.
- Memory changes require proposal/confirmation/rollback lifecycle.
- Permissions must be exact to action/tool/target/scope.
- Final delivery must distinguish completed, proposed, blocked, skipped, and
  pending work.
- External live evidence is opt-in and never substitutes for deterministic
  readiness.
- No API keys, `.env`, `target/`, `frontend/node_modules/`, generated Tauri
  artifacts, or local provider secrets may be committed.

## 5. Acceptance

The goal is complete only when:

- all five workstream contracts are implemented for their required default
  deterministic scope;
- `plans/main_chat_agent_beta_v1_foundation_inventory.md` exists and includes
  component, status, runtime evidence, command-surface evidence, UI evidence,
  tests, blockers, and development decision for every Beta dependency;
- the foundation inventory covers every foundation listed in
  `plans/main_chat_agent_beta_v1_preparation_index.md` Section 3 and every
  readiness dimension listed in
  `plans/main_chat_agent_beta_v1_hardening_readiness_contract.md`;
- blockers are accepted only for explicitly unsupported, risky, or opt-in live
  behavior. A blocker cannot replace required default Main Chat execution,
  memory lifecycle, event replay, plan interaction, knowledge inspection, or
  final delivery behavior;
- at least 28 deterministic real task scenarios run with structured product
  evidence, and the two external live scenarios remain opt-in and separately
  reported;
- every real task fixture declares `expected_outcome`, and the harness enforces
  success/proposal/expected-blocker/opt-in-live semantics;
- ordinary `send_message` and `start_stream_message` paths are covered;
- UI/task/event/final-delivery states are evidence-backed;
- `run_main_chat_agent_beta_v1_readiness_gate` returns a structured
  `MainChatAgentBetaV1ReadinessReport`;
- deterministic local gates pass;
- opt-in live gates still fail closed without credentials and pass only with
  auditable external provider evidence;
- `git status --short` contains only intentional source/docs changes.

## 6. Required Final Report

The implementation report must list:

- completed work by workstream;
- files changed;
- tests run and results;
- any readiness blockers;
- foundation inventory: verified / partial / missing;
- unsupported behavior that remains out of scope;
- whether external live tests were run;
- whether the branch is safe to commit.
