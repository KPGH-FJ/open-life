# Phase 4 Frontend Migration And Deletion Ledger

Status: `PHASE4E_ATOMIC_SWITCH_EXECUTED`
Date: 2026-07-21

State vocabulary:

- `identified`: replacement and deletion boundary are mapped.
- `contract_ready`: replacement contract exists, but the old product caller is
  still active.
- `delete_ready`: all callers have moved and the old owner can be removed in
  the atomic switch.
- `guarded_absent`: deletion ran and an executable guard proves absence.

## Ledger

| New owner | Old owner | Last product caller | Deletion condition | Absence guard | State |
| --- | --- | --- | --- | --- | --- |
| `frontend/src/ui/shell/OpenLifeWorkbenchShell.tsx`, composed by `ReadOnlySpineJourney.tsx` | `frontend/src/components/ProductShell.tsx` and `frontend/src/productShellContract.ts` | `frontend/src/App.tsx`, moved in Phase 4E | Production App composes the Workbench and canonical route contract; no old shell fallback remains | Directory/file absence, App owner assertions, production bundle Workbench assertion, Rust `single_system` guard | `guarded_absent` |
| `frontend/src/ui/foundation/**` semantic tokens and primitives | `frontend/src/components/product/ProductPrimitives.tsx` and page-local component tree | Old pages/components, all deleted in Phase 4E | Every production journey imports the semantic Foundation; the old component directory is absent | Production source scan, directory absence, typecheck, release build | `guarded_absent` |
| `frontend/src/ui/journeys/readOnly/TodayReadOnlyView.tsx` over `readOnlySpineDataSource.ts` and strict `openlife.today-adapter.v1` | `frontend/src/pages/TodayPage.tsx` local layout/composition | `/today` production route, moved in Phase 4E | `/today` resolves to the Workbench journey; Today consumes projection/daily goals/boundary without raw fallback | Route tests, raw-read inventory match, old page/directory absence, release guard | `guarded_absent` |
| Dev-only Phase 4B harness | deleted `TodayV2PreviewPage` and deleted `/today-v2-preview` production route | no product caller remains | Completed in Phase 4B; independent dev entry owns layout fixtures | Frontend build scan, App route test, inventory contract, and Rust single-system guard prove absence | `guarded_absent` |
| `WorkspaceGovernedView.tsx`, `WorkspaceConversationPanel.tsx`, and governed data sources | `CompanionPage`/`ChatPage` joins and conversation subcomponents | `/companion` and old Chat owner, removed in Phase 4E | `/workspace` owns execution and conversation; send delegates to governed Main Chat, then refreshes exact read models | Old page/component directory absence, command/source tests, production route test | `guarded_absent` |
| `TasksReadOnlyView.tsx`, `taskControlContract.ts`, and governed refresh | `RunsPage.tsx`, `AgentRunDetail.tsx`, and local task/run joins | `/runs` and `/runs/*`, retired in Phase 4E | `/tasks` consumes TasksViewModel; control dispatch requires exact identity, confirmation where required, refresh, and no callback completion proof | UI control integration test, reducer tests, old owner absence, retired route test | `guarded_absent` |
| `ReviewGovernedView.tsx` over rich `ReviewItem` and `reviewDispatchReducer` | `MailboxPage.tsx`, raw proposal display helpers, and Chat permission decision UI | `/mailbox`, retired in Phase 4E | `/review` renders backend decision context and allowed actions; decisions refresh the same item and never resume/apply automatically | Review journey tests, raw proposal reconstruction scan, old owner absence | `guarded_absent` |
| `frontend/src/ui/journeys/durableTruth/**` over LifeModelViewModel, MemoryViewModel, and exact ReviewItem lifecycle | `LifeModelPage.tsx`, `MemorySearch.tsx`, LifeModel editor, and local trust/quality helpers | `/life-model` and `/memory`, with `/memory` retired | `/life-model` distinguishes pending, approved-not-applied, applying, applied, failed, rolled-back, rejected, and unknown from refreshed backend proof | Durable presentation/journey tests, raw-read inventory, old owner absence | `guarded_absent` |
| `LifeModelBuilderPanel.tsx` and `lifeModelBuilderDataSource.ts` | `BuilderPage.tsx` and direct builder page composition | `/builder`, retired in Phase 4E | First build is embedded only when LifeModel is not built; it creates review proposals and never applies truth directly | Builder reducer/journey tests, exact ReviewItem fixture mapping, old page absence | `guarded_absent` |
| `frontend/src/ui/journeys/settingsPrivacy/**` over sanitized AppConfig, ProviderPrivacyBoundarySummary, `LifeStateProjection.safeMode`, provider receipts, exact ReviewItem, and the existing governed credential-recovery command | `SettingsPage.tsx`, settings tabs, and local provider readiness/recovery UI helpers | `/settings`, moved in Phase 4E and repaired in Phase 4F | `/settings` owns draft/test/review/save/refresh and the user-initiated Safe Mode recovery entry; unknown boundary or Safe Mode source never becomes local/green, and recovery return never clears Safe Mode | Settings data-source/hook/view tests, provider boundary assertions, old page/component/helper absence | `guarded_absent` |
| Settings utility context and explicit unavailable routes | ProductShell advanced groups and same-level advanced pages | Old shell secondary navigation, removed in Phase 4E | Settings stays outside primary product navigation; retired routes show unavailable state; no Advanced product route is synthesized | Route matrix tests and release bundle guard | `guarded_absent` |
| `frontend/src/ui/productRouteContract.ts` | `LEGACY_PRODUCT_REDIRECTS` and old route aliases | Old `App.tsx` route map, replaced in Phase 4E | Canonical routes are `/today`, `/workspace`, `/tasks`, `/review`, `/life-model`, `/settings`; root alone redirects to Today | Route contract tests, App tests, release guard, Rust authority guard | `guarded_absent` |

