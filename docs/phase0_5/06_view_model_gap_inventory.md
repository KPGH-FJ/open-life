# ViewModel Gap Inventory

## Existing Shared Read Models

### LifeStateProjection

`LifeStateProjection` is the current shared product-state projection.

It includes:

- `pending`: pending/edited/high-risk review counts and store status.
- `readiness`: chat/usage/LifeModel readiness, model empty, builder sessions, database status, readiness issues.
- `taskState`: latest task id/status and running/waiting/blocked/failed/cancelled/completed counts.
- `safeMode`: active state, reason, source refs.
- `toolPermissions`: total/active/consumed/allow/deny/ask/once/until-revoked counts.
- `safePaths`.
- `surfaces`: per-surface rows for `today`, `mailbox`, `chat`, `companion`, `life_model`, `settings`.
- `sourceRefs`.

Finding: `LifeStateProjection` is real and should be preferred for shared product state.
Evidence: backend projection structs and frontend mirror types; helper `reviewRequiredCountFromProjection`.
File location: `src-tauri/src/life_state_projection.rs`; `frontend/src/tauri.ts`; `frontend/src/utils/lifeStateProjection.ts`.
Confidence: High.
Impact: V2 should not reconstruct pending/readiness/safe-mode/task counts from raw page-local fragments where projection fields exist.

Finding: The current per-surface projection rows are not yet surface-specific.
Evidence: `build_surface_projection` maps the same pending, task, safe-mode, and tool-permission counts into each surface id.
File location: `src-tauri/src/life_state_projection.rs`.
Confidence: High.
Impact: V2 may need richer backend projections before it can remove page-local interpretation.

### Adjacent Frontend View Helpers

Existing helper/view surfaces:

- `runtimeDisclosure.ts`: route, boundary, outcome, tools, proposals, blockers, next action.
- `providerReadiness.ts`: provider validation and external transmission display.
- `proposalDisplay.ts`: proposal domain/type/diff/evidence display.
- `reviewDecision.ts`: review grouping, risk, confidence, evidence, impact.
- `runDisplaySummary.ts`: run/task summary display.
- `lifeModelTrust.ts`: LifeModel dimension trust display.

Finding: ViewModel logic exists, but it is fragmented across utility helpers and pages.
Evidence: helpers above are consumed by Chat, Runs, Mailbox, LifeModel, and Settings.
File location: `frontend/src/utils/`; `frontend/src/pages/`.
Confidence: High.
Impact: V2 should define explicit page-level ViewModels instead of further expanding page-local data assembly.

## Page Data Dependencies

| Page | Shared projection usage | Raw domain reads | Local state concerns | Risk |
| --- | --- | --- | --- | --- |
| Today | Uses projection for safe mode and review count. | `getDailyGoals`. | Classifies daily goals into goal/state/suggestion/blocker cards locally. | Medium |
| Companion | No direct projection; wraps Chat. | Inherits Chat. | Local `AgentStage` state derived from Chat callback. | Medium |
| Chat | Uses projection for review count/fix suggestions. | Diagnostics, scheduler config, LifeModel, daily goals, chat sessions/history, runs, pending proposals, task state/detail, durable events, skills/tools, plan controls. | Very large local state surface for messages, streaming, trace, tool calls, runs, tasks, proposals, stage, diagnostics. | High |
| Mailbox | Uses projection for safe mode, safe paths, review count. | `listProposals`, proposal actions, task resume. | Local folders, filters, can-accept rules, edit state, route-state resume. | High |
| LifeModel | Uses projection for builder review/session counts and review count. | LifeModel, current view, diagnostics, memory tier, pending proposals, builder completion/sessions. | Locally computes dimensions, trust, quality issues, readiness labels. | High |
| Runs | Does not use projection. | `listAgentRuns`, `listMainChatAgentTasks`, task detail/control, danger preflight/delete. | Locally merges run status with task summaries and evidence views. | High |
| Run Detail | Does not use projection. | `getAgentRun`, task summaries/detail, task controls, danger preflight/delete. | Builds activity timeline and lifecycle locally from run/task/evidence. | High |
| Settings | Uses projection plus many raw diagnostics/admin sources. | Config, diagnostics, router statuses, hot cache, privacy policy, tool permissions, plugins, manifests. | Mixes setup readiness, provider health, data/privacy/admin/debug in one page. | High |
| Memory | Does not use projection. | Search/index/archive/restore, tier stats, diagnostics. | Local safe-mode via diagnostics; memory lane semantics are page-local. | Medium |
| Builder | Does not use projection directly. | Builder session/proposals/completion/diagnostics. | Creates proposal candidates and local review UI. | Medium |
| LifeModelEditor | Does not use projection. | LifeModel and diagnostics; direct save command. | Manual edit/autosave state, safe-mode write blocking from diagnostics. | High |

