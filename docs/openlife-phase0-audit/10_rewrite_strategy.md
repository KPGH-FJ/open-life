# Rewrite Strategy

## Is A Frontend Rewrite Justified?

Finding: A frontend rewrite is justified if it is framed as product experience
and state-boundary redesign, not as a blank rebuild.

Evidence:

- Backend/domain primitives are substantial and should be preserved.
- Current frontend pages are feature-rich but state-heavy and page-local.
- Shared product state already has a backend projection but pages still combine
  raw diagnostics/proposals/config/LifeModel calls.

File location:

- `openlife-core/src/`
- `src-tauri/src/life_state_projection.rs`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`

Confidence: High.

Impact: Rewriting everything from scratch would be high risk. Reworking the UI
architecture around backend read models is justified.

## Preserve

- `OpenLifeTurnRuntime` send/stream convergence.
- `ToolGateway`, tool permission store, and manifest contract checks.
- `ReviewWorkflow` and ProposalStore semantics.
- `MemoryGateway` and LifeModelWriteGateway direction.
- LifeModel, patch, evidence, lifecycle, and provenance structures.
- LifeStateProjection as shared product-state authority.
- Danger action preflight and safe-path file write constraints.
- Existing guard tests and Phase7 deletion manifest semantics.

## Redesign

- Chat as a workspace with visible intent, plan, execution, evidence, and
  review states.
- Page-local state fetching into view-model/read-model boundaries.
- Review Center as a governed decision system, not just a mailbox list.
- Memory and LifeModel experience around lane/status/provenance.
- Settings into user-facing readiness, privacy, provider, and advanced admin
  groups.
- Advanced diagnostics into inspectable drawers/panels instead of default
  product clutter.

## Avoid

- Do not restore old Stage/Beta/migration/cutover routes.
- Do not replace backend projections with frontend guesses.
- Do not hide blockers as successful completions.
- Do not move implementation before human review of this audit.
- Do not use a marketing landing page as the first screen of the product.

## Recommended Rewrite Shape

1. Define frontend v2 information architecture around six product surfaces:
   Today, Companion, Workbench/Chat, Review, LifeModel, Settings.
2. Define one page-level view model per surface, fed by backend read models.
3. Keep current Tauri commands and backend stores stable during design.
4. Build V2 components around agent timeline, intent frame, plan artifact,
   review decision, memory lane, and evidence/provenance primitives.
5. Migrate incrementally behind route/component boundaries after human approval.
