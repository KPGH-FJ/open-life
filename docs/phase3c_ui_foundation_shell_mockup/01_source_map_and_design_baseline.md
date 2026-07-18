# Phase 3C Source Map And Design Baseline

Status: source-backed baseline for Phase 3C.

This file records the current state that the implementation Agent must preserve
or consciously use as input.

## Current Product Shell Source Map

Verified files:

- `frontend/src/App.tsx`
- `frontend/src/components/ProductShell.tsx`
- `frontend/src/productShellContract.ts`
- `frontend/src/index.css`
- `frontend/tailwind.config.js`
- `frontend/package.json`

Current route facts:

- `App.tsx` wraps product routes in `ProductShell`.
- `/today-v2-preview` is registered as an unlisted preview route.
- `ProductShell` reads route groups from `productShellContract.ts`.
- Primary route labels are:
  `Today`, `Companion`, `Mailbox`, `Life Model`, `Runs`, `Settings`.
- Secondary routes are:
  `Life Model Build`, `Memory`.
- Advanced route groups are:
  `Advanced connections` and `Maintenance`.

Current shell facts:

- `ProductShell` renders a top header with centered tab navigation.
- `MainTabs` uses a six-column tab group on larger viewports and a three-column
  grid on small viewports.
- `SecondaryToolsMenu` exposes technical surfaces from a top-right `Advanced`
  button.
- Safe mode and usage-readiness banners are global shell banners.
- The shell background is hardcoded to `#f5f6f2`; header background is
  hardcoded to `#fcfcf8`.

Current style facts:

- `frontend/src/index.css` only imports Tailwind and applies body
  `bg-gray-50 text-gray-900`.
- `frontend/tailwind.config.js` has an empty `theme.extend`.
- The frontend uses Tailwind 3.4, React 18, React Router 6, Vite, Vitest,
  Playwright, and `lucide-react`.

## Current Read-Model Baseline Relevant To UI Foundation

Backend-owned or backend-backed contracts now exist for:

- `ViewModelEnvelope<T>`
- `ReviewCenterViewModel`
- `LifeModelViewModel`
- `TasksViewModel`
- limited `WorkspaceViewModel`
- `MemoryViewModel`
- `ProviderPrivacyBoundarySummary`

Important limitations:

- `WorkspaceViewModel` is explicitly a limited baseline and does not replace
  Frontend V2.
- Today remains projection-backed/limited. The Today V2 preview builds a
  frontend `TodayViewModelEnvelope` from existing sources; it is not a backend
  TodayViewModel owner.
- Settings remains a mixed settings/support page, even though Memory and
  provider/privacy product truth now come from backend read models.

Phase 3C may use these contracts as semantic inspiration for fixture data, but
must not claim the fixture values are live product proof.

## Existing UI Foundation Input

`docs/phase1_ux_ia/11_ui_foundation_study.md` already established the
direction:

- OpenLife should feel like a local-first AI workbench.
- Long-term desktop navigation should use a left sidebar/rail, not top tabs.
- Main product state and evidence/debug detail should be separated.
- Typography should be compact and fixed-size.
- Tokenized spacing, radius, color, and states should precede page rewrites.
- Visual validation should use real browser screenshots, not image generation.

Phase 3C converts that study into a deliverable static shell mockup.

## External Reference Refresh

Checked on 2026-07-10:

- OpenAI Codex app docs list app work surfaces such as Review, Automations,
  Worktrees, Local Environments, In-app browser, Computer Use, Appshots, and
  Commands.
- Cursor's visual editor article emphasizes direct rendered-UI inspection,
  component state controls, layout manipulation, typography, color tokens, and
  design-system-backed visual controls.
- Cursor 3.0 describes an agent-centered interface with task/agent focus, while
  Cursor 3.4 documents full-screen focus and configurable compact tool-call
  density.

Cursor references:

- https://cursor.com/changelog/3-0
- https://cursor.com/changelog/page/4

OpenLife translation:

- learn the workbench density and left-sidebar work organization pattern;
- learn the discipline of exposing component states and token controls;
- keep the current execution thread primary, compact inactive tool/event
  details, and open evidence in a focused pane only when needed;
- make OpenLife quieter than a coding IDE because privacy and permission
  decisions must be understandable to non-developer users;
- do not copy their brand, exact layout, iconography, copy, or product model.

## Design Problems Phase 3C Should Address

1. Navigation hierarchy:
   Current top tabs make OpenLife feel like a small web app. The mockup should
   test a left sidebar with primary product areas and an advanced/developer
   zone below.

2. Typography:
   Current pages mix `text-xs`, `text-sm`, `text-lg`, and `text-xl` locally.
   The mockup should prove fixed-size type with a compact hierarchy.

3. Spacing:
   Current cards often repeat page-local padding and gap choices. The mockup
   should use a 4px grid and stable panel dimensions.

4. Color:
   Current neutral colors are close but not tokenized. The mockup should use a
   restrained neutral base plus semantic color roles.

5. State language:
   Unknown, stale, waiting permission, needs review, blocked, and error states
   must not collapse into success or completion.

6. Evidence:
   Evidence should be accessible but not dominate the primary task surface.

## Anti-Hallucination Checklist For The Agent

Before claiming completion, answer these with source-backed evidence:

- Did any production ProductShell or route contract file change?
- Did any backend Rust/Tauri file change?
- Did any mockup fixture state get described as live backend truth?
- Did the mockup show `completed` or `ready` without a fixture label?
- Did the mockup preserve fail-closed vocabulary for unknown/stale/error data?
- Did screenshots cover desktop and narrow viewports?
- Did `git diff --check` pass?
