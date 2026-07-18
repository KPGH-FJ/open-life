# Rebirth Strategy

## Strategic Direction

OpenLife should be reborn as a local-first personal AI operating system, not a
generic chat app and not a generic life-planning dashboard.

The code already supports this direction:

- local/private LifeModel;
- governed runtime and policy routing;
- proposal-first durable writes;
- tool and permission gates;
- memory lifecycle and evidence;
- task state and final delivery;
- desktop shell and local storage.

## Product Narrative

The product should make this promise:

OpenLife helps the user think, plan, act, and remember through a private
LifeModel, while keeping consequential changes reviewable and auditable.

## Rebirth Principles

1. State what the agent understood before acting.
2. Show what it is doing while it acts.
3. Separate observation, proposal, and durable truth.
4. Keep local privacy and external-provider boundaries visible.
5. Make user approval a product control, not a modal afterthought.
6. Preserve evidence without forcing users to read debug logs.
7. Treat unknowns and blockers as honest states.

## Architecture Direction

- Runtime: keep `OpenLifeTurnRuntime` as the product turn owner and continue
  reducing hidden kernel/fallback residue.
- State: expand `LifeStateProjection` or adjacent backend view models rather
  than adding page-local truth.
- Memory: expose gateway/lifecycle status directly.
- Review: make Review Center a workflow for proposals, permissions, memory, and
  LifeModel updates.
- Tools: expose capability/risk/action evidence from `ToolGateway`.
- Frontend: reorganize into a work-focused desktop app with dense, readable
  operational surfaces.

## Sequencing

1. Human review of this Phase 0 audit.
2. Agent UX model design.
3. Frontend v2 IA and component model.
4. View-model/read-model contract updates.
5. Incremental frontend implementation behind guarded route boundaries.
6. Real product trial with isolated data and evidence.

## Non-Goals

- No production code changes in Phase 0.
- No old-route restoration.
- No claim that Main Chat Agent Execution v1 is complete.
- No live-provider readiness claim without live evidence.
- No frontend rewrite before approval of the UX model.
