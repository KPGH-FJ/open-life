# Phase 3F Summary And Phase 4A Gate

Status: `PRE4A_GATE_COMPLETE_AWAITING_START_DECISION`
Date: 2026-07-19

## 1. Review Entry

Open the standalone prototype:

`docs/phase3f_ux_interaction_blueprint/prototype/index.html`

Start with these scenarios in the external QA selector:

1. Workspace: exact one-time permission known;
2. Workspace: permission context unknown;
3. Workspace: attachments and governed Web evidence;
4. Review Center: pending decision;
5. Review Center: approved, not applied;
6. Settings: provider and privacy boundary unknown;
7. Today: stale/unknown fail closed.

The selector is outside the product shell and is never proposed as a product
mode switch.

## 2. Approved Design Decisions

- Visual language: Codex/Cursor white workbench, neutral lines, minimal radius,
  one dark primary action, amber for waiting/protection, red for concrete error.
- Primary navigation: Today, Workspace, Tasks when mature, Review Center, and
  LifeModel. Settings/support are utility destinations; Advanced is not a
  primary product destination.
- Information order: current goal, blocker/risk, next action, evidence, debug.
- Workspace: execution narrative and composer, not a dashboard grid.
- Settings: dedicated category navigation and one-column forms; test, save, and
  refreshed privacy truth are separate transitions.
- Mobile: compact app bar, bottom primary navigation, drawer for utilities, and
  bottom-sheet evidence.
- Review: pending decision, approved, applying, applied, failed, and rolled back
  remain distinct.
- Permission: grant exact backend-projected scope, refresh, verify resumability,
  then resume. Navigation or approval alone never signals success.

## 3. Backend Map Outcome

The old capability map was reconciled against current shipped handlers, typed
bridges, read models, backend owners, roadshow state, the bounded freeze tag,
and current source snapshot `e1b43161f78a`.

The refreshed map adds the frontend consequences of canonical resources and
citations, governed Web evidence, canonical StateStore daily tasks, reviewed
artifacts, exact action-bound one-time permission, provider transmission
receipts, memory commit/undo, task recovery, and release dev-extension
quarantine. It also records that post-freeze portability hardening did not add
new frontend-ready product capabilities.

## 4. Open Contract Blockers

Before rich production screens are migrated, backend/product-contract work must
resolve or explicitly exclude:

1. readable Review before/after, reason, impact, affected objects, and expiry;
2. readable exact permission scope, target, capability, input digest,
   transmission boundary, and blocked action;
3. a reviewed Workspace composition contract rather than unrestricted page
   joins over separate authorities;
4. a bounded Today adapter contract or a backend Today read model;
5. tested Settings orchestration across config, connection test, privacy
   summary, permissions, and recovery.

## 5. Human Review Questions

Approve or revise these points before Phase 4A:

- Does Workspace now feel calm enough for sustained task execution?
- Is the Review current-to-suggested comparison sufficient for a safe decision?
- Is `仅允许本次并继续` understandable with the displayed scope?
- Should Tasks remain primary navigation at launch or stay under Workspace until
  its production contract matures?
- Is Settings navigation sufficiently close to the selected Cursor/Codex
  baseline while remaining OpenLife-specific?
- Are evidence and privacy details discoverable without dominating the product
  surface?

## 6. Phase 4A Entry Gate

```text
PHASE3F_HUMAN_APPROVAL = YES
VISUAL_AND_INTERACTION_DIRECTION = YES
BACKEND_CAPABILITY_MAP = MERGED_CONVERGENCE_SOURCE_REVIEWED
STATIC_PROTOTYPE_QA = PASS
RICH_REVIEW_CONTRACT = PHASE4A_TECHNICAL_PASS
EXACT_PERMISSION_PRESENTATION_CONTRACT = PHASE4A_TECHNICAL_PASS
CONVERGENCE_MERGED_TO_MAIN = YES
MERGED_MAIN_PUSH_CI = PASS_AT_974b416
MAIN_REVERIFIED = YES_AT_974b416_CI_AND_TARGETED_TESTS
CURRENT_MAIN_CI_STABILITY = PASS
MAINLINE_TIMEZONE_FIX_PR = MERGED
MAINLINE_COVERAGE_TIMING_FIX_PR = MERGED
FRONTEND_REFACTOR_READY = YES
DESIGN_AUTHORITY_COMMITTED = YES
DESIGN_AUTHORITY_MERGED_TO_MAIN = YES_AT_8b3e493
PHASE4A_BRANCH_CREATED_FROM_VERIFIED_MAIN = YES_AT_1267EE4
PHASE4A_START_DECISION = APPROVED_2026_07_19
PRODUCTION_CONTRACT_READ_MODEL_SOURCE_MODIFIED = YES_PHASE4A
PRODUCTION_PAGE_ROUTE_SHELL_MODIFIED = NO
PRODUCTION_BUSINESS_COMMAND_AUTHORITY_MODIFIED = NO
TEST_ONLY_RUST_SOURCE_MODIFIED = YES_FOR_CI_DETERMINISM
REACT_PORT_READY = YES_FOR_PHASE4B_FOUNDATION_NOT_PAGE_MIGRATION
```

Approval was recorded on 2026-07-18. Convergence was merged through PR #50; its
main push CI passed and exact `origin/main` was independently reverified.
Publishing the design authority later exposed a pre-existing UTC-runner defect
in two transient-state test clocks. The isolated fix in PR #52 has now been
merged as `a58f4e2`; protected-main CI and targeted local UTC regression checks
passed. The refreshed design authority then passed its full PR matrix and was
merged through PR #51 as `8b3e493`; the resulting protected-main CI passed and
its file tree is identical to the locally verified candidate. A coverage-only
wall-clock assertion then recurred while validating this status correction, and
a second coverage-sensitive observation test exposed the same class of problem.
PR #55 replaced those test-only sub-second assumptions with bounded watchdogs
while preserving the product timeout and scheduler assertions. It merged as
`974b416`; its pull-request matrix and protected-main CI both passed, including
Rust Coverage and Smoke. The user approved Phase 4A Contract Closure on
2026-07-19, and `codex/phase4a-contract-closure` was created from verified
`origin/main` at `1267ee4` after protected-main CI run `29661506060` passed.
Phase 4A now owns the rich Review and exact Permission presentation contract
work. This update does not authorize React page migration or a production route
switch. Phase 4A subsequently passed its complete technical gate set; Phase 4B
still requires separate human approval.
