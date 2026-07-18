# Phase 3F Summary And Phase 4A Gate

Status: `HUMAN_APPROVED_PRE_4A_BLOCKED`
Date: 2026-07-18

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
BACKEND_CAPABILITY_MAP = PRE_MERGE_REVIEW_CANDIDATE
STATIC_PROTOTYPE_QA = PASS
RICH_REVIEW_CONTRACT = BLOCKED
EXACT_PERMISSION_PRESENTATION_CONTRACT = BLOCKED
CONVERGENCE_MERGED_TO_MAIN = NO
MAIN_CI_GREEN = UNKNOWN
MAIN_REVERIFIED = NO
PHASE4A_BRANCH_CREATED_FROM_VERIFIED_MAIN = NO
PRODUCTION_SOURCE_MODIFIED = NO
REACT_PORT_READY = NO
```

Approval was recorded on 2026-07-18. The next allowed slice is the Pre-4A
convergence gate. It must merge convergence and the approved design authority
through protected main, pass remote CI, fetch the resulting `origin/main`, and
reverify it before any Phase 4A branch or contract implementation begins.
