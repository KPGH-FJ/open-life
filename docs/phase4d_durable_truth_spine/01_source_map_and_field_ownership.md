# Phase 4D Durable-Truth Source Map And Field Ownership

Status: `IMPLEMENTED`
Date: 2026-07-21

## Runtime Source Map

```text
dev-only Phase 4D Shell
  -> DurableTruthDataSource.loadDurableTruth
      -> get_life_model_view_model
      -> get_memory_view_model
      -> get_review_center_view_model
  -> durableTruthPresentation
      -> exact ReviewItem identity
      -> exact proposal:<proposalId> LifeModel change identity
      -> fail-closed lifecycle reconciliation
  -> DurableTruthView

decision path
  -> existing ReviewGovernedView
  -> typed ReviewAction approve/reject/later
  -> command callback (not completion proof)
  -> refresh Review Center
  -> return to LifeModel
  -> refresh all durable-truth read models
```

## Field Ownership

| Visible fact | Backend owner and field | Rendering rule |
| --- | --- | --- |
| current long-term view | `LifeModelViewModel.currentViewSummary` | prefer this compatibility projection; never invent canonical truth |
| canonical summary | `canonicalSummary` | render only when non-null; null remains unavailable |
| dimension summaries | `dimensionSummaries[]` | preserve confidence, stale, provenance, and pending refs |
| trust limitation | `trustQualityState`, `contractLimitations[]` | limited/stale/unknown never becomes verified readiness |
| candidate change | `candidateChanges[]` plus exact `ReviewItem` | use the exact review/proposal refs; no title-based join |
| decision | `ReviewItem.status` | pending, deferred, approved, and rejected stay distinct |
| materialization | `ReviewItem.materializationStatus` | approved is not applied; applying/failed/rolled-back remain explicit |
| LifeModel applied proof | `materializedChanges[].changeRef.id` | must equal `proposal:<ReviewItem.source.proposalId>` and report `applied` |
| Memory aggregate | `MemoryViewModel.summary` and `lifecycleSummary` | supporting context only; not proof for a specific item |
| Memory lanes | `laneSummaries[]` | show backend counts and refs without reconstructing memory records |
| evidence | all relevant `EvidenceRef` values | preserve id, label, source, sensitivity in Inspector |
| limitations | all three envelopes and model limitations | evidence and limits remain visible; stale/missing sources fail closed |

## Exact Lifecycle Reconciliation

The page derives one presentation state from exact backend fields:

1. any required stale, error, loading, missing, or incoherent owner -> `unknown`;
2. pending/edited -> `pending_review`;
3. deferred -> `deferred`;
4. approved plus `not_started`/`not_applicable` -> `approved_not_applied`;
5. approved plus `applying` -> `applying`;
6. approved plus `failed` -> `failed`;
7. approved plus `rolled_back` -> `rolled_back`;
8. LifeModel `applied` requires matching ReviewItem and matching
   `materializedChanges.changeRef`; Memory `applied` requires the exact refreshed
   ReviewItem plus a ready Memory envelope;
9. rejected -> `rejected`.

Only step 8 may use the green verified treatment.

## Contract Gap Kept Visible

`MemoryViewModel` exposes aggregate lifecycle and lane summaries, but no exact
per-record typed rollback action. The current generic Review apply action is
disabled when no backend materialization request command exists. Therefore:

- this slice does not call raw memory asset APIs;
- this slice does not synthesize Apply or rollback from store data;
- disabled backend Apply may be shown with its backend reason;
- rollback remains read-only until the backend projects an exact action and
  target into the product read model.

`TYPED_DURABLE_APPLY_ACTION_READY=NO`

`TYPED_MEMORY_ROLLBACK_ACTION_READY=NO`
