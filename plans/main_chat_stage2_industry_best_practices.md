# Main Chat Stage 2 Industry Best Practices

> Date: 2026-06-19
> Stage: Main Chat Agent Stage 2 - Internal Trial Readiness
> Status: preparation reference

## 1. Purpose

Stage 2 should move OpenLife from automated engineering dogfood to limited
internal trial readiness. This document records the external practices used to
shape the plan. It intentionally relies on primary or official sources where
possible.

Hermes and OpenClaw are useful product references from prior observation, but
their internal architecture is not treated as a public source here. Any
Hermes/OpenClaw comparison in Stage 2 must be written as a product benchmark
claim, not as a sourced implementation fact.

## 2. Source-backed Principles

### 2.1 Trace first, then eval at scale

OpenAI's Agents eval guidance says workflow-level issues should first be
debugged through traces, because traces capture the end-to-end record of model
calls, tool calls, guardrails, and handoffs. Once the team knows what good looks
like, the work should move into repeatable datasets and eval runs.

Stage 2 implication:

- do not rely only on aggregate pass/fail counts;
- every internal trial task must produce a reviewable run trace;
- manual reviewers should annotate trace failures into new deterministic evals;
- live provider runs must preserve provider, model, tool, blocker, and final
  delivery evidence separately.

Reference:

- https://developers.openai.com/api/docs/guides/agent-evals

### 2.2 Observability is part of the runtime, not after-the-fact logging

OpenAI's Agents observability guidance treats tracing as a structured record of
model calls, tool calls, handoffs, guardrails, and custom spans.

Stage 2 implication:

- OpenLife's existing ExecutionTranscript, task events, ActionQueue,
  ProposalStore, Memory lifecycle, and final delivery records should remain the
  source of truth;
- UI should render from typed runtime payloads, not assistant prose;
- internal trial reports must include trace ids and scenario ids.

Reference:

- https://developers.openai.com/api/docs/guides/agents/integrations-observability

### 2.3 Human review must pause execution, not merely advise the model

OpenAI's guardrails and human review guidance separates automatic checks from
human approvals. Guardrails validate input/output/tool behavior; human review
pauses the run so a person or policy can approve or reject sensitive work.

Stage 2 implication:

- permission and proposal flows must be runtime states, not prompt text;
- write-like, high-risk, or privacy-sensitive actions stay paused until the user
  approves exact scope;
- the UI must show what approving, denying, or deferring will do.

Reference:

- https://developers.openai.com/api/docs/guides/agents/guardrails-approvals

### 2.4 Network and web access are elevated risk

Codex cloud blocks agent internet access by default during the agent phase and
warns that enabling it increases risks including prompt injection, secret
exfiltration, malware/vulnerable downloads, and license exposure. Codex
recommends limiting domains and HTTP methods and reviewing the work log.

Stage 2 implication:

- external live provider evidence is opt-in and separate from deterministic
  readiness;
- web/MCP reads must keep domain, method, provider, and source evidence;
- browser/web results are untrusted content and cannot override system/runtime
  policy;
- no internal trial task may require broad unrestricted network access.

References:

- https://developers.openai.com/codex/cloud/internet-access
- https://developers.openai.com/codex/agent-approvals-security

### 2.5 Approvals and sandboxing are separate control layers

Codex documents sandbox mode as what the agent can technically touch and
approval policy as when it must stop and ask before acting. It also notes that
destructive app/MCP tool calls require approval when the tool advertises
destructive side effects.

Stage 2 implication:

- OpenLife must continue separating ExecutionPolicy from prompt guidance;
- tool permission proposals should apply to the exact pending action only;
- critical actions fail closed, even if a model asks to continue.

Reference:

- https://developers.openai.com/codex/agent-approvals-security

### 2.6 File-based knowledge is context, not enforcement

Codex reads `AGENTS.md` guidance through layered discovery. Claude Code
separates written instruction files from auto memory and explicitly says
memory/instruction files are context, not enforced configuration; deterministic
blocking should use hooks or policy.

Stage 2 implication:

- OpenLife should treat `AGENTS.md`, `USER.md`, `MEMORY.md`, `SOUL.md`, and
  selected `SKILL.md` as bounded context surfaces;
- knowledge files cannot override privacy, model routing, tool policy, or
  proposal-first memory governance;
- user-facing knowledge edits should create proposals or explicit file-change
  drafts, not silent writes.

References:

- https://developers.openai.com/codex/guides/agents-md
- https://docs.anthropic.com/en/docs/claude-code/memory

### 2.7 Skills should package repeatable workflows and load on demand

Claude Code documents skills as a way to extend capabilities and package
repeatable workflows. Anthropic's tool guidance emphasizes prototyping tools,
running comprehensive evaluations with agents, and improving tool interfaces.

Stage 2 implication:

- do not turn every prompt into a giant prompt contract;
- selected skills must remain explicit and bounded;
- internal trial should include a small number of realistic skill/tool tasks
  with action, observation, blocker, and final delivery evidence.

References:

- https://docs.anthropic.com/en/docs/claude-code/skills
- https://www.anthropic.com/engineering/writing-tools-for-agents

### 2.8 Deterministic hooks/policies are needed for mandatory behavior

Claude Code describes hooks as lifecycle controls that can format, block
commands, notify users, or inject context at fixed points.

Stage 2 implication:

- behavior that must always happen should live in runtime policy, command
  handlers, or eval gates;
- prompts can guide, but cannot be the only enforcement for no-silent-write,
  proposal-first memory, or permission replay.

Reference:

- https://docs.anthropic.com/en/docs/claude-code/hooks-guide

### 2.9 Effective agents favor simplicity, transparency, and strong tool docs

Anthropic's agent guidance emphasizes keeping agent designs simple, showing
planning steps transparently, and carefully crafting the agent-computer
interface through tool documentation and testing.

Stage 2 implication:

- Stage 2 should not create new parallel runtime systems;
- AgentControlPlane should show goal, plan/action, observation, blocker, and
  final delivery without becoming noisy for simple answers;
- every tool exposed to the model should have bounded candidate metadata and
  eval coverage.

Reference:

- https://www.anthropic.com/engineering/building-effective-agents

### 2.10 MCP must be treated as a security boundary

The MCP security best-practices document covers confused deputy risks,
per-client consent, exact redirect URI validation, token passthrough as an
anti-pattern, SSRF, session hijacking, local server compromise, and scope
minimization.

Stage 2 implication:

- MCP manifests and responses remain untrusted until validated;
- per-tool and per-target permission scope must be exact;
- token/session state must not be leaked into prompt or trace;
- web/MCP network targets need allowlist and SSRF-style validation.

Reference:

- https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices

## 3. What This Means For OpenLife Stage 2

Stage 2 should not be a broad feature expansion. It should convert the Stage 1
engineering dogfood proof into a limited internal trial product by hardening:

- human-observable task execution;
- live provider behavior under real models;
- manual dogfood and failure annotation;
- proposal-first memory and knowledge flows;
- task recovery and final delivery trust;
- permission and network boundaries.

The output of Stage 2 should be a clear `ready_for_limited_internal_trial`
recommendation or a named blocker list. It should not claim public beta
readiness.
