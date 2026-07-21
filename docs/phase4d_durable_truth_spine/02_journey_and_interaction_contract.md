# Phase 4D Durable-Truth Journey And Interaction Contract

Status: `IMPLEMENTED`
Date: 2026-07-21

## Information Priority

The desktop LifeModel surface uses this fixed order:

1. current source-backed understanding;
2. one selected pending or recent durable change;
3. the exact decision/materialization blocker or result;
4. the next available action;
5. supporting Memory summary and lanes;
6. evidence and technical fields in Inspector.

The page does not repeat the same state as top metadata, banner, metric, list,
and Inspector conclusion. The context bar carries the single short status; the
work surface explains the selected change once.

## Interaction Contract

| User action | Result | Forbidden interpretation |
| --- | --- | --- |
| open LifeModel | load LifeModel, Memory, and Review Center | does not create truth |
| select a change | changes local detail context | does not decide or apply |
| view suggestion | navigates to the exact ReviewItem | does not approve |
| approve/reject/later | uses existing typed ReviewAction and refreshes | command callback is not completion |
| return to LifeModel | reloads all durable read models | does not reuse optimistic state |
| refresh | reloads all three owners | does not retain stale success |
| open evidence | opens structured Inspector metadata | does not reveal or mutate source content |

## Actions

- Product navigation and refresh are explicit UI commands with stable IDs and
  exact targets.
- Review decisions remain `data-action-category=review` and preserve backend
  id, kind, effect, enabled, disabledReason, target, confirmation, expected
  materialization, and `completionProofAfterDispatch=false`.
- Apply is displayed only when present in `ReviewItem.allowedActions`; without a
  callable typed command it remains disabled with a visible reason.
- Debug information remains in the Inspector technical-details disclosure.

## Accessibility And Desktop Scope

- LifeModel navigation uses the Shell's `aria-current="page"` behavior.
- navigation and refreshed status changes use the existing polite live region.
- all controls use native buttons and the Foundation focus treatment.
- disabled controls always expose a visible disabled reason.
- desktop acceptance viewports are `1440x900`, `1280x800`, and `1024x720`.
- mobile is intentionally outside this product and acceptance scope.
