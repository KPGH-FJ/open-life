# Phase 3D/3E Visual Scheme Summary

Status: `HUMAN_APPROVED_VISUAL_BLUEPRINT`.
Date: 2026-07-18.

## Approved Direction

Recommended direction: `Codex / Cursor White Workbench`.

The system presents OpenLife with the established Codex/Cursor light-interface
grammar:

- white canvas and work surface;
- very light gray sidebar and selected rows;
- near-black text and primary actions;
- 1px neutral gray dividers;
- amber for waiting, stale, unknown, and protective safe mode;
- red only for concrete errors or blocked actions;
- 14-15px Chinese-first reading scale;
- fine dividers, low radius, minimal elevation;
- one main conclusion per page;
- evidence on demand and debug detail last.

## Complete Review Package

Primary review entry:

`docs/phase3e_product_blueprints/review/index.html`

Interactive blueprint:

`docs/phase3e_product_blueprints/prototype/index.html`

Supporting specifications:

- `docs/phase3d_visual_direction/`
- `docs/phase3e_product_blueprints/00_blueprint_scope_and_authority.md`
- `docs/phase3e_product_blueprints/01_design_system.md`
- `docs/phase3e_product_blueprints/02_screen_and_state_matrix.md`
- `docs/phase3e_product_blueprints/03_interaction_responsive_accessibility.md`
- `docs/phase3e_product_blueprints/04_field_source_map.md`
- `docs/phase3e_product_blueprints/05_visual_qa_report.md`

Screenshots:

`docs/phase3e_product_blueprints/artifacts/`

Figma supporting reference:

`https://www.figma.com/design/NncIE0ZWOaxAT9jYsFFKek`

The Figma import predates the white-baseline correction and is layout-only.
The review board, prototype, and regenerated screenshots own current colors,
fonts, lines, and component appearance.

## Covered Product States

1. Today ready with pending review.
2. Today stale/unknown fail-closed.
3. Workspace active task waiting for permission.
4. Tasks queue and continuity detail.
5. Review pending decision.
6. Review approved but not applied.
7. LifeModel limited current compatibility.
8. Settings provider/privacy unknown or possible external transmission.

## Preserved Safety Semantics

- Unknown provider/privacy never becomes a green local status.
- Stale, missing, or unproven state remains fail closed.
- viewing a review item does not approve it.
- approval is distinct from applying and completing.
- only refreshed backend read-model proof may show `已应用`.
- fixtures remain visibly classified outside the product shell.
- product, review, and debug action contracts stay distinct in data even when
  ordinary users see plain Chinese labels.

## Production Boundary

This pass did not modify:

- `frontend/src/components/ProductShell.tsx`;
- `frontend/src/productShellContract.ts`;
- `frontend/src/App.tsx`;
- production routes or navigation;
- Rust/Tauri/backend code.

Any unrelated concurrent production-source changes remain outside this visual
design pass and are not reverted, staged, or modified here.

## Next Gate

React work remains blocked:

```text
VISUAL_DIRECTION_SELECTED = YES
KEY_SCREEN_BLUEPRINTS_COMPLETE = YES
DESIGN_TOKENS_FROZEN = YES
MOBILE_BLUEPRINTS_COMPLETE = YES
CRITICAL_STATE_DESIGNS_COMPLETE = YES
REACT_PORT_READY = NO
```

Human approval was recorded on 2026-07-18. The next allowed work is the Pre-4A
mainline convergence gate, followed by contract closure from a reverified
`origin/main`. Production implementation must not start directly from this
package or the current convergence branch.
