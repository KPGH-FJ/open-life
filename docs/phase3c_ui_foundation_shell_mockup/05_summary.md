# Phase 3C Static Mockup Summary

Status: third-pass UI foundation and workbench shell mockup complete.
Date: 2026-07-10.

## Changed Files

Phase 3C-only additions or updates:

- `01_source_map_and_design_baseline.md`
- `02_ui_foundation_and_static_mockup_spec.md`
- `static_mockup/index.html`
- `static_mockup/openlife-ui-foundation.css`
- `static_mockup/mockup-data.js`
- `static_mockup/mockup-app.js`
- `static_mockup/validate-fixtures.mjs`
- `04_visual_qa_report.md`
- `05_summary.md`
- `06_fixture_field_source_map.md`
- `07_component_state_and_interaction_matrix.md`
- `08_review_decision_projection_gap.md`
- `artifacts/phase3c_*.png`

All paths above are relative to
`docs/phase3c_ui_foundation_shell_mockup/`.

## Open Path

Open directly:

```text
/Users/tw/Desktop/open-life/docs/phase3c_ui_foundation_shell_mockup/static_mockup/index.html
```

## Third-Pass Workspace Refinement

- Replaced the Workspace metric/table/banner/action/section stack with a
  timeline-first task surface.
- Kept one compact completed event, one expanded permission interruption, and
  one pending next event.
- Removed decorative Workspace metrics and duplicate progress rows.
- Reduced the permission request in the main surface to purpose plus three
  bounded scope facts.
- Moved full target, transmission, duration, revocation, tool, capability, and
  policy details to the Inspector.
- Made the Workspace desktop Inspector on-demand; opening and closing it changes
  the shell between two and three columns with focus restoration.
- Kept the mobile permission decisions in the first viewport above the bottom
  navigation.
- Added validator guards that prevent Workspace metrics/duplicate sections from
  returning and require source-backed timeline and permission-summary fields.

## Earlier Second-Pass Changes

- Added `review-pending-decision`; Today now opens that state instead of an
  approved fixture.
- Added current-to-proposed comparison, reason, source, risk, impact, expiry,
  and four decision actions.
- Approval requires confirmation and ends at `已批准，尚未应用`.
- Added permission tool, target, data scope, transmission, duration, and
  revocation presentation.
- `仅允许本次` fails closed because the current backend cannot enforce it.
- Replaced developer self-test content with one continuous personal-work story.
- Removed Product/Review lane labels and engineering copy from product surfaces.
- Reordered Inspector content to outcome, risk, next action, detailed scope,
  references, limitations, then collapsed technical information.
- Reduced mobile primary navigation to 今日 / 工作区 / 审核中心 / LifeModel.
- Moved 设置 to utility navigation and replaced top-level 高级 with a support
  button that opens collapsed technical details.
- Increased body/caption sizes and mobile control dimensions.

## Validation

```sh
node --check docs/phase3c_ui_foundation_shell_mockup/static_mockup/mockup-data.js
node --check docs/phase3c_ui_foundation_shell_mockup/static_mockup/mockup-app.js
node docs/phase3c_ui_foundation_shell_mockup/static_mockup/validate-fixtures.mjs
git diff --check
```

Results:

- JavaScript syntax: pass.
- Fixture validator: pass for 7 states, 25 actions, 6 navigation entries.
- Browser matrix: 21/21 state and viewport combinations passed after the
  Workspace refinement.
- Interaction paths: pending review, confirmation/cancel, approved-not-applied,
  permission scope, Workspace Inspector open/close, explicit permission reject,
  unavailable task, utility navigation, focus trap, and focus restoration
  passed.
- Browser errors/warnings: none.
- Screenshot artifacts: 26 actual PNG files.

## Production Boundaries

This slice changed only
`docs/phase3c_ui_foundation_shell_mockup/`. It did not modify:

- `frontend/src/components/ProductShell.tsx`;
- `frontend/src/productShellContract.ts`;
- `frontend/src/App.tsx`;
- production routes;
- Rust, Tauri, or backend source.

The worktree contains unrelated production changes from other work. They were
left untouched and are not claimed by this slice.

## Safety Self-Review

- Unknown/stale/missing evidence remains fail closed.
- Viewing a suggestion is not an approval action.
- Approved, applying, applied, and completed remain distinct.
- Current approval does not claim `applying` because the backend advertises
  `unknown` and has no enabled application command.
- Fixture values do not claim backend truth.
- Product, Review, and Debug action contracts remain separate.
- Rich decision and permission values are explicitly `PROPOSED` projections.
- Workspace permission details remain fail closed even though the main surface
  is visually compact.

## Known Limitations

- This is not React and does not replace ProductShell.
- Rich approval/permission context is not yet owned by ReviewItem.
- No real dispatch, persistence, route, or provider transmission occurs.
- The task timeline is a static target layout, not a live runtime event stream.
- No full VoiceOver or automated WCAG suite was run.
- Inline SVGs must become `lucide-react` icons during React migration.

## Next Phase Recommendation

Do not port the approval UI to React yet. First implement and test the
backend-owned decision and permission projection described in
`08_review_decision_projection_gap.md`, including enforceable one-time grants
and explicit materialization transitions. After that contract is available,
port the accepted static patterns into a separately scoped React preview and
repeat keyboard, screen-reader, contrast, interaction, and mobile QA before any
ProductShell or route replacement.

The accepted Workspace pattern should remain timeline-first with an on-demand
Inspector; React migration should not restore decorative metrics or a permanent
three-pane permission dashboard.
