# Phase 4 Frontend Migration And Deletion Ledger

Status: `ACTIVE_DURING_V2_MIGRATION`
Date: 2026-07-20

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
| `frontend/src/ui/shell/OpenLifeWorkbenchShell.tsx`, consumed by `frontend/src/ui/journeys/readOnly/ReadOnlySpineJourney.tsx` in the isolated Phase 4D harness, then the future production shell owner | `frontend/src/components/ProductShell.tsx`, current `productShellContract.ts` primary IA | `frontend/src/App.tsx` | Desktop Tauri Shell keyboard and all Phase 4D business journeys accepted; remaining product callers migrated; 4E atomic route switch ready | Production source/import scan and release route test show old shell authority absent | `contract_ready` |
| `frontend/src/ui/foundation/**` semantic tokens and primitives | `frontend/src/components/product/ProductPrimitives.tsx` plus page-local visual primitives | `AgentRunDetail`, `LifeModelPage`, settings tabs, `RuntimeDisclosureStrip`, `ReviewDecisionCard`, and provider/runtime utility type imports | Every Phase 4D journey uses the new semantic owner; no old primitive caller remains | Phase 4E production import scan and old-owner file absence test | `contract_ready` |
| `frontend/src/ui/journeys/readOnly/TodayReadOnlyView.tsx` over `readOnlySpineDataSource.ts` and strict `openlife.today-adapter.v1` | `frontend/src/pages/TodayPage.tsx` local layout/composition | `/today` route in `frontend/src/App.tsx` | Today journey passes ready/empty/stale/error/unknown-boundary and real Tauri fail-closed dogfood; production caller moves in the 4E atomic switch | `/today` renders the new owner; old page/import absent; release bundle excludes Phase 4D harness markers | `contract_ready` |
| Dev-only Phase 4B harness | deleted `TodayV2PreviewPage` and deleted `/today-v2-preview` production route | no product caller remains | Completed in Phase 4B; independent dev entry owns layout fixtures | Frontend build scan, App route test, inventory contract, and Rust single-system guard prove absence | `guarded_absent` |
| `frontend/src/ui/journeys/governedAction/WorkspaceGovernedView.tsx` over backend `WorkspaceViewModel` plus `useGovernedActionJourney.ts` | `CompanionPage`/`ChatPage` joins over Tasks, projection, page state, permission decision, and automatic resume | `/companion`; old Chat/Workspace production callers remain | Governed-action journey consumes `activeTask`, linked review items, metadata activity, exact resume `TaskControl`, and refreshed task identity/state; remaining production callers move in 4E | Product page scan has no parallel lifecycle/review/privacy reconstruction; release bundle excludes governed journey markers | `contract_ready` |
| `frontend/src/ui/journeys/readOnly/TasksReadOnlyView.tsx` plus governed refresh over backend `TasksViewModel` | `frontend/src/pages/RunsPage.tsx` task-list presentation plus remaining task-control joins | `/runs` route in `frontend/src/App.tsx`; task-control callers remain in old pages | Read-only Tasks journey accepted; governed controller now owns exact resume through dispatch -> refresh -> identity/state verification; production caller moves only in 4E | Old task-list owner absent; no frontend `AgentRun` list join; release bundle excludes Phase 4D journey markers | `contract_ready` |
| `frontend/src/ui/journeys/governedAction/ReviewGovernedView.tsx` over `ReviewItem.decisionContext` and `reviewDispatchReducer` | `MailboxPage` join of raw `listProposals`, `proposalDisplay`, and ReviewItem; Chat page automatic permission approval | `/mailbox` and remaining Chat permission caller | Pending decision journey renders before/after and exact permission solely from `ReviewItem`; approve/reject/later dispatch always refreshes the same review target and never resumes automatically | Product scan forbids raw proposal reconstruction and automatic approve/resume in the candidate owner; release bundle excludes governed journey markers | `contract_ready` |
| `frontend/src/ui/journeys/durableTruth/**` over backend `LifeModelViewModel`, `MemoryViewModel`, and exact `ReviewItem` lifecycle | current LifeModel and Memory page owners | `/life-model`, `/memory` | Durable-truth journey distinguishes pending/approved/applying/applied/failed/rolled-back, requires exact refreshed proof, and receives typed apply/rollback actions before exposing those controls | Old page owners absent; no raw proposal/store reconstruction; release bundle excludes durable journey markers | `contract_ready` |
| `settingsOrchestrationContract` plus future Settings V2 | `SettingsPage` page-local test/save/refresh lifecycle | `/settings` | Privacy/config journey uses edit -> test -> save -> boundary refresh and fails closed | Old reducer/lifecycle absent; route test proves unknown boundary is not green | `contract_ready` |
| Shell Settings utility context plus collapsed diagnostic access | ProductShell advanced groups and same-level advanced routes | ProductShell secondary/advanced navigation | Shell V2 keeps Settings outside primary product navigation and Advanced inside Settings/Inspector; unavailable states are explicit | Navigation matrix and release route inventory | `contract_ready` |
| Future V2 route authority | `LEGACY_PRODUCT_REDIRECTS` compatibility routes | current `App.tsx` redirect map | Every external caller and active doc uses canonical routes; removal reviewed under Phase7 deletion contract | Redirect path absence tests | `identified` |

## Per-Slice Rule

Every Phase 4D journey must update this ledger in the same change:

1. list the new component owner;
2. identify every remaining old caller;
3. move the row to `delete_ready` only when no production caller remains;
4. add the exact absence guard before 4E;
5. perform deletion in the same 4E atomic authority switch.

The ledger does not authorize a long-lived `/v2` route or production fallback.

## Phase 4D Read-Only Spine Update

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

## Phase 4D Governed-Action Spine Update

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

## Phase 4D Durable-Truth Spine Update

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
