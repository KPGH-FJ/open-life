# Audit Summary

## What OpenLife Actually Has Today

OpenLife has a real backend and product shell:

- Tauri 2 desktop shell.
- Rust command layer.
- `openlife-core` domain/runtime crate.
- React/Tailwind frontend.
- Main Chat send/stream path converging through `OpenLifeTurnRuntime`.
- Policy routing, task sessions, transcripts, kernel events, final delivery.
- ToolGateway, permissions, proposal store, ReviewWorkflow.
- MemoryStore, MemoryGateway, LifeModel, LifeModelWriteGateway.
- Evidence, memory lifecycle, MCP audit, privacy engine, danger preflight.
- Product pages for Today, Companion/Chat, LifeModel, Mailbox, Runs, Settings,
  and advanced tools.

## What Frontend Problems Are Real

- Chat page owns too many concerns in local state.
- Product pages still combine backend projection with raw page-local source
  interpretation.
- The old reply-only chat wrapper still exists.
- Technical diagnostics are available but not shaped into an everyday agent
  timeline.
- Memory and LifeModel governance are stronger in backend code than in product
  explanation.
- Frontend typecheck is currently `UNKNOWN` because dependencies are absent.

## Is Frontend Rewrite Justified?

Yes, if it is a bounded product-experience and state-contract rewrite.

No, if it means discarding backend/domain primitives or rebuilding old routes.

## What Should Be Preserved?

- Core LifeModel and provenance model.
- Memory, proposal, review, tool, privacy, and audit primitives.
- `OpenLifeTurnRuntime` as current send/stream owner.
- `LifeStateProjection` as shared product-state authority.
- Phase7 deletion manifest and single-system guards.
- Safe mode, danger preflight, and proposal-first policy.

## What Should Be Redesigned?

- Chat/Companion as an agent workspace.
- Review Center as the central approval/workflow surface.
- Memory and LifeModel UX around lanes, status, and provenance.
- Settings and diagnostics hierarchy.
- Frontend view-model boundaries.
- Execution timeline and user controls.

## What Decisions Require Human Approval?

- Final frontend v2 IA and route names.
- Companion vs Chat merge/split.
- Memory lane policy for direct local materialization.
- How manual LifeModel editing is exposed.
- Which advanced diagnostics remain visible to normal users.
- Whether to install frontend dependencies and run full frontend gates in the
  next validation pass.

## Major Findings

1. Backend capability is substantial and should be preserved.
2. Runtime convergence exists, but Main Chat Agent Execution v1 remains partial.
3. Governance primitives are real, but single authority is not fully closed in
   every code path.
4. Frontend rewrite is justified by UX/state-boundary debt, not by lack of
   backend implementation.
5. Product trial status remains blocked/red until a real desktop trial proves
   otherwise.
