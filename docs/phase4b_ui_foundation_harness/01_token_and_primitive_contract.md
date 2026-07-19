# Phase 4B Token And Primitive Contract

Status: `TECHNICAL_EXIT_PASS`
Date: 2026-07-19

## 1. Single Token Authority

`frontend/src/ui/foundation/openlife.tokens.css` is the Phase 4B visual token
authority. React components and semantic Tailwind aliases consume these CSS
variables; neither surface owns a second palette or spacing scale.

| Group | Contract |
| --- | --- |
| Typography | caption 12px, body 14px, reading 15px, surface 20px, display 24px |
| Spacing | fixed 4, 8, 12, 16, 20, 24, 32, 40, and 48px steps |
| Radius | 4, 6, and 8px only |
| Neutral | white canvas, #f5f5f5 sidebar, #111111 ink, #e6e6e6 line |
| Protection | amber for waiting, stale, unknown, blocked, and Safe Mode |
| Error | red only for a concrete error or failed/blocked action |
| Verified | green only when the caller supplies explicit verification |
| Focus | 2px #2563eb visible focus ring with 2px offset |
| Controls | 36px desktop; 44px mobile targets, including toggle hit targets |

The system font stack follows the approved Codex/Cursor white-workbench
direction and retains fixed sizes and zero letter spacing.

## 2. Primitive Owner

The new owner is `frontend/src/ui/foundation/`:

- action and icon buttons;
- status labels and protection/error notices;
- text fields and three-state toggles;
- navigation and EvidenceRef-like rows with `id`, `label`, `source`, and
  `sensitivity`;
- modal dialog with focus containment and restoration;
- polite live-region feedback.

This owner does not replace the old production `ProductPrimitives.tsx` yet.
Production callers remain listed in the migration/deletion ledger and must move
journey by journey before the old owner can become delete-ready.

## 3. Component State Matrix

| Component | Default | Hover/focus | Disabled/loading | Stale/blocked/unknown |
| --- | --- | --- | --- | --- |
| Action button | primary, secondary, quiet, danger | semantic hover plus visible focus ring | disabled reason required; loading uses busy state | caller supplies a fail-closed reason |
| Status label | neutral | focus not applicable | not interactive | amber for stale, waiting, blocked, unknown; green requires `verified=true` |
| Notice | neutral/protection/error | not interactive | not applicable | protection is amber; concrete error is red |
| Text field | label plus optional description | border and focus ring | native disabled styling | error has `aria-invalid` and linked text |
| Toggle | on/off switch | 44px focus target | disabled reason required | unknown is status text, never a false off switch |
| Nav/evidence row | button with visible result | hover and focus ring | unavailable navigation reports feedback | current navigation uses `aria-current=page` |
| Dialog | closed/open | focus moves in and is trapped | busy blocks close with reason | Escape closes only when not busy; trigger focus is restored |

## 4. Runtime Invariants

- A disabled OpenLife control without a visible `disabledReason` throws.
- A success status without `verified=true` throws.
- Unknown toggle truth renders no switch and cannot imply `off` or local-only.
- Dialog background becomes inert and is removed from the accessibility tree.
- Dialog focus/inert lifecycle follows `open`; busy-state or callback rerenders
  do not reset focus, while Escape always reads the latest busy state.
- All enabled harness controls produce a visible status result.
- Approval feedback says `approved, not applied`; only refreshed backend truth
  may later present applied/completed.

These guards are foundation constraints, not claims that current production
pages already use them.

## 5. Tailwind Boundary

`frontend/tailwind.config.js` exposes semantic `ol-*` aliases backed by CSS
variables. Phase 4B source tests reject arbitrary color/spacing classes and
letter-spacing utilities in the new foundation scope. Existing production
Tailwind styles are unchanged and remain migration work for later phases.
