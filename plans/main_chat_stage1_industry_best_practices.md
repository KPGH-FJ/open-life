# Main Chat Stage 1 Industry Best Practices

> Date: 2026-06-18
> Scope: source-backed preparation notes for Stage 1 dogfood development
> Status: preparation artifact

## 1. Purpose

Stage 1 moves OpenLife from Beta readiness evidence to real end-to-end dogfood.
This document records the external practices that should shape the work. It is
not a claim about private internals. Only public documentation is treated as
source evidence.

## 2. Source Confidence

High-confidence public sources used for Stage 1 planning:

- OpenAI Codex public documentation, checked on 2026-06-18:
  `https://developers.openai.com/codex/learn/best-practices`,
  `https://developers.openai.com/codex/skills`,
  `https://developers.openai.com/codex/guides/agents-md`, and
  `https://developers.openai.com/codex/memories`.
- Anthropic, "Writing effective tools for agents", 2025-09-11:
  `https://www.anthropic.com/engineering/writing-tools-for-agents`.
- Anthropic, "Demystifying evals for AI agents", checked on 2026-06-18:
  `https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents`.
- Anthropic, "Building effective agents", checked on 2026-06-18:
  `https://www.anthropic.com/research/building-effective-agents`.
- Claude Code memory documentation, checked on 2026-06-18:
  `https://code.claude.com/docs/en/memory`.
- Claude Code skills documentation, checked on 2026-06-18:
  `https://code.claude.com/docs/en/skills`.
- Hermes Agent public docs, checked on 2026-06-18:
  `https://hermes-agent.nousresearch.com/docs/developer-guide/agent-loop`
  and
  `https://hermes-agent.nousresearch.com/docs/developer-guide/prompt-assembly`.
- OpenClaw public docs, checked on 2026-06-18:
  `https://docs.openclaw.ai/tools`,
  `https://docs.openclaw.ai/tools/browser`, and
  `https://docs.openclaw.ai/gateway/sandboxing`.

Lower-confidence material such as blogs, videos, Reddit posts, and news articles
may inform intuition, but must not become acceptance criteria without a primary
source or local OpenLife evidence.

## 3. Patterns To Apply

### 3.1 Treat difficult agent work as context + plan + verification

Codex best practices emphasize giving the agent goal, context, constraints, and
done conditions; planning before difficult changes; reusable guidance in
`AGENTS.md`; and running tests/review before accepting work.

OpenLife Stage 1 implication:

- Every dogfood scenario must state the user goal, relevant seeded context,
  constraints, and done condition.
- A scenario cannot pass by producing a plausible answer. It must produce
  runtime evidence and a user-visible final delivery.
- The Stage 1 goal spec must be short enough for CLI goal mode, with details in
  linked contracts.

### 3.2 Skills teach workflows; tools perform actions

Codex and Claude skills use progressive disclosure: name/description are visible
upfront and full `SKILL.md` body loads only when selected or relevant. OpenClaw
separates tools, skills, plugins, and automation surfaces.

OpenLife Stage 1 implication:

- Stage 1 must verify that selected `SKILL.md` affects guidance only when
  selected.
- Tool execution must remain governed by `ExecutionPolicy`, allowlists, and
  target/action scope.
- A dogfood scenario must not treat skill context as proof that a tool action
  happened.

### 3.3 Memory and instruction files are inspectable context, not hidden truth

Codex memory is optional local generated state, while durable team rules belong
in `AGENTS.md` or checked-in docs. Claude distinguishes human-written
`CLAUDE.md` files from auto memory and treats both as context, not enforced
configuration. Hermes publicly separates `SOUL.md`, project context files,
`MEMORY.md`, `USER.md`, and ephemeral prompt overlays.

OpenLife Stage 1 implication:

- Stage 1 must expose loaded/skipped knowledge assets, scope, digest, and policy
  boundary.
- A memory or knowledge update must remain proposal-first until accepted.
- The dogfood harness must reject assistant text being used as user fact.

### 3.4 Real tools need real evaluations

Anthropic's tool guidance recommends prototyping tools, testing locally, and
running comprehensive evaluations grounded in real-world tasks. Their evals
guidance distinguishes code-based, model-based, and human graders.

OpenLife Stage 1 implication:

- Stage 1 needs deterministic code-based checks for route/action/observation/UI
  states.
- It also needs human dogfood review for product usefulness and confusing UI.
- Tool pass/fail must include tool used, parameters/target, observation, and
  final answer grounding.

### 3.5 Agent loop quality is more than one model call

Hermes public docs describe prompt/tool assembly, provider selection,
interruptible model calls, tool execution, conversation history, compression,
retries, fallback, and iteration budgets.

OpenLife Stage 1 implication:

- Dogfood reports must show whether the task used DirectAnswer, ReAct,
  Plan-Execute, proposal, task-control, or blocker path.
- Multi-step scenarios must prove action/result pairing, not just final prose.
- Cancellation, retry, stale resume, and fallback states must be visible.

### 3.6 Execution surfaces need permissions and isolation

OpenClaw public docs separate tool policy, provider restrictions, sandboxing,
channel permissions, plugins, and browser profiles. Browser automation uses a
separate managed profile and carries explicit security considerations.

OpenLife Stage 1 implication:

- External live/browser/web/MCP scenarios must be opt-in and cannot count by
  local/mock/fixture provider identity.
- Read-only automatic execution is acceptable only when policy allows it.
- External write or high-risk action remains blocker/proposal/confirmation.

## 4. Stage 1 Translation

Stage 1 should not copy names from Codex, Hermes, Claude, or OpenClaw. It should
copy the proven behavior patterns:

- visible task frame for work-like requests;
- concrete action and observation evidence;
- bounded, inspectable context sources;
- progressive skill loading;
- exact permission and target scope;
- deterministic plus human dogfood evaluation;
- fail-closed external live evidence;
- final delivery that separates done, proposed, blocked, skipped, and next user
  action.

## 5. Non-Claims

- This document does not claim OpenLife has reached Codex, Hermes, Claude, or
  OpenClaw maturity.
- This document does not claim external live provider evidence has been run.
- This document does not claim full browser automation, marketplace skills, or
  background autonomy are Stage 1 requirements.
