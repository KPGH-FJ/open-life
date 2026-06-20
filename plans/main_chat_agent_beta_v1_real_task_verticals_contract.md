# Main Chat Agent Beta v1 Real Task Verticals Contract

> Date: 2026-06-18
> Workstream: 2 of 5
> Status: preparation artifact

## 1. Product Goal

Prove OpenLife can complete realistic Main Chat tasks, not only synthetic unit
paths. The eval harness should answer the same question a serious user will ask:

> Did the agent actually help me do the task, with visible evidence and safe
> recovery?

## 2. Benchmark Insight

Anthropic's public guidance on tool writing recommends prototypes followed by
evaluations grounded in realistic use cases and data, including tasks that may
require multiple tool calls. Codex best practices emphasize "done when" criteria,
tests, and review. OpenClaw's public docs separate tools, skills, plugins,
automation, and sub-agents because real work crosses multiple capability types.

OpenLife implication:

- We need scenario-level product evals with realistic prompts, expected runtime
  objects, expected UI states, and failure assertions.
- The eval must not only check backend booleans. It must check product truth:
  what the user saw, what was executed, what was proposed, what was blocked, and
  what final delivery claimed.

## 3. Vertical Selection

Beta v1 should use a small set of realistic verticals, not one artificial demo:

1. Personal planning and review.
2. Knowledge and memory management.
3. Workspace/project research.
4. Tool/skill-assisted read tasks.
5. Failure/permission recovery.

These verticals fit OpenLife's identity and reuse existing Main Chat runtime
objects. They also avoid broad external writes.

## 4. Scenario Matrix

| ID | User task | Required path | Required proof |
| --- | --- | --- | --- |
| B1 | "Answer this conceptual question." | DirectAnswer | compact trace, no tool timeline |
| B2 | "Summarize this workspace file." | Governed file read | action, observation, source, final |
| B3 | "Find what we discussed about Agent memory." | session search | session query, observation, citation |
| B4 | "Use my current memory/preferences when answering." | bounded memory context | loaded source digest and direct answer |
| B5 | "Search the web and summarize with sources." | governed web read | web action, source observation, final |
| B6 | "Use the selected skill to review this plan." | selected skill context | selected skill id, digest, no unselected skill |
| B7 | "Pick the right read-only MCP source and answer." | MCP candidate selection | candidate set, selected target, observation |
| B8 | "Plan my week and execute the first safe step." | Plan-Execute | plan draft, confirm/edit path, first action |
| B9 | "Skip this unsupported plan step and continue." | plan skip | skip event, reason, remaining plan |
| B10 | "Remember that I prefer morning deep work." | memory proposal | candidate, evidence, confirmation needed |
| B11 | "Accept that memory update." | memory accept | accepted record, active context, provenance |
| B12 | "Roll back the memory I accepted." | memory rollback | rollback event, active exclusion |
| B13 | "Continue the task from earlier." | task continuity | task lookup, stale check, resume proof |
| B14 | "Retry the failed read." | failure recovery | failed action, retry, new observation |
| B15 | "Cancel this task." | task control | cancelled state, no further execution |
| B16 | "Do this external/risky action." | permission/blocker | exact permission or blocked state |
| B17 | "Explain why you chose that tool." | tool trace | selected candidate reason and policy proof |
| B18 | "Use a skill that is not selected." | skill boundary | blocker or no injection proof |
| B19 | "Summarize completed vs blocked work." | final delivery | completed/proposed/blocked/skipped sections |
| B20 | "Reconnect and show current task state." | event replay | event stream replay, no duplicate events |
| B21 | "Compare two memory facts that conflict." | memory conflict | evidence/conflict state, no silent overwrite |
| B22 | "Ask a task that needs multiple reads." | multi-step ReAct | at least two actions/observations |
| B23 | "Use web when network policy blocks it." | blocker | named web policy blocker |
| B24 | "Use MCP when no manifest exists." | blocker | missing target blocker |
| B25 | "Run external live DirectAnswer." | opt-in live, excluded from default deterministic readiness | live provider trace, no fallback |
| B26 | "Run external live web/MCP path." | opt-in live ReAct, excluded from default deterministic readiness | live action trace and final credit |
| B27 | "Inspect loaded knowledge assets." | knowledge manager | source list, scope, digest, loaded state |
| B28 | "Edit a knowledge asset proposal." | knowledge proposal | proposed diff, confirmation, no direct write |
| B29 | "Recover from stale resume context." | continuity guard | stale diagnostic and refresh path |
| B30 | "Finish and tell me exactly what changed." | final delivery | final state plus durable-change inventory |

