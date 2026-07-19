# Phase 4A Contract Closure Execution Baseline

Status: `TECHNICAL_EXIT_PASS_PENDING_HUMAN_REVIEW`
Date: 2026-07-19

## 1. Authority And Branch

Phase 4A started only after the user approved proceeding from Phase 3F.

```text
BASE_BRANCH = origin/main
BASE_COMMIT = 1267ee40dbd49ca52f7bd286ba64dbc4f8c98164
BASE_MAIN_CI = PASS_RUN_29661506060
WORK_BRANCH = codex/phase4a-contract-closure
BACKEND_CAPABILITY_INPUT = merged roadshow/convergence mainline
BACKEND_REMEDIATION_V4 = EXCLUDED_PAUSED_BACKLOG
```

The base CI evidence is the protected-main push run at:
`https://github.com/KPGH-FJ/open-life/actions/runs/29661506060`.

## 2. Allowed Scope

Phase 4A may:

- add read-only product projections over existing backend authority;
- strengthen serialized DTO and Action Contract invariants;
- add Rust/TypeScript parity and state-machine tests;
- freeze Today and Settings composition boundaries;
- replace the limited Workspace composition in its existing owner;
- create the migration/deletion ledger before any V2 frontend owner;
- document field owners, refresh order, limitations, and exit gates.

## 3. Explicit Non-Goals

Phase 4A does not:

- replace `ProductShell`;
- change `App.tsx`, production routes, or primary navigation;
- migrate Today, Workspace, Tasks, Review, LifeModel, or Settings pages;
- add a V2 route or long-lived dual frontend;
- change ReviewWorkflow authorization, ToolPermission consumption, durable-write
  policy, or materialization authority;
- import Backend Remediation v4 work or claim its findings closed;
- treat a dispatched action as completion;
- call approved-but-not-materialized `applied` or `completed`.

## 4. Implementation Slices

1. Rich Review decision projection from `AgentProposal` into `ReviewItem`.
2. Exact permission projection for both real scope families:
   `action_bound` and `network_policy`.
3. ReviewAction invariant closure and dispatch-then-refresh orchestration.
4. Workspace composition in the existing backend owner.
5. Today strict-adapter and Settings orchestration freeze.
6. Cross-language golden fixture, absence guards, and deletion ledger.

## 5. Production Surface Boundary

No production React page, shell, route, or backend command handler is added in
this phase. `frontend/src/tauri.ts` changes only mirror serialized backend
contracts. The new frontend reducer modules are not imported by `App.tsx`,
`ProductShell`, product pages, or product components.

The pre-existing `/today-v2-preview` route is still compiled by `App.tsx`.
Phase 4A does not claim that this is a dev-only harness. Moving or deleting it
is an explicit Phase 4B ledger item with a release-bundle absence guard.
