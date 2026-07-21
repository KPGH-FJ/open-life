# Phase 4E Execution Baseline

Status: `READY_FOR_HUMAN_REVIEW`
Date: 2026-07-21

## Authority And Starting Point

Phase 4E started from verified `main` commit
`820ce45b6c87925dcb435347c27d28c6c8e0d8b6`, the merge of Phase 4D PR #62.
The authority order for this slice is:

1. `AGENTS.md`;
2. `plans/README.md`;
3. `plans/openlife_single_system_deletion_manifest.md`;
4. `plans/openlife_single_system_development_preparation.md`;
5. accepted Phase 3F and Phase 4A-D contracts.

## Scope

This slice performs one desktop production frontend authority switch:

- compose the accepted white Workbench Shell from `frontend/src/App.tsx`;
- expose only the canonical desktop routes;
- connect the Phase 4D journeys to their real Tauri data sources;
- complete the bounded Workspace conversation, task-control, and first-build
  proposal surfaces needed by those journeys;
- delete the old page, component, shell, route, and local-truth owners in the
  same change;
- invert source, inventory, Rust, route, and bundle guards so deletion remains
  executable evidence.

This slice does not change Rust/Tauri business authority, provider policy,
durable-write policy, or backend command behavior. It does not add a mobile
product surface. The earlier mobile drawings remain design reference only and
are not an implementation or acceptance target.

## Truth Boundary

- Product state is read from backend projections and ViewModels where they
  exist. Missing, stale, incoherent, or failed reads remain unknown/error.
- A command callback only starts a refresh. It is not completion proof.
- Review approval is not application, materialization, or task completion.
- Provider/privacy status comes from `ProviderPrivacyBoundarySummary`; an
  unknown boundary cannot render a green/local conclusion.
- Builder creates review proposals only. It never writes LifeModel truth
  directly.

## Entry And Evidence Boundary

- Production source entry: `frontend/src/main.tsx` -> `frontend/src/App.tsx`.
- Browser review entry: `http://127.0.0.1:4187/#/today`.
- Browser QA has no Tauri IPC and earns no backend-truth credit.
- A production `tauri dev` launch uses a fresh isolated
  `OPENLIFE_DATA_DIR`; it is read-only for this slice.
- Phase 4F remains the owner of complete real-action dogfood, assistive
  technology acceptance, and the final product trial.

`PHASE4E_DESKTOP_ONLY=YES`

`BACKEND_BUSINESS_AUTHORITY_CHANGED=NO`

`PRODUCTION_FALLBACK_TO_OLD_UI=NO`