## 5. Scenario Fixture Shape

The table above is a product matrix. The implementation must convert it into a
machine-readable fixture before claiming eval coverage. Each fixture entry must
include:

- `id`;
- `vertical`;
- `prompt`;
- `default_readiness`: true for deterministic local readiness, false for opt-in
  live scenarios;
- `requires_live_provider`: true only for B25/B26-style scenarios;
- `expected_outcome`: one of `success`, `proposal`, `expected_blocker`, or
  `opt_in_live`;
- `preconditions`;
- `expected_strategy`;
- `command_surface`: `send_message`, `start_stream_message`, `both`, or
  `not_applicable_with_reason`;
- `required_runtime_events`;
- `required_actions`;
- `required_observations`;
- `required_ui_states`;
- `required_final_delivery_sections`;
- `expected_blockers`;
- `forbidden_evidence`, such as legacy fallback, silent durable write, local
  provider credit for external live, or assistant text used as state;
- `pass_criteria`.

Fixture ids must be stable. Adding scenarios must not renumber existing ids.

Outcome rules:

- `success`: the task must complete with the required runtime/UI/final-delivery
  evidence. A blocker is failure unless explicitly listed as an allowed recovery
  state.
- `proposal`: the task must create a proposal/confirmation path and must not
  perform the durable write directly.
- `expected_blocker`: the task must fail closed with the named blocker and
  next-action guidance. A plain answer or silent no-op is failure.
- `opt_in_live`: the task is excluded from default readiness and can pass only
  with audited external-provider evidence.

## 6. Eval Output Shape

Each scenario should produce:

- scenario id and prompt;
- expected strategy;
- task session id;
- event count and required event names;
- actions attempted/executed;
- observations recorded;
- proposals created;
- permissions requested;
- memory records changed;
- UI states expected;
- final delivery sections;
- blockers;
- legacy fallback count;
- silent durable write count;
- pass/fail reason.

## 7. Acceptance

The real task vertical harness is acceptable when:

- at least 28 default-readiness scenarios run deterministically;
- external live scenarios are run only when explicitly opted in and are reported
  separately from default readiness;
- every scenario fixture declares `expected_outcome`, and the harness enforces
  outcome-specific pass/fail rules;
- every default-readiness scenario that represents a user Chat request must
  exercise ordinary `send_message`, `start_stream_message`, or both. A
  non-command-surface exception is allowed only when the fixture sets
  `command_surface=not_applicable_with_reason` and explains why the scenario is a
  pure inspection/readiness-report case rather than a Main Chat task;
- UI/event assertions are included for all non-direct-answer scenarios;
- unsupported scenarios fail with named blockers, not plain answers;
- memory scenarios prove proposal/accept/rollback lifecycle;
- task continuity scenarios prove resume safety;
- skills scenarios prove selected/unselected boundaries;
- external live scenarios remain opt-in and do not affect default readiness.

## 8. Anti-gaming Rules

- Do not add prompts that only test keyword routing.
- Do not count a final answer as execution proof.
- Do not use fixture success to claim external live success.
- Do not make the model's text the source of truth for memory, plan, or task
  state.
- Do not allow test-only hooks to satisfy product command-surface acceptance
  unless the hook is explicitly labeled and excluded from beta readiness.
