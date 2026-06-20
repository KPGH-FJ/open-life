# Main Chat Stage 1 Manual Dogfood Protocol

> Date: 2026-06-18
> Scope: human dogfood process for Stage 1
> Status: preparation artifact

## 1. Purpose

Automated gates cannot fully judge whether OpenLife feels like a useful agent.
Stage 1 needs manual dogfood to catch confusing UI, weak final delivery, poor
tool-selection explanations, and failure states that technically pass but feel
bad.

## 2. Dogfood Roles

Minimum reviewers:

- one engineer familiar with runtime internals;
- one product reviewer focused on user experience;
- one reviewer who did not implement the current Stage 1 changes.

## 3. Manual Run Set

Manual reviewers should run:

- every P0 scenario from the Stage 1 matrix;
- at least 8 P1 scenarios;
- at least 4 seeded task-control scenarios;
- at least 3 memory/proposal scenarios;
- at least 2 plan scenarios;
- at least 3 failure/recovery scenarios;
- opt-in live scenarios only when provider credentials are intentionally
  available.

## 4. What To Record

For each manual run:

- scenario id;
- prompt/action used;
- route observed;
- whether the Agent Control Plane was understandable;
- whether action/observation evidence matched the final answer;
- whether controls were discoverable and correctly enabled;
- whether final delivery separated done/proposed/blocked/skipped;
- whether any behavior felt like ordinary chat instead of agent execution;
- screenshots or transcript snippets when useful;
- severity: blocker, major, minor, polish.

## 5. Blocker Criteria

Stage 1 cannot be considered internal-trial-ready if manual dogfood finds:

- work-like prompt returns plain chat with no task frame;
- final answer claims execution without action evidence;
- silent durable memory/knowledge write;
- high-risk or external write runs without permission;
- proposal accept/reject/rollback corrupts state;
- stale resume replays unsafe action;
- event replay loses current task state;
- UI hides a blocker or makes completion status ambiguous.

## 6. Non-Blocking Issues

These can be tracked for later polish if core evidence is correct:

- wording is awkward but clear;
- icon choice could improve;
- spacing/layout can be tightened;
- trace labels need friendlier copy;
- scenario takes longer than ideal but completes correctly.
- one P1 scenario is manually skipped with an explicit reason and automated
  coverage remains passing.

## 7. Manual Report

The implementation should create or update:

- `plans/main_chat_stage1_manual_dogfood_report.md`

The report should list completed manual runs, blockers, accepted residual risks,
and recommendation:

- `not_ready`;
- `ready_for_engineering_dogfood`;
- `ready_for_internal_trial`.

`ready_for_internal_trial` requires every P0 manual run to pass and no blocker
or major issue in the required P1 manual sample. Otherwise, the highest
recommendation is `ready_for_engineering_dogfood`.
