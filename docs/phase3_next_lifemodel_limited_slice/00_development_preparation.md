# LifeModel Limited Slice Development Preparation

Status: preparation for the next candidate slice after Phase 3A-2.

Important naming boundary: this document does not declare an official
`Phase 3B`. The source-backed next candidates after Phase 3A-2 are LifeModel
limited slice or Settings limited slice. This preparation recommends the
LifeModel limited slice because the handoff document says it best validates
OpenLife's differentiated product value, while `LifeModelViewModel` is
classified `READY_WITH_LIMITS`.

## 1. Read First

Read in this order before implementation:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`
5. User-provided handoff context, if present.
6. `docs/phase3a2_today_preview/04_phase3a2_summary.md`
7. `docs/phase2_viewmodel_contract/14_phase2_summary_and_phase3_readiness.md`
8. `docs/phase2_viewmodel_contract/08_lifemodel_viewmodel_contract.md`
9. `docs/phase2_viewmodel_contract/12_backend_contract_gap_register.md`
10. `docs/phase2_viewmodel_contract/13_contract_test_plan.md`
11. Current source files listed in this document's source map.

If a historical roadmap conflicts with the Phase7 or ViewModel contract
boundary, follow the active authority stack above.

## 2. Recommended Next Slice

Recommended candidate:

```text
LifeModel limited slice
```

Recommended implementation shape:

```text
Create a frontend-only LifeModelViewModel limited adapter, fixtures, and
contract tests. Do not replace the current LifeModelPage and do not create a
LifeModel V2 page unless a later task explicitly asks for a preview surface.
```

Rationale:

- `LifeModelViewModel` is `READY_WITH_LIMITS`.
- LifeModel is core to OpenLife's product differentiation.
- Existing primitives can support a limited contract layer if the UI labels
  canonical/current/compatibility limitations clearly.
- Full LifeModel V2 remains blocked until a backend-owned read model provides
  canonical truth mode, provenance, materialization state, and Memory linkage.

Settings remains a valid alternative candidate if the human priority changes to
provider/privacy/tool/data visibility with lower product-differentiation risk.

## 3. Current Source Map

Current product page:

- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/pages/LifeModelPage.test.tsx`

Current helpers:

- `frontend/src/utils/lifeModelTrust.ts`
- `frontend/src/utils/lifeModelQuality.ts`
- `frontend/src/utils/lifeStateProjection.ts`
- `frontend/src/utils/reviewPendingCount.ts`

Existing bridge primitives currently consumed by the page:

- `getLifeModel()`
- `getLifeModelCurrentView()`
- `getModel4DCompletion()`
- `getLifeStateProjection()`
- `getSystemDiagnostics()`
- `builderListUnfinished()`
- `countMemoryChunks()`
- `getMemoryTierStats()`
- `listProposals(...)`

Important observation:

`LifeModelPage` currently assembles product state page-locally from model,
current view, diagnostics, projection, completion, builder sessions, memory
stats, and proposal lists. The limited slice should move that interpretation
into a pure ViewModel adapter, while preserving unknowns instead of pretending a
backend `LifeModelViewModel` owner exists.

## 4. Proposed Files

If approved, implement only the minimal contract package:

```text
frontend/src/viewmodels/lifemodel/lifeModelViewModel.ts
frontend/src/viewmodels/lifemodel/lifeModelViewModelAdapter.ts
frontend/src/viewmodels/lifemodel/lifeModelViewModel.fixtures.ts
frontend/src/viewmodels/lifemodel/lifeModelViewModel.test.ts
docs/phase3_next_lifemodel_limited_slice/01_lifemodel_viewmodel_mapping.md
docs/phase3_next_lifemodel_limited_slice/02_files_changed.md
docs/phase3_next_lifemodel_limited_slice/03_test_report.md
docs/phase3_next_lifemodel_limited_slice/04_self_review_and_hallucination_check.md
docs/phase3_next_lifemodel_limited_slice/05_summary.md
```

