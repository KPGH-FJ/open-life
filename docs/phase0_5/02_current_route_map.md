# Current Route Map

## Product Routes

| Route / Surface | Component | User-facing purpose | Main data sources | Notes |
| --- | --- | --- | --- | --- |
| `/today` | `TodayPage` | Daily entry point with today's goal, state signals, blockers, safe mode, and review entry. | `getLifeStateProjection`, `getDailyGoals` through `frontend/src/tauri.ts`. | `EXISTING`. Uses projection for pending review and safe mode, but classifies daily-goal cards locally. |
| `/companion` | `CompanionPage` wrapping `ChatPage companionMode` | Companion-style chat/workspace entry with compact agent stage. | Inherits `ChatPage` data sources; local `AgentStage` state. | `EXISTING`. Companion is a wrapper, not a separate backend workflow. |
| `/mailbox` | `MailboxPage` | Review and decide proposals. | `listProposals`, `getLifeStateProjection`, `acceptProposal`, `rejectProposal`, `postponeProposal`, `editProposal`, `resumeMainChatAgentTask`. | `EXISTING`. This is the current Review Center candidate despite the Mailbox name. |
| `/life-model` | `LifeModelPage` | LifeModel build state, overview, evidence, trust, pending update entry. | `getLifeModel`, `getLifeModelCurrentView`, `getSystemDiagnostics`, `getLifeStateProjection`, `getMemoryTierStats`, `listProposals`, builder session/completion calls. | `EXISTING`. Aggregates many sources and needs a clearer ViewModel boundary before V2. |
| `/runs` | `RunsPage` | Run/task history, task controls, deletion preflight. | `listAgentRuns`, `listMainChatAgentTasks`, task control commands, danger preflight. | `EXISTING`. Merges AgentRun history with Main Chat task summaries locally. |
| `/runs/:runId` | `AgentRunDetail` | Detailed run evidence, timeline, trace, task controls, delete preflight. | `getAgentRun`, `listMainChatAgentTasks`, `getMainChatAgentTaskDetail`, task controls, danger preflight. | `EXISTING`. Product subroute under Runs. |
| `/settings` | `SettingsPage` | Configuration, readiness, privacy/data/tools/provider/advanced settings. | Config, diagnostics, projection, router statuses, hot cache, privacy policy, tool permissions, plugins, manifests. | `EXISTING`. Mixes everyday setup with advanced diagnostics/admin surfaces. |

Finding: The shipped product route set is current and does not include old Stage/Beta/migration/cutover routes.
Evidence: `App.tsx` routes current product, secondary, and advanced routes through `productShellContract`; Phase7 manifest classifies old routes as deleted/test-only/historical.
File location: `frontend/src/App.tsx`; `frontend/src/productShellContract.ts`; `plans/openlife_single_system_deletion_manifest.md`.
Confidence: High.
Impact: Frontend V2 planning should map from current product surfaces, not restore deleted route families.

## Secondary Product Routes

| Route / Surface | Component | User-facing purpose | Main data sources | Notes |
| --- | --- | --- | --- | --- |
| `/life-model/build` | `BuilderPage` | Build LifeModel candidates and create review proposals. | Builder commands, `getSystemDiagnostics`, completion/session commands. | `EXISTING`. Product-valid secondary surface grouped under Life Model navigation aliases. |
| `/memory` | `MemorySearch` | Search, inspect, archive, restore, and manually index memory. | `searchMemory`, `indexMemoryChunk`, archive/restore, tier stats, diagnostics. | `EXISTING`. Uses diagnostics/safe mode but not `LifeStateProjection`. |

## Advanced / Diagnostic Routes

| Route / Surface | Component | Purpose | Should be default visible? | Notes |
| --- | --- | --- | --- | --- |
| `/mcp` | `McpPage` | MCP recommendations, server/tool state, audit cleanup/export. | MCP and audit commands through product bridge. | No. | Candidate `高级/开发者`. |
| `/a2a` | `A2APage` | External A2A service and task send/handle flow. | A2A commands through product bridge. | No. | External connection surface with confirmation risk. |
| `/metrics` | `MetricsPage` | Metrics/operational inspection. | Metrics-related product bridge calls. | No. | Developer/advanced visibility. |
| `/calibration` | `CalibrationPage` | Calibration report and apply flow. | Calibration commands and proposal creation. | Needs human decision. | Could remain advanced unless product narrative makes calibration everyday. |
| `/versions` | `VersionControl` | Snapshots, diff, restore/rollback. | Snapshot/version commands. | Needs human decision. | Trust/safety value is user-facing, but details should be controlled. |

Finding: Advanced navigation is already visually separated behind an `Advanced` menu.
Evidence: `ProductShell` renders `SecondaryToolsMenu` with `Advanced technical navigation`; route groups are `Advanced connections` and `Maintenance`.
File location: `frontend/src/components/ProductShell.tsx`; `frontend/src/productShellContract.ts`.
Confidence: High.
Impact: V2 can preserve this separation and refine which advanced surfaces are user-visible.

