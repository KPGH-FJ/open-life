# OpenLife Phase 2 Goal
## ViewModel / Backend ReadModel Contract v1.0
### Codex Goal Mode Specification

## Goal

Generate the Phase 2 ViewModel / Backend ReadModel Contract package for OpenLife.

Phase 2 converts the accepted Phase 1 UX / IA / Product Language decisions into an engineering contract for backend-owned read models and frontend ViewModel consumption.

This is still **contract design and verification only**.

Do **not** implement Frontend V2 UI.

Do **not** create React pages, routes, components, CSS, backend migrations, or Tauri command changes.

---

# Role

You are acting as a Principal Engineer and Backend/Frontend Contract Architect.

Your job is to:

1. Read Phase 0, Phase 0.5, and Phase 1 documents.
2. Inspect current source code where needed.
3. Identify existing backend/read-model support.
4. Define exact ViewModel contract shapes.
5. Mark missing contract fields as `PHASE_2_REQUIRED`.
6. Produce a human-reviewable Phase 2 package.
7. Stop before implementation.

---

# Required Inputs

Read:

```text
docs/openlife-phase0-audit/
docs/phase0_5/
docs/phase1_ux_ia/
```

Prioritize:

```text
docs/phase1_ux_ia/01_v2_decision_record.md
docs/phase1_ux_ia/03_v2_information_architecture.md
docs/phase1_ux_ia/04_agent_workspace_model.md
docs/phase1_ux_ia/05_review_center_model.md
docs/phase1_ux_ia/06_lifemodel_memory_model.md
docs/phase1_ux_ia/08_diagnostics_visibility_policy.md
docs/phase1_ux_ia/09_view_model_contract_proposal.md
docs/phase1_ux_ia/10_phase1_summary.md

docs/phase0_5/06_view_model_gap_inventory.md
docs/phase0_5/07_phase0_5_summary.md

docs/openlife-phase0-audit/02_backend_capability_map.md
docs/openlife-phase0-audit/03_agent_system_analysis.md
docs/openlife-phase0-audit/05_backend_frontend_contract.md
docs/openlife-phase0-audit/06_security_governance_audit.md
docs/openlife-phase0-audit/13_audit_summary.md
```

If a required input is missing, record it in:

```text
docs/phase2_viewmodel_contract/00_phase2_methodology.md
docs/phase2_viewmodel_contract/14_phase2_summary_and_phase3_readiness.md
```

Continue with available evidence, but mark missing evidence as `UNKNOWN`.

---

# Hard Non-Goals

Phase 2 outputs must not include:

- React component implementation
- frontend route creation
- ProductShell changes
- ChatPage refactor
- MailboxPage refactor
- SettingsPage refactor
- CSS/design implementation
- backend schema migration
- new Tauri command implementation
- backend store implementation
- mock API pretending to be product truth
- hardcoded frontend-only ViewModel
- changes to production source code

Only create documentation under:

```text
docs/phase2_viewmodel_contract/
```

---

# Evidence / Hallucination Rules

Every major statement must be classified as one of:

```text
VERIFIED_FACT
EXISTING_CODE
DESIGN_DECISION
DESIGN_ASSUMPTION
CANDIDATE
UNKNOWN
PHASE_2_REQUIRED
```

Every proposed contract field must include:

```text
Field:
Type:
Required:
Source of truth:
Owner status:
Evidence:
Frontend may infer? Yes/No
Empty behavior:
Error behavior:
Stale behavior:
Auditability:
```

Allowed owner statuses:

```text
EXISTING
PARTIAL
PROPOSED
UNKNOWN
PHASE_2_REQUIRED
```

Do not invent backend ViewModels, endpoints, projections, stores, workflows, or Tauri commands and describe them as existing.

If a future capability is needed but not currently implemented, mark it:

```text
PROPOSED
PHASE_2_REQUIRED
UNKNOWN
```

Use this phrasing:

```text
Backend owner: Proposed <Name>
Owner status: PHASE_2_REQUIRED
Required validation: <what Phase 3/engineering must verify or implement>
```

---

# Product Capability Preservation Rule

Do not remove important OpenLife product capability just because the read model is incomplete.

If a capability is important but incomplete:

- keep it as `CANDIDATE` or `PHASE_2_REQUIRED`;
- define required contract fields;
- define fallback UI behavior;
- define validation needed before implementation.

This applies especially to:

