# Existing Contract Inventory

Status: source-backed inventory of current contract assets.

## Summary

`EXISTING_CODE`: `LifeStateProjection` is the only verified backend-owned shared product-state read model in this inventory.

`EXISTING_CODE`: `frontend/src/tauri.ts` exposes many useful typed primitives, including structured Main Chat results, task state/detail, AgentRun history, proposal actions, Memory lifecycle, LifeModel current view, diagnostics, tool permissions, and `getLifeStateProjection`.

`VERIFIED_FACT`: Phase 0.5 and Phase 1 agree that current pages still combine projection reads with raw domain reads. This package treats those page-local assemblies as evidence inputs, not approved ViewModel owners.

## Asset Inventory

| Asset | Current purpose | Product truth currently owned | Classification | Phase 2 disposition |
| --- | --- | --- | --- | --- |
| `src-tauri/src/life_state_projection.rs` | Backend read model for shared product state. | Pending review counts, readiness, task counts, safe mode, tool permission counts, safe paths, source refs. | `EXISTING_CODE`; owner status `EXISTING/PARTIAL`. | Remain backend authority for global/shared state; do not overload with rich page-specific detail. |
| `frontend/src/tauri.ts` | Typed product bridge over Tauri invoke. | Does not own truth; mirrors command contracts and exposes raw primitives. | `EXISTING_CODE`; owner status `EXISTING` as bridge, not ViewModel. | Keep as product bridge. ViewModel truth must come from backend contracts, not bridge-side merging. |
| `frontend/src/utils/lifeStateProjection.ts` | Finds surface row and review count from projection. | Display/access helper only. | `EXISTING_CODE`; owner status `PARTIAL`. | Remain formatter/access helper. Must not invent missing surface truth. |
| `frontend/src/utils/runtimeDisclosure.ts` | Builds route/boundary/outcome/tool/proposal/blocker labels from run/task/evidence. | Page-local display view over raw run/task evidence. | `EXISTING_CODE`; owner status `PARTIAL`. | Keep as display formatter or move logic backend-side for default product truth. Debug/technical rows stay advanced. |
| `frontend/src/utils/reviewDecision.ts` | Groups proposals and formats review decision copy. | Infers grouping, impact, and risk tone from `AgentProposal`. | `EXISTING_CODE`; owner status `PARTIAL`. | Can remain display formatter only after backend owns ReviewItem type, allowed actions, risk, expiration, and materialization. |
| `frontend/src/utils/proposalDisplay.ts` | Formats proposal title, domain, diff, evidence, and technical rows. | Display-only proposal interpretation. | `EXISTING_CODE`; owner status `PARTIAL`. | Keep for rendering approved backend fields; do not treat as Review Center authority. |
| `frontend/src/utils/runDisplaySummary.ts` | Summarizes AgentRun and optional Main Chat task summary. | Locally merges run/task display state. | `EXISTING_CODE`; owner status `PARTIAL`. | Candidate display formatter after TasksViewModel owns merged lifecycle. |
| `frontend/src/utils/lifeModelTrust.ts` | Builds dimension trust views from model, diagnostics, completion, and proposals. | Locally computes LifeModel display state. | `EXISTING_CODE`; owner status `PARTIAL`. | Candidate formatter after backend owns canonical/current/provenance/trust summary. |
| `frontend/src/pages/TodayPage.tsx` | Daily page. | Uses projection plus daily goals; locally classifies daily cards. | `EXISTING_CODE`; owner status `PARTIAL`. | Can support limited TodayViewModel; classified suggestions/blockers need backend owner. |
| `frontend/src/pages/ChatPage.tsx` | Current Chat/Companion work surface. | Locally assembles messages, streaming, diagnostics, LifeModel, goals, runs, tasks, events, proposals, skills/tools, controls, trace, and projection. | `EXISTING_CODE`; owner status `PARTIAL` primitives only. | Must not be renamed into V2 Workspace without WorkspaceViewModel. |
| `frontend/src/pages/MailboxPage.tsx` | Proposal review page. | Lists proposals, formats review decisions, applies local safe-path/safe-mode accept checks, resumes tasks after review. | `EXISTING_CODE`; owner status `PARTIAL`. | Review Center requires backend ReviewItem owner for grouping, allowed actions, expiration, materialization, and non-proposal types. |
| `frontend/src/pages/RunsPage.tsx` | Run/task list and controls. | Locally merges `AgentRun` and Main Chat task summaries plus danger preflight. | `EXISTING_CODE`; owner status `PARTIAL`. | TasksViewModel must own merged AgentRun/task lifecycle before V2 task UI. |
| `frontend/src/pages/LifeModelPage.tsx` | LifeModel overview/build/evidence page. | Combines LifeModel, current view, diagnostics, projection, completion, memory tier stats, and pending proposals. | `EXISTING_CODE`; owner status `PARTIAL`. | LifeModelViewModel needs backend-owned canonical/current/provenance/trust distinction. |
| `frontend/src/pages/MemorySearch.tsx` | Technical memory search/index/archive surface. | Uses memory search, tier stats, diagnostics, indexing, archive state. | `EXISTING_CODE`; owner status `PARTIAL`. | Not sufficient for top-level Memory; MemoryViewModel requires lane/lifecycle/provenance summaries. |
| `frontend/src/pages/SettingsPage.tsx` | Settings, setup, privacy, provider, tools, data, and advanced diagnostics. | Combines config, diagnostics, projection, hot cache, privacy policy, tool permissions, plugins, manifests, router statuses, danger preflight. | `EXISTING_CODE`; owner status `PARTIAL`. | SettingsViewModel must separate product settings from support/developer diagnostics. |

