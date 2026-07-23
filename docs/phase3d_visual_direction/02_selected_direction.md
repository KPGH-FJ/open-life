# Selected Direction: Codex / Cursor White Workbench

Status: `SELECTED_AND_HUMAN_APPROVED`.
Date: 2026-07-18.

## Product Expression

OpenLife uses the familiar white workbench language already proven by Codex
and Cursor: white content, a light gray navigation plane, black/gray type,
thin neutral borders, and black primary actions.

The visual system should communicate:

- the interface is a tool, not a branded moodboard;
- content and current work are more important than decoration;
- local-first does not mean assumed-local;
- evidence is available without occupying the work surface;
- waiting and safe mode are protection, not generic errors;
- approval is a recorded decision, not a finished write.

## Visual Rules

1. White `#FFFFFF` owns the canvas and main surface.
2. Light gray `#F5F5F5` owns the sidebar and stable utility planes.
3. Near-black `#111111` owns primary text and primary actions.
4. Gray `#F2F2F2` owns selection and disabled/quiet controls.
5. Structural borders use `#E6E6E6`; stronger borders use `#D4D4D4`.
6. The UI font is the native system sans stack with PingFang SC fallback.
7. Normal hierarchy uses weights 400, 500, and 600.
8. Panels do not float by default; only dialogs, sheets, drawers, and the
   Workspace composer receive restrained shadow.
9. Navigation selection is a gray row, not a colored indicator.
10. Amber, red, green, and focus blue remain bounded semantic exceptions.

## Product Patterns

- compact sidebar and top context bar;
- large unframed work surface;
- one primary conclusion per page;
- timeline-first Workspace and Tasks detail;
- diff-first Review Center with a fixed decision bar;
- readable LifeModel content with provenance;
- on-demand Inspector with raw ids and debug fields last;
- separate mobile app bar, drawer, bottom navigation, and evidence sheet.

## Deliberate Differences From Phase 3C

- removes repeated metric clusters;
- closes Inspector by default;
- replaces the persistent three-column density with focused work plus
  on-demand evidence;
- adds a composer/control surface to Workspace;
- gives Review a queue/detail structure and persistent decisions;
- uses a separate mobile composition rather than stacking the desktop shell;
- replaces all custom product-wide colors with the Codex/Cursor neutral light
  baseline.

## Review Authority And Figma Reference

Primary review artifacts:

- `docs/phase3e_product_blueprints/review/index.html`
- `docs/phase3e_product_blueprints/prototype/index.html`

Editable Figma reference:

`https://www.figma.com/design/NncIE0ZWOaxAT9jYsFFKek`

The current Figma file predates this palette correction and remains only a
supporting layout reference. The Starter-plan MCP quota prevents updating its
full component library in this pass. The repository review board, prototype,
and regenerated screenshots are the current visual review authority.

Human approval was recorded on 2026-07-18. The repository prototype and
screenshots now own the approved visual baseline; the older Figma import remains
a layout-only reference. Approval does not imply backend readiness or authorize
production React migration before the Phase 3F mainline and contract gates.
