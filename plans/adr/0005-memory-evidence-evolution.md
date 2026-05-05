# ADR 0005: MemoryEvidence and LifeModel Evolution Proposal Policy

Date: 2026-05-06
Status: accepted

## Context

OpenLife currently supports memory writes, memory search, feedback, calibration, micro-evolution, and LifeModel proposals. But memory is not yet a formal evidence layer for LifeModel evolution.

vNext should let accepted memories influence LifeModel updates without allowing silent personality drift or speculative inference.

## Decision

Introduce `MemoryEvidence` and a `LifeModelEvolutionEngine` that generates evidence-backed proposals only. It must not directly mutate LifeModel.

## Evolution Chain

```text
Accepted Memories
-> Evidence Aggregation
-> Pattern / Contradiction / Trend Detection
-> LifeModel Impact Analysis
-> Risk Classification
-> Evolution Proposal
-> User Review
-> Patch / Snapshot / Audit
```

## Evidence Sources

Allowed evidence:

- user-accepted MemoryWrite records
- explicit user statements in current conversation
- accepted/rejected proposal history
- feedback signals
- repeated vector/search hits that point to accepted records

Not sufficient alone:

- raw unaccepted chat transcript
- model speculation
- one-off ambiguous phrasing
- inferred identity changes without explicit user evidence

## Proposed MemoryEvidence Schema

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

## Evidence Thresholds

Initial conservative defaults:

- Low-risk state proposal: 1 explicit evidence item may be enough.
- Medium-risk preference/capability/goal proposal: 2 or more consistent evidence items or 1 explicit user statement.
- High-risk identity/value/mission proposal: explicit user statement plus review; repeated implicit memories are not enough.

## Options Considered

### Option A: Memory never updates LifeModel

Pros:

- Safest.

Cons:

- LifeModel becomes stale.
- OpenLife loses a key differentiator.

### Option B: Memory generates proposals only

Pros:

- Evolves while preserving user sovereignty.
- Evidence-backed and auditable.

Cons:

- Requires more UI and trace.

### Option C: Memory auto-updates low/medium fields

Pros:

- Fresh model.

Cons:

- Risk of silent drift.

## Recommendation

Use Option B. Consider low-risk auto-state only after a separate accepted ADR.

## Consequences

Positive:

- Memory becomes meaningful beyond retrieval.
- LifeModel evolution becomes explainable.
- Rejections can teach the system.

Tradeoffs:

- More proposal volume.
- Need evidence UI.

## Implementation Guardrails

- EvolutionEngine creates proposals only.
- Every evolution proposal must include evidence links.
- High-risk fields require explicit review.
- Contradictions should produce clarification proposals/questions rather than confident patches.
- Rejected proposals should reduce future confidence for similar claims.

## Verification

Tests should prove:

- Repeated accepted memories create a medium-risk proposal.
- One ambiguous memory does not create high-risk proposal.
- Contradictory memories produce a contradiction/clarification result.
- Rejected proposal affects future evidence scoring.
- Evolution proposals include memory IDs.

## Open Questions

1. Where should MemoryEvidence be stored?
2. Should evidence be generated continuously, scheduled, or on-demand?
3. How should evidence be displayed in Review Center?
4. How long should rejected proposal negative evidence persist?
