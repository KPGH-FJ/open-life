# Main Chat Stage 1 User-Visible Acceptance Contract

> Date: 2026-06-18
> Scope: user-visible product acceptance for Stage 1 dogfood
> Status: preparation artifact

## 1. Purpose

Stage 1 succeeds only if a user can see what the agent is doing. Backend records
are necessary but not sufficient.

## 2. Required User-Visible States

Every applicable scenario must render these states from runtime evidence:

- task frame;
- route/strategy;
- context sources;
- plan draft or active plan;
- action queue;
- action running/completed/failed;
- observation preview and source;
- blocker with named reason;
- permission request with exact action/tool/target/scope;
- proposal with accept/reject/defer/edit path;
- memory rollback for materialized memory;
- event replay/reconnect state;
- final delivery sections.

## 3. Final Delivery Contract

Final delivery must separate:

- completed work;
- observations used;
- proposed work;
- pending user action;
- blocked work;
- skipped work;
- durable changes;
- next action.

The UI must not show "done" for proposed, blocked, skipped, or unexecuted work.

## 4. Non-Fake Rules

The UI must not claim:

- an action ran unless `ActionQueue` / transcript / event evidence exists;
- an observation exists unless the runtime produced one;
- web or MCP was used when the result came from model knowledge;
- memory changed unless proposal/acceptance/rollback lifecycle evidence exists;
- a knowledge file was changed when only a proposal was created;
- a plan step executed when only a draft was created;
- a task resumed when stale/terminal state blocked it;
- external live provider was used when a local/mock/fixture provider responded.

## 5. Required Control Behavior

Controls must be enabled only when runtime state supports them:

- resume: resumable task only;
- retry: retryable failed action only;
- cancel: non-terminal task only;
- approve once: exact pending permission proposal/action only;
- deny/defer: pending proposal or permission only;
- accept/reject/edit: pending proposal only;
- rollback memory: accepted materialized memory only;
- plan confirm/edit/execute/skip/review: revisioned PlanExecute session only.

## 6. Acceptance Grading

Each scenario receives:

- `runtimeEvidence`: pass/fail;
- `uiEvidence`: pass/fail;
- `controlEvidence`: pass/fail/not_applicable;
- `finalDeliveryEvidence`: pass/fail;
- `nonFakeEvidence`: pass/fail;
- `overall`: pass/fail/expected_blocker/opt_in_live_blocked.

Expected blockers are acceptable only when the blocker is visible, named, and
the final delivery does not overclaim.
