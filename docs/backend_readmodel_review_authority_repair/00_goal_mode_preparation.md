# Backend ReadModel And Review Authority Repair Preparation

Status: goal-mode preparation artifact, not implementation completion.

Date: 2026-07-09

## Purpose

This package prepares a dedicated Goal-mode development round:

```text
Backend ReadModel & Review Authority Repair Phase
```

The phase exists because the frontend rewrite cannot become high quality while
product pages still infer product truth from raw diagnostics, proposals,
config, task fragments, and memory fragments. The root repair is to move
product truth, review action eligibility, durable materialization state, and
provider/privacy boundary summaries behind backend-owned read models.

This preparation does not start the implementation. It defines the contract for
the implementation so the next Goal-mode run can execute deliberately.

## Authority Stack

Read these files before coding, in this order:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`
5. `docs/phase2_viewmodel_contract/14_phase2_summary_and_phase3_readiness.md`
6. `docs/phase2_viewmodel_contract/12_backend_contract_gap_register.md`
7. `docs/phase3a2_today_preview/04_phase3a2_summary.md`
8. `docs/phase3_next_lifemodel_limited_slice/05_summary.md`
9. This preparation package.

If any older plan, migration, beta, stage, maturity, or roadmap document
conflicts with this stack, keep the older document as historical context only.

## Current Verified Baseline

- Phase7 remains `red-until-trial-green`.
- Today V2 Preview Surface is a preview-only slice over current primitives.
- LifeModel limited slice is frontend-only and explicitly has no backend-owned
  `LifeModelViewModel`, no endpoint, no projection, no store, and no Tauri
  command.
- Phase2 classified `WorkspaceViewModel`, `ReviewCenterViewModel`,
  `TasksViewModel`, and `MemoryViewModel` as `NOT_READY`.
- Phase2 classified `LifeModelViewModel`, `TodayViewModel`, and
  `SettingsViewModel` as `READY_WITH_LIMITS`, not ready for full product
  replacement.
- Backend primitives exist, but product read models are incomplete:
  `LifeStateProjection`, partial `ReviewWorkflow`, `ProposalStore`,
  `MemoryGateway`, `MemoryLifecycleStore`, `LifeModelWriteGateway`,
  Main Chat task/session/event primitives, provider/privacy primitives, and
  danger preflight.
- Frontend product pages still contain raw reconstruction paths. The repair
  must reduce those paths to display-only formatting or debug-only surfaces.

## Non-Goals

- Do not implement full Frontend V2.
- Do not replace `ProductShell`, primary navigation, or route IA in this phase.
- Do not turn Today/LifeModel preview adapters into official product owners.
- Do not add a new frontend state manager to compensate for missing backend
  authority.
- Do not restore any deleted Phase7 legacy route, command, module, wrapper, or
  doc authority.
- Do not make Review Center, Workspace, Tasks, Memory, LifeModel, or Settings
  product pages look complete by hiding unknowns or incomplete materialization.
- Do not claim live provider, external write, product trial, or Phase7 readiness
  unless the active acceptance gates prove it.

## Product Architecture Target

The target architecture is one backend-owned product truth path per concern.

- Shared envelope and evidence: backend read-model contract; frontend renders
  typed data and warnings.
- Pending review count and safe mode: `LifeStateProjection` plus
  surface-specific read models; frontend uses helpers only, with no raw count
  fallback.
- Review grouping and actions: `ReviewCenterViewModel` with unified
  `ReviewItem`; frontend renders actions returned by backend.
- Durable materialization: review/materialization owners and gateways; frontend
  refreshes and renders status, never infers applied state.
- LifeModel state: backend `LifeModelViewModel`; frontend renders canonical,
  current, candidate, and materialized labels.
- Task/workspace lifecycle: backend `TasksViewModel` and `WorkspaceViewModel`;
  frontend renders lifecycle and allowed controls.
- Memory lanes and lifecycle: backend `MemoryViewModel`; frontend renders lane,
  status, and provenance summaries.
- Provider/privacy boundary: backend `ProviderPrivacyBoundarySummary`;
  frontend renders shared boundary status.
- Support/debug visibility: backend/settings support policy; frontend keeps raw
  diagnostics developer/support scoped.

## Technical Direction

Use existing Rust/Tauri boundaries rather than adding a new application layer.

- Define shared product read-model structs in a backend-owned module where they
  can be tested without React.
- Keep Tauri commands as thin wrappers that aggregate from `AppState` and
  return backend read models.
- Keep frontend `frontend/src/viewmodels/**` adapters only as transitional
  preview or display helpers. New official product surfaces should consume
  backend-owned shapes.
- Prefer computed read models from existing stores before adding durable
  read-model tables. Add storage only for evidence that cannot be reconstructed
  deterministically or where performance requires it and tests prove cache
  invalidation.
- Keep durable writes behind existing gateways:
  `ReviewWorkflow`, `MemoryGateway`, `LifeModelWriteGateway`, ToolGateway/
  tool-permission authority, safe-write/danger preflight.

Recommended module shape for implementation planning:

```text
openlife-core/src/agent/product_read_model.rs
openlife-core/src/agent/review_item.rs
src-tauri/src/read_models/review_center.rs
src-tauri/src/read_models/life_model.rs
src-tauri/src/read_models/tasks.rs
src-tauri/src/read_models/workspace.rs
src-tauri/src/read_models/memory.rs
src-tauri/src/read_models/provider_privacy.rs
```

These names are target recommendations, not evidence that the modules already
exist.

## Goal-Mode Success Definition

The Goal-mode run succeeds only if it leaves the repo with:

1. A backend-owned shared read-model envelope contract.
2. A backend-owned `ReviewItem` contract that separates decision status from
   durable materialization/apply state.
3. Backend read-model commands for the first repair surfaces, with focused
   tests.
4. Product pages or preview surfaces no longer making product-truth decisions
   where backend read models exist.
5. Static or focused guards that prevent regression to page-local truth
   reconstruction.
6. Documentation that clearly states what remains unknown, limited, or blocked.

The phase is not complete if it only adds new backend structs while the frontend
continues to infer the same truth from raw proposals, diagnostics, tasks, or
config.

## Stop Conditions

Stop and report instead of continuing if any of these occur:

- Implementing a slice would require restoring a deleted Phase7 legacy object.
- A proposed read model cannot be sourced from current stores without inventing
  truth.
- Review action eligibility cannot be proven from backend state.
- Materialization status is unavailable and would need to be inferred from
  proposal status alone.
- A product page would need to present an unknown as ready/completed.
- Tests reveal accepted proposals can overwrite newer LifeModel truth without
  `base_hash` conflict handling.
