# Phase 2 Summary And Phase 3 Readiness

Status: Phase 2 contract package summary.

## Contracts Defined

`DESIGN_DECISION`: This package defines contracts for:

- shared `ViewModelEnvelope<T>` and action/evidence/review types;
- hybrid `LifeStateProjection` plus dedicated ViewModels approach;
- `WorkspaceViewModel`;
- `ReviewCenterViewModel`;
- `TasksViewModel`;
- `LifeModelViewModel`;
- `MemoryViewModel`;
- `TodayViewModel`;
- `SettingsViewModel`;
- backend gap register;
- contract test plan.

## Existing Owners

`EXISTING_CODE`: Existing backend/read-model or bridge owners:

- `LifeStateProjection` for shared pending/readiness/task/safe-mode/tool-permission/safe-path state.
- `frontend/src/tauri.ts` as typed product bridge, not ViewModel owner.
- Main Chat task/session/snapshot/event/final-delivery primitives.
- AgentRun history and evidence primitives.
- Proposal store/actions and partial ReviewWorkflow.
- MemoryGateway/lifecycle primitives.
- LifeModel/current/provenance primitives.
- Privacy, safe-path, danger preflight, tool permission, and audit primitives.

## Proposed Owners

`PHASE_2_REQUIRED`: Proposed owners needing approval or implementation:

- `WorkspaceViewModel`
- `ReviewCenterViewModel`
- `TasksViewModel`
- `LifeModelViewModel`
- `MemoryViewModel`
- `TodayViewModel` extension or dedicated read model
- `SettingsViewModel`
- ReviewItem materialization/apply state owner
- Provider/privacy trust summary owner
- Support/debug visibility policy owner

## ViewModel Readiness Classification

| ViewModel | Classification | Rationale |
| --- | --- | --- |
| `WorkspaceViewModel` | `NOT_READY` | Existing primitives are rich, but a consolidated backend owner, timeline model, allowed controls, review refs, and provider/privacy boundary are missing. |
| `ReviewCenterViewModel` | `NOT_READY` | Proposal review exists, but unified ReviewItem, backend allowed actions, expiration, non-proposal item types, and materialization state are missing. |
| `TasksViewModel` | `NOT_READY` | AgentRun and Main Chat task summaries exist, but canonical merged identity/lifecycle remains page-local and unresolved. |
| `LifeModelViewModel` | `READY_WITH_LIMITS` | Existing LifeModel/current/completion/proposal primitives can support a limited labeled view, but full canonical/current/provenance/materialization contract is required. |
| `MemoryViewModel` | `NOT_READY` | Memory primitives exist, but lane/status/provenance read model and top-level Memory approval are missing. |
| `TodayViewModel` | `READY_WITH_LIMITS` | Projection plus daily goals can support a limited Today surface if card classification/next action are not treated as product truth. |
| `SettingsViewModel` | `READY_WITH_LIMITS` | Existing config/diagnostics/projection/privacy/tool primitives can support limited settings, but provider/privacy summary and support/debug policy are required for full V2. |

## Major Blockers

1. `PHASE_2_REQUIRED`: Unified ReviewItem model must separate decision status from durable materialization/apply state.
2. `PHASE_2_REQUIRED`: Workspace, Tasks, Review Center, Memory, LifeModel, and Settings need backend-owned read-model fields before full V2 implementation.
3. `PHASE_2_REQUIRED`: Human approval is needed for hybrid projection strategy, Memory top-level readiness, diagnostics visibility, provider/privacy summary, and first vertical slice.

## Memory Top-level Readiness

`NOT_READY`: Top-level `记忆` should not be implemented until lane counts/status/provenance, review refs, lifecycle, and Memory/LifeModel linkage are backend-owned or explicitly scoped.

`CANDIDATE`: Memory remains product-critical. If top-level Memory is deferred, preserve it as LifeModel sub-surface, Settings/Data Management sub-surface, and Workspace evidence preview.

## Review Center Readiness

`NOT_READY`: Current proposal review can inform design, but V2 Review Center needs backend-owned ReviewItem type/status/materialization/action fields before implementation.

## Workspace Readiness

`NOT_READY`: Current ChatPage should not be converted into V2 Workspace without a backend-owned WorkspaceViewModel. Doing so would preserve the current page-local state debt.

## Phase 3 Go / No-go

`DESIGN_DECISION`: Phase 3 should be `NO_GO` for full Frontend V2 implementation.

`CANDIDATE`: Phase 3 may proceed only as a narrow first vertical slice after human approval. Recommended candidate slices:

- `TodayViewModel` limited slice using existing `LifeStateProjection` plus explicit unknowns.
- `LifeModelViewModel` limited slice that labels current/compatibility limitations.
- `SettingsViewModel` limited slice for setup/tool/data controls with provider/privacy gaps visible.

`DESIGN_DECISION`: Do not implement `NOT_READY` surfaces in Phase 3.

## Required Human Approvals

1. Whether `记忆` remains top-level or moves under LifeModel / Settings.
2. ReviewItem materialization model.
3. WorkspaceViewModel contract.
4. ReviewCenterViewModel contract.
5. Hybrid projection strategy: expanded `LifeStateProjection` plus dedicated read models.
6. Diagnostics visibility and support/developer mode.
7. Provider/privacy trust summary.
8. First vertical slice scope.

## Next Recommended Codex Goal

`CANDIDATE`: Recommended next goal:

```text
OpenLife Phase 3: approve and implement one backend-owned ViewModel vertical slice, starting with the smallest READY_WITH_LIMITS surface, with contract tests and no raw-domain product truth reconstruction.
```

## Final Statement

No production source code was intentionally modified. Frontend V2 implementation was not started.