Reuse `frontend/src/viewmodels/shared/viewModelEnvelope.ts`. Do not add another
shared envelope type.

## 5. Limited Adapter Inputs

The adapter should be pure and perform no Tauri invocation.

Recommended input object:

```ts
type BuildLifeModelViewModelInput = {
  lifeModel: LifeModel | null
  currentView: LifeModelCurrentView | null
  completion: Model4DCompletion | null
  projection: LifeStateProjection | null
  pendingProposals: AgentProposal[]
  memoryCount: number | null
  tierStats: TierStats | null
  now?: string
  stale?: boolean
  error?: string | null
}
```

`getSystemDiagnostics()` may remain a current-page input for legacy display, but
the new limited adapter should prefer `LifeStateProjection` for readiness,
safe mode, model-empty, and review-pending state whenever projection fields
exist. Do not rebuild projection-covered truth from diagnostics.

## 6. Limited Output Contract

Follow `docs/phase2_viewmodel_contract/08_lifemodel_viewmodel_contract.md`, but
mark unavailable fields as limited or unknown.

Minimum `LifeModelViewModel` data output inside `ViewModelEnvelope.data`:

- `truthMode`
- `canonicalSummary`
- `currentViewSummary`
- `dimensionSummaries`
- `trustQualityState`
- `pendingUpdateCounts`
- `provenanceRefs`
- `candidateChanges`
- `materializedChanges`
- `manualOverrideState`
- `relatedReviewItemRefs`
- `memoryLinkage`
- `sourceRefs`

Envelope-level action and metadata lanes:

- `status`
- `lastUpdatedAt`
- `evidenceRefs`
- `actions.primary`
- `actions.review`
- `actions.debugOnly`
- `warnings`

The adapter should return `ViewModelEnvelope<LifeModelViewModel>`. Do not put
action lanes inside the `LifeModelViewModel` data object, and do not put
LifeModel data fields directly on the envelope root. `debugRawControls` from
the Phase 2 target contract maps to `actions.debugOnly` in the shared envelope
for this limited frontend slice.

Conservative mapping rules:

| Field | Limited behavior |
| --- | --- |
| `truthMode` | Use a labeled limited/current compatibility state when rendering existing `LifeModel` or `LifeModelCurrentView`; include a warning that backend truth mode is missing. |
| `canonicalSummary` | `null` unless a backend-owned canonical summary is explicitly available. Do not call raw `LifeModel` canonical truth. |
| `currentViewSummary` | May summarize `getLifeModelCurrentView()` as current/compatibility view with evidence gaps preserved. |
| `dimensionSummaries` | May format Identity, Goals, Capabilities, and State from the current `LifeModel`; label confidence/provenance as limited. |
| `trustQualityState` | May use `getModel4DCompletion()` and projection model-empty/readiness state, but must not declare final readiness. |
| `pendingUpdateCounts` | May count passed pending LifeModel proposals as partial evidence; do not claim Review Center materialization state. |
| `candidateChanges` | May map pending LifeModel proposals to candidate changes. |
| `materializedChanges` | Empty or unknown unless explicit materialization evidence exists. Accepted proposal is not automatically applied. |
| `manualOverrideState` | Disabled or unknown. Do not expose direct save/apply as a product action. |
| `memoryLinkage` | May show memory count/tier summary as partial linkage; do not claim MemoryViewModel lane ownership. |
| `debugRawControls` | Debug-only lane only; never primary product action. |

## 7. Hard Non-goals

Do not:

- replace `LifeModelPage`;
- create a `LifeModelV2Page` preview unless a later task explicitly asks for it;
- change `ProductShell`, primary navigation, route aliases, or IA;
- implement Workspace, Review Center, Tasks, Memory, or Settings;
- add top-level `记忆`;
- modify backend Rust or add Tauri commands;
- invent backend `LifeModelViewModel` owner, endpoint, projection, store, or
  materialization status;
