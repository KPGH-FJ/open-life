# Phase 3D Controlled Visual Direction Study

Status: `UPDATED_AFTER_HUMAN_VISUAL_REVIEW`.
Date: 2026-07-18.

## Initial Comparison

The first review package compared three custom directions over the same Today
fixture, hierarchy, shell geometry, and semantic state:

| Direction | Character | Human review result |
| --- | --- | --- |
| Focus Studio | dark, technical, execution-first | rejected |
| Quiet Ledger | pale mineral canvas, pine accent | rejected: palette, font, and lines felt invented |
| Personal Archive | warm, editorial, reflective | rejected |

The information architecture, responsive behavior, evidence model, and safety
semantics were not rejected. The custom visual language was rejected.

## Human Direction Decision

The selected reference is now the shared visual baseline of current Codex and
Cursor light interfaces. OpenLife should not create another branded palette
before the frontend rewrite.

Reference observations:

- current local Cursor Agents window: white main work area, light gray sidebar,
  gray selected row, black primary send action, thin neutral dividers, system
  sans typography;
- official Codex product surfaces: white content planes, black/gray type,
  black primary controls, sparse borders, and restrained floating elevation.

Primary references:

- `https://openai.com/codex/`
- `https://openai.com/index/introducing-the-codex-app/`
- `https://cursor.com/download`

## Selected Direction: Codex / Cursor White Workbench

Common visual rules:

1. The main work surface is white.
2. The sidebar is a very light neutral gray, not a tinted brand color.
3. Primary text and actions are near-black.
4. Selection uses a light gray row; no colored navigation rail.
5. Structure uses 1px neutral gray lines and spacing.
6. The font is the operating-system UI sans stack with Chinese fallback.
7. Font weights stay primarily at 400, 500, and 600.
8. Radius is restrained; shadow is reserved for floating composers, dialogs,
   drawers, and sheets.
9. Evidence is neutral by default rather than blue.
10. Amber, red, and green appear only when product state needs them.

## OpenLife Ownership

OpenLife copies the mature visual grammar, not Codex or Cursor branding. It
keeps its own:

- Today, Workspace, Tasks, Review Center, LifeModel, and Settings IA;
- provider/privacy boundary semantics;
- evidence and limitations structure;
- fail-closed stale/unknown behavior;
- approval versus application lifecycle;
- Chinese-first product language.

No Codex/Cursor logos, brand marks, proprietary assets, or product copy are
included.

## Recommendation

Use `Codex / Cursor White Workbench` as the only Phase 3E visual candidate.
Do not reintroduce pine, blue-green, beige, purple, or other product-wide
accent palettes during the React port. A future brand pass may revisit identity
after the frontend workflow is stable, but it must not change the approved
information hierarchy or safety semantics.
