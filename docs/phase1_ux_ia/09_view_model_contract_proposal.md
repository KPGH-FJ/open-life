# ViewModel Contract Proposal

Status: Phase 1 ViewModel contract proposal.
Scope: Required contract shape and backend ownership validation list only. No backend implementation.

## Classification Legend

- `VERIFIED_FACT`
- `DESIGN_DECISION`
- `DESIGN_ASSUMPTION`
- `CANDIDATE`
- `UNKNOWN`
- `PHASE_2_REQUIRED`

## Hard Rules

1. `DESIGN_DECISION`: Pages cannot reconstruct product truth from raw domain reads.
2. `DESIGN_DECISION`: Pages can only render backend-owned ViewModels / ReadModels, or raw data explicitly marked as debug-only.
3. `DESIGN_DECISION`: Do not invent backend ViewModels, endpoints, projections, stores, or workflows and describe them as existing.
4. `DESIGN_DECISION`: Future required backend fields must be marked `PHASE_2_REQUIRED`.
5. `VERIFIED_FACT`: `LifeStateProjection` exists and is the preferred shared product-state authority for pending review, readiness, task state, safe mode, tool permissions, safe paths, surfaces, and source refs. Source: `docs/phase0_5/06_view_model_gap_inventory.md`.

## Phase 2 Contract Stop Rules

1. `PHASE_2_REQUIRED`: Do not implement Frontend V2 surfaces until each page's ViewModel owner is `EXISTING`, `PARTIAL` with explicit fallback, or explicitly approved as `PROPOSED`.
2. `PHASE_2_REQUIRED`: Do not let `TodayPage`, `Settings`, `Workspace`, `Tasks`, `Review Center`, `LifeModel`, or `Memory` infer any field listed in "Fields That Must Not Be Page-local".
3. `PHASE_2_REQUIRED`: Do not treat `EvidenceRef`, `WorkspaceViewModel`, `TasksViewModel`, `ReviewCenterViewModel`, `LifeModelViewModel`, `MemoryViewModel`, or `SettingsViewModel` as existing backend contracts until current code or Phase 2 implementation verifies them.
4. `PHASE_2_REQUIRED`: If a required backend field is missing, render loading/empty/error/stale/unknown states rather than page-local product truth.

## ViewModel Envelope

```ts
type ViewModelEnvelope<T> = {
  data: T | null
  status: 'loading' | 'ready' | 'empty' | 'error' | 'stale'
  lastUpdatedAt: string | null
  source: 'backend-readmodel'
  evidenceRefs?: EvidenceRef[]
  warnings?: ViewModelWarning[]
  actions: {
    primary: ProductAction[]
    review?: ReviewAction[]
    debugOnly?: DebugAction[]
  }
}
```

## Action Types

- `ProductAction`: default user-facing action required to complete the task.
- `ReviewAction`: approval / rejection / edit / later / evidence action for consequential change.
- `DebugAction`: advanced or developer-only action such as raw trace, export JSON, provider health.

`DESIGN_DECISION`: Debug actions must not be mixed into default product actions.

## Evidence Types

```ts
type EvidenceRef = {
  id: string
  label: string
  source: 'backend-readmodel' | 'audit' | 'task' | 'review' | 'memory' | 'lifemodel'
}
```

Status: `CANDIDATE`. This is a proposed frontend-facing display shape, not an existing backend schema, owner, endpoint, projection, store, or workflow. Backend validation remains `PHASE_2_REQUIRED`.

## TodayViewModel

Purpose: Daily landing page with current goal, review pressure, blockers, safe mode, and next action.

Backend owner: `LifeStateProjection` for shared state plus existing daily-goal read path.
Owner status: PARTIAL.

UI cannot infer: readiness, pending review count, safe mode, global task pressure, product blocker truth.

