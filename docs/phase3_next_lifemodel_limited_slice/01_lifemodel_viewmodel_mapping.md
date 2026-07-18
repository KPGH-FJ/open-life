# LifeModel Limited Slice ViewModel Mapping

Status: implemented frontend-only limited adapter.

Naming boundary: this is the LifeModel limited slice after Phase 3A-2. It is
not declared as official Phase 3B.

## Scope

Implemented:

- `frontend/src/viewmodels/lifemodel/lifeModelViewModel.ts`
- `frontend/src/viewmodels/lifemodel/lifeModelViewModelAdapter.ts`
- `frontend/src/viewmodels/lifemodel/lifeModelViewModel.fixtures.ts`
- `frontend/src/viewmodels/lifemodel/lifeModelViewModel.test.ts`

Not implemented:

- no backend `LifeModelViewModel` owner;
- no LifeModel V2 UI or preview surface;
- no replacement of `LifeModelPage`;
- no ProductShell, route, primary navigation, or IA change;
- no backend Rust or Tauri command;
- no durable writes or Review Center action execution.

Phase7 remains `red-until-trial-green`.

## Inputs

The pure adapter accepts existing frontend primitives only:

- `LifeModel | null`
- `LifeModelCurrentView | null`
- `Model4DCompletion | null`
- `LifeStateProjection | null`
- `AgentProposal[]`
- `memoryCount`
- `TierStats | null`
- optional stale/error metadata

The adapter does not call Tauri. It type-imports current primitives and returns
`ViewModelEnvelope<LifeModelViewModel>` using
`frontend/src/viewmodels/shared/viewModelEnvelope.ts`.

## Field Mapping

| Output field | Mapping | Limit |
| --- | --- | --- |
| `truthMode` | `current_compatibility` when `LifeModelCurrentView` or meaningful raw `LifeModel` exists; otherwise `unknown`. | Backend canonical/current truth mode remains `PHASE_2_REQUIRED`. |
| `canonicalSummary` | Always `null`. | Raw `LifeModel` is not canonical truth. |
| `currentViewSummary` | Uses `LifeModelCurrentView` when present, otherwise labels meaningful raw `LifeModel` as an existing compatibility primitive. | Divergence from canonical remains `unknown`. |
| `dimensionSummaries` | Formats Identity, Goals, Capabilities, and State from raw `LifeModel` with completion-derived confidence. | Provenance is explicitly `limited`; owner status is `PHASE_2_REQUIRED`. |
| `trustQualityState` | Uses stale flag, `LifeStateProjection.readiness`, empty model state, and `Model4DCompletion.overall`. | Does not claim final readiness. |
| `pendingUpdateCounts` | Counts LifeModel-shaped pending proposals as candidate/pending review and accepted proposals as approved-not-applied. | Materialization status owner remains missing. |
| `provenanceRefs` | Uses projection, LifeModel, current view, completion, proposal, and memory count/tier evidence refs. | Evidence refs are partial frontend mapping, not backend provenance ownership. |
| `candidateChanges` | Maps pending LifeModel-shaped proposals to candidate changes. | Rejected, expired, or accepted proposals are not materialized. |
| `materializedChanges` | Always empty in this slice. | Accepted proposal and current-view evidence do not prove durable materialization. |
| `manualOverrideState` | Disabled with `saveAction: null`. | Manual override/write gateway state is `PHASE_2_REQUIRED`. |
| `relatedReviewItemRefs` | Maps proposal IDs to review item refs for navigation/evidence only. | No approve/edit/reject/postpone action is exposed. |
| `memoryLinkage` | Shows memory count and tier stats as partial linkage when provided. | Memory lane/materialization linkage remains `PHASE_2_REQUIRED`; unknown when only absent inputs exist. |
| `sourceRefs` | Deduplicated evidence refs from provided primitives. | No additional raw-domain fallback. |
| `contractLimitations` | Static limitation statements carried in data for tests/docs. | Documents non-claims rather than product readiness. |

## Action Lanes

`actions.primary` includes refresh, inspect evidence, and a disabled request
update action. The update action is disabled because backend ReviewWorkflow and
LifeModelViewModel ownership are missing, or because Safe Mode/stale state
blocks risky behavior.

`actions.review` is empty. The adapter does not create local review decisions.

`actions.debugOnly` contains the raw limited-input inspection action only. Tests
verify debug-only action IDs do not appear in primary actions.

## No Raw Reconstruction

The adapter uses `LifeStateProjection` for Safe Mode and readiness flags when
available. It does not rebuild projection-covered truth from diagnostics and it
does not import `tauriDev`.
