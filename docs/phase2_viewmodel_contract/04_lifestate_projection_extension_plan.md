# LifeStateProjection Extension Plan

Status: contract analysis. No projection implementation.

## Current State

`EXISTING_CODE`: `LifeStateProjection` is a backend read model for shared product state.

Current fields:

| Field | Owner status | Current role |
| --- | --- | --- |
| `version` | `EXISTING` | Projection schema version. |
| `generatedAt` | `EXISTING` | Read-model generation timestamp. |
| `pending` | `EXISTING` | Pending/edited/high-risk review counts and proposal store status. |
| `readiness` | `EXISTING` | Chat/usage/LifeModel readiness, model-empty state, builder session counts, database status, readiness issues. |
| `taskState` | `EXISTING` | Latest task id/status and task status counts. |
| `safeMode` | `EXISTING` | Safe mode active/reason/source refs. |
| `toolPermissions` | `EXISTING` | Tool permission counts by status/policy. |
| `safePaths` | `EXISTING` | Configured safe paths. |
| `surfaces` | `EXISTING/PARTIAL` | Uniform per-surface summary rows. |
| `sourceRefs` | `EXISTING` | Source refs such as diagnostics, proposal store, task store, permission store, config. |

## Current Limitations

`EXISTING_CODE`: `build_surface_projection` currently emits the same pending/readiness/task/safe-mode/tool-permission values for each surface id. The rows identify surfaces but are not rich surface-specific ViewModels.

`VERIFIED_FACT`: Phase 0.5 identified the bigger gap: Today/Chat/Mailbox/Runs/LifeModel/Memory/Settings still combine projection data with raw domain reads and page-local interpretation.

`DESIGN_DECISION`: `LifeStateProjection` should not become a monolithic page ViewModel that absorbs every surface-specific field.

## Options

| Option | Description | Benefits | Risks | Status |
| --- | --- | --- | --- | --- |
| Expand `LifeStateProjection` only | Add all surface fields to the existing projection. | One read-model command and consistent global state. | Bloated global projection; page-specific lifecycle and review contracts become coupled. | `CANDIDATE`, not recommended as sole approach. |
| Dedicated read models only | Create separate `TodayViewModel`, `WorkspaceViewModel`, etc. with little shared projection. | Clear surface ownership. | Duplicates pending/readiness/safe-mode/task counts and can reintroduce inconsistency. | `CANDIDATE`, not recommended as sole approach. |
| Hybrid | Keep `LifeStateProjection` for global/shared truth; add dedicated ViewModels for rich surface contracts. | Preserves shared authority and avoids page-local reconstruction. | Requires clear boundaries and tests for duplication. | `DESIGN_DECISION` recommendation. |

## Recommended Hybrid Approach

`DESIGN_DECISION`: Use a hybrid approach.

Global/shared state that should remain in `LifeStateProjection`:

- pending review totals;
- edited/high-risk review totals;
- setup/readiness summary;
- safe mode;
- task pressure counts;
- active tool permission counts;
- safe paths;
- global source refs;
- surface-level lightweight status rows.

Surface-specific fields that should move to dedicated ViewModels:

- Workspace intent, timeline, blockers, final result, current controls, review refs.
- Review Center grouped items, allowed actions, expiration, materialization status.
- Tasks merged AgentRun/task lifecycle, stale state, deletion eligibility.
- LifeModel canonical/current/compatibility distinction, dimension provenance, materialized/candidate changes.
- Memory lane counts/status/provenance, candidate/confirmed/used/withdrawn states.
- Today daily goal classification, suggestions, blockers, next daily action.
- Settings provider/privacy trust summary, support/debug visibility, data-control readiness.

## Migration Risks

| Risk | Classification | Mitigation |
| --- | --- | --- |
| Duplicate pending/readiness state | `PHASE_2_REQUIRED` | Dedicated ViewModels reference projection-derived fields or backend shared source, not recompute from raw proposals/diagnostics. |
| Page-local fallbacks survive | `PHASE_2_REQUIRED` | Add static/frontend tests banning product truth reconstruction from raw reads. |
| `LifeStateProjection` becomes too broad | `DESIGN_ASSUMPTION` | Keep it for global status and compact surface summaries only. |
| Dedicated ViewModels hide blockers | `PHASE_2_REQUIRED` | Shared envelope must expose `warnings`, `status`, and evidence refs; blockers remain default-visible. |
| Review approval overclaims durable apply | `PHASE_2_REQUIRED` | ReviewItem must include separate `materializationStatus`. |

## Required Validation Before Phase 3

`PHASE_2_REQUIRED`: Engineering must approve:

1. The exact field split between `LifeStateProjection` and each dedicated ViewModel.
2. Whether dedicated ViewModels are exposed through new commands, existing command composition, or an internal read-model layer.
3. Static guards that product pages do not rebuild covered truth from diagnostics/proposals/config fragments.
4. Contract tests that `LifeStateProjection` remains the source for global pending/readiness/safe-mode/task pressure.
