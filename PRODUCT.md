# OpenLife Product Definition

## Purpose

OpenLife is a local-first personal Agent OS for general knowledge work. It lets
a user delegate meaningful, potentially multi-step tasks to a capable Agent,
follow and steer the work, and receive results that can be inspected and
continued. It is not limited to coding work.

OpenLife may use bounded Agent Memory and confirmed LifeModel context to improve
continuity and personalization. Those systems remain optional collaborators of
the Agent harness rather than owners of task execution, permission, or
completion.

## Current Product Loop

1. The user describes an outcome in Workspace and optionally supplies files,
   sources, a workspace, constraints, and a desired deliverable.
2. OpenLife derives a bounded task contract and works through one canonical task
   lifecycle. Planning, adaptive tool use, approvals, and future subagents are
   phases or capabilities inside that lifecycle, not separate product runtimes.
3. The user can follow progress, steer the active task, answer questions, stop
   work, or approve an important boundary without losing the task context.
4. OpenLife returns a reviewable result with relevant artifacts, changes,
   sources, limitations, and verification state.
5. Durable or external effects follow the applicable scope and risk contract.
   A Review proposal is used only when a governed change needs asynchronous or
   durable review; it is not the container for every task or action.

## Core Surfaces

- **Today**: current read model and safe status summary.
- **Workspace**: task delegation, conversation, live progress, steering, and
  results.
- **Tasks**: task lifecycle, blockers, retry, resume, cancellation, and result
  history.
- **Review**: governed proposals, approval, rejection, postponement, and
  materialization evidence.
- **Personal Intelligence** (`/life-model`): two peer areas with separate
  backend owners: user-owned long-term understanding in LifeModel, and
  user-controlled Agent Memory for work continuity.
- **Settings**: model configuration, privacy boundaries, and credential state.

## Non-Negotiable Boundaries

- No silent durable writes.
- Assistant text is not write authorization.
- A task grant authorizes ordinary low-risk, recoverable work inside its
  explicit workspace, resource, provider, and tool scopes. Scope expansion,
  consequential external actions, and destructive or irreversible effects
  require a just-in-time decision.
- External and sensitive actions require a confirmed capability and risk
  contract.
- The provider and model selected by the user remain bound to the task. OpenLife
  may retry that route, but it must not silently switch model or provider.
- Missing, stale, or failed evidence must remain visibly unknown or blocked.
- Plans, tool activity, streaming text, and proposal acceptance are progress
  evidence, not proof that the requested result was completed.
- Product state must come from its backend read model when one exists.
- Local, scripted, mock, browser-shell, native-Tauri, and external-live evidence
  are different evidence levels.

## Current Development Priority

Build a genuinely capable Agent harness through complete vertical task paths,
starting with local documents plus Web research producing a sourced,
previewable, editable, and verified Markdown report. Complete a useful and
reliable path before broadening it.

Repository governance remains small and conventional: one active plan, normal
source tests, normal CI, and concise architecture and decision records.
OpenLife must not grow a second internal platform for planning or evaluating
its own development.
