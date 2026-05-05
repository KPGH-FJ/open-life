# ADR 0004: LifeModel Field Risk Classification

Date: 2026-05-06
Status: accepted

## Context

LifeModel is central to OpenLife. Updating it changes how future agents understand and guide the user. vNext needs a field-level risk classification so memory, feedback, calibration, chat, and proposal flows do not treat all LifeModel mutations equally.

## Decision

Classify LifeModel fields into high, medium, and low risk. Risk determines proposal requirements, evidence threshold, cloud disclosure, and auto-apply eligibility.

## Proposed Risk Classes

### High Risk

Never auto-apply. Requires explicit user review.

- identity values
- life philosophy
- mission statement
- role definition
- long-term goals
- relationship definitions with sensitive implications
- major preference shifts that affect identity or life direction

### Medium Risk

Proposal-first. May use memory evidence, but must remain reviewable.

- short/medium-term goals
- capabilities and skill proficiency
- work style preferences
- communication preferences
- routines and habits
- important relationships
- stable constraints and resources

### Low Risk

Traceable and possibly eligible for lightweight confirmation or policy-based update after approval.

- current focus
- temporary state
- recent activity summary
- low-impact metadata like `last_updated`
- transient emotional/health status when explicitly stated

## Options Considered

### Option A: All LifeModel changes require explicit proposal

Pros:

- Safest.
- Simple mental model.

Cons:

- Too much review friction.
- State updates become noisy.

### Option B: Risk-based policy

Pros:

- Balances sovereignty and usability.
- Lets low-risk state stay fresh.

Cons:

- Requires careful classification and tests.

### Option C: AI decides risk dynamically

Pros:

- Flexible.

Cons:

- Too much implicit power.
- Harder to trust and audit.

## Recommendation

Use Option B, with conservative defaults:

- High risk: explicit proposal only.
- Medium risk: proposal-first.
- Low risk: proposal-first initially; policy-based lightweight updates only after separate acceptance.

## Consequences

Positive:

- LifeModel governance becomes explicit.
- Memory-driven evolution has safe boundaries.
- Cloud disclosure can use the same classification.

Tradeoffs:

- Need field mapping.
- Need UI risk explanations.

## Implementation Guardrails

- High-risk fields never auto-apply.
- Memory-driven evolution cannot directly write any field.
- Rejected high-risk proposals should reduce confidence in similar future proposals.
- Cloud prompts should not include high-risk raw fields unless explicitly allowed.

## Verification

Tests should prove:

- High-risk proposal cannot auto-apply.
- MemoryEvidence proposal for high-risk field requires explicit review.
- Low-risk update still creates trace.
- Risk classification is stable and covered by tests.

## Open Questions

1. Should current emotional/health state be low or medium risk?
2. Which relationship fields are high risk?
3. Should users be able to customize field risk levels?
4. Should direct calibration apply mode be removed or hidden behind debug/migration only?