Empty state: No daily goal or current task; show start-work action without claiming product readiness.
Error state: Could not load backend read model; show retry and avoid fallback truth.
Stale state: Show last updated time and disable destructive actions until refreshed.
Evidence model: projection source refs plus daily-goal refs where available.
Product actions: start in `工作区`, open `审核中心`, retry load.
Review actions: none directly; link to Review Center.
Debug-only actions: inspect projection/source refs.
Auditability: daily recommendations should link to source refs when available.

Required fields: safe mode active/reason, pending review count, primary daily goal, state signals, suggestions, blockers, next recommended action.

Existing backend support: `LifeStateProjection.safeMode`, `LifeStateProjection.pending`, `getDailyGoals`.

Missing backend projection fields: classified goal card types, backend-backed next recommended daily action, blocker/suggestion provenance.

Phase 2 implication: Decide whether to expand `LifeStateProjection` or create a Today-specific read model. Until then, daily-goal classification may be display-only and must not become product truth.

## WorkspaceViewModel

Purpose: Current agent work area for input, understanding, plan, execution timeline, review-needed items, result, and evidence.

Backend owner: Proposed `WorkspaceViewModel`.
Owner status: PHASE_2_REQUIRED.

UI cannot infer: canonical intent, lifecycle state, blocker category, proposal/materialization state, route/privacy boundary, final result status.

Empty state: No active task; show composer and recent context if backend allows.
Error state: Cannot load current task/session; show retry and no fabricated timeline.
Stale state: Task state may have moved; require refresh before resume/retry.
Evidence model: task, final-delivery, kernel/durable event, tool, proposal, and projection refs.
Product actions: start, continue, retry, cancel, inspect evidence, open task.
Review actions: open related ReviewItem; actual approve/reject/edit/later belongs to Review Center.
Debug-only actions: raw trace, kernel events, transcript, provider health.
Auditability: every timeline stage links to source/evidence refs.

Required fields: current session/task id, user goal, agent understanding, lifecycle status, timeline stages, blockers, review item refs, tool call summary, final result, next controls, advanced evidence refs.

Existing backend support: `MainChatAgentStateSnapshot`, `MainChatAgentTaskState`, `MainChatTaskDetail`, `StreamMessageDonePayload`, kernel events, durable events.

Missing backend projection fields: consolidated workspace summary, editable/confirmable intent frame, default timeline stage model, unified blocker/proposal/control summary.

Phase 2 implication: Define the read model before touching ChatPage or creating V2 components.

## TasksViewModel

Purpose: Current and historical tasks with lifecycle, controls, evidence, and run detail links.

Backend owner: Proposed `TasksViewModel`.
Owner status: PHASE_2_REQUIRED.

UI cannot infer: merged AgentRun/task lifecycle, deletion eligibility, stale state, canonical next control.

Empty state: No tasks yet; route user to `工作区`.
Error state: Could not load tasks; no local reconstruction from partial run list.
Stale state: Task controls disabled until refresh.
Evidence model: task summary/detail, AgentRun evidence, final delivery, danger preflight refs.
Product actions: resume, retry, cancel, open detail, inspect, delete when safe.
Review actions: open related review items.
Debug-only actions: run trace, raw JSON/export.
Auditability: task detail should show source refs for lifecycle and actions.

Required fields: active/past task list, lifecycle status, latest result preview, next recommended control, blocker/proposal counts, run/evidence ids, deletion safety state.

Existing backend support: `listAgentRuns`, `listMainChatAgentTasks`, `getMainChatAgentTaskDetail`, `RunEvidenceView`, danger preflight.

Missing backend projection fields: server-owned merged run/task list, stale classification, deletion eligibility summary.

Phase 2 implication: Decide canonical relationship between AgentRun and Main Chat task session.

## ReviewCenterViewModel

Purpose: Central place for proposal, permission, memory, LifeModel, policy, external-write, and dangerous-action decisions.

Backend owner: Proposed `ReviewCenterViewModel`.
Owner status: PHASE_2_REQUIRED.

UI cannot infer: allowed actions, risk authority, expiration behavior, durable apply state, task resume relation.

