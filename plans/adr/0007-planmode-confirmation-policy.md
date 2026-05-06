# ADR 0007: PlanMode Confirmation Policy

Date: 2026-05-06
Status: accepted

## Context

P0/P1 establishes trace, tool policy, PromptStack, LifeModel risk classification, and MemoryEvidence. The next stage needs structured planning before complex or risky execution. PlanMode must avoid turning the model into an unbounded executor.

## Decision

Introduce PlanMode as a governed runtime mode that can perform read-only exploration, produce a structured `AgentPlan`, request confirmation when required, and then execute the confirmed plan through ToolRuntime.

## PlanMode Triggers

PlanMode should be required for:

- high-risk LifeModel changes
- external writes
- multi-step tool execution
- tasks involving unknown files/workspaces
- tasks requiring sub-agent delegation
- ambiguous user intent with nontrivial side effects

PlanMode may be optional for:

- normal chat
- simple read-only answers
- low-risk memory search
- simple proposal generation

## Planner Permissions

The Planner can:

- use read-only tools under policy
- inspect allowed context
- generate `AgentPlan`
- generate proposals

The Planner cannot:

- write files
- mutate LifeModel or Memory
- call bash/shell
- execute external side effects
- bypass Proposal/Permission/Audit

## AgentPlan Required Fields

- goal
- assumptions
- missing_context
- steps
- tool_intents
- subagent_assignments
- permission_requirements
- rollback_plan
- success_criteria
- risk_level

## Confirmation Rules

Require user confirmation when:

- risk is medium or higher
- any write/external side effect exists
- sub-agent handoff is planned
- bash/shell is involved
- plan changes LifeModel or durable memory

No confirmation required when:

- plan is purely read-only and low risk
- the task is short and direct
- the user explicitly asks for immediate execution and policy allows it

## Implementation Guardrails

- PlanMode must record `plan.created` and `plan.confirmation_requested` events.
- ExecuteMode must record deviations from the confirmed plan.
- Plan confirmation UI can be minimal in first implementation.
- Planner prompt must be built via PromptStack.

## Verification

Tests should prove:

- high-risk task enters PlanMode
- read-only low-risk task can bypass PlanMode
- Planner cannot execute write tools
- confirmed plan executes through ToolRuntime
- plan deviation is recorded as an event

## Open Questions

1. Should PlanMode be user-selectable in the UI?
2. Should users be able to edit plan steps before execution?
3. What is the minimal first Plan UI?
