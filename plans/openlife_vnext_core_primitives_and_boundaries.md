# OpenLife vNext Core Primitives and Boundaries

Date: 2026-05-06

This document defines the core primitives that should become first-class architecture concepts in the vNext Agent Framework upgrade.

## Priority Primitives

Implement these first:

1. `AgentRunEvent`
2. `ToolRuntime` / hardened `ActionExecutor`
3. `PromptStack`
4. `AgentPlan`
5. `AgentSpec`
6. `MemoryEvidence`

Implement these after the runtime is more stable:

1. `PlanMode`
2. `SubAgentRuntime`
3. `CompactionSummary`
4. `ExecutionSandbox`
5. `BashExecutor`
6. `Frontend Agent Workspace` state model

## AgentSpec

Purpose:

Defines an agent as a governed runtime unit, not only a prompt.

Scope:

`AgentSpec` is the canonical definition for any first-class OpenLife agent. It can describe the main agent and sub-agents.

`SubAgentSpec` should not become a competing second agent model. It should be modeled as either `AgentSpec + delegation constraints` or as a constrained profile that references an underlying `AgentSpec`.

Suggested fields:

```rust
pub struct AgentSpec {
    pub id: String,
    pub name: String,
    pub role: AgentRoleKind,
    pub base_prompt_id: String,
    pub prompt_blocks: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub allowed_subagents: Vec<String>,
    pub context_policy: ContextPolicy,
    pub tool_policy: ToolPolicy,
    pub memory_policy: MemoryPolicy,
    pub privacy_policy: PrivacyPolicy,
    pub output_schema: Option<OutputSchemaRef>,
    pub max_steps: u32,
}
```

Boundary decisions:

- AgentSpec may define permissions but must not bypass ToolRuntime.
- AgentSpec may request LifeModel context, but ContextAssembler decides what is actually provided under policy.
- Sub-agent AgentSpecs inherit the OpenLife base constitution but may have role-specific prompt blocks.

## AgentTask

Purpose:

Represents the user's or system's intent before execution.

Key fields:

- `id`
- `kind`
- `user_intent`
- `session_id`
- `workspace_scope`
- `privacy_level`
- `requires_plan`
- `expected_output`
- `initiator`: user, proactive, scheduled, replay, sub-agent

Boundary decisions:

- AgentTask is intent and constraints, not execution trace.
- AgentTask should not contain full LifeModel or raw memory payloads.

## AgentPlan

Purpose:

Provides a structured plan before tool execution or complex action.

Suggested fields:

```rust
pub struct AgentPlan {
    pub goal: String,
    pub assumptions: Vec<String>,
    pub missing_context: Vec<String>,
    pub steps: Vec<PlanStep>,
    pub tool_intents: Vec<ToolIntent>,
    pub subagent_assignments: Vec<SubAgentAssignment>,
    pub permission_requirements: Vec<PermissionRequirement>,
    pub rollback_plan: Option<String>,
    pub success_criteria: Vec<String>,
    pub risk_level: RiskLevel,
}
```

Boundary decisions:

- Planner can use read-only tools under policy.
- Planner can generate proposals.
- Planner cannot directly write external state or high-risk LifeModel fields.
- Plan confirmation should be required for high-risk, long-running, external-write, or multi-step tasks.

## AgentRunEvent

Purpose:

Append-only event record for every meaningful runtime transition.

Suggested event kinds:

- `run.created`
- `prompt.assembled`
- `context.assembled`
- `model.route_selected`
- `model.call_started`
- `model.call_completed`
- `model.call_failed`
- `plan.created`
- `plan.confirmation_requested`
- `tool.call_started`
- `tool.call_blocked`
- `tool.call_completed`
- `tool.call_failed`
- `observation.created`
- `proposal.created`
- `proposal.accepted`
- `proposal.rejected`
- `proposal.applied`
- `proposal.apply_failed`
- `fallback.started`
- `fallback.completed`
- `json_repair.started`
- `json_repair.completed`
- `subagent.started`
- `subagent.completed`
- `run.completed`
- `run.failed`

Boundary decisions:

- Events are append-only.
- Sensitive payloads may be redacted, but event kind, timestamp, source, and linkage must remain.
- UI status updates can be derived from events; events should not be derived from UI status updates.

## PromptStack and PromptBlock

Purpose:

Make prompt architecture explicit, versioned, policy-aware, and traceable.

Suggested fields:

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

Boundary decisions:

- The base system prompt must be inherited by all first-class agents.
- Tool instructions belong in ToolPrompt, not scattered across model calls.
- PrivacyPrompt must be applied before cloud model calls.
- Prompt block IDs and versions must be traceable.

## ToolRuntime / ToolPolicy

Purpose:

Make tool execution consistent, inspectable, and safe.

Required tool metadata:

- `name`
- `description`
- `source`
- `executable`
- `declarative_only`
- `risk_level`
- `permission_policy`
- `executor_kind`
- `input_schema`
- `output_schema`
- `side_effect_type`

Boundary decisions:

- Declarative-only tools cannot be model-callable.
- Write tools should create proposals by default.
- ToolRuntime must log attempts, not only successful executions.
- Replay must re-check permission and policy.

## ExecutionSandbox

Purpose:

Controls file, shell, network, browser, and external execution.

Required policies:

- `cwd`
- `safe_paths`
- `deny_read_patterns`
- `deny_write_patterns`
- `network_policy`
- `timeout_ms`
- `max_output_bytes`
- `env_allowlist`
- `command_allowlist`
- `dangerous_command_denylist`

Boundary decisions:

- Bash is default-off until this abstraction exists.
- File reads require safe path plus deny-read checks.
- File writes are proposal-first.
- Shell execution should not inherit the user's full environment.

## SubAgentSpec and SubAgentRuntime

Purpose:

Allows specialized agents without losing governance.

Relationship to AgentSpec:

Sub-agents still run as governed agents. A `SubAgentSpec` should therefore extend or wrap `AgentSpec`, not replace it.

Suggested shape:

```rust
pub struct SubAgentSpec {
    pub agent: AgentSpec,
    pub delegation_modes: Vec<DelegationMode>,
    pub parent_context_policy: ParentContextPolicy,
    pub result_policy: DelegationResultPolicy,
}
```

Suggested fields:

- `id`
- `description`
- `system_prompt_blocks`
- `allowed_tools`
- `context_policy`
- `model_policy`
- `max_turns`
- `delegation_modes`
- `output_schema`

Delegation modes:

- `call_as_tool`
- `handoff`
- `parallel`
- `review`

Boundary decisions:

- Sub-agents must have isolated context windows.
- Sub-agents must have role-specific tool policies.
- Sub-agents cannot silently mutate LifeModel, Memory, filesystem, calendar, email, or external systems.
- Child runs must link to parent AgentRun.

## MemoryEvidence

Purpose:

Turns memory from retrieval context into LifeModel evolution evidence.

Suggested fields:

```rust
pub struct MemoryEvidence {
    pub id: String,
    pub memory_ids: Vec<String>,
    pub evidence_type: EvidenceType,
    pub claim: String,
    pub affected_life_model_path: String,
    pub confidence: f32,
    pub recency_score: f32,
    pub contradiction_ids: Vec<String>,
    pub source_summary: String,
}
```

Evidence types:

- repeated preference
- recurring goal
- capability signal
- state trend
- contradiction
- relationship update
- value signal

Boundary decisions:

- Raw memory is not automatically valid evidence.
- Evidence must link to accepted memory records or explicit user signals.
- High-risk LifeModel changes require multiple evidence points or explicit user confirmation.
- Rejected evolution proposals should become negative evidence.

## LifeModelEvolutionEngine

Purpose:

Generates evidence-backed proposals from memory and feedback.

Responsibilities:

- aggregate accepted memories
- identify repeated patterns
- identify contradictions
- classify affected LifeModel path
- classify risk
- generate proposal with evidence links
- learn from accepted/rejected proposals

Boundary decisions:

- Engine generates proposals, not direct writes.
- It should not bypass ProposalEngine.
- It should respect field risk classification.
- It should summarize evidence safely for cloud models if cloud analysis is allowed.

## ChatPage State Model

Current concern:

ChatPage is a high-risk refactor because streaming, trace display, proposal banners, tool status, and message state are tightly coupled.

Boundary decisions before rewrite:

- Preserve streaming UX.
- Preserve proposal banner behavior.
- Preserve tool/trace visibility.
- Split one component at a time.
- Do not combine backend runtime migration with frontend state rewrite in one task.

Suggested future modules:

- `AgentSurface`
- `ChatTimeline`
- `Composer`
- `ToolPanel`
- `ContextSummary`
- `RunTracePanel`
- `ProposalBanner`
- `StreamingController`

## Open Questions for Human Decision

These must be decided through ADR-first review:

1. Which LifeModel paths are high, medium, and low risk?
2. Can low-risk state fields auto-apply under any policy?
3. Which LifeModel fields are never allowed in cloud prompts?
4. Should PlanMode be explicit, automatic for high-risk tasks, or both?
5. Which sub-agent roles ship first?
6. Can any sub-agent use write tools by default?
7. When should Bash become available, and to which initiators?
8. What is the default deny-read list?
9. How should rejected evolution proposals influence future evidence scoring?
10. How much AgentRun trace should normal users see by default?