Empty state: No review items; show clear "没有待确认项."
Error state: Could not load review read model; no local proposal action guesses.
Stale state: Disable review actions until item refreshed.
Evidence model: review item evidence refs, source task/tool/memory/LifeModel refs, audit refs.
Product actions: filter/group/search, open item.
Review actions: approve, reject, later, modify, view evidence.
Debug-only actions: raw proposal payload, audit/export details.
Auditability: every decision records user action, time, source, result, and materialization status where applicable.

Required fields: grouped review items, lifecycle/status, risk/confidence, before/after summary, evidence summary/source refs, allowed actions, related task resume info, safe-mode/safe-path constraints.

Existing backend support: `listProposals`, proposal actions, `LifeStateProjection.pending`, safe paths, partial ReviewWorkflow.

Missing backend projection fields: server-owned review grouping, allowed actions per item, task resume relation summary, safe-path acceptability per external-write proposal, non-proposal item types.

Phase 2 implication: Define unified ReviewItem model and source mapping.

## LifeModelViewModel

Purpose: Show canonical, pending, and provenance state of LifeModel.

Backend owner: Proposed `LifeModelViewModel`.
Owner status: PHASE_2_REQUIRED.

UI cannot infer: canonical truth versus compatibility/current view, dimension trust, pending update materialization, provenance completeness.

Empty state: LifeModel not built; show build/update route without claiming readiness.
Error state: Could not load model/current view; show retry and do not use stale model as current.
Stale state: Show last updated and pending change warnings.
Evidence model: LifeModel source refs, proposal/evidence/patch refs, memory lifecycle refs.
Product actions: build/update LifeModel, open pending review, inspect evidence.
Review actions: linked Review Center actions for changes.
Debug-only actions: compatibility/provenance internals, raw patch/debug export.
Auditability: model dimensions should explain source, last change, and review state.

Required fields: canonical/current model summary, model empty/completion/trust state, dimension summaries, pending update counts, provenance/source refs, quality issues, safe actions.

Existing backend support: `getLifeModel`, `getLifeModelCurrentView`, `getModel4DCompletion`, `LifeStateProjection.readiness`, pending proposal list.

Missing backend projection fields: canonical vs compatibility/current-view truth state, dimension-level provenance summary, memory/LifeModel lifecycle status by candidate.

Phase 2 implication: Validate canonical user-facing source of truth before redesigning LifeModel page.

## MemoryViewModel

Purpose: Explain what OpenLife remembers, what is context-only, active, pending, archived, restored, rolled back, or used in LifeModel.

Backend owner: Proposed `MemoryViewModel`.
Owner status: PHASE_2_REQUIRED.

UI cannot infer: lane counts, lifecycle status, provenance, review requirement, direct write safety.

Empty state: No confirmed memories or no search results; explain candidate vs durable memory.
Error state: Could not load memory read model; do not fall back to raw rows as product truth.
Stale state: Show last updated and disable archive/restore until refreshed.
Evidence model: memory source refs, lifecycle refs, linked review items, linked LifeModel impacts.
Product actions: search, inspect, open review, archive/restore when allowed.
Review actions: approve/reject/edit/later memory candidates through Review Center.
Debug-only actions: raw memory row, vector/index details, diagnostics.
Auditability: memory changes must show source and lifecycle.

Required fields: memory lane counts, active/archived summaries, pending memory proposals, safe-mode write blocking, provenance/evidence refs, restore/archive controls.

Existing backend support: memory search, tier stats, archive/restore, proposal list, MemoryGateway/lifecycle primitives.

Missing backend projection fields: lane-level counts/status in `LifeStateProjection`, context-only vs durable memory summary, memory lifecycle read model for UI.

Phase 2 implication: Validate before top-level `记忆` implementation. Otherwise use reduced-risk IA.

## SettingsViewModel

Purpose: Product-safe setup, privacy, provider, data, tool-permission, and advanced/debug settings.

Backend owner: Proposed `SettingsViewModel` or expanded settings projections.
Owner status: PHASE_2_REQUIRED.

UI cannot infer: setup readiness, provider trust, external transmission state, support/debug mode, safe path authority, permission policy truth.