## Current `LifeStateProjection` Fields

`EXISTING_CODE`: Backend `LifeStateProjection` currently includes:

- `version`
- `generatedAt`
- `pending`
- `readiness`
- `taskState`
- `safeMode`
- `toolPermissions`
- `safePaths`
- `surfaces`
- `sourceRefs`

`EXISTING_CODE`: `surfaces` currently produces rows for `today`, `mailbox`, `chat`, `companion`, `life_model`, and `settings`.

`EXISTING_CODE`: Current surface rows copy the same pending/readiness/task/safe-mode/tool-permission values for each surface. They are not yet rich surface-specific ViewModels.

## Existing Backend Support By Domain

| Domain | Existing support | Gap |
| --- | --- | --- |
| Workspace | `MainChatAgentStateSnapshot`, `MainChatAgentTaskState`, `MainChatTaskDetail`, kernel events, durable events, `StreamMessageDonePayload`. | No consolidated `WorkspaceViewModel`, no default timeline stage model, no editable/confirmable understanding object. |
| Review Center | `AgentProposal`, proposal actions, `LifeStateProjection.pending`, safe paths, partial ReviewWorkflow. | No unified `ReviewItem`, no backend-owned allowed actions per item, no separate materialization/apply status for every review type. |
| Tasks | `AgentRun`, `MainChatTaskSummary`, `MainChatTaskDetail`, `RunEvidenceView`, danger preflight. | No backend-owned merged AgentRun/task read model and unresolved canonical relationship. |
| LifeModel | `getLifeModel`, `getLifeModelCurrentView`, `getModel4DCompletion`, proposal list, provenance primitives. | No LifeModelViewModel that labels canonical/current/compatibility, candidate, materialized, and provenance state. |
| Memory | Memory search, tier stats, MemoryGateway/lifecycle primitives, `MemoryLifecycleRecord`, archive/rollback controls. | No lane-level product read model for candidate/confirmed/used-in-LifeModel/withdrawn states. |
| Today | `LifeStateProjection`, `getDailyGoals`. | No backend next-action/suggestion/blocker classification owner. |
| Settings | Config, diagnostics, projection, privacy policy, transmission history, tool permissions/manifests, safe paths, danger preflight. | No product-safe settings summary or support/debug visibility mode contract. |
