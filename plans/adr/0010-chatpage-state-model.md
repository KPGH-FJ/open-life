# ADR 0010: ChatPage State Model Migration Policy

Date: 2026-05-06
Status: proposed

## Context

ChatPage is central to user experience and runtime visibility. It handles streaming, messages, proposals, tool status, trace panels, and input state. A large rewrite is risky, especially while backend runtime convergence is ongoing.

## Decision

Migrate ChatPage incrementally. Do not combine backend runtime migration with a large frontend state rewrite.

## Target Component Split

Suggested modules:

- `AgentSurface`
- `ChatTimeline`
- `Composer`
- `ToolPanel`
- `ContextSummary`
- `RunTracePanel`
- `ProposalBanner`
- `StreamingController`

## Migration Rules

- One component extraction per task.
- Preserve streaming behavior.
- Preserve proposal banner behavior.
- Preserve tool/trace visibility.
- Keep Tauri mocks updated with new event types.
- Add regression tests for each extraction.

## Implementation Guardrails

- No wholesale rewrite.
- No visual redesign bundled with state migration.
- No backend API reshaping in the same task unless required by a documented contract.
- New AgentRunEvent UI should consume a stable frontend type.

## Verification

Tests should prove:

- streaming still renders chunks
- pending proposal banner still appears
- tool events still render
- run trace timeline renders AgentRunEvent data
- mocks include new event shape

## Open Questions

1. Should AgentRunEvent timeline be visible by default or behind details?
2. Which component should own stream lifecycle?
3. Should old ReasoningTracePanel be replaced or adapted?
