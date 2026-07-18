# Phase 3C UI Foundation And Static Mockup Spec

Status: implementation spec for the static mockup slice.

## Design Direction

OpenLife should look like a serious local workbench for a personal agent, not a
marketing dashboard and not a generic chat app.

Use this structure:

```text
OpenLife Workbench Shell
├── left sidebar
│   ├── identity and local/private status
│   ├── 今日
│   ├── 工作区
│   ├── 任务
│   ├── 审核中心
│   ├── LifeModel
│   └── 设置 / 高级
├── top context bar
│   ├── current surface title
│   ├── read-model status chips
│   └── local page actions
├── main work surface
│   ├── current objective or active task
│   ├── next action
│   ├── review-required items
│   └── state-specific panels
└── right inspector
    ├── evidence
    ├── limitations
    ├── warnings
    └── debug-only detail
```

The mockup may show multiple static states in one HTML file through tabs or
segmented controls. It should not rely on generated bitmap images for text or
layout.

## Required Mockup States

Represent at least these views:

1. `今日: ready with pending review`
2. `今日: stale/unknown fail-closed`
3. `工作区: active task with waiting permission`
4. `审核中心: review item approved but not materialized`
5. `LifeModel: limited current compatibility`
6. `设置: provider/privacy unknown or possible external transmission`

Each state must include:

- primary title and summary;
- one main action area;
- one or more backend read-model status chips;
- an evidence or limitation section;
- a visible distinction between product actions and debug/advanced actions.

## Workspace Focused Density Pattern

The Workspace is an execution surface, not a permanent status dashboard. Its
accepted static structure is:

1. current task objective;
2. compact execution timeline;
3. one expanded waiting or blocked event;
4. the exact next decision at that event;
5. evidence and full scope in an on-demand Inspector.

Workspace rules:

- do not add decorative task metrics above the timeline;
- do not repeat the permission request as a full table, banner, action area,
  progress list, and Inspector at the same time;
- keep completed and pending events compact, with only the current event
  expanded;
- keep a bounded permission summary in the event and move target detail,
  transmission, duration, revocation, capability, and policy to the Inspector;
- default desktop Workspace to sidebar plus main surface; open the Inspector as
  a third pane on demand;
- at `390x844`, permission decision controls must remain above the fixed bottom
  navigation without horizontal scrolling.

## Typography Tokens

Use CSS custom properties in the static mockup:

```css
--ol-font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
--ol-font-caption: 11px;
--ol-font-detail: 12px;
--ol-font-body: 13px;
--ol-font-body-comfortable: 14px;
--ol-font-section-title: 14px;
--ol-font-surface-title: 18px;
--ol-font-surface-title-large: 20px;
--ol-font-metric: 20px;
```

Line-height rules:

- caption/detail: 16px;
- body: 20px;
- comfortable body: 22px;
- section title: 20px;
- surface title: 24px;
- large surface title and metric: 28px.

Rules:

- do not scale font size with viewport width;
- keep letter spacing at `0`;
- use oversized type nowhere except a single surface title;
- use monospace only for IDs, hashes, event keys, and stable numeric columns;
- long English technical labels must wrap or truncate intentionally.

## Spacing, Radius, And Density Tokens

Use a 4px grid:

```css
--ol-space-1: 4px;
--ol-space-2: 8px;
--ol-space-3: 12px;
--ol-space-4: 16px;
--ol-space-5: 20px;
--ol-space-6: 24px;
--ol-space-8: 32px;
```

Use small radii:

```css
--ol-radius-1: 4px;
--ol-radius-2: 6px;
--ol-radius-3: 8px;
```

Rules:

- cards and panels should be radius 6px or 8px;
- do not put cards inside cards;
- button heights should be 30px, 32px, or 36px;
- chips should be 22px to 26px high;
- icon buttons should be 28px or 32px square;
- list rows should have stable min-height so state changes do not shift layout.

## Layout Tokens

Desktop target:

```css
--ol-sidebar-width: 260px;
--ol-sidebar-collapsed-width: 60px;
--ol-topbar-height: 48px;
--ol-inspector-width: 360px;
--ol-content-max: 1280px;
```

Viewport checks:

- `1440x900`: primary desktop target.
- `1280x800`: compact laptop target.
- `390x844`: narrow/mobile target.

Responsive behavior:

- desktop uses persistent left sidebar and optional right inspector;
- 1280px may keep inspector narrower or collapsible;
- narrow viewport collapses sidebar and inspector into drawer-like sections;
- text must not overlap, clip, or force horizontal page scroll.

## Color Tokens

Use restrained neutral foundations:

```css
--ol-color-app-bg: #f6f6f3;
--ol-color-sidebar-bg: #f7f7f4;
--ol-color-surface: #ffffff;
--ol-color-surface-muted: #f3f3ef;
--ol-color-border: #deded8;
--ol-color-border-subtle: #e8e8e2;
--ol-color-text-primary: #1c1c1a;
--ol-color-text-secondary: #5f5f57;
--ol-color-text-muted: #85857b;
--ol-color-accent: #2f5f56;
--ol-color-accent-soft: #e7f0ed;
--ol-color-danger: #b42318;
--ol-color-danger-soft: #fff1f0;
--ol-color-warning: #a16207;
--ol-color-warning-soft: #fff7e6;
--ol-color-success: #16703a;
--ol-color-success-soft: #e8f5ec;
--ol-color-info: #2563a8;
--ol-color-info-soft: #eaf2ff;
```

Rules:

- red only for errors, blockers, and destructive states;
- amber for waiting, stale, limited, risk, and needs review;
- green only for verified success, safe, or ready fixture states;
- blue for informational status, not primary branding;
- no gradient orbs, bokeh, decorative blobs, or one-hue palette.

## Component Inventory For The Mockup

The static mockup should represent these primitives, even if implemented as
HTML/CSS classes rather than React components:

- `WorkbenchShell`
- `SidebarNav`
- `TopContextBar`
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

State classes should include:

- `is-active`
- `is-selected`
- `is-loading`
- `is-empty`
- `is-stale`
- `is-warning`
- `is-danger`
- `is-safe-mode`
- `is-disabled`
- `is-advanced`

## Copy And IA Rules

Primary Chinese labels for mockup:

- `今日`
- `工作区`
- `任务`
- `审核中心`
- `LifeModel`
- `设置`
- `高级`

Use Chinese-first labels for user-facing copy. Keep backend terms only where
they clarify evidence:

- `ReviewCenterViewModel`
- `TasksViewModel`
- `LifeModelViewModel`
- `MemoryViewModel`
- `ProviderPrivacyBoundarySummary`
- `ViewModelEnvelope`

Recommended status vocabulary:

- `可用`
- `受限`
- `未知`
- `陈旧`
- `等待确认`
- `需要证据`
- `已批准，未物化`
- `已阻塞`
- `失败`

Do not render `完成` for tasks that still have pending review or missing final
delivery evidence.

## Visual QA Requirements

The Agent should inspect the mockup in a browser and record:

- desktop screenshot at `1440x900`;
- compact desktop screenshot at `1280x800`;
- narrow screenshot at `390x844`;
- no horizontal scroll at these widths;
- sidebar, main surface, and inspector are visible or intentionally collapsed;
- no text overlaps;
- no button label overflows its container;
- fail-closed states are visible but not visually louder than the whole shell;
- color tokens are used consistently.

If browser tooling is unavailable, the Agent must say that clearly and provide
manual inspection notes based on static file review.
