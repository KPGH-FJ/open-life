# Phase 4 Frontend Migration And Deletion Ledger

Status: `ACTIVE_DURING_V2_MIGRATION`
Date: 2026-07-19

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
| `frontend/src/ui/shell/OpenLifeWorkbenchShell.tsx` in the isolated Phase 4C harness, then the future production shell owner | `frontend/src/components/ProductShell.tsx`, current `productShellContract.ts` primary IA | `frontend/src/App.tsx` | Desktop Tauri Shell keyboard and product journeys accepted; all Phase 4D callers migrated; 4E atomic route switch ready | Production source/import scan and release route test show old shell authority absent | `contract_ready` |
| `frontend/src/ui/foundation/**` semantic tokens and primitives | `frontend/src/components/product/ProductPrimitives.tsx` plus page-local visual primitives | `AgentRunDetail`, `LifeModelPage`, settings tabs, `RuntimeDisclosureStrip`, `ReviewDecisionCard`, and provider/runtime utility type imports | Every Phase 4D journey uses the new semantic owner; no old primitive caller remains | Phase 4E production import scan and old-owner file absence test | `contract_ready` |
| Strict Today V2 adapter over named backend owners | `frontend/src/pages/TodayPage.tsx` local layout/composition | `/today` route in `App.tsx` | Today V2 passes stale/error/unknown and real Tauri journey; no raw fallback | `/today` renders new owner; old page/import absent | `contract_ready` |
| Dev-only Phase 4B harness | deleted `TodayV2PreviewPage` and deleted `/today-v2-preview` production route | no product caller remains | Completed in Phase 4B; independent dev entry owns layout fixtures | Frontend build scan, App route test, inventory contract, and Rust single-system guard prove absence | `guarded_absent` |
| Existing backend `WorkspaceViewModel` plus future Workspace V2 renderer | `CompanionPage`/`ChatPage` joins over Tasks, projection, and page state | `/companion` | Governed-action journey consumes activeTask, pendingReviewItems, activity, actions, and refreshed state | Product page scan has no parallel lifecycle/review/privacy reconstruction | `contract_ready` |
| Existing backend `TasksViewModel` plus future Tasks V2 renderer | `RunsPage` current task-list presentation | `/runs` | Read-only spine accepted and controls dispatch through contract | Old page owner absent; no list/detail reconstruction | `contract_ready` |
| `ReviewItem.decisionContext` plus future Review Center V2 | `MailboxPage` join of raw `listProposals`, `proposalDisplay`, and ReviewItem | `/mailbox` | Pending decision journey renders before/after and exact permission solely from ReviewItem; dispatch always refreshes | Product scan forbids `listProposals` and `buildReviewDecisionView` in Review V2 | `contract_ready` |
| Existing `LifeModelViewModel`/`MemoryViewModel` plus future durable-truth renderer | current LifeModel and Memory page owners | `/life-model`, `/memory` | Durable-truth journey distinguishes approved/applying/applied/failed/rollback | Old page owners absent; no raw proposal or store reconstruction | `identified` |
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