Empty state: No configured provider/tool permissions; show setup actions.
Error state: Could not load settings/readiness; avoid declaring system ready.
Stale state: show last refresh and require reload before risky actions.
Evidence model: config refs, diagnostics refs, projection refs, privacy/audit/tool refs.
Product actions: configure provider, manage permissions, manage data, open advanced inspection.
Review actions: dangerous actions may create or use ReviewItem/confirmation flows.
Debug-only actions: router status, provider health, MCP/A2A internals, metrics, export debug.
Auditability: risky settings changes need preflight/audit refs.

Required fields: setup readiness, provider route/trust, external transmission status, safe paths/network policy, tool permission summary/detail, data backup/recovery state, advanced/debug visibility state.

Existing backend support: `getConfig`, `getSystemDiagnostics`, `getLifeStateProjection`, router statuses, privacy policy/audit/transmission history, tool permissions/manifests.

Missing backend projection fields: single settings readiness ViewModel, user-facing provider trust summary, support/debug mode policy.

Phase 2 implication: Separate product settings from developer diagnostics before route/component work.

## Expand LifeStateProjection vs Dedicated Read Models

`PHASE_2_REQUIRED`: Humans and engineers must decide whether to:

- expand `LifeStateProjection` with richer surface-specific rows; or
- add adjacent dedicated read models for Workspace, Tasks, Review Center, LifeModel, Memory, and Settings.

`DESIGN_DECISION`: Either approach is acceptable if backend owns product truth and pages do not reconstruct it from raw fragments.

## Fields That Must Not Be Page-local

- readiness and setup status;
- pending review counts;
- safe mode state;
- task lifecycle;
- blocker category;
- final delivery status;
- proposal/review allowed actions;
- durable apply/materialization state;
- memory lane status;
- LifeModel canonical/current/pending distinction;
- provider/privacy boundary summary;
- dangerous action eligibility.

## Backend Contract Non-Hallucination Check

| Proposed backend owner/read model | Status | Evidence | Phase 2 required validation |
| --- | --- | --- | --- |
| `LifeStateProjection` | EXISTING / PARTIAL | Verified in `docs/phase0_5/06_view_model_gap_inventory.md`. | Decide expansion scope and surface-specific fields. |
| `TodayViewModel` | PROPOSED / PHASE_2_REQUIRED | Candidate from Phase 0.5 gap inventory. | Validate daily next-action and goal classification ownership. |
| `WorkspaceViewModel` | PROPOSED / PHASE_2_REQUIRED | Backend task/final-delivery/event primitives exist. | Define consolidated current-work summary and timeline stage model. |
| `TasksViewModel` | PROPOSED / PHASE_2_REQUIRED | Run/task commands and pages exist. | Define server-owned merged task/run lifecycle. |
| `ReviewCenterViewModel` | PROPOSED / PHASE_2_REQUIRED | Proposal actions exist; ReviewWorkflow partial. | Define unified ReviewItem and allowed actions. |
| `LifeModelViewModel` | PROPOSED / PHASE_2_REQUIRED | LifeModel/current/provenance primitives exist. | Define canonical/current/pending/provenance projection. |
| `MemoryViewModel` | PROPOSED / PHASE_2_REQUIRED | Memory search/gateway/lifecycle primitives exist. | Define lane/lifecycle/provenance read model. |
| `SettingsViewModel` | PROPOSED / PHASE_2_REQUIRED | Config/diagnostics/projection/provider/tool data exist. | Define user-facing trust/readiness and support/debug policy. |

## Phase 2 Engineering Questions

1. Which fields are added to `LifeStateProjection` versus dedicated read models?
2. Which current frontend helpers are display-only formatters and which must move backend-side?
3. How does the backend expose allowed actions for ReviewItems?
4. What is the canonical AgentRun/task relationship?
5. How are approved proposal, applied proposal, and materialized memory/LifeModel states represented?
6. Which provider/privacy summary is safe and useful for default UI?
7. What stale/error semantics should block risky actions?
