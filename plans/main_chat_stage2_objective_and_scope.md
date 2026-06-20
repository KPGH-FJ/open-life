# Main Chat Stage 2 Objective And Scope

> Date: 2026-06-19
> Stage: Main Chat Agent Stage 2 - Internal Trial Readiness
> Status: preparation contract

## 1. Objective

Stage 2 turns Stage 1 automated engineering dogfood into a product state that a
small internal group can use and review.

The target recommendation is:

```text
ready_for_limited_internal_trial
```

This is stricter than `ready_for_engineering_dogfood` and narrower than public
beta. It means OpenLife can be used by internal reviewers on a bounded task set
with visible execution, recoverable failures, proposal-first memory, explicit
permissions, and auditable traces.

## 2. Entry Evidence

Stage 2 starts from these verified foundations:

- Stage 1 automated engineering dogfood passed in Linux CI with real
  `tauri_command_surface_browser_observed` evidence.
- Stage 1 observed 36 scenarios, passed 36 journeys, had 0 failed journeys, and
  had no blockers in the successful CI artifact.
- Beta v1 foundations already include governed Main Chat ingress, DirectAnswer,
  ReAct read/blocker paths, Plan-Execute foundations, proposal-first memory,
  event replay, long task continuity, selected `SKILL.md`, knowledge assets,
  and final readiness aggregation.
- External live provider proof and manual dogfood remain separate and were not
  part of Stage 1 completion.

## 3. Product Definition For Stage 2

OpenLife Stage 2 is successful only if an internal reviewer can:

- start from Main Chat;
- issue a realistic work-like request;
- see the Agent classify, plan, execute, block, or ask for confirmation;
- inspect actions and observations;
- recover from safe failures;
- review memory/knowledge proposals before they become durable context;
- understand final delivery and any remaining pending work;
- report failures with trace ids and scenario ids.

## 4. Non-goals

Stage 2 must not expand into:

- broad background autonomy;
- arbitrary external writes;
- public Skills Hub or marketplace;
- new parallel task/event/memory/plan/tool object models;
- unrestricted web or MCP access;
- full enterprise sync or multi-device rollout;
- public beta release;
- self-evolution or automatic prompt/memory rewrites without review.

## 5. Required Workstreams

| Workstream | Purpose | Reuse from current repo | Stage 2 output |
| --- | --- | --- | --- |
| Manual dogfood | Discover product failures not visible in deterministic gates. | Stage 1 manual protocol and scenario matrix. | Filled internal trial dogfood report. |
| Live provider eval | Prove real model/provider behavior for DirectAnswer, ReAct, MCP, proposal, and recovery slices. | Existing live-provider harness and fail-closed gates. | External-live report with pass/fail/blocker metadata. |
| Control plane UX | Make execution understandable to internal users. | `AgentControlPlane`, ChatPage, task events, final delivery. | Productized task panel states and UI acceptance tests. |
| Memory proposal flow | Make memory/knowledge updates reviewable and reversible. | ProposalStore, Memory lifecycle, EvidenceStore, bounded knowledge loader. | Reviewable memory proposal trial flow. |
| Failure recovery | Ensure every failure has an actionable next state. | ActionQueue, blockers, retry/resume/cancel, event stream. | Recovery matrix and E2E coverage. |
| Trial readiness gate | Aggregate automated, live, and manual evidence. | Beta/Stage1 readiness gate patterns. | `ready_for_limited_internal_trial` or named blockers. |

## 6. Exit Criteria

Stage 2 exits only when all P0 criteria pass:

- manual internal dogfood protocol completed with named reviewer notes;
- P0 live provider scenarios pass with real provider evidence;
- AgentControlPlane shows required states for real Chat tasks;
- memory and knowledge updates remain proposal-first;
- permission-sensitive actions pause and resume with exact scope;
- failure recovery matrix has UI and runtime evidence;
- final delivery distinguishes executed, proposed, blocked, skipped, and pending
  work;
- no hidden legacy fallback;
- no silent durable writes;
- no fake browser/live evidence;
- readiness report says `ready_for_limited_internal_trial`.

## 7. Residual Risks Allowed At Stage 2 Exit

These can remain documented risks:

- external provider behavior may vary by model/version;
- public beta polish is not complete;
- broad Skills Hub remains unavailable;
- background autonomy remains disabled;
- some non-P0 task types may still return explicit blockers;
- local macOS Tauri WebDriver remains unsupported and must continue to fail
  closed for real Tauri browser proof.
