# Main Chat Agent Stage 3 Preparation Index

> Date: 2026-06-20
> Stage: Stage 3 - Execution UX and Main Chat Internal Alpha Candidate
> Status: preparation draft

## 1. Direction

Stage 2 added an auditable readiness mechanism. It did not make OpenLife ready
for limited internal trial because real manual dogfood is intentionally still
missing.

Stage 3 should now make the default Main Chat experience usable enough for an
internal alpha candidate. The target is not another backend-only proof. The user
must see an Agent that is doing work: goal, route, plan/action, observation,
blocker, permission/proposal, recovery controls, and final delivery.

Stage 3 does not run the full S2-D01 through S2-D24 manual dogfood gate. That
gate remains the final internal-trial acceptance suite after Stage 3, Stage 4,
and Stage 5 are complete.

## 2. Preparation Documents

| Document | Purpose |
| --- | --- |
| `plans/main_chat_stage3_execution_ux_best_practices.md` | Source-backed execution UX principles from OpenAI Agents/Codex, Anthropic agent guidance, Claude skills/context docs, and MCP security. |
| `plans/main_chat_stage3_current_gap_inventory.md` | Current OpenLife assets and Stage 3 gaps, mapped to files and product risk. |
| `plans/main_chat_stage3_execution_ux_product_contract.md` | Product contract for Main Chat execution UX, data sources, UI states, controls, and non-fake rules. |
| `plans/main_chat_agent_stage3_execution_ux_goal_spec.md` | CLI goal-mode implementation entrypoint. |

## 3. Stage 3 Target

Stage 3 must produce a Main Chat internal alpha candidate where:

- ordinary chat still works;
- task-like chat creates or resumes a governed Agent task session;
- active execution is visible without opening debug pages;
- read/tool actions show real queued/running/observed/blocked states;
- blockers show exact reason and a safe next action or terminal explanation;
- permission and proposal states are actionable from the same task surface;
- retry, resume, cancel, plan controls, and proposal controls operate against
  existing runtime objects only;
- final delivery separates completed, proposed, blocked, skipped, and pending
  work;
- a reviewer can copy task/run/scenario/blocker evidence for later manual
  dogfood;
- no UI claim is inferred from assistant prose.

## 4. Recommended Development Order

1. Collapse duplicate Main Chat execution surfaces into one primary
   `AgentControlPlane` path while preserving compact fallback for missing state.
2. Make active task state visible during send/stream, not only after final
   response. Use existing `MainChatAgentStateSnapshot`, task detail, and event
   stream payloads first; if those are not yet available, render a compact
   diagnostic shell from existing ingress/task ids only. Do not create a second
   frontend-only task state model.
3. Upgrade action/observation/blocker rendering into a timeline with current
   action emphasis and bounded observation previews.
4. Make permission, proposal, retry, resume, cancel, and plan controls visibly
   scoped to the exact runtime object they affect.
5. Make final delivery the stable terminal summary for completed, blocked,
   failed, cancelled, and proposal-pending tasks.
6. Add reload/resume behavior so the current conversation can recover the most
   recent task state after navigation or refresh from existing task session /
   event stream / conversation-linked task metadata. Do not recover by fuzzy
   matching message text.
7. Add focused frontend and command-surface tests that prove UI claims come from
   typed runtime state.
8. Add a focused Stage 3 UX eval/test surface for `UX3-01` through `UX3-13`.
   This is not a new readiness gate; it is a deterministic product UX coverage
   report/test suite.

## 5. CLI Goal Prompt

Use this short prompt for CLI goal mode after review:

```text
Implement Main Chat Agent Stage 3 Execution UX.
Read plans/main_chat_agent_stage3_execution_ux_goal_spec.md and the documents it
lists. Keep scope to Stage 3. Reuse existing AgentIngress, AgentTaskSession,
ActionQueue, ExecutionTranscript, Main Chat event stream, ProposalStore,
Memory lifecycle, PlanExecute, AgentControlPlane, and ChatPage objects. Do not
create a parallel runtime, memory system, proposal system, task panel, or
readiness gate. Make Main Chat visibly execution-first for internal alpha:
active task, action/observation timeline, blockers, permission/proposal
controls, retry/resume/cancel, final delivery, reload recovery, and reviewer
trace export. Keep Stage 2 readiness not_ready unless real manual dogfood and
current-commit live evidence exist. Do not run or fabricate the 24 manual
dogfood rows in this stage. Add a focused Stage 3 UX coverage test/report for
UX3-01 through UX3-13 without creating a new readiness gate.
```

## 6. Readiness To Start Stage 3 Development

Stage 3 development can start after:

- these preparation documents are reviewed;
- working tree is clean or intentionally staged;
- Stage 2 mechanism remains committed and green;
- Stage 2 manual dogfood is explicitly deferred to the post Stage 3/4/5 final
  internal-trial acceptance run;
- the implementer accepts that product UI must render from typed runtime
  evidence, not assistant prose.

## 7. Non-negotiable Invariants

- No silent durable LifeModel, memory, file, external, plugin, or provider
  writes.
- No hidden legacy fallback.
- No fake browser/live/manual evidence.
- No duplicate task panel or parallel task/event/proposal/memory model.
- No knowledge file can override runtime privacy/tool/model policy.
- No "done" final delivery for proposed, blocked, skipped, or unexecuted work.
- No claim that Stage 3 alone grants `ready_for_limited_internal_trial`.
