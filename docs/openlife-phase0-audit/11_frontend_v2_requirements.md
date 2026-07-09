# Frontend V2 Requirements

## Product Requirements

1. Show intent as a first-class object.
   - Source: backend `IntentFrame` and route decision.
   - Requirement: user can see and correct what OpenLife thinks the task is.

2. Show agent work as a staged timeline.
   - Source: final delivery, kernel events, durable events, tool calls,
     transcript entries.
   - Requirement: normal users see stages; advanced users can inspect evidence.

3. Preserve proposal-first governance.
   - Source: ReviewWorkflow, ProposalStore, Mailbox, LifeStateProjection.
   - Requirement: proposal creation never reads as durable completion.

4. Use backend read models for shared truth.
   - Source: `LifeStateProjection`.
   - Requirement: readiness, pending review count, safe mode, task state, and
     tool permission summaries come from backend projection.

5. Make memory lanes understandable.
   - Source: MemoryGateway, memory lifecycle, LifeModel provenance.
   - Requirement: distinguish context-only, local memory, proposal required,
     accepted, materialized, and rolled back.

6. Keep safe-mode and danger actions visible.
   - Source: danger preflight, safe mode projection, safe paths.
   - Requirement: risky actions require clear preflight and typed confirmation
     where applicable.

7. Support task continuity.
   - Source: task sessions, agent state snapshot, task controls.
   - Requirement: users can resume, cancel, retry, inspect, or unblock tasks.

8. Separate everyday product from advanced diagnostics.
   - Source: current advanced route group and diagnostic panels.
   - Requirement: diagnostics remain available but do not dominate default
     workflows.

## Technical Requirements

- Use `sendMessageV2` or streaming structured result, not reply-only
  `sendMessage`.
- Keep `frontend/src/tauri.ts` as one product bridge.
- Keep `frontend/src/tauriDev.ts` out of product pages.
- Do not reconstruct readiness or proposal counts from raw fragments when
  projection exists.
- Model all blocking and pending states explicitly in UI.
- Add tests around view-model state classification before moving routes.

## Human Approval Required

- Product IA and route naming.
- Whether Companion and Chat remain separate or merge into one workbench.
- Which memory lanes allow direct local materialization.
- Which advanced diagnostics remain user-visible.
- Whether manual LifeModel editor save remains in v2 and what warning copy it
  uses.
