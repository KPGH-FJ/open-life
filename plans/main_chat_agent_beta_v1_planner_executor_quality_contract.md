# Main Chat Agent Beta v1 Planner/Executor Quality Contract

> Date: 2026-06-18
> Workstream: 3 of 5
> Status: preparation artifact

## 1. Product Goal

Improve the reliability of planning, tool execution, recovery, and final
delivery without replacing the existing runtime.

The target is not a new planner architecture. The target is a better user
outcome:

- requests are decomposed only when decomposition helps;
- plan steps map to executable actions or clear blockers;
- tools are chosen for concrete reasons;
- failures recover without hiding risk;
- final delivery is accurate and auditable.

## 2. Benchmark Insight

Hermes public docs describe a loop with prompt assembly, provider calls, parsed
tool calls, tool execution, appended tool results, iteration budgets, fallback
behavior, compression, and session persistence. Codex best practices emphasize
clear goal/context/constraints/done-when prompts plus tests and review. OpenClaw
shows tools filtered by policy/provider/sandbox/channel before the model sees
them.

OpenLife implication:

- Planning must be tied to supported action surfaces.
- Tool candidates must be bounded and policy-filtered before execution.
- Recovery and final delivery are part of the runtime, not after-the-fact prose.

## 3. Current OpenLife Reality

Expected foundations to verify and reuse:

- ReAct AgentLoop and ActionExecutor-backed read path;
- exact target/action allowlists;
- ExecutionPolicy;
- plan edit/confirm/skip/execute/review object status, classified by the
  foundation inventory;
- failure blockers and retry/cancel/resume controls;
- final delivery contract;
- external live proof gates, only for opt-in evidence.

The implementation must not assume these foundations are complete. The
foundation inventory must say which planner/executor dependencies are verified,
partial, or missing before quality work starts.

Plan interaction is not optional for claimed Beta readiness. If plan
edit/confirm/skip/execute/review objects are not verified, the implementation
must either complete the minimum default-readiness slice or block Beta readiness
for plan-related scenarios.

Remaining quality gap:

- Multi-step real tasks need stronger planning heuristics and recovery behavior.
- Final delivery must be consistently grounded in task events.
- Tool selection and plan execution should be easier for the user to inspect.

## 4. Planner Requirements

Planner quality requires:

- classify whether a plan is needed;
- keep plans short unless the task genuinely requires more steps;
- each step must have one of these types:
  - direct answer;
  - read action;
  - ReAct/tool action;
  - proposal/memory action;
  - user confirmation;
  - unsupported/blocker;
  - review/final delivery;
- each executable step must include expected action type and required evidence;
- edited plans must create a new revision;
- executing a stale plan revision must fail closed;
- skipped steps must include reason and downstream impact.

## 5. Executor Requirements

Executor quality requires:

- execute only exact allowed action-target pairs;
- ignore model-supplied arguments when governed candidate input exists;
- record action start, success, failure, retry, cancel, and observation events;
- never treat no-tool final text as successful tool execution;
- keep fallback explicit and product-visible;
- stop on policy blocker unless an exact permission/proposal flow exists;
- support retry only when action id, input digest, and permission scope remain
  valid.

## 6. Recovery Requirements

Every failure must produce one of:

- retry available;
- ask user for missing information;
- request permission;
- edit plan;
- skip step;
- cancel task;
- terminal blocker with clear reason.

No failure may disappear into a generic assistant apology.

## 7. Final Delivery Requirements

Final delivery must be generated from runtime evidence and include:

- completed work;
- observations used;
- proposals created;
- memory changes accepted/rejected/rolled back;
- blocked work;
- skipped work;
- pending user action;
- external/live evidence status when relevant.

The final answer must not overclaim. "Planned" is not "done". "Proposed" is not
"remembered". "Blocked" is not "completed".

## 8. Acceptance

Planner/Executor quality is acceptable when:

- multi-step scenarios include at least two executed observations or a clear
  blocker;
- plan revisions, skips, and stale revision guards are tested;
- tool selection trace is inspectable;
- retry/cancel/resume behavior is validated against event records;
- final delivery is mechanically checked against task events;
- unsupported actions fail closed;
- no new planner/executor object duplicates Product Maturity v2 foundations.

## 9. Metrics

Track:

- task completion rate;
- correct strategy route rate;
- tool selection correctness;
- action success rate;
- blocker correctness;
- retry success rate;
- final delivery accuracy;
- memory/proposal correctness;
- legacy fallback count;
- silent durable write count.

Each metric must define numerator, denominator, and default-readiness scope. A
metric without a scenario set or pass threshold is reporting-only and cannot be
used to claim Beta completion.

## 10. Out Of Scope

- Background autonomous planning.
- Large multi-agent orchestration.
- File/calendar/email/external writes unless already proposal-first and
  explicitly approved by existing policy.
- Self-improving planner changes.