## Per-Slice Rule

Every Phase 4D journey must update this ledger in the same change:

1. list the new component owner;
2. identify every remaining old caller;
3. move the row to `delete_ready` only when no production caller remains;
4. add the exact absence guard before 4E;
5. perform deletion in the same 4E atomic authority switch.

The ledger does not authorize a long-lived `/v2` route or production fallback.

## Phase 4E Atomic Switch Update

Phase 4E moved the accepted desktop Workbench into production in one change:

- `App.tsx` now composes one route-driven `ReadOnlySpineJourney` with real Tauri
  data sources for Today, Workspace, Tasks, Review, LifeModel, Builder, and
  Settings/privacy;
- canonical production paths are `/today`, `/workspace`, `/tasks`, `/review`,
  `/life-model`, and `/settings`; root redirects only to Today;
- `/companion`, `/mailbox`, `/runs`, `/runs/*`, `/memory`, `/builder`, and the
  former advanced/support paths display explicit unavailable state and never
  redirect to another product action;
- the complete old `frontend/src/pages` and `frontend/src/components` trees,
  `ProductShell.tsx`, `productShellContract.ts`, and consumed local truth
  reconstruction helpers were deleted in the same switch;
- source, inventory, Rust authority, route, and production-bundle guards prove
  the old owners and Phase 4 dev harnesses are absent;
- no production fallback to the old UI remains.

All migrated rows are now `guarded_absent`. This state does not claim that
external live-provider evidence, manual VoiceOver, or final Phase 4F product
trial is complete.

Phase 4F found that the Phase 4E deletion removed the old Settings recovery UI
while the governed backend command remained shipped. The repair is owned by the
new Settings journey and does not restore any old page, component, route, or
frontend truth helper.

The Phase 4D sections below record the earlier candidate-only checkpoints.
Their `contract_ready` statements are historical evidence and do not override
the current Phase 4E ledger above.

## Historical Phase 4D Read-Only Spine Update

The first Phase 4D slice established exact desktop candidate owners for Shell,
Today, and Tasks without changing production authority:

- dev-only owner: `/dev/phase4d/` plus `tauri.phase4d.conf.json`;
- production owner still active: `App.tsx` -> `ProductShell`, `TodayPage`, and
  `RunsPage`;
