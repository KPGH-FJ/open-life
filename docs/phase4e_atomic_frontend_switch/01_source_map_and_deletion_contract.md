# Phase 4E Source Map And Deletion Contract

Date: 2026-07-21

## Production Composition

```text
frontend/src/main.tsx
  -> HashRouter
  -> frontend/src/App.tsx
  -> frontend/src/ui/productRouteContract.ts
  -> frontend/src/ui/journeys/readOnly/ReadOnlySpineJourney.tsx
  -> frontend/src/ui/shell/OpenLifeWorkbenchShell.tsx
```

The Shell is one production owner. It composes the route-selected work surface,
desktop primary navigation, utility/settings access, context bar, evidence
inspector, and visually subordinate debug detail.

## Product Data Owners

| Product area | Frontend owner | Backend truth consumed |
| --- | --- | --- |
| Today | `ui/journeys/readOnly/readOnlySpineDataSource.ts` | strict Today adapter over LifeState projection, daily goals, and privacy boundary |
| Workspace | `ui/journeys/governedAction/governedActionDataSource.ts` | `WorkspaceViewModel`, exact blocker/action identity, refreshed Tasks and Review models |
| Conversation | `ui/journeys/governedAction/workspaceConversationDataSource.ts` | governed Main Chat session/history/send commands; history and read models are re-read after send |
| Tasks | `ui/journeys/readOnly/TasksReadOnlyView.tsx` and `taskControlContract.ts` | `TasksViewModel` plus exact typed resume/retry/cancel/refresh commands |
| Review | `ui/journeys/governedAction/ReviewGovernedView.tsx` | rich `ReviewItem`, allowed actions, evidence, target refs, and refreshed decision state |
| LifeModel/Memory | `ui/journeys/durableTruth/**` | LifeModel, Memory, and exact Review lifecycle projections |
| First build | `ui/journeys/durableTruth/lifeModelBuilderDataSource.ts` | Builder candidate decisions and proposal creation only |
| Settings/privacy | `ui/journeys/settingsPrivacy/**` | sanitized config, provider test receipt, exact ReviewItem, and `ProviderPrivacyBoundarySummary` |

## Canonical Desktop Routes

| Path | Owner |
| --- | --- |
| `/today` | Today current focus |
| `/workspace` | current execution and governed conversation |
| `/tasks` | queue and continuity |
| `/review` | user decisions |
| `/life-model` | durable truth and first-build proposal flow |
| `/settings` | provider, privacy, and configuration utility |

Only `/` redirects, and only to `/today`. Retired paths such as `/companion`,
`/mailbox`, `/runs`, `/runs/*`, `/memory`, and `/builder` show an explicit
unavailable state. They do not redirect into a different product action.

## Deleted Production Owners

The atomic switch deletes:

- `frontend/src/pages/**`;
- `frontend/src/components/**`, including `ProductShell.tsx` and old product
  primitives;
- `frontend/src/productShellContract.ts`;
- page-local/local-reconstruction utilities for runtime disclosure, run
  summaries, capability/provider readiness, proposal display/review decision,
  and related inferred product truth.

The complete migration state is recorded in
`docs/phase4a_contract_closure/02_migration_deletion_ledger.md`. All migrated
rows are `guarded_absent`.

## Executable Absence Proof

- `frontend/scripts/verify-production-absence.mjs` requires the old page and
  component directories to be absent, requires the Workbench production marker,
  and rejects Phase 4 harness/fixture/preview markers in the release bundle.
- route tests require canonical routes and explicit retired-route behavior.
- `plans/openlife_single_system_phase1_inventory.json` records retired owners as
  expected absent and current owners as product-valid.
- Rust `single_system` tests guard the shipped frontend owner map and the
  absence of the deleted authority graph.

`OLD_FRONTEND_AUTHORITY_GUARDED_ABSENT=YES`

`PRODUCTION_ROUTE_AUTHORITY_SINGLE=YES`
