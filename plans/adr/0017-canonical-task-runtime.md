# ADR 0017: Canonical Task Runtime

Status: Accepted

Date: 2026-08-12

## Context

OpenLife has a real production Main Chat owner in `OpenLifeTurnRuntime`, but its
durable lifecycle is distributed across task sessions, Agent runs, action
queues, event streams, proposals, artifacts derived from proposals, and a
separate PlanExecute session. A Main Chat operation is effectively a one-turn
task slice. This makes general multi-step work, steering, approval continuation,
artifact delivery, and recovery harder to reason about.

The product target is a general knowledge-work Agent comparable in task shape
to leading desktop Agents. A user delegates an outcome; the product may answer
directly, plan, use tools adaptively, pause for a decision, or later coordinate
subagents without changing the task's identity or product lifecycle.

## Decision

OpenLife will evolve the existing production runtime into one canonical Task
Runtime. It will not build a parallel canonical runtime.

The durable identity hierarchy is:

```text
Task -> Run -> Item -> ItemAttempt
                    -> ReviewCheckpoint
                    -> ArtifactVersion
```

- `Task` owns the user outcome, task contract, steering history, result, and
  terminal product status.
- `Run` owns one execution or recovery attempt for the Task.
- `Item` is a typed lifecycle fact such as instruction, contract, plan,
  progress, tool call, observation, steering, checkpoint, artifact change,
  verification, or final result.
- `ItemAttempt` owns retries, cancellation epoch, receipt, and effect certainty.
- `Artifact` has an identity independent of Proposal and owns versioned result
  references. The actual file remains the content authority.

Policy decides authorization, risk, allowed capability, scope, and data route.
It does not choose or persist an execution algorithm. Planning, direct response,
adaptive tool use, and future subagent coordination are internal policies of the
same Task Runtime.

Review is an Item checkpoint. Approval mints the narrow grant needed to resume
the same Item; it does not create a new task or prove materialization.

Agent Memory and LifeModel are accessed through narrow ports. They may supply
bounded context or receive governed materialization requests, but they do not
own task state, permission, execution strategy, or completion.

SQLite is the single canonical recovery authority for Task, Run, Item,
approval, receipt, scope, and artifact metadata. JSONL may be diagnostic or
export output only. Backend ViewModels are rebuildable projections.

## Migration constraints

- Retain and adapt `OpenLifeTurnRuntime`, ToolGateway, ReviewWorkflow,
  ArtifactMaterializer, provider validation, cancellation, receipts, outbox,
  and explicit `effect_unknown` semantics.
- Remove the equation between turn, operation, and Task.
- Retire independent PlanExecute lifecycle ownership; a Plan becomes an Item.
- Replace multiple action, observation, transcript, and event lifecycle owners
  with canonical typed Item transitions.
- Never dual-write a production concern and never silently fall back to a
  retired runtime.
- A temporary development/test switch is allowed only while one production
  write owner remains unambiguous.
- Migrate by complete vertical product paths. The first path is local documents
  plus Web research producing a sourced and verified Markdown report.

## Consequences

The migration reuses proven safety and execution contracts while deleting
duplicated lifecycle ownership. It requires schema and ViewModel changes, but
does not require a big-bang rewrite. UI redesign remains independent because
the frontend consumes backend-owned Task and Artifact ViewModels.

This ADR does not authorize computer use, arbitrary shell, automatic
cross-provider routing, full subagent orchestration, or broader LifeModel
learning. Those remain outside the first product path.