- executable guard: production Vite compile flags, release-bundle marker scan,
  and `single_system_phase4d_read_only_spine_is_dev_only_and_product_authority_is_unchanged`;
- deletion status: `DELETE_READY=NO` for Shell, Today, and Tasks;
- reason: governed Workspace/Permission/Review/Resume behavior and the final
  route-authority switch are intentionally not part of this read-only slice.

The real isolated Tauri run returned Today `stale` and Tasks `error` while
required stores were unavailable. That is fail-closed evidence, not proof that
the ready/empty backend path is available, and it does not advance any row to
`delete_ready`.

## Historical Phase 4D Governed-Action Spine Update

The second Phase 4D slice adds candidate owners for the continuous desktop
journey `Workspace -> Permission -> Review -> Refresh -> Resume` inside the
same dev-only Shell:

- `WorkspaceGovernedView` owns one current task, its blocker, the next exact
  action, metadata activity, and an evidence entry;
- `ReviewGovernedView` owns rich permission context and typed
  approve/reject/later decisions;
- `useGovernedActionJourney` owns command dispatch followed by a three-read-
  model refresh and exact review/task identity verification;
- approved permission remains separate from task resume, and a resumed task
  remains separate from completion;
- production owner remains `App.tsx` -> `ProductShell` and the old route pages;
- executable source and bundle guards keep the governed candidate owners out of
  production.

Workspace, Tasks, and Review remain `contract_ready`, not `delete_ready`.
Production callers have not moved, no old owner was deleted, and Phase 4E is
still the only authorized route-authority switch.

## Historical Phase 4D Durable-Truth Spine Update

The third Phase 4D slice adds the candidate journey
`LifeModel/Memory -> Review -> Refresh -> durable result` inside the same
dev-only desktop Shell:

- `DurableTruthDataSource` reads LifeModel, Memory, and Review Center as three
  backend owners and preserves partial failure as an error envelope;
- `durableTruthPresentation` joins only exact ReviewItem and proposal change
  identities, and requires matching LifeModel materialization proof before a
  verified applied treatment;
- `DurableTruthView` keeps pending, approved-not-applied, applying, applied,
  failed, rolled-back, rejected, and unknown distinct;
- the existing governed Review controller records decisions, refreshes the
  exact ReviewItem, and never treats command return as application proof;
- no Apply or rollback control is synthesized while the backend has no typed
  callable action for those effects;
- production owners remain `App.tsx` -> `ProductShell` and the current
  LifeModel/Memory pages.

LifeModel and Memory move to `contract_ready`, not `delete_ready`. Production
callers have not moved and old page owners remain required until Phase 4E.

## Historical Phase 4D Privacy And Configuration Spine Update

The fourth Phase 4D slice adds the candidate journey
`Settings draft -> provider test -> exact permission review -> save -> boundary
refresh` inside the same dev-only desktop Shell:

- `SettingsPrivacyDataSource` reads sanitized AppConfig and
  ProviderPrivacyBoundarySummary independently, resolves only the ReviewItem
  whose proposal ID matches the test result, and calls the existing typed test
  and save commands;
- `useSettingsPrivacyJourney` owns local draft/test/save/refresh sequencing and
  never turns command callbacks into provider or boundary truth;
- `SettingsPrivacyView` implements model/provider and privacy/network surfaces,
  while the other confirmed Settings categories remain explicitly unavailable;
- a provider or endpoint change clears the masked credential in the draft;
- a verified test requires an exact non-simulated completed provider receipt,
  but remains separate from save;
- save always re-reads config and boundary, and unknown refresh remains
  non-green;
- production owners remain `App.tsx` -> `ProductShell` and the current
  `SettingsPage` hierarchy;
- source and bundle guards keep the candidate Settings owner out of production.

Settings remains `contract_ready`, not `delete_ready`. The old product caller
has not moved, no production route changed, and the real Tauri action E2E is
still an explicit validation limit before final production acceptance.