Finding: `ChatPage` is the largest ViewModel gap.
Evidence: It imports many product bridge commands, owns dozens of `useState` values, listens to Tauri events, and handles task/proposal/plan controls.
File location: `frontend/src/pages/ChatPage.tsx`; `frontend/src/tauri.ts`.
Confidence: High.
Impact: V2 should not begin by editing ChatPage; it needs a workspace read-model contract first.

## Product Truth Conflicts

| Concern | Current state | Conflict / gap |
| --- | --- | --- |
| Pending review count | Projection provides `pending.totalReviewRequiredCount`; pages also list proposals. | Pages need item lists, but count/readiness should come from projection. |
| Readiness | Projection and diagnostics both expose readiness. | Settings still combines projection, diagnostics, provider readiness, and local checklist logic. |
| Safe mode | Projection exposes `safeMode`; some pages use diagnostics helpers. | Memory and LifeModelEditor still derive safe mode from diagnostics. |
| Task state | Projection counts tasks; Chat/Runs read full task state/detail. | Need `WorkspaceViewModel` / `TasksViewModel` to avoid page-local lifecycle merging. |
| Tool permissions | Projection counts permissions; Settings reads full permission records/manifests. | Summary can be projection-backed; detail needs a permissions ViewModel. |
| Blocker state | Chat/Runs derive blockers from task state, run evidence, final delivery, and kernel events. | Need canonical blocker summary for default UI. |
| Proposal state | Mailbox owns lists/actions; Chat and LifeModel also show proposal previews. | Review Center should own decision state; other pages should link/preview only. |
| Memory lane state | MemoryGateway/lifecycle exist in backend, but UI pieces are spread across Mailbox, Memory, LifeModel, Chat. | Need memory lane projection fields. |
| LifeModel state | LifeModel page combines canonical model, current view, completion, pending proposals, trust helpers. | Need LifeModelViewModel to separate canonical truth, candidate updates, and provenance. |

## Candidate V2 ViewModels

### TodayViewModel

Purpose: Daily landing page with current goal, review pressure, blockers, safe-mode, and next action.

Source of truth: `LifeStateProjection` plus daily-goal read model.

Required fields:

- safe mode active/reason
- pending review count
- primary daily goal
- state signals
- suggestions
- blockers
- next recommended action

Known backend support:

- `LifeStateProjection.safeMode`
- `LifeStateProjection.pending`
- `getDailyGoals`

Missing backend projection fields:

- classified goal card types
- backend-backed next recommended daily action
- blocker/suggestion provenance

Risks:

- Current local classification may present page-local truth as product truth.

### WorkspaceViewModel

Purpose: Current agent work area for input, understanding, plan, execution timeline, review-needed items, result, and evidence.

Source of truth: Main Chat task/run read model, final delivery, `LifeStateProjection`, and runtime route evidence.

Required fields:

- current session/task id
- user goal / input draft
- agent understanding / route / privacy boundary
- lifecycle status
- timeline stages
- blockers
- proposals requiring review
- tool calls summary
- final result
- next controls
- advanced evidence refs

Known backend support:

- `MainChatAgentStateSnapshot`
- `MainChatAgentTaskState`
- `MainChatTaskDetail`
- `StreamMessageDonePayload`
- kernel/durable events

Missing backend projection fields:

- consolidated workspace summary
- editable/confirmable intent frame
- default timeline stage model
- unified blocker/proposal/control summary

Risks:

- Implementing V2 directly in `ChatPage` would preserve current page-local complexity.

### TasksViewModel

Purpose: Current and historical tasks with lifecycle, controls, evidence, and run detail links.

Source of truth: agent task summaries/details plus AgentRun metadata.

Required fields:

- active/past task list
- lifecycle status
- latest result preview
- next recommended control
- blockers/proposals count
- run/evidence ids
- deletion safety state

Known backend support:

