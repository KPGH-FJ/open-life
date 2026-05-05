# OpenLife vNext Architecture Principles

Date: 2026-05-06

This document is the architecture baseline for the Agent Framework upgrade. It defines what must be true for OpenLife to become a stronger framework rather than a larger app.

## Target Identity

OpenLife vNext should be a:

```text
LifeModel-governed Personal Agent Framework
```

It should borrow execution discipline from coding-agent and multi-agent frameworks, but it should not become a clone of Claude Code, OpenAI Agents, LangGraph, or Codex. OpenLife's differentiator is governance by LifeModel, Memory, Privacy, Proposal, Permission, and Audit.

## Core Thesis

OpenLife is not only an agent that knows the user.

OpenLife is an agent framework where the user's own LifeModel and Memory govern how the agent understands, plans, acts, remembers, evolves, asks for permission, and uses cloud models.

## Non-Negotiable Principles

1. Every formal agent behavior must create or attach to an `AgentRun`.
2. Every `AgentRun` must have an append-only event trace.
3. Every tool call must go through `ToolRuntime` / `ActionExecutor`.
4. Every tool must declare `executable`, `declarative_only`, `risk_level`, and `permission_policy`.
5. Declarative-only tools must never enter the model-callable tools prompt.
6. Every external side effect must pass Permission, Proposal, and Audit.
7. Every LifeModel mutation must be proposal-first unless explicitly classified as low-risk auto-state under a reviewed policy.
8. Every prompt must be assembled by `PromptStack`, not ad hoc string splicing.
9. The base system prompt is framework constitution, not page-level copy.
10. Every planning operation must produce a structured `AgentPlan`.
11. Every sub-agent must be defined by the canonical `AgentSpec` model, with `SubAgentSpec` adding only delegation-specific constraints when needed.
12. Every sub-agent must have independent context policy and tool policy.
13. Every cloud model call must pass `ModelRouter` and `PrivacyPolicy`.
14. Every fallback, repair, block, replay, and rollback must be traceable.
15. Memory is not only retrieval context; memory is LifeModel evolution evidence.
16. LifeModel evolution must be evidence-backed, proposal-first, auditable, and reversible.

## Product Philosophy as Runtime Policy

OpenLife should encode user sovereignty as runtime behavior:

- The user owns the LifeModel.
- The model may infer, summarize, suggest, and prepare proposals.
- The model must not silently rewrite high-impact user identity, values, mission, long-term goals, preferences, or external state.
- The model must be able to explain why a piece of memory influenced a LifeModel proposal.
- Rejected proposals should teach future evolution logic that the inference was not accepted.

## PromptStack as First-Class Architecture

OpenLife must not rely on one giant system prompt or scattered prompt fragments.

Required prompt blocks:

- `BaseSystemPrompt`
- `LifeModelPrompt`
- `MemoryEvidencePrompt`
- `TaskPrompt`
- `PlanningPrompt`
- `ToolPrompt`
- `ProposalPrompt`
- `PrivacyPrompt`
- `OutputFormatPrompt`
- `SubAgentPrompt`

Each prompt block should declare:

- `id`
- `version`
- `purpose`
- `privacy_level`
- `applies_to`
- `token_budget`
- `cloud_allowed`
- `trace_policy`

Prompt assembly must be recorded in `AgentRunEvent` at the metadata level. Sensitive prompt content may be summarized or redacted, but the block identity and version should remain traceable.

## Memory as Evidence

Memory should have three distinct roles:

1. Context: relevant memories help answer the current task.
2. Record: accepted memory stores durable facts or user-approved recollections.
3. Evidence: repeated or high-confidence memories can support LifeModel evolution proposals.

vNext must distinguish these roles. A memory being useful as chat context does not automatically make it valid evidence for LifeModel mutation.

## Proposal-First Evolution

The default LifeModel evolution path is:

```text
Observation
-> Evidence
-> Proposal
-> User decision
-> Snapshot
-> Patch
-> Audit
-> Future context
```

Risk rules:

- Identity, values, mission, role definition, long-term goals: high risk, never auto-apply.
- Preferences, relationships, capabilities: medium risk, proposal-first.
- Current focus, temporary state, low-impact operational metadata: possible low-risk policy, but still traceable.

## Tool and Sandbox Philosophy

Tools are the agent's hands, not UI decorations.

Required tool constraints:

- Safe paths for filesystem access.
- Deny-read rules for secrets and sensitive directories.
- Write operations as proposals unless explicitly governed.
- Network/private address policy.
- Timeout and output-size limits.
- Environment allowlist for any shell execution.
- Audit event for every attempt, not only success.

Bash/Shell should be introduced late, default-off, and only after `ExecutionSandbox` exists.

## Sub-Agent Philosophy

Sub-agents are not a workaround for missing runtime discipline.

They should be added after:

- `AgentRunEvent`
- `ToolRuntime`
- `PromptStack`
- `AgentSpec`
- `AgentPlan`

Sub-agent modes:

- `call_as_tool`: main agent keeps control.
- `handoff`: another agent takes control under explicit trace.
- `parallel`: bounded independent tasks with merge.
- `review`: second agent reviews output, plan, or patch.

Default sub-agent permissions should be minimal and role-specific.

## AI Coding Governance Principle

For high-risk architecture topics, use:

```text
AI propose
-> Human review
-> Human decide
-> AI implement
-> Tests verify
-> Docs update
```

High-risk topics include:

- LifeModel governance.
- Bash/Shell safety boundary.
- Sub-agent default permissions.
- Cloud privacy policy.
- ChatPage state model.
- PromptStack/system prompt architecture.
- Memory to LifeModel evolution.

## Definition of a Legal Agent Behavior

A vNext legal agent behavior must be explainable by this record:

```text
who/what initiated it
which AgentSpec ran
which PromptStack blocks were used
which LifeModel and Memory context was used
which model route was chosen
which plan was produced
which tools were called or blocked
which observations came back
which proposals were created
which side effects happened
how the user can inspect, replay, reject, or roll back
```

If a behavior cannot be represented this way, it should not be considered a first-class OpenLife agent behavior.