- Memory top-level navigation;
- LifeModel provenance and change explanation;
- Review Center beyond proposals;
- tool permission and external-write review;
- advanced evidence inspection;
- workspace execution timeline;
- provider/privacy boundary summary.

Guardrails prevent hallucination. They must not reduce OpenLife into a generic chat app, todo app, dashboard, settings panel, or knowledge base.

---

# Required Shared Contract Principles

Use this envelope unless there is a documented reason to change it:

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

Required separation:

- `ProductAction`: default user-facing action required to complete the task.
- `ReviewAction`: approval / rejection / edit / later / evidence action for consequential changes.
- `DebugAction`: advanced or developer-only action such as raw trace, export JSON, provider health.

Hard rule:

```text
Pages cannot reconstruct product truth from raw domain reads.
Pages can only render backend-owned ViewModels / ReadModels, or raw data explicitly marked as debug-only.
```

---

# Required Review Center Contract Principle

Use Phase 1's ReviewItem types and statuses.

Required item type:

```ts
type ReviewItemType =
  | 'proposal'
  | 'permission_request'
  | 'external_write'
  | 'memory_update'
  | 'lifemodel_change'
  | 'policy_change'
  | 'dangerous_action'
```

Required decision status:

```ts
type ReviewItemStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'expired'
  | 'blocked'
  | 'revoked'
  | 'failed'
```

Important:

Do not mix user decision status with backend durable application/materialization state.

Define a separate field such as:

```ts
type ReviewItemMaterializationStatus =
  | 'not_applicable'
  | 'not_started'
  | 'applying'
  | 'applied'
  | 'failed'
  | 'rolled_back'
  | 'unknown'
```

Use this or propose an equivalent model.

---

# Required Outputs

Create:

```text
docs/phase2_viewmodel_contract/
```

Generate exactly these files:

```text
00_phase2_methodology.md
01_phase2_scope_and_constraints.md
02_existing_contract_inventory.md
03_shared_viewmodel_envelope_and_types.md
04_lifestate_projection_extension_plan.md
05_workspace_viewmodel_contract.md
06_review_center_viewmodel_contract.md
07_tasks_viewmodel_contract.md
08_lifemodel_viewmodel_contract.md
09_memory_viewmodel_contract.md
10_today_viewmodel_contract.md
11_settings_viewmodel_contract.md
12_backend_contract_gap_register.md
13_contract_test_plan.md
14_phase2_summary_and_phase3_readiness.md
```

---

# Required Content By File

## 00_phase2_methodology.md

Include:

- documents read;
- source-code areas inspected;
- commands run, if any;
- evidence standard;
- classification legend;
- known limits;
- production-code modification statement.

No production code should be modified.

---

## 01_phase2_scope_and_constraints.md

Define:

- Phase 2 goal;
- non-goals;
- source-of-truth rule;
- product capability preservation rule;
- no fake backend contract rule;
- Phase 3 entry boundary.

Also restate:

```text
Phase 2 is not Frontend V2 implementation.
```

---

## 02_existing_contract_inventory.md

Inventory existing contract assets.

Must inspect/report:

```text
src-tauri/src/life_state_projection.rs
frontend/src/tauri.ts
frontend/src/utils/lifeStateProjection.ts
frontend/src/utils/runtimeDisclosure.ts
frontend/src/utils/reviewDecision.ts
frontend/src/utils/proposalDisplay.ts
frontend/src/utils/runDisplaySummary.ts
frontend/src/utils/lifeModelTrust.ts
frontend/src/pages/TodayPage.tsx
frontend/src/pages/ChatPage.tsx
frontend/src/pages/MailboxPage.tsx
frontend/src/pages/RunsPage.tsx
frontend/src/pages/LifeModelPage.tsx
frontend/src/pages/MemorySearch.tsx
frontend/src/pages/SettingsPage.tsx
```

Adjust if actual paths differ.

For each asset include:

- current purpose;
- product truth it currently owns;
- whether it is backend-owned, frontend helper, page-local, or debug-only;
- whether it should remain, move backend-side, or become display formatter only.

---

## 03_shared_viewmodel_envelope_and_types.md

Define the shared contract types.

Must include:

- `ViewModelEnvelope<T>`;
- `EvidenceRef`;
- `ViewModelWarning`;
- `ProductAction`;
- `ReviewAction`;
- `DebugAction`;
- `ReviewItem`;
- `ReviewItemType`;
- `ReviewItemStatus`;
- `ReviewItemMaterializationStatus`;
- `RiskLevel`;
- `ImpactScope`;
- stale/error/empty semantics;
- source ownership rules.