## Dev/Test/Historical Surfaces

| Surface | Location | Classification | Notes |
| --- | --- | --- | --- |
| Dev/test bridge | `frontend/src/tauriDev.ts` | `DEV_TEST_COMPATIBILITY` | Contains old wrapper names but is not imported by product pages/components in the inspected product surface. |
| Archived Stage1/Step6 helpers | `frontend/src/test/archive/` | `TEST_ARCHIVE` | Historical/test evidence, not product routes. |
| Stage1/Step6 E2E specs | `frontend/e2e/main-chat-stage1-dogfood.spec.ts`, `frontend/e2e/main-chat-step6-product-acceptance.spec.ts` | `DEV_TEST_HISTORICAL` | Specs now emit explicit blockers/retired evidence in non-Tauri browser contexts. |
| Legacy product redirects | `LEGACY_PRODUCT_REDIRECTS` in `productShellContract.ts` | `COMPAT_REDIRECT` | Redirects `/chat` to `/companion`, `/review` to `/mailbox`, `/builder` to `/life-model/build`, etc.; not independent old product routes. |
| Phase 0 audit reports | `docs/openlife-phase0-audit/` | `ANALYSIS_REFERENCE` | Existing untracked audit package used as prior evidence only. |

Finding: `frontend/src/tauri.ts` is the product bridge; `frontend/src/tauriDev.ts` is dev/test compatibility.
Evidence: Product pages/components import from `../tauri`; raw route-old wrappers live in `tauriDev.ts`; Phase7 manifest explicitly classifies `tauriDev.ts` old wrapper aliases as test-only archive.
File location: `frontend/src/tauri.ts`; `frontend/src/tauriDev.ts`; `frontend/src/pages/`; `frontend/src/components/`; `plans/openlife_single_system_deletion_manifest.md`.
Confidence: High.
Impact: V2 contract work must not treat `tauriDev.ts` as product authority.

## Current Navigation Shape

`ProductShell` uses six primary tabs:

- `Today`
- `Companion`
- `Mailbox`
- `Life Model`
- `Runs`
- `Settings`

It also exposes an `Advanced` menu containing grouped technical surfaces:

- `Advanced connections`: `MCP / Tools`, `A2A`
- `Maintenance`: `Metrics`, `Calibration`, `Versions`

The Life Model tab is active for `/life-model`, `/life-model/build`, and `/memory`.

Finding: Product navigation is already route-contract-driven.
Evidence: `PRIMARY_PRODUCT_ROUTES`, `SECONDARY_PRODUCT_ROUTES`, `ADVANCED_PRODUCT_ROUTES`, `ADVANCED_PRODUCT_ROUTE_GROUPS`, and `PRODUCT_ROUTE_ALIASES`.
File location: `frontend/src/productShellContract.ts`; `frontend/src/components/ProductShell.tsx`.
Confidence: High.
Impact: V2 should update a route/IA contract first, not scatter route labels across pages.

## Initial V2 IA Mapping Candidate

| Current Surface | Candidate V2 Surface | Confidence | Reason |
| --- | --- | --- | --- |
| `/today` Today | 今日 | High | Already Chinese-facing on page and reads shared projection for daily review/safe-mode state. |
| `/companion` Companion | 工作区 | Medium | Companion is a wrapper over Chat; future Agent Workspace likely absorbs it. |
| Current embedded Chat surface | 工作区 | High | Chat owns user input, intent, task lifecycle, traces, proposals, and execution result. |
| `/runs` and `/runs/:runId` | 任务 | High | Runs already represent history/detail/control for task/run lifecycle. |
| `/mailbox` | 审核中心 | High | It owns accept/reject/postpone/edit decisions and task resume after review. |
| `/life-model` | LifeModel | High | Strong domain object should remain its own top-level surface. |
| `/life-model/build` | LifeModel | Medium | Build is LifeModel creation/update workflow, but confirmations should route through 审核中心. |
| `/memory` | 记忆 | Medium | Memory has enough product meaning to be a candidate top-level surface, but current nav groups it under Life Model. |
| `/settings` | 设置 | High | Product settings remain needed. |
| `/mcp`, `/a2a`, `/metrics`, `/calibration`, `/versions` | 高级/开发者 | Medium | Some trust/safety parts may become default product details, but route-level surfaces should not dominate first-use IA. |

Human decisions required:

1. Whether `Companion` and `Chat` merge into one `工作区`.
2. Whether `Runs` becomes `任务` or stays as history/evidence under `工作区`.
3. Whether `Memory` becomes top-level `记忆` or remains under `LifeModel`.
4. Whether `Calibration` and `Versions` are everyday trust features or advanced tools.
