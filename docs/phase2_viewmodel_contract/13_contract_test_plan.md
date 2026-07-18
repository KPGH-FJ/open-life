# Contract Test Plan

Status: test plan only. No tests implemented in Phase 2.

## Backend Read-model Tests

`PHASE_2_REQUIRED`: Add focused Rust tests for:

- `LifeStateProjection` global shared fields: pending, readiness, task state, safe mode, tool permissions, safe paths, source refs.
- Surface summary rows: ensure they do not pretend to be full page ViewModels.
- Workspace lifecycle mapping from task/final-delivery/event evidence.
- Workspace nested schema validation for `WorkspaceIntentComposer`, `WorkspaceUnderstanding`, `TimelineStage`, `WorkspaceResult`, `BlockerSummary`, `WorkspaceToolSummary`, and `ProviderPrivacyBoundarySummary`.
- ReviewItem allowed actions, risk, expiration, materialization status, and `ReviewAction.kind`/`effect` invariants.
- Review Center relation schema validation for source, task resume, tool permission, Memory, LifeModel, and external write relations.
- Tasks merged AgentRun/Main Chat task identity plus `TaskListItem`, `TaskLifecycleStatus`, `TaskResultPreview`, `TaskDetailSummary`, and `TaskDeletionPreflight`.
- LifeModel truth mode and dimension provenance plus canonical/current/candidate/materialized/manual-override nested schemas.
- Memory lane counts/status/provenance.
- Memory nested schema validation for `MemoryLaneSummary`, `MemoryItemSummary`, `MemoryLifeModelLink`, `MemoryProvenanceSummary`, `MemorySearchSummary`, and `MemoryLifecycleSummary`.
- Today limited-slice schema validation for daily state, safe mode, task pressure, blockers, suggestions, and daily goal classification.
- Settings provider/privacy summary, external transmission, tool permission, data controls, developer gate, visibility, and support/debug policy.

## Frontend TypeScript Contract Tests

`PHASE_2_REQUIRED`: Add TypeScript tests for:

- `ViewModelEnvelope<T>` status handling.
- Empty/error/stale rendering contracts.
- Risky action disabled behavior when envelope status is `stale` or `error`.
- `ProductAction`, `ReviewAction`, and `DebugAction` separation.
- `ReviewAction.kind` and `ReviewAction.effect` mismatch rejection.
- Evidence refs shown or hidden according to visibility policy.
- No debug-only actions in default product controls.

## ViewModel Fixture Tests

`PHASE_2_REQUIRED`: Create fixtures for:

- Today empty, ready, safe mode, pending review, stale.
- Workspace idle, running, waiting permission, blocked, failed, completed, completed_with_pending_items.
- Workspace apply/resume request actions that do not change `materializationStatus` or task lifecycle until refreshed backend data proves it.
- Review Center empty, pending, approved-not-applied, applied, failed materialization, expired, revoked.
- Tasks active, historical, stale, deletion preflight blocked, delete allowed.
- LifeModel empty, current compatibility view, canonical view, pending changes, materialized changes, manual override.
- Memory candidate, confirmed, used in LifeModel, withdrawn/expired, archive/restore blocked.
- Settings no provider, local-only, cloud route, unknown external transmission, developer-only hidden.
- Provider/privacy summary reused consistently across Workspace, Today, Settings, and external-write ReviewItems.

## ReviewItem Allowed-action Tests

`PHASE_2_REQUIRED`: Backend tests must prove:

- frontend cannot infer allowed actions from type/status alone;
- stale review items expose no approving action;
- safe mode disables risky review actions;
- external writes outside safe paths are blocked by backend item state;
- approved decision does not imply `applied`;
- `apply` ReviewAction means materialization request only, not applied state;
- `resume` ReviewAction means task-resume request only, not resumed lifecycle;
- `ReviewAction.kind = apply` cannot carry `decision_only`;
- `ReviewAction.kind = resume` cannot carry `decision_only`;
- decision actions cannot carry `materialization_request`;
- failed materialization remains visible and auditable.

## Stale / Error / Empty State Tests

`PHASE_2_REQUIRED`: Each ViewModel needs tests proving:

- `empty` is a successful backend state;
- `error` does not fall back to raw domain reads;
- `stale` disables risky product/review actions;
- warnings preserve evidence refs;
- last updated timestamps are rendered and testable.

## No Raw Domain Product Truth Reconstruction Tests

`PHASE_2_REQUIRED`: Add static scans or lint-style tests to block product pages from reconstructing:

- pending review counts from proposal lists when projection fields exist;
- readiness from diagnostics when projection/read model exists;
- safe mode from diagnostics when projection exists;
- task lifecycle from AgentRun plus task fragments when TasksViewModel exists;
- ReviewItem allowed actions from proposal type/local safe path checks;
- memory lane state from raw memory rows;
- LifeModel canonical/current state from mixed raw model/current/proposal reads;
- provider/privacy boundary from frontend-only runtime disclosure.

## Diagnostics Visibility Tests

`PHASE_2_REQUIRED`: Test that:

- blocker, failed, waiting-permission, safe-mode, pending-review, and provider/privacy boundary states are default-visible;
- reasoning trace, kernel events, durable events, raw transcript, provider health, PolicyRouter, ModelRouter, metrics, MCP/A2A internals, and `tauriDev` surfaces are not default product UI;
- advanced inspector actions are accessible only through approved `DebugAction` entries.

## Migration Guard Tests

`PHASE_2_REQUIRED`: Preserve Phase7 constraints:

- no restored old Stage/Beta/migration/cutover/productization command surfaces;
- no product page imports `frontend/src/tauriDev.ts`;
- no old reply-only chat wrapper in V2 Workspace product path;
- no hidden legacy fallback completion.

## Smoke / E2E Expectations

`UNKNOWN`: Browser smoke and desktop trial readiness remain separate from this docs-only package.

Before Phase 3 readiness claims:

- clarify or fix the `127.0.0.1:5173` browser smoke server issue;
- run a bounded browser smoke for the selected first vertical slice;
- run a desktop/Tauri product trial for no-silent-write, review, blocker, and stale states;
- keep external live-provider/Web/MCP readiness claims separate unless live evidence is collected.

## Suggested Gate Set For Phase 3 Implementation

`CANDIDATE`: Start with focused gates:

```sh
git diff --check
cargo test -p openlife-tauri single_system -- --nocapture
cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
```

Add contract-specific backend/frontend tests as each ViewModel owner is implemented.
