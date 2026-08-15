# ADR 0019: Capable Agent Harness and Clean Replacement

Status: Accepted

Date: 2026-08-14

## Context

R0-R8 reconstructed a canonical Conversation and Work baseline, but retained
execution stores, broad persistence admission, deterministic intent routing,
and compatibility modules can still constrain or distort the new product.
Completing one report path is not the same as delivering a general Agent.

OpenLife has one developer, one current user, and no user-owned historical task
execution data that requires compatibility migration. Complexity spent keeping
retired execution owners alive would directly reduce product capability and
development speed.

## Decision

OpenLife will build one general knowledge-work harness:

```text
Conversation -> TaskContract -> Task -> Run
  -> PlanRevision -> Item -> ItemAttempt -> Observation
  -> CompletionEvaluator -> FinalResult
                      \-> ReviewCheckpoint
                      \-> ArtifactVersion
```

- The model owns semantic goal understanding, structured planning, eligible
  tool selection, replanning, and proposing completion.
- Deterministic runtime code owns validation, budgets, dependencies,
  idempotency, receipts, and completion proof.
- Policy owns scope, authorization, risk, and data transmission.
- A Task stores the default budget policy. Each Run stores an immutable budget
  snapshot; each Attempt stores actual usage. Child work shares the parent Run
  ceiling. A limit produces a non-success state and never grants permission.
- Plans are visible, concise Items. Detailed Attempts and receipts remain
  inspectable without becoming the default user interface.
- Artifact content remains in Project/workspace files. SQLite stores Artifact
  identity, versions, digests, provenance, and lifecycle metadata.
- Review is an Item checkpoint. Approval resumes the same Run and is not proof
  of materialization.
- Source content is untrusted data and cannot modify TaskContract, Policy,
  budget, or eligible tools.

The ordinary product exposes model choice and progress rather than raw turn,
token, cost, timeout, retry, and concurrency controls. Those limits remain
explicit runtime contracts. Advanced controls may be added later without
changing lifecycle ownership.

## Clean replacement

Every H0-H6 stage deletes the backend and frontend path it replaces. A short
migration adapter may exist inside a stage, but no release fallback, dual
writer, or second lifecycle owner survives stage closure. Old test data may be
deleted. Verified settings, provider references, Memory, LifeModel, Project,
and resource configuration remain outside that clean break.

Cleanup uses ordinary source deletion, compilation, product tests, and a few
targeted absence guards. OpenLife will not build a compatibility framework,
development ledger, or repository self-governance platform for this work.

## Consequences

This change may rewrite runtime orchestration, persistence admission, provider
and tool integration, read models, and frontend journeys. That is intended.
Reusable safety and execution contracts survive only when they accept canonical
Task/Run/Item/Attempt identity and do not retain an old lifecycle owner.

LifeModel and Agent Memory remain narrow typed ports so their later evolution,
including possible AI-assisted maintenance, does not require rewriting the
Agent harness.
