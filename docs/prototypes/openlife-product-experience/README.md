# OpenLife Product Experience Prototype

This is the zero-subscription Gate B design artifact for OpenLife. It is a
standalone, deterministic HTML/CSS/JavaScript prototype. It does not connect to
Tauri IPC, SQLite, credentials, providers, local files, or external services.

## Preview

From the repository root:

```sh
python3 -m http.server 4173
```

Then open:

`http://127.0.0.1:4173/docs/prototypes/openlife-product-experience/`

## Boundary

- All content and state are fixtures for design review.
- The prototype imports the production OpenLife CSS token file directly.
- No prototype success state is runtime, provider, filesystem, native, or
  formal-release evidence.
- Production UI and runtime remain frozen until Gate C acceptance.

## Review

The product surface intentionally hides prototype-only controls. Press
`Alt+P` to open the review drawer and walk all 12 journeys. For each journey,
review the start, active or decision, and result or recovery states. Also
review at 1440×900, 1024×768, and browser zoom 200%.

The default UI follows progressive disclosure: Projects and Recents live in a
quiet left sidebar; the conversation and composer remain primary; progress is
collapsed to the current step; diff, review, sources, and technical details
open only when requested.

## Verified on 2026-08-24

- All 12 journeys and all 36 deterministic states rendered and advanced.
- The composer model control opens a working Profile/model/reasoning picker;
  incompatible unverified profiles remain unavailable.
- Opening a folder covers available, permission-required, missing, and system
  picker handoff states without reading a real directory.
- Project edits require line-by-line review of every selected file before the
  apply action becomes available.
- Project, conversation, and New Chat active states stay synchronized with the
  selected journey.
- 1440×900 desktop shell, 1024×768 overlay behavior, and 720×450 narrow layout
  were visually inspected. The 720×450 check is the CSS-layout equivalent of a
  1440×900 viewport at 200%; native app zoom remains a later release gate.
- No page console warnings or errors were present.
- Keyboard state advance and Escape panel dismissal worked.
- Document-level horizontal and vertical overflow checks passed after fixing
  minimum-height containment for the workspace and sidebar.
