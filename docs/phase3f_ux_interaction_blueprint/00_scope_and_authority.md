# Phase 3F UX Interaction Blueprint

Status: `HUMAN_APPROVED_INTERACTION_BASELINE`
Date: 2026-07-18
Scope: backend capability map refresh, UX interaction authority, and standalone
interactive prototype. This phase does not implement the production frontend.

## 1. Purpose

Phase 3D selected the visual direction. Phase 3E turned that direction into
screen blueprints. Phase 3F now freezes the interaction grammar that a later
React refactor may implement.

The sequence is deliberate:

1. refresh the backend capability map after the roadshow backend work;
2. separate verified backend facts from product projection gaps;
3. define navigation, settings, review, permission, evidence, and recovery
   behavior;
4. prove the behavior in a standalone HTML/CSS/JavaScript prototype;
5. stop for human review before any ProductShell, route, or production page
   migration.

## 2. Authority Order

This package is subordinate to:

1. `AGENTS.md`;
2. `plans/README.md`;
3. `plans/openlife_single_system_deletion_manifest.md`;
4. `plans/openlife_single_system_development_preparation.md`;
5. `docs/openlife_frontend_refactor_readiness_report.md`;
6. current Rust/Tauri/product-bridge source;
7. Phase 3D selected visual direction and Phase 3E blueprint package.

When an older UX document conflicts with current code, this package records the
conflict and follows the current source. It does not silently upgrade the older
claim.

## 3. Truth Labels

Every Phase 3F field or interaction uses one of these labels:

| Label | Meaning |
|---|---|
| `VERIFIED_BACKEND` | Current shipped backend source and bounded verification support the capability. |
| `PRODUCT_READ_MODEL` | A typed backend product read model owns presentation truth. |
| `PRODUCT_BRIDGE` | A shipped typed Tauri bridge exists, but a dedicated V2 read model may not. |
| `TARGET_CONTRACT` | Needed for the intended UX but not currently projected by the backend. |
| `LAYOUT_FIXTURE` | Purely fictional content used to test hierarchy and density. |
| `DEV_ONLY` | Compiled or exposed only under development extensions; never a product nav promise. |
| `UNVERIFIED` | No current evidence is sufficient; UI must remain unknown or unavailable. |

The QA toolbar is outside the product shell and identifies every scenario as a
fixture. A scenario selector is not a user-selectable product mode.

## 4. Included Work

- refreshed backend capability and frontend-consumption map;
- industry pattern study grounded in official Codex/Cursor material and the
  user-supplied Cursor settings reference;
- final candidate primary navigation and utility navigation;
- dedicated settings information architecture and interaction model;
- Workspace task, resource, Web evidence, permission, cancellation, and resume
  behavior;
- Review decision, edit, defer, approval, application, and materialization
  behavior;
- attachment/citation, StateStore Today, LifeModel, and provider/privacy rules;
- component state matrix, keyboard order, focus restoration, live-region rules,
  desktop/mobile behavior, and contrast target;
- standalone interactive prototype and automated browser checks;
- explicit hallucination and contract-gap audit.

## 5. Excluded Work

Phase 3F must not:

- modify `frontend/src/components/ProductShell.tsx`;
- modify `frontend/src/productShellContract.ts`;
- modify `frontend/src/App.tsx`;
- add, replace, or redirect a production route;
- change Rust/Tauri/backend behavior;
- add the prototype to production navigation;
- describe `WorkspaceViewModel` as a complete Frontend V2 contract;
- expose MCP, A2A, plugins, scheduler, vector internals, or raw traces as primary
  product navigation;
- treat approval as application, a command dispatch as completion, or a
  connection test as a persisted provider route;
- claim global roadshow, Phase7, native trial, signing, or external-provider
  readiness from the bounded backend freeze.

## 6. Required Safety Invariants

1. Unknown or incoherent provider/privacy state never renders a green local
   claim.
2. Stale, missing, error, remote-unknown, or evidence-incomplete state remains
   fail closed.
3. A permission approval grants only the exact backend-projected action scope;
   the frontend never derives or broadens scope.
4. Approval and task resume are separate dispatches separated by a refreshed
   read-model check.
5. A proposal can be pending, approved, applying, applied, failed, or rolled
   back. These states are never collapsed into `completed`.
6. Product, Review, Task Control, and Debug actions remain distinct contracts.
7. Fixture values never become backend readiness evidence.
8. The UI does not infer product truth from raw config, diagnostics, proposal
   fragments, or assistant prose when a backend read model exists.

## 7. Phase Gate

```text
VISUAL_DIRECTION_SELECTED = YES
DESIGN_TOKENS_FROZEN = YES
BACKEND_CAPABILITY_MAP_REFRESHED = PRE_MERGE_REVIEW_CANDIDATE
UX_INTERACTION_SPEC_COMPLETE = YES
CRITICAL_FLOW_PROTOTYPE_VALIDATED = YES
ACTION_CONTRACT_MAPPING_COMPLETE = PARTIAL_BLOCKED
REACT_PORT_READY = NO
```

`ACTION_CONTRACT_MAPPING_COMPLETE` remains blocked by named projection gaps,
especially user-readable Review content and exact permission context. The
prototype demonstrates the intended shape without pretending those gaps are
closed. `REVIEW_CANDIDATE` means automated and visual QA passed for this static
prototype; it does not replace product-owner review or authorize a React port.

Human approval was recorded on 2026-07-18. It approves the visual and
interaction direction only. The backend map must be rerun against merged,
CI-green, reverified `origin/main`, and the named contract gaps remain blocking.
