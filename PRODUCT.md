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

## Target Product Loop

1. The user opens or creates a Conversation in the Workbench. Chat provides a
   direct answer; Work accepts a meaningful outcome and optional files,
   sources, Project scope, constraints, and desired deliverables.
2. Chat and Work share one canonical Conversation, Turn, and typed Item spine.
   Work adds a durable Task and Run with an editable completion contract.
3. OpenLife plans when useful, uses authorized capabilities adaptively, and
   lets the user follow progress, steer, answer questions, pause, resume, or
   approve an important boundary without losing context.
4. OpenLife returns one canonical FinalResult with relevant Artifacts, changes,
   sources, limitations, and verification state.
5. Durable or external effects follow the applicable task scope and risk contract.
   A Review proposal is used only when a governed change needs asynchronous or
   durable review; it is not the container for every task or action.

## Target Core Surfaces

- **Workbench** (`/workspace`): Projects, Conversations, Chat and Work,
  progress, steering, inline decisions, results, and a Needs Attention filter.
- **Personal Intelligence** (`/life-model`): two peer areas with separate
  backend owners: user-owned long-term understanding in LifeModel, and
  user-controlled Agent Memory for work continuity.
- **Settings**: provider/model profiles, privacy and transmission boundaries,
  credential recovery, local data controls, and diagnostics.

Task, Run, Item, and Approval remain explicit backend facts. They do not each
require a separate top-level product page.

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

## Current Development Baseline

OpenLife has one canonical Chat/Work spine and one Workbench snapshot boundary.
The current repository cleanup removes replaced backend, frontend, test,
script, and documentation owners without changing that product contract. This
is an engineering baseline, not a claim of market readiness or complete future
capability coverage.

Earlier development programs remain in Git history. ADR 0018 and ADR 0019
remain the accepted reconstruction and harness contracts; the active repository
cleanup plan is the sole current implementation plan.

Repository governance remains small and conventional: at most one active plan, normal
source tests, normal CI, and concise architecture and decision records.
OpenLife must not grow a second internal platform for planning or evaluating
its own development.
