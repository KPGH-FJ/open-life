# ADR 0011: Plan Recovery and Rollback Policy

Date: 2026-05-06
Status: accepted

## Context

P4 introduced confirmed plan execution, step events, review gates, and trace UI integration. P5 needs cancellation, retry, blocked action continuation, and eventually rollback. These operations can affect external systems and user trust if they are implemented as generic undo. OpenLife needs a conservative recovery policy before adding operational controls.

## Decision

Plan recovery must be explicit, traceable, and proposal-first for side effects.

Initial P5 policy:

- Cancellation stops future plan progress; it does not undo completed side effects.
- Retry creates a new execution attempt and never deletes prior events.
- Blocked action continuation reuses existing Permission / Proposal / Replay mechanisms.
- Rollback is not automatic in P5 unless the affected operation has a known reversible local representation.
- Irreversible external side effects must be surfaced as irreversible before execution.

## Retry Policy

Allowed:

- retry a plan from `failed`
- retry a plan from `failed_review`

Not allowed in first implementation:

- retry from arbitrary step
- retry completed plans
- retry rejected plans
- retry by mutating historical events

Every retry must record a new attempt marker in `AgentRunEvent`.

## Cancellation Policy

Allowed:

- cancel `published`
- cancel `confirmed`
- cancel `executing`

Not allowed:

- cancel `completed`
- cancel `rejected`

Cancellation must record both requested and resolved events when possible.

## Rollback Policy

Rollback-capable candidates:

- pending proposal state changes
- local file writes created from `ExternalWriteAction` when previous content/hash is available
- local scheduled task proposals before execution
- local generated draft artifacts before external send

Not rollback-capable by default:

- sent email
- external API side effects
- A2A remote agent actions
- calendar writes to external services
- any operation without a prior reversible snapshot

Rollback requires explicit user confirmation for medium/high risk effects.

## Implementation Guardrails

- Never implement shell-based rollback in P5.
- Never delete AgentRunEvent history during retry or rollback.
- Never directly mutate LifeModel or Memory as rollback; use Proposal or versioned snapshot paths.
- Every recovery action must include `plan_id`, `run_id`, and attempt or action linkage in event payload.
- Recovery commands must fail closed when linkage is missing.

## Verification

Tests should prove:

- retry is rejected from illegal statuses
- retry appends new events without deleting old events
- cancellation is rejected after completed/rejected status
- blocked action continuation uses ActionExecutor replay policy
- rollback-capable operations require stored reversible metadata
- irreversible operations report non-rollbackable status

## Open Questions

1. Should retry create a new `AgentRun` or reuse the parent run with attempt markers?
2. Should rollback proposals appear in the same Review Center queue as other proposals?
3. Should users be able to override failed review and complete anyway?
