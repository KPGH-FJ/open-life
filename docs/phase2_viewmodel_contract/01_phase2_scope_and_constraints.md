# Phase 2 Scope And Constraints

Status: Phase 2 contract authority for this documentation package.

## Phase 2 Goal

`DESIGN_DECISION`: Phase 2 converts the accepted Phase 1 UX / IA / product-language decisions into an engineering contract for backend-owned read models and frontend ViewModel consumption.

`DESIGN_DECISION`: Phase 2 is not Frontend V2 implementation.

`DESIGN_DECISION`: The output of this phase is a human-reviewable contract package under `docs/phase2_viewmodel_contract/`.

## Non-goals

`DESIGN_DECISION`: This phase must not create or modify:

- React pages, routes, components, or CSS.
- ProductShell or navigation implementation.
- ChatPage, MailboxPage, RunsPage, LifeModelPage, MemorySearch, or SettingsPage implementation.
- Backend schema migrations.
- New Tauri commands.
- Backend stores, mock APIs, or fake read-model implementations.
- Hardcoded frontend-only ViewModels.
- Production Rust, Tauri, React, or TypeScript behavior.

## Source-of-truth Rule

`DESIGN_DECISION`: Pages cannot reconstruct product truth from raw domain reads.

`DESIGN_DECISION`: Pages may only render backend-owned ViewModels / ReadModels, or raw data explicitly marked as debug-only.

`DESIGN_DECISION`: If the backend owner is missing, the contract must mark the field `PROPOSED`, `UNKNOWN`, or `PHASE_2_REQUIRED`; the page must render loading, empty, error, stale, or unknown states instead of inventing truth.

## Product Capability Preservation Rule

`DESIGN_DECISION`: Important OpenLife capabilities must not be deleted because implementation is incomplete.

`CANDIDATE`: The following capabilities remain in scope as candidate or Phase 2 required contracts:

- Memory top-level navigation and lane model.
- LifeModel provenance and change explanation.
- Review Center beyond proposals.
- Tool permission and external-write review.
- Advanced evidence inspection.
- Workspace execution timeline.
- Provider/privacy boundary summary.

`DESIGN_DECISION`: Guardrails prevent hallucination. They must not reduce OpenLife into a generic chat app, todo app, dashboard, settings panel, or knowledge base.

## No Fake Backend Contract Rule

`DESIGN_DECISION`: A contract shape in this package is not an implementation claim.

Required wording for missing owners:

```text
Backend owner: Proposed <Name>
Owner status: PHASE_2_REQUIRED
Required validation: <what Phase 3/engineering must verify or implement>
```

`DESIGN_DECISION`: Frontend helpers such as display formatters may remain display-only helpers, but they must not become source-of-truth owners for readiness, risk, allowed actions, materialization, task lifecycle, Memory lane state, LifeModel canonical/current state, or provider/privacy boundary truth.

## Phase 3 Entry Boundary

`PHASE_2_REQUIRED`: Phase 3 should not start unless:

- humans approve ViewModel owners or accept explicit limits;
- contract gaps are accepted or scheduled;
- no fake backend owner exists;
- implementation scope is narrowed to a first vertical slice;
- `NOT_READY` surfaces are excluded from implementation;
- raw domain reads are either removed from product truth paths or marked debug-only;
- ReviewItem decision status is separate from durable materialization/apply state.

## Human Review Gates

`PHASE_2_REQUIRED`: Before Phase 3, humans must approve:

1. Whether `记忆` remains top-level or moves under LifeModel / Settings.
2. ReviewItem materialization model.
3. WorkspaceViewModel contract.
4. ReviewCenterViewModel contract.
5. Whether to expand `LifeStateProjection` or add dedicated read models.
6. Diagnostics visibility and support/developer mode.
7. Provider/privacy trust summary.
8. First vertical slice implementation scope.
