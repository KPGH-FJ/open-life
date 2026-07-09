# OpenLife UI Foundation Study

Status: preparation artifact, not implementation completion.
Scope: visual foundation, workbench shell direction, typography, spacing, color,
and density rules for future frontend preview slices.

This document is subordinate to the active Phase7 authority stack:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`

It does not authorize replacing `/today`, changing `ProductShell`, changing
primary navigation, adding backend read models, restoring old Phase7 routes, or
claiming product readiness.

## 1. Why This Exists

The current Today V2 preview proved the ViewModel-to-surface path, but the
visual shell still feels like a web page with top tabs. The next frontend work
needs a clearer product foundation before more preview slices are implemented.

OpenLife should feel like a local-first personal AI operating workbench:

- left-side spatial navigation for durable product areas;
- center surface for the active user job;
- optional right-side evidence/context inspector;
- compact typography and spacing suitable for repeated daily work;
- restrained semantic color, not decorative dashboard color;
- default product language in Chinese, with `LifeModel` preserved as a branded
  domain term.

## 2. Reference Study

Sources checked on 2026-07-09:

- OpenAI Codex product page:
  `https://openai.com/codex/`
- OpenAI Codex app features:
  `https://developers.openai.com/codex/app/features`
- Cursor visual editor blog:
  `https://cursor.com/blog/browser-visual-editor`

### 2.1 Codex Pattern To Learn

Codex is useful as a structural reference because it is a focused desktop
command center for parallel agent work. The public Codex materials show:

- a left-hand navigation panel for workspaces/projects/tasks;
- a main pane for threads and active work;
- built-in worktree, diff, Git, skills, and automations surfaces;
- sidebar access to project/thread organization rather than a marketing-style
  top navigation.

OpenLife translation:

- use a stable left rail/sidebar for `今日`, `工作区`, `任务`, `审核中心`,
  `LifeModel`, `设置`, and possibly constrained `记忆`;
- keep the active product surface in the center;
- move evidence, task trace, privacy/provider details, and debug actions into a
  controlled inspector layer;
- make advanced tooling discoverable without letting it define the ordinary
  product shape.

Do not copy Codex branding, iconography, exact spacing, or wording.

### 2.2 Cursor Pattern To Learn

Cursor is useful as a workflow-density and design-system reference because its
visual editor connects live UI, code, component states, CSS properties, color
tokens, typography, and layout controls in one work surface.

OpenLife translation:

- future UI work should be token-driven and inspectable, not page-local
  Tailwind improvisation;
- font size, spacing, radius, color, and density should be explicit enough for
  agents to implement repeatably;
- component states should be first-class: default, hover, active, selected,
  loading, empty, stale, blocked, error, disabled, safe-mode, and advanced;
- screenshots and browser/Tauri visual checks should verify the live product,
  not only static mockups.

Do not copy Cursor's IDE metaphor wholesale. OpenLife is not a code editor.

## 3. Local Baseline

Verified current surfaces:

- `frontend/src/components/ProductShell.tsx`
- `frontend/src/productShellContract.ts`
- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/TodayV2PreviewPage.tsx`
- `frontend/src/index.css`
- `frontend/tailwind.config.js`

Current UI facts:

- `ProductShell` uses a top tab group centered inside the header.
- Primary route labels are English: `Today`, `Companion`, `Mailbox`,
  `Life Model`, `Runs`, `Settings`.
- Page content already tends toward compact Tailwind classes:
  `text-xs`, `text-sm`, `text-lg`, `text-xl`, `px-3`, `px-4`, `py-3`,
  `py-4`, `gap-2`, `gap-4`, `gap-5`, `rounded-md`, `rounded-lg`.
- There is no extracted design-token layer beyond Tailwind defaults and
  component-level class strings.
- Background color is currently hardcoded as `#f5f6f2`; header uses
  `#fcfcf8`.
- Today V2 preview renders fail-closed in pure browser mode when Tauri
  projection data is unavailable. This is correct behavior, but the visual
  treatment currently makes the error state dominate the page.

Key problem:

The product has an IA and read-model direction, but it does not yet have a
stable pixel-level foundation. Without that foundation, each slice will
re-decide font scale, spacing, card density, navigation shape, and status color.

## 4. Workbench Shell Direction

Future preview slices should use an `OpenLife Workbench Shell` direction.

Recommended shell structure:

```text
OpenLifeWorkbenchShell
├── Sidebar / Primary rail
│   ├── Product identity and status
│   ├── 今日
│   ├── 工作区
│   ├── 任务
│   ├── 审核中心
│   ├── LifeModel
│   ├── 记忆 or LifeModel sub-surface, if approved
│   └── 设置 / Advanced at bottom
├── Top local toolbar
│   ├── Current surface title
│   ├── read-model status chips
│   └── primary page actions
├── Main surface
│   └── Active product content
└── Inspector / evidence drawer
    ├── Evidence
    ├── warnings
    ├── debug-only actions
    └── raw trace only when explicitly opened
```

Do not implement this globally until the current phase authorizes a
ProductShell change. A preview slice may use a local `PreviewWorkbenchShell`
only if it stays unlisted and does not replace existing product routes.

## 5. Typography Foundation

OpenLife is a workbench, not a landing page. Type should be compact and calm.

Recommended scale:

| Token | Size | Line height | Use |
| --- | ---: | ---: | --- |
| `font.caption` | 11px | 16px | labels, metadata, badges, table hints |
| `font.detail` | 12px | 16px | secondary status, IDs, timestamps |
| `font.body` | 13px | 20px | dense panels, lists, inspector text |
| `font.body.comfortable` | 14px | 22px | primary reading text |
| `font.sectionTitle` | 14px | 20px | panel titles and grouped controls |
| `font.surfaceTitle` | 18px | 24px | normal page/surface title |
| `font.surfaceTitle.large` | 20px | 28px | important first-level surface only |
| `font.metric` | 20px | 28px | small numeric emphasis, used sparingly |

Rules:

- Do not scale font size with viewport width.
- Keep letter spacing at `0` unless uppercase technical labels need normal
  tracking. Do not use negative letter spacing.
- Prefer weight, placement, and tone over oversized headings.
- Use monospace only for IDs, hashes, technical event keys, and stable numeric
  columns.
- Long English technical words must wrap or be contained; they must not push
  layout widths.

## 6. Spacing And Density Foundation

Use a 4px base grid.

Recommended spacing tokens:

| Token | Value | Use |
| --- | ---: | --- |
| `space.1` | 4px | tight internal gaps |
| `space.2` | 8px | icon/text gap, compact rows |
| `space.3` | 12px | small panel padding, list row padding |
| `space.4` | 16px | standard panel padding, page gutter mobile |
| `space.5` | 20px | vertical page rhythm |
| `space.6` | 24px | desktop surface gutter |
| `space.8` | 32px | major section separation, rare |

Density modes:

| Mode | Purpose | Padding | Text |
| --- | --- | --- | --- |
| `compact` | sidebar, inspector, lists, task rows | 8-12px | 12-13px |
| `standard` | Today, Review summary, Settings overview | 12-16px | 13-14px |
| `comfortable` | prose-like explanation or onboarding only | 16-24px | 14px |

Rules:

- Avoid large blank bands above the first useful content.
- Do not nest cards inside cards.
- Cards/panels should usually use `radius.2` or `radius.3`, not pill-like
  marketing shapes.
- Fixed-format UI such as sidebars, rails, chips, icon buttons, evidence rows,
  and status counters need stable dimensions to prevent layout shift.

## 7. Layout Dimensions

Recommended desktop dimensions:

| Area | Width / height |
| --- | --- |
| sidebar expanded | 240-280px |
| sidebar collapsed rail | 56-64px |
| top toolbar | 44-52px |
| main content max width for focused pages | 1040-1200px |
| main content max width for dense workbench pages | 1280-1440px |
| right inspector | 320-420px |
| normal button height | 32-36px |
| compact icon button | 28-32px |
| chip height | 22-28px |

Responsive direction:

- Desktop and tablet: left sidebar remains primary.
- Narrow mobile: sidebar may become a bottom sheet/drawer, but the same IA
  hierarchy must remain.
- Do not keep the current top tab pattern as the long-term primary desktop
  navigation.

## 8. Color Foundation

OpenLife should use a restrained neutral foundation with semantic color.

Recommended token roles:

