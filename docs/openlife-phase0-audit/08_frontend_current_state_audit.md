# Frontend Current State Audit

## Product Shell

Finding: The frontend has a real product shell, not only prototype pages.

Evidence:

- `App.tsx` lazy-loads primary and secondary routes.
- `ProductShell` provides primary navigation, advanced technical navigation,
  safe mode banner, and usage readiness banner.
- Primary routes include Today, Companion, Life Model, Mailbox, Runs, and
  Settings.

File location:

- `frontend/src/App.tsx`
- `frontend/src/components/ProductShell.tsx`
- `frontend/src/productShellContract.ts`

Confidence: High.

Impact: V2 should preserve the product-route map while simplifying workflow
hierarchy and state ownership.

## Pages

Finding: The frontend has many implemented operational surfaces.

Evidence:

- Pages include Today, Companion/Chat, LifeModel, Mailbox, Runs, AgentRunDetail,
  Settings, Builder, Memory Search, MCP, A2A, Calibration, Metrics, and Version
  Control.
- These pages use Tauri commands directly through `frontend/src/tauri.ts`.

File location:

- `frontend/src/pages/`
- `frontend/src/tauri.ts`

Confidence: High.

Impact: A rewrite is a product redesign and information architecture exercise,
not a blank frontend scaffold.

## State Management

Finding: Frontend state is mostly page-local React state plus direct Tauri
calls, with no shared client state store.

Evidence:

- Chat owns many local state values: messages, runs, task state, projection,
  tool calls, diagnostics, LifeModel, proposals, stream events, and UI toggles.
- Today and Mailbox fetch projection plus raw page-specific data.

File location:

- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`

Confidence: High.

Impact: This is a major source of complexity and inconsistent product truth.

## UX Structure

Finding: Current UI is functionally rich but dense, with technical surfaces
mixed into product flows.

Evidence:

- ProductShell has an Advanced menu for technical surfaces.
- Chat exposes reasoning trace, tool calls, agent events, kernel events, skill
  tools, task continuity, and streaming states.
- Settings includes multiple operational tabs and recovery/privacy/tool
  controls.

File location:

- `frontend/src/components/ProductShell.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/SettingsPage.tsx`
- `frontend/src/pages/settings/tabs/`

Confidence: High.

Impact: V2 should separate everyday guidance from advanced inspection, while
keeping advanced evidence available.

## Visual System

Finding: The frontend has a consistent Tailwind-based operational style, but no
clearly extracted design system beyond components and utility functions.

Evidence:

- Reusable components exist for run trace, reasoning trace, tool cards,
  product primitives, review cards, error/loading states, and runtime
  disclosure.
- Styling is mostly in page/component Tailwind class strings.

File location:

- `frontend/src/components/`
- `frontend/src/components/product/ProductPrimitives.tsx`
- `frontend/src/index.css`

Confidence: Medium.

Impact: V2 should extract product primitives and workflow components before
large layout rewrites.

## Frontend Problems That Are Real

- Page-local state competes with backend read models.
- Chat page is too broad: conversation, runtime trace, skill tools, task
  continuity, proposal surfacing, and diagnostics live in one component.
- Reply-only chat bridge still exists.
- Dev/test bridge and historical route names remain nearby and require guard
  discipline.
- Frontend type health is `UNKNOWN` in this audit because dependencies are
  absent.
