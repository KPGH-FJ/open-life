# OpenLife Product Definition

## Purpose

OpenLife is a private personal Agent OS. Its purpose is to let a user work with
AI that can understand bounded personal context, carry out useful tasks, and
improve that context over time without taking ownership away from the user.

## Current Product Loop

1. The user starts or continues work in Workspace.
2. Main Chat selects an answer, planning, read, tool, or governed-action path.
3. Tasks and execution evidence remain visible in the Workbench.
4. Proposed durable changes appear in Review Center.
5. Only an explicit user decision may materialize governed LifeModel or memory
   changes.

## Core Surfaces

- **Today**: current read model and safe status summary.
- **Workspace**: conversation and task execution.
- **Tasks**: task state, blockers, retry, resume, and cancellation.
- **Review**: approval, rejection, postponement, and evidence.
- **Personal Intelligence** (`/life-model`): two peer areas with separate
  backend owners: user-owned long-term understanding in LifeModel, and
  user-controlled Agent Memory for work continuity.
- **Settings**: model configuration, privacy boundaries, and credential state.

## Non-Negotiable Boundaries

- No silent durable writes.
- Assistant text is not write authorization.
- External and sensitive actions require a confirmed capability and risk
  contract.
- Missing, stale, or failed evidence must remain visibly unknown or blocked.
- Product state must come from its backend read model when one exists.
- Local, scripted, mock, browser-shell, native-Tauri, and external-live evidence
  are different evidence levels.

## Current Development Priority

Improve the real product and its user experience. Repository governance should
remain small and conventional: one active plan, normal source tests, normal CI,
and concise architecture/decision records. OpenLife must not grow a second
internal platform for planning or evaluating its own development.