Every type must indicate whether it is:

```text
EXISTING
PARTIAL
PROPOSED
UNKNOWN
PHASE_2_REQUIRED
```

---

## 04_lifestate_projection_extension_plan.md

Analyze whether to:

1. expand `LifeStateProjection`, or
2. add adjacent dedicated read models, or
3. use a hybrid approach.

Must include:

- current `LifeStateProjection` fields;
- current limitations;
- what global state should remain in `LifeStateProjection`;
- what surface-specific fields should move to dedicated ViewModels;
- migration risks;
- recommended approach.

The likely recommended approach may be hybrid, but verify through evidence.

---

## 05_workspace_viewmodel_contract.md

Define `WorkspaceViewModel`.

It must support:

```text
Intent Composer
Understanding Panel
Execution Timeline
Control / Review Drawer
Result
Evidence Drawer
Advanced Inspector refs
```

Required sections:

- purpose;
- backend owner;
- owner status;
- required fields;
- existing backend support;
- missing backend fields;
- UI cannot infer;
- ProductAction / ReviewAction / DebugAction split;
- state model;
- blocker/failure taxonomy;
- review item refs;
- evidence refs;
- empty/error/stale behavior;
- tests needed;
- implementation stop rules.

Do not implement it.

---

## 06_review_center_viewmodel_contract.md

Define `ReviewCenterViewModel`.

It must include:

- grouped review items;
- ReviewItem type/status/materialization status;
- allowed actions;
- risk;
- impact;
- source;
- evidence;
- expiration;
- audit refs;
- task resume relation;
- tool permission relation;
- memory relation;
- LifeModel relation;
- external write relation.

Important:

Do not let frontend infer allowed actions, risk, expiration, or materialization status.

---

## 07_tasks_viewmodel_contract.md

Define `TasksViewModel`.

It must include:

- active task list;
- historical task list;
- run/task relationship;
- lifecycle status;
- latest result preview;
- blocker/review counts;
- next recommended action;
- task detail contract;
- evidence refs;
- deletion / danger preflight contract;
- retry/resume/cancel action contracts.

Must explicitly address canonical relationship between AgentRun and Main Chat task session.

If unresolved, mark `PHASE_2_REQUIRED`.

---

## 08_lifemodel_viewmodel_contract.md

Define `LifeModelViewModel`.

It must include:

- canonical vs current/compatibility view distinction;
- dimension summaries;
- trust/quality state;
- pending update counts;
- provenance/evidence refs;
- candidate changes;
- materialized changes;
- manual override state;
- related ReviewItem refs;
- Memory/LifeModel linkage;
- empty/error/stale behavior;
- debug-only raw patch/provenance controls.

Must not claim canonical truth when only compatibility/current view is available.

---

## 09_memory_viewmodel_contract.md

Define `MemoryViewModel`.

It must include:

- memory lane model;
- candidate / confirmed / used-in-LifeModel / withdrawn-expired states;
- lane counts;
- source/provenance;
- review item refs;
- archive/restore controls;
- memory search;
- memory lifecycle;
- evidence refs;
- raw memory/vector/index details as debug-only;
- fallback if top-level Memory is not approved.

Must explicitly state whether top-level Memory is implementation-ready or still `PHASE_2_REQUIRED`.

---

## 10_today_viewmodel_contract.md

Define `TodayViewModel`.

It must include:

- daily state summary;
- safe mode;
- pending review count;
- current task pressure;
- blockers;
- suggestions;
- primary daily goal;
- next recommended action;
- links to Workspace and Review Center;
- source-of-truth fields from LifeStateProjection;
- fields that need a Today-specific read model.

Must not let Today reconstruct global pending/readiness/task truth locally.

---

## 11_settings_viewmodel_contract.md

Define `SettingsViewModel`.

It must include:

- setup readiness;
- provider/privacy boundary summary;
- external transmission status;
- tool permission summary;
- data controls;
- safe paths;
- advanced inspection entry;
- developer-only gate;
- MCP/A2A/calibration/versions/metrics visibility status;
- support/debug visibility policy.

Must not let Settings become a diagnostic junk drawer.

---

## 12_backend_contract_gap_register.md

Create a complete register of missing or partial contract items.

Required table:

| Gap ID | Area | Missing contract | Current evidence | Required owner | Status | Phase 3 blocker? | Recommended action |
|---|---|---|---|---|---|---|

Statuses:

```text
EXISTING
PARTIAL
PROPOSED
UNKNOWN
PHASE_2_REQUIRED
```

Must include at least:

- WorkspaceViewModel;
- ReviewCenterViewModel;
- TasksViewModel;
- LifeModelViewModel;
- MemoryViewModel;
- SettingsViewModel;
- TodayViewModel;
- ReviewItemMaterializationStatus;
- Memory lane read model;
- LifeModel canonical/current distinction;
- provider/privacy trust summary;
- browser smoke / desktop trial readiness boundary.

---

## 13_contract_test_plan.md

Define tests required before Phase 3 implementation.

Include:

- Rust/backend read-model tests;
- frontend TypeScript contract tests;
- ViewModel fixture tests;
- ReviewItem allowed-action tests;
- stale/error/empty state tests;
- no raw domain product truth reconstruction tests;
- diagnostics visibility tests;
- migration guard tests;
- smoke/E2E expectations.

Do not write tests. Only define test plan.

---

## 14_phase2_summary_and_phase3_readiness.md

Summarize:

- contracts defined;
- existing owners;
- proposed owners;
- unresolved blockers;
- Memory top-level readiness;
- Review Center readiness;
- Workspace readiness;
- Phase 3 go/no-go;
- required human approvals;
- next recommended Codex Goal.

Phase 3 should not start unless:

- ViewModel owners are approved;
- contract gaps are accepted or scheduled;
- no fake backend owner exists;
- implementation scope is narrowed to a first vertical slice.

---

# Phase 2 Readiness Rules

At the end of Phase 2, classify each ViewModel:

```text
READY_FOR_IMPLEMENTATION
READY_WITH_LIMITS
NOT_READY
```

Definitions:

- `READY_FOR_IMPLEMENTATION`: owner exists or approved; fields are clear; missing fields are not blockers.
- `READY_WITH_LIMITS`: can build a limited UI using existing fields and explicit fallbacks.
- `NOT_READY`: would require frontend guessing or fake backend ownership.

Do not recommend Phase 3 implementation for `NOT_READY` surfaces.

---

# Required Human Review Gates

Before Phase 3, humans must approve:

1. whether `记忆` remains top-level or moves under LifeModel / Settings;
2. ReviewItem materialization model;
3. WorkspaceViewModel contract;
4. ReviewCenterViewModel contract;
5. whether to expand `LifeStateProjection` or add dedicated read models;
6. diagnostics visibility and support/developer mode;
7. provider/privacy trust summary;
8. first vertical slice implementation scope.

---

# Final Response Format

When complete, respond:

```markdown
Phase 2 ViewModel / ReadModel contract documentation complete.

Created:
- docs/phase2_viewmodel_contract/00_phase2_methodology.md
- docs/phase2_viewmodel_contract/01_phase2_scope_and_constraints.md
- docs/phase2_viewmodel_contract/02_existing_contract_inventory.md
- docs/phase2_viewmodel_contract/03_shared_viewmodel_envelope_and_types.md
- docs/phase2_viewmodel_contract/04_lifestate_projection_extension_plan.md
- docs/phase2_viewmodel_contract/05_workspace_viewmodel_contract.md
- docs/phase2_viewmodel_contract/06_review_center_viewmodel_contract.md
- docs/phase2_viewmodel_contract/07_tasks_viewmodel_contract.md
- docs/phase2_viewmodel_contract/08_lifemodel_viewmodel_contract.md
- docs/phase2_viewmodel_contract/09_memory_viewmodel_contract.md
- docs/phase2_viewmodel_contract/10_today_viewmodel_contract.md
- docs/phase2_viewmodel_contract/11_settings_viewmodel_contract.md
- docs/phase2_viewmodel_contract/12_backend_contract_gap_register.md
- docs/phase2_viewmodel_contract/13_contract_test_plan.md
- docs/phase2_viewmodel_contract/14_phase2_summary_and_phase3_readiness.md

Readiness:
- WorkspaceViewModel:
- ReviewCenterViewModel:
- TasksViewModel:
- LifeModelViewModel:
- MemoryViewModel:
- TodayViewModel:
- SettingsViewModel:

Major blockers:
1.
2.
3.

Human approvals required:
1.
2.
3.

No production code was modified.
Frontend V2 implementation was not started.
```
