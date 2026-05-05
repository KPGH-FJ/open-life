# ADR 0002: PromptStack and Base System Prompt Constitution

Date: 2026-05-06
Status: accepted

## Context

System prompt design is central to Agent Framework quality. OpenLife cannot rely on scattered prompt fragments as it adds PlanMode, SubAgents, MemoryEvidence, cloud privacy policy, and stricter ToolRuntime behavior.

OpenLife's system prompt must encode framework behavior, not just assistant personality.

## Decision

Introduce `PromptStack` and `PromptBlock` as first-class architecture primitives.

The OpenLife base system prompt is a constitution inherited by all first-class agents and sub-agents.

## Base System Prompt Principles

The base system prompt should include:

1. OpenLife is a LifeModel-governed personal agent framework.
2. The user owns the LifeModel and memory.
3. LifeModel is both context and governance signal, not a silent mutation target.
4. Memory is evidence only when accepted, relevant, and policy-allowed.
5. Tools can only be used through the provided tool protocol.
6. The agent must not pretend to execute unavailable tools.
7. External side effects require Permission/Proposal/Audit.
8. High-risk LifeModel changes require explicit user confirmation.
9. Sensitive data and cloud disclosure must follow PrivacyPolicy.
10. Planning is required for risky, multi-step, ambiguous, or external-write tasks.

## PromptBlock Types

Required blocks:

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

## Proposed Schema

```rust
pub struct PromptBlock {
    pub id: String,
    pub version: String,
    pub purpose: PromptPurpose,
    pub content: String,
    pub privacy_level: PrivacyLevel,
    pub cloud_allowed: bool,
    pub token_budget: usize,
    pub applies_to: Vec<String>,
}

pub struct PromptStack {
    pub blocks: Vec<PromptBlock>,
    pub output_schema: Option<OutputSchemaRef>,
    pub assembled_preview: String,
    pub redaction_summary: Option<String>,
}
```

## Options Considered

### Option A: Keep prompt fragments local to each flow

Pros:

- Fastest.
- No new abstraction.

Cons:

- Prompt drift.
- Hard to audit.
- Sub-agent and PlanMode prompts become inconsistent.

### Option B: One giant global prompt

Pros:

- Centralized.
- Easy to inspect.

Cons:

- Bloated.
- Hard to adapt per task.
- Bad for privacy and token budgets.

### Option C: Versioned PromptStack

Pros:

- Modular.
- Auditable.
- Privacy-aware.
- Supports AgentSpec and SubAgentSpec.

Cons:

- Requires builder and tests.
- Requires migration from existing prompt fragments.

## Recommendation

Use Option C.

## Consequences

Positive:

- Prompt behavior becomes part of the framework.
- Cloud filtering and prompt trace become possible.
- Sub-agents can inherit base constitution safely.

Tradeoffs:

- More upfront design.
- Need careful token budgeting.

## Implementation Guardrails

- No new major runtime feature should add ad hoc prompt fragments after PromptStack exists.
- Prompt block IDs/versions must be recorded in AgentRunEvent.
- Cloud-disallowed prompt blocks must be filtered or summarized before cloud model calls.
- Tool schema and tool discipline belong in ToolPrompt.
- PrivacyPrompt must be included for cloud-routable tasks.

## Verification

Tests should prove:

- PromptStack includes required blocks for chat.
- Cloud-disallowed blocks are omitted or summarized.
- Prompt block IDs/versions are traceable.
- ToolPrompt excludes declarative-only tools.
- PlanMode uses planning output schema.

## Open Questions

1. What exact wording should the BaseSystemPrompt use?
2. Should prompt blocks be stored in code, config files, or database?
3. Which prompt blocks can be user-customized?
4. How much assembled prompt text should be visible in Runs UI?