- `listAgentRuns`
- `listMainChatAgentTasks`
- `getMainChatAgentTaskDetail`
- `RunEvidenceView`

Missing backend projection fields:

- server-owned merged run/task list
- stale run classification
- deletion eligibility summary

Risks:

- Current Runs page reconstructs lifecycle locally from run plus task summary.

### ReviewCenterViewModel

Purpose: Central place for proposal, permission, memory, LifeModel, and external action decisions.

Source of truth: proposal/review workflow read model plus `LifeStateProjection.pending`.

Required fields:

- grouped review items
- lifecycle/status
- risk/confidence
- before/after summary
- evidence summary and source refs
- allowed actions
- related task resume info
- safe-mode/safe-path constraints

Known backend support:

- `listProposals`
- proposal actions
- `LifeStateProjection.pending`
- `safePaths`

Missing backend projection fields:

- server-owned review grouping
- allowed-actions per proposal
- task resume relation summary
- safe-path acceptability per external-write proposal

Risks:

- Local `canAccept` rules may diverge from backend ReviewWorkflow/gateway authority.

### LifeModelViewModel

Purpose: Show canonical/pending/provenance state of LifeModel.

Source of truth: LifeModel/current view/provenance, projection readiness, proposal summaries, memory lifecycle.

Required fields:

- canonical/current model summary
- model empty/completion/trust state
- dimension summaries
- pending update counts
- provenance/source refs
- quality issues and safe actions

Known backend support:

- `getLifeModel`
- `getLifeModelCurrentView`
- `getModel4DCompletion`
- `LifeStateProjection.readiness`
- pending proposal list

Missing backend projection fields:

- canonical vs compatibility/current-view truth state
- dimension-level provenance summary
- memory/LifeModel lifecycle status by candidate

Risks:

- Page-local trust computation may drift from backend governance semantics.

### MemoryViewModel

Purpose: Explain what OpenLife remembers, what is context-only, active, pending, archived, restored, or rolled back.

Source of truth: MemoryGateway/lifecycle/tier read models, proposal state, projection safe mode.

Required fields:

- memory lane counts
- active/archived summaries
- pending memory proposals
- safe-mode write blocking
- provenance/evidence refs
- restore/archive controls

Known backend support:

- memory search
- tier stats
- archive/restore
- proposal list
- MemoryGateway/lifecycle backend primitives

Missing backend projection fields:

- lane-level counts/status in `LifeStateProjection`
- context-only vs durable memory summary
- memory lifecycle read model for UI

Risks:

- Current MemorySearch can read/write/index from a technical memory view without showing full governance lane semantics.

### SettingsViewModel

Purpose: Product-safe setup, privacy, provider, data, tool-permission, and advanced/debug settings.

Source of truth: config, diagnostics, projection, provider readiness, privacy/audit read models, tool permissions.

Required fields:

- setup readiness
- provider route/trust
- external transmission status
- safe paths and network policy
- tool permission summary/detail
- data backup/recovery state
- advanced/debug visibility state

Known backend support:

- `getConfig`
- `getSystemDiagnostics`
- `getLifeStateProjection`
- router statuses
- privacy policy/audit/transmission history
- tool permissions/manifests

Missing backend projection fields:

- single settings readiness ViewModel
- user-facing provider trust summary
- support/debug mode policy

Risks:

- Current Settings mixes product setup and developer diagnostics.

## Backend Projection Gaps

1. Surface-specific `LifeStateProjection.surfaces` fields instead of uniform copies.
2. Workspace/task summary projection that merges task state, final delivery, route evidence, blockers, proposals, and next controls.
3. Review Center read model with server-owned grouping and allowed actions.
4. Memory lane/lifecycle projection.
5. LifeModel provenance/trust projection for current/candidate/canonical state.
6. Provider/external-transmission trust summary that is user-facing and not just diagnostics.
7. Advanced/debug visibility policy so product UI knows what to hide by default.

## Human Decisions Needed

1. Which ViewModels must be backend-owned before Frontend V2 begins?
2. Which page-local helpers can be kept as display-only formatters?
3. Should `LifeStateProjection` be expanded, or should adjacent dedicated read models be added?
4. What is the canonical relationship between AgentRun and task session in the UI?
5. Should manual LifeModel editing remain, and what ViewModel marks it as manual override?
6. What memory lane states must be visible to users?
