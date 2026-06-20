# Main Chat Stage 3 Execution UX Best Practices

> Date: 2026-06-20
> Stage: Stage 3 - Execution UX and Main Chat Internal Alpha Candidate
> Status: preparation reference

## 1. Purpose

Stage 3 should learn from first-line agent products without pretending to know
private Hermes/OpenClaw internals. Public, source-backed references are used for
engineering principles. Hermes/OpenClaw remain product benchmarks: users feel
the agent is doing work because action, state, recovery, and final output are
visible.

## 2. Source-backed Principles

### 2.1 Trace is the product surface for execution trust

OpenAI Agents tracing documents describe traces as the record of LLM
generations, tool calls, handoffs, guardrails, and custom events. For OpenLife,
that means traces should not remain backend-only. The product surface should
render the typed runtime trace in bounded form.

Stage 3 implication:

- render active task, action, observation, blocker, and final delivery from
  `AgentTaskSession`, `ActionQueue`, `ExecutionTranscript`, event stream, and
  final delivery payloads;
- reviewer trace export should copy exact task/run ids and blocker codes;
- never infer execution from assistant prose.

Reference:

- https://openai.github.io/openai-agents-python/tracing/

### 2.2 Guardrails and human review are runtime states

OpenAI guidance separates automatic guardrails from human review. Human review
pauses execution so a person or policy can approve or reject sensitive work.

Stage 3 implication:

- permission and proposal UI must be actionable runtime states;
- approve, deny, defer, retry, resume, and cancel must target exact existing
  task/action/proposal ids;
- a blocker should show reason and next safe control, not just apology text.

Reference:

- https://developers.openai.com/api/docs/guides/agents/guardrails-approvals

### 2.3 Sandboxing, approvals, and network controls are separate layers

Codex documents sandboxing, approval policy, and network controls as separate
control layers. Stage 3 should show the user which layer stopped or allowed
work.

Stage 3 implication:

- blocker UI should distinguish missing input, policy blocked, permission
  required, network blocked, provider unavailable, invalid model action, and
  stale context;
- network/web/MCP actions need visible target/source identity;
- no UI should make a local/mock/scripted execution look provider-backed.

Reference:

- https://developers.openai.com/codex/agent-approvals-security

### 2.4 Effective agents are simple, composable, and transparent

Anthropic's agent guidance emphasizes simple composable patterns, transparent
planning, and strong agent-computer interfaces instead of unnecessary
framework complexity.

Stage 3 implication:

- keep one Main Chat control plane, not multiple competing panels;
- show concise plan/action/observation progress, not decorative "thinking";
- keep common flows scannable and recoverable before adding new capability.

Reference:

- https://www.anthropic.com/research/building-effective-agents

### 2.5 Tools need eval-backed interfaces

Anthropic's tool guidance emphasizes building evals around tools and improving
tool interfaces from failures. For OpenLife, the model/tool interface and the
user/tool interface must both be observable.

Stage 3 implication:

- every visible action should include action type, target, status, and linked
  observation or blocker;
- tool failure should become a deterministic regression where possible;
- UI tests should assert state payloads, not static text alone.

Reference:

- https://www.anthropic.com/engineering/writing-tools-for-agents

### 2.6 Context and knowledge files are context, not policy

Anthropic's context engineering guidance and Claude skill/memory docs treat
loaded instructions/resources as context that helps the model work. Mandatory
behavior still needs deterministic runtime controls.

Stage 3 implication:

- `AGENTS.md`, `USER.md`, `MEMORY.md`, `SOUL.md`, and selected `SKILL.md`
  should appear as bounded context sources when relevant;
- these files cannot override privacy, model routing, tool policy, or
  proposal-first memory governance;
- selected skill/context reasons should be visible but not overstate execution.

References:

- https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview

### 2.7 MCP is a security boundary

MCP security guidance treats tools, tokens, redirects, sessions, and network
targets as security-sensitive. OpenLife should continue treating MCP manifests
and read results as untrusted until validated.

Stage 3 implication:

- MCP target/source identity must be visible in bounded form;
- permission scope must be exact per action/target;
- MCP/web content cannot override runtime policy or final delivery honesty.

Reference:

- https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices

## 3. Product Lessons For Stage 3

Stage 3 should make OpenLife feel closer to a serious agent product by making
work observable and controllable:

- Chat remains the entry point, but execution state is the center.
- The task panel must be useful during execution, after failure, and after
  completion.
- Human controls must be exact and visible.
- Failures are first-class product states, not hidden backend details.
- The final answer must be grounded in completed actions, observations, and
  proposals.

The practical goal is an internal alpha candidate: good enough for real
internal use, honest enough to generate useful bug reports, and constrained
enough to avoid inventing parallel systems.
