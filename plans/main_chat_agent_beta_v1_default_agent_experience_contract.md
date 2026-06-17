# Main Chat Agent Beta v1 Default Agent Experience Contract

> Date: 2026-06-18
> Workstream: 1 of 5
> Status: preparation artifact

## 1. Product Goal

Make ordinary Main Chat feel like an execution-first agent without making simple
answers noisy.

The user can still type naturally into Chat. Internally, Main Chat must classify
the request and route it into a governed strategy:

- DirectAnswer for answer-only prompts;
- Governed Read for read-only tool tasks;
- ReAct for multi-step tool tasks;
- Plan-Execute-Review for goal/task workflows;
- Memory/Proposal flow for durable user knowledge;
- Permission/Blocker flow for risky or unsupported actions.

## 2. Benchmark Insight

First-class agents do not hide execution behind a secondary preview mode. Codex
keeps work tied to inspectable context, diffs, tests, and verification. Hermes
documents an agent loop where tool calls and tool results are part of the turn
lifecycle. OpenClaw treats tools as the surface for acting and filters them by
policy and runtime availability.

OpenLife should apply this as:

- default Main Chat task frame for work-like prompts;
- compact direct-answer trace for simple prompts;
- visible execution states for tool, plan, memory, and permission flows;
- no "it sounds like the agent did work" unless a runtime object proves it.

## 3. Current OpenLife Reality

Expected foundations to verify and reuse:

- Main Chat governed ingress;
- strategy routing;
- `AgentTaskSession`;
- `ActionQueue`;
- execution transcript;
- task controls;
- event delta stream or replayable task event adapter;
- plan interaction objects;
- memory lifecycle and rollback status, classified by the foundation inventory;
- skills/tool surface;
- final delivery and final readiness gates.

If any of these foundations are partial or missing, the implementation must
either complete the minimum required slice for the default experience or mark the
affected Beta readiness dimension blocked. The UI must not pretend an unverified
runtime object exists.

Memory lifecycle and rollback are not optional Beta claims. If the foundation
inventory cannot verify them, the implementation must either complete the minimum
default-experience slice or block Beta readiness for memory-related default
scenarios.

Remaining product gap:

- The default user experience can still feel like a chat answer unless the UI
  consistently renders the runtime state and controls.
- The distinction between answer-only, executing, proposed, blocked, and
  delivered states needs to be obvious in the main composer flow.

## 4. Required Product States

| State | Meaning | Required runtime proof |
| --- | --- | --- |
| `classifying` | Request is being routed. | strategy route trace |
| `answering` | DirectAnswer is generating. | DirectAnswer run id and provider/model trace |
| `planning` | Agent is building a plan or ReAct intent. | plan/reasoning trace or AgentLoop attempt |
| `action_queued` | A concrete action is queued. | `ActionQueue` entry |
| `action_running` | An action is executing. | action status event |
| `observation_ready` | Tool/read result exists. | transcript observation event |
| `permission_needed` | User approval is required. | permission/proposal record |
| `memory_candidate` | A durable memory update is proposed. | memory proposal/evidence record |
| `blocked` | Execution cannot proceed. | named blocker with source |
| `retry_available` | User can retry safely. | retry policy and last failed action |
| `completed` | Task final delivery is available. | final delivery record |

## 5. UI Requirements

Main Chat should show:

- compact task header: goal, status, strategy, final state;
- timeline: plan/action/observation/proposal/blocker/final delivery events;
- controls: continue, retry, cancel, edit plan, approve, deny, inspect;
- trace drawer: context sources, selected skill/tool, policy decision, provider
  route, evidence ids;
- low-noise direct answer mode: no fake action timeline, but expandable trace.

The UI must not invent states from assistant text. It must render from runtime
records and events.

Each user-visible state must have a UI mapping assertion in the Beta readiness
report. A TypeScript typecheck is not enough proof for this workstream.

## 6. Backend Dependencies

- `AgentIngress`
- Strategy router / classifier
- `AgentTaskSession`
- `ActionQueue`
- execution transcript
- delta event stream
- task controls
- final delivery object
- memory/proposal store
- skills/tool selection read model
- readiness/eval report

## 7. Acceptance

Default experience is acceptable when:

- the UI states are mapped to verified runtime objects from the foundation
  inventory;
- every required state in Section 4 has render/mapping coverage in the UI gate or
  explicit readiness blocker;
- ordinary `send_message` and `start_stream_message` both route through governed
  task/session paths;
- direct-answer prompts render compact trace and no fake tool timeline;
- read/tool prompts render action and observation states;
- plan prompts render plan draft and controls;
- memory prompts render proposal/confirmation states;
- permission prompts render exact pending approval;
- blocked prompts render named blockers and next actions;
- final delivery separates completed/proposed/blocked/skipped/next action;
- UI reconnect uses event replay instead of local reconstruction;
- no silent durable writes occur.

## 8. Negative Tests

The implementation must fail if:

- a tool-required prompt returns only a plain answer without blocker/fallback
  trace;
- UI shows an observation without a transcript event;
- UI shows memory accepted without confirmation/provenance;
- a permission approval resumes a different action than the pending one;
- a terminal task can be resumed without a fresh task/session;
- external live failure is hidden as local success.

## 9. Not In Scope

- New autonomy system.
- Background task scheduler.
- Marketplace-scale skills hub.
- New memory engine.
- Dangerous writes.
