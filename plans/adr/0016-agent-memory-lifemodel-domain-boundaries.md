# ADR 0016: Agent Memory, LifeModel, Domain, Safety, And Runtime Boundaries

Date: 2026-08-06
Status: accepted
Supersedes: ADR 0013
Preserves: ADR 0014 explicit user Memory lane and ADR 0015 transient StateStore lane

## Context

OpenLife is a personal Agent OS. Its Agent must remain useful without turning
LifeModel into a replacement for Agent Memory, business data, policy, or the
runtime itself. ADR 0013 combined evidence, transient state, heuristics,
policies, regression, audit, and the user model into a broad LifeModel-HS
target. That made ownership unclear and allowed an optional personalization
store failure to disable unrelated Agent work.

The shipped Main Chat path also produced generic
`lifemodel.pending.chat_conversation` proposals. Review could approve those
records, but the LifeModel patch materializer requires an existing structured
field path and a typed replacement value. Such a record was therefore a review
artifact without a valid materialization contract.

## Decision

OpenLife has five cooperating owners:

1. **Agent Runtime** owns turn orchestration, planning, model calls, tool
   execution, task progress, action receipts, cancellation, and recovery.
2. **Agent Memory** owns conversation, Workspace/project context, episodic and
   semantic retrieval, procedural working rules, Reflection, and bounded
   Markdown working memory.
3. **LifeModel** owns confirmed, durable understanding of the user: identity,
   values, long-term goals, stable preferences, personal boundaries, important
   relationships, collaboration style, and decision principles.
4. **Domain stores** own their business facts. StateStore owns transient state;
   task and action stores own execution state; connectors own calendar, email,
   and other external objects.
5. **Safety and governance** own privacy policy, permissions, confirmation,
   ReviewWorkflow, audit, and materialization admission. LifeModel and Memory
   may influence a decision but can never grant authority.

Evidence and proposals are bridges, not a sixth source of truth. Observations
may become candidates. A LifeModel proposal exists only when it carries an
exact supported field path and typed value. Approval is not materialization;
the accepted proposal must still pass version, conflict, gateway, and commit
checks.

## Persistence And Projection Rules

- SQLite and local files are infrastructure, not the product architecture.
- Every durable fact has one domain owner.
- FTS, vector indexes, caches, runtime packets, YAML, and Markdown views are
  projections or bounded context surfaces unless an owning contract explicitly
  says otherwise.
- YAML is the deterministic human-readable LifeModel view. It must not drift as
  an independently writable authority. User edits become structured diffs and
  use the same proposal path.
- A failure in optional LifeModel, learning, or enriched-retrieval stores makes
  that capability unavailable and visible. It does not disable a healthy base
  Agent. The affected gateway still fails closed.
- Startup reconciliation and multi-owner recovery remain conservative and may
  require every participating store to be healthy.

## Learning Boundary

Heuristic learning is not a separate product or policy system. Task outcomes,
Memory, and Reflection may derive evidence candidates. Stable, user-related
information can later become a typed LifeModel proposal after relevance,
stability, duplication, conflict, and sensitivity checks. Procedural rules for
how the Agent performs work stay in Agent Memory unless they are explicitly a
long-term user collaboration preference supported by a LifeModel field.

## Immediate Consequences

- ADR 0013 remains historical design evidence but is no longer current
  architecture authority.
- `PolicyStore`, StateStore, regression tests, and audit records are not
  canonical LifeModel assets.
- Existing EvidenceStore and HeuristicStore code may remain while later slices
  decide whether to narrow, migrate, or remove it; its existence does not grant
  ownership.
- Generic LifeModel chat requests fail closed with
  `lifemodel_typed_diff_required`; they do not create fake pending proposals.
- This decision does not implement the full Agent Memory system, redesign the
  complete LifeModel schema, or complete the learning loop.

## Verification

- A LifeModel store failure leaves base Agent effect admission available while
  an exact LifeModel write admission is rejected.
- Missing lifecycle Memory contributes an explicit degraded context marker
  rather than aborting the turn or pretending healthy data exists.
- Procedural future rules route to Agent Memory proposal candidates.
- Supported Main Chat LifeModel changes create proposals with existing field
  paths and typed values; generic and Markdown requests create no LifeModel
  proposal.
