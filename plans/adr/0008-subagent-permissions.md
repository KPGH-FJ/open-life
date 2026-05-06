# ADR 0008: SubAgent Default Permissions and Delegation Modes

Date: 2026-05-06
Status: proposed

## Context

Sub-agents are important for a modern Agent Framework, but they amplify any weakness in runtime, tool, context, and privacy governance. P0/P1 establishes the foundation; sub-agents should be introduced only after AgentSpec, PromptStack, ToolRuntime, and AgentRunEvent are stable.

## Decision

Sub-agents must be governed agents. `AgentSpec` is canonical; `SubAgentSpec` adds delegation constraints.

First implementation should support `call_as_tool`. Handoff, parallel, and review modes should come later.

## First Sub-Agent Roles

Recommended first roles:

- `PlannerAgent`: read-only planning and task decomposition.
- `CodebaseExplorerAgent`: read-only codebase exploration.
- `MemoryCuratorAgent`: memory evidence analysis, proposal draft only.
- `LifeModelGuardianAgent`: risk and governance review.
- `ReviewAgent`: review plan/output/patch.

## Default Permissions

Default stance:

- no direct writes
- no shell
- no unrestricted network
- no raw full LifeModel unless explicitly allowed
- no durable Memory or LifeModel mutation
- child AgentRun links to parent

## Delegation Modes

Phase order:

1. `call_as_tool`
2. `review`
3. `parallel`
4. `handoff`

`handoff` should be last because it changes control ownership.

## Implementation Guardrails

- Every child run must have parent run linkage.
- Each sub-agent has isolated context policy.
- Each sub-agent has its own PromptStack.
- Tool calls still go through ToolRuntime.
- Sub-agent output must have a schema.

## Verification

Tests should prove:

- sub-agent context isolation
- denied tool cannot be called
- child AgentRun links to parent
- main agent can cite sub-agent result
- sub-agent cannot mutate LifeModel directly

## Open Questions

1. Should sub-agent specs be stored as files, database rows, or Rust defaults?
2. Which sub-agent role ships first?
3. Should users see child runs by default?
