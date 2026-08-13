# ADR 0018: Product Reconstruction Contract

Status: Accepted

Date: 2026-08-13

## Context

S2-S6 proved a useful report path and S7 retired the independent PlanExecute
surface, but the production product still distributes ordinary Chat and task
lifecycle responsibility across TaskSession, AgentRun, ActionQueue,
EventStream, Proposal, and a report-specific canonical store. The shipped
frontend also exposes repeated Task and Review surfaces and can turn one store
or credential warning into a globally unusable Workbench.

The report path is evidence that several lower-level contracts are reusable.
It is not evidence that the general Agent product has one canonical lifecycle.
OpenLife has no user-owned historical task data that requires retaining those
execution stores or a compatibility runtime.

## Decision

OpenLife will perform a product reconstruction rather than extend the report
slice or append more stages to S0-S7.

The user model is:

```text
Workbench
  -> optional Project
    -> Conversation
      -> Turn
        -> typed Item
        -> optional durable Work Task
          -> Run
            -> typed Item
              -> ItemAttempt / ApprovalCheckpoint
          -> FinalResult
          -> Artifact -> ArtifactVersion
```

- Chat and Work share Conversation, Turn, Item, context, provider, streaming,
  and persistence infrastructure.
- A direct Chat response does not create a Task. Work creates or continues a
  durable Task with an explicit outcome and completion contract.
- One Work Conversation normally focuses on one active outcome. The schema may
  retain multiple historical Tasks and must not enforce Conversation=Task.
- A Run is one execution attempt. Approval, user input, pause, safe checkpoint
  recovery, and steering continue the Run. Retry after failure or cancellation
  creates another Run and preserves prior evidence.
- Plan is an Item. ReAct is an internal scheduling technique. Neither owns a
  product identity, store, route, or terminal state.
- Artifact identity is independent of Proposal. Approval is an Item checkpoint,
  not a task container and not proof of materialization.

SQLite is the only canonical lifecycle and recovery authority. Files remain
content authority for Artifacts; the database stores references, versions, and
digests. Backend ViewModels project canonical state for the frontend.

The shipped top-level surfaces become Workbench, Personal Intelligence, and
Settings. Conversation status and a Needs Attention filter replace duplicate
Tasks and Review navigation. Memory and LifeModel suggestions remain within
Personal Intelligence.

Migration is a clean break for legacy execution/test data. User-owned settings,
credential references, confirmed Memory, canonical LifeModel, and necessary
Project/resource configuration may be retained only through an explicit,
verified migration. The new runtime never reads a retired execution store as a
fallback.

## Retained assets

The reconstruction should retain and adapt contracts that prove their new
responsibility: provider request validation, ToolGateway, ReviewWorkflow,
ArtifactMaterializer, cancellation fencing, typed receipts, persistence outbox,
effect-unknown handling, bounded source evidence, Memory ownership, and
LifeModel ownership.

Retention is not based on age. A component with duplicate ownership,
report-only assumptions, or a real consumer that has not migrated must be
adapted or replaced before its old path is deleted.

## Completion rule

A reconstruction stage is complete only when the user-visible capability,
canonical lifecycle, controls, recovery, ViewModel, frontend, old-path deletion,
behavior tests, and required native/live evidence all agree. Stage completion
cannot be inferred from a schema, plan, process launch, proposal, or a lower
evidence level.

## Relationship to ADR 0017

ADR 0017 remains accepted for the Task/Run/Item direction and reusable lower
level contracts. This ADR supersedes its report-first migration strategy and
adds the Conversation/Turn spine, clean-break authority, reduced product
surfaces, and full vertical completion rule.

## Consequences

The work is larger than a bug fix and may rewrite schema, runtime orchestration,
read models, and frontend journeys. It is still delivered in runnable vertical
stages. Each stage retires the corresponding old consumers instead of leaving
long-lived dual paths.

Computer Use, arbitrary shell, concrete mail/calendar connectors, deep PPTX
editing, scheduling, cloud execution, account sync, and advanced LifeModel
learning remain after R8. They must later enter through the same Task, Item,
Capability, Artifact, and approval contracts.