- call `saveLifeModel`, `acceptProposal`, `batchAcceptLowRiskProposals`,
  `editProposal`, `rejectProposal`, or other write/apply wrappers from the
  ViewModel adapter;
- treat pending proposal decision status as durable LifeModel materialization;
- import `frontend/src/tauriDev.ts` in product code.

## 8. Expected Tests

Add focused tests for:

1. Empty LifeModel returns an `empty` or limited envelope without fake canonical
   summary.
2. Current compatibility view renders as current/compatibility, not canonical.
3. Dimension summaries preserve source refs and limited confidence labels.
4. Pending LifeModel proposals become candidate changes or pending counts only,
   not materialized changes.
5. Accepted proposal/current-view evidence does not become `applied` unless the
   input explicitly proves materialization.
6. Safe Mode or stale envelope disables risky product/review actions.
7. Error envelope does not fall back to raw LifeModel data.
8. Memory linkage remains partial/unknown when only memory count or tier stats
   exist.
9. `actions.debugOnly` never appears in primary actions.
10. Static source scan prevents direct write wrappers and `tauriDev` imports in
    the new ViewModel files.

The test file should use fixtures rather than Tauri mocks because the adapter
must be pure.

## 9. Suggested Static Guards

Use a test or script scan over the new `frontend/src/viewmodels/lifemodel/**`
files for these forbidden symbols:

```text
getSystemDiagnostics
saveLifeModel
acceptProposal
batchAcceptLowRiskProposals
editProposal
rejectProposal
postponeProposal
tauriDev
safeInvoke
invoke(
```

`getSystemDiagnostics` can remain in existing `LifeModelPage` until a later page
replacement/refactor is approved; it should not enter the pure ViewModel
adapter.

## 10. Documentation To Generate

After implementation, add:

- field-by-field mapping;
- files changed;
- test report;
- self-review and hallucination check;
- summary and recommendation for whether to add a LifeModel preview surface next
  or stop for backend read-model work.

Each doc should explicitly state:

- this is a limited slice;
- no backend owner was created;
- no LifeModel V2 UI was implemented;
- no durable writes or Review Center actions were performed;
- Phase7 remains `red-until-trial-green`.

## 11. Suggested Gates

Run at minimum:

```sh
git diff --check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend test -- lifeModelViewModel
corepack pnpm --dir frontend test -- LifeModelPage
```

If `App.tsx`, route wiring, `ProductShell`, or existing LifeModel page behavior
is touched, stop and reassess scope before proceeding.

## 12. Agent Prompt

Use this instruction for the next implementation Agent:

```text
Implement the next candidate OpenLife frontend modernization slice:
LifeModel limited slice.

First read AGENTS.md, plans/README.md,
plans/openlife_single_system_deletion_manifest.md,
plans/openlife_single_system_development_preparation.md,
docs/phase3a2_today_preview/04_phase3a2_summary.md,
docs/phase2_viewmodel_contract/08_lifemodel_viewmodel_contract.md,
docs/phase2_viewmodel_contract/12_backend_contract_gap_register.md,
docs/phase2_viewmodel_contract/13_contract_test_plan.md, and
docs/phase3_next_lifemodel_limited_slice/00_development_preparation.md.

Implement only a frontend-only LifeModelViewModel limited adapter, fixtures, and
contract tests under frontend/src/viewmodels/lifemodel/. Reuse the existing
shared ViewModelEnvelope. The adapter must be pure and must not call Tauri.

Do not replace LifeModelPage, do not change ProductShell or routes, do not
implement LifeModel V2 UI, do not add backend Rust/Tauri commands, and do not
invent a backend LifeModelViewModel owner. Preserve missing canonical/current,
materialization, provenance, and Memory linkage fields as limited/unknown.

Run the suggested gates and generate the required docs under
docs/phase3_next_lifemodel_limited_slice/.
```