| Token | Suggested value | Use |
| --- | --- | --- |
| `color.app.bg` | `#f6f6f3` | whole app background |
| `color.sidebar.bg` | `#f7f7f4` | left navigation |
| `color.surface` | `#ffffff` | main panels |
| `color.surface.muted` | `#f3f3ef` | subtle row/panel fill |
| `color.border` | `#deded8` | normal border |
| `color.border.subtle` | `#e8e8e2` | internal dividers |
| `color.text.primary` | `#1c1c1a` | main text |
| `color.text.secondary` | `#5f5f57` | support text |
| `color.text.muted` | `#85857b` | metadata |
| `color.accent` | `#2f5f56` | selected navigation / primary action |
| `color.accent.soft` | `#e7f0ed` | selected background |
| `color.danger` | `#b42318` | error/blocker only |
| `color.warning` | `#a16207` | stale/waiting/risk |
| `color.success` | `#16703a` | confirmed success |
| `color.info` | `#2563a8` | neutral information |

Rules:

- Red is only for errors, blockers, and destructive states.
- Warning amber is for waiting, stale, limited, or needs-review states.
- Green is only for verified success or safe/ready states.
- Blue is informational, not primary branding.
- Avoid one-note palettes dominated by purple, blue gradients, beige, tan,
  brown, or dark slate.
- Do not use decorative gradient orbs or bokeh backgrounds.

## 9. Component Foundation

Initial primitive set for future implementation:

- `WorkbenchShell`
- `SidebarNav`
- `SurfaceToolbar`
- `StatusChip`
- `SemanticBanner`
- `Panel`
- `PanelHeader`
- `ActionButton`
- `IconButton`
- `InspectorDrawer`
- `EvidenceRow`
- `EmptyState`
- `FailClosedState`
- `LoadingState`

State requirements for every primitive:

- default
- hover
- active/selected
- focus-visible
- disabled
- loading
- stale
- warning
- danger/error
- safe-mode

Do not create one-off visual components in page files once the foundation pass
starts. Use primitives or explicitly document why a page-level component is
temporary.

## 10. Today V2 Preview Implications

Current Today V2 preview should be treated as a data-bound preview, not a final
visual direction.

Good:

- it consumes `TodayViewModelEnvelope`;
- it fails closed when projection is unavailable;
- it hides debug actions under an advanced lane;
- it avoids replacing `/today`.

Visual issues to fix in a future authorized slice:

- top navigation should not remain the long-term desktop pattern;
- error state takes too much visual authority in pure browser mode;
- `Advanced` is a separate top-right button instead of a coherent inspector;
- English route labels conflict with Chinese-first product copy;
- spacing is serviceable but not tokenized;
- cards use repeated class strings rather than shared primitives;
- status colors are semantically correct but not yet harmonized into tokens.

Recommended next design-only output before implementation:

1. A static `PreviewWorkbenchShell` spec for Today.
2. A normal ready-state screenshot target using fixture data.
3. A fail-closed/error-state screenshot target.
4. A narrow viewport screenshot target.
5. A token table in code or CSS only after human approval.

## 11. Implementation Boundaries For Agents

Allowed in a future UI-foundation implementation slice, if explicitly assigned:

- create scoped CSS variables or Tailwind theme extensions for tokens;
- create preview-only workbench shell components;
- restyle an unlisted preview page to use those primitives;
- add visual regression/screenshot checks for preview routes;
- add tests proving `/today` and `ProductShell` primary navigation are unchanged
  when the slice is preview-only.

Not allowed without explicit approval:

- replace `ProductShell`;
- move primary product navigation globally;
- rename routes;
- replace `/today`;
- implement full Frontend V2;
- promote `记忆` to top-level navigation;
- change backend Rust/Tauri commands;
- infer product truth from page-local diagnostics or raw proposal lists;
- claim Phase7, desktop trial, live-provider, Web AgentLoop, or MCP AgentLoop
  readiness from visual work.

## 12. Acceptance Checklist For A Future UI Foundation Slice

- The app has a clear token source for typography, spacing, radius, color, and
  density.
- Preview shell uses left navigation on desktop.
- Current route replacement is not performed unless specifically authorized.
- Everyday product state and advanced evidence are visually separated.
- Missing projection/read-model data renders as fail-closed, not fake success.
- Chinese-first copy is used on normal product surfaces.
- `LifeModel` remains branded and explained in Chinese copy.
- No dev/test/historical route is promoted into product navigation.
- Desktop and narrow viewport screenshots are captured.
- `corepack pnpm --dir frontend typecheck` and focused frontend tests pass for
  any implementation slice.
